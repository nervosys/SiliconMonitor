//! XMP / EXPO profile decoder for DDR4 and DDR5 SPD blobs (Linux only).
//!
//! Sources scanned on Linux:
//! - `/sys/bus/i2c/devices/<bus>-00<addr>/eeprom` — exported by the
//!   `at24` / `ee1004` driver when the SPD-EEPROM kernel module is loaded.
//!   DIMM SPD slaves live at I2C addresses 0x50–0x57.
//! - DDR5 modules expose a 1024-byte SPD; DDR4 modules expose 512 bytes
//!   (or 1024 when extended).
//!
//! Layout reference:
//! - SPD byte 2 (DDR type): 0x0C = DDR4, 0x12 = DDR5.
//! - XMP 2.0 magic `0x0C 0x4A` at offset 384 (DDR4 modules). Two profile
//!   slots follow (35 bytes each), each carrying voltage, min cycle time,
//!   tCL, tRCD, tRP, tRAS, tRC.
//! - XMP 3.0 (DDR5) keeps the same magic at 384, but the slot count expands
//!   to five and slots are 64 bytes.
//! - AMD EXPO magic `0x45 0x58 0x50 0x4F` ("EXPO") at offset 768 with two
//!   profile slots.
//!
//! Writing to SPD is BIOS territory; this module is read-only.

#[allow(unused_imports)]
use super::{ProfileGroup, Setting, SettingRisk, SettingValue, Subsystem};

#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DramType {
    Ddr4,
    Ddr5,
    Other(u8),
}

#[cfg(target_os = "linux")]
pub fn scan_xmp_groups() -> Vec<ProfileGroup> {
    use std::fs;
    let base = std::path::Path::new("/sys/bus/i2c/devices");
    let Ok(rd) = fs::read_dir(base) else {
        return Vec::new();
    };
    let mut groups = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let eeprom = path.join("eeprom");
        if !eeprom.exists() {
            continue;
        }
        let Ok(bytes) = fs::read(&eeprom) else {
            continue;
        };
        if bytes.len() < 384 {
            continue;
        }
        if let Some(g) = decode_blob(
            &bytes,
            &eeprom.display().to_string(),
            entry.file_name().to_string_lossy().to_string(),
        ) {
            groups.push(g);
        }
    }
    groups
}

#[cfg(not(target_os = "linux"))]
pub fn scan_xmp_groups() -> Vec<ProfileGroup> {
    Vec::new()
}

#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn dram_type(bytes: &[u8]) -> DramType {
    match bytes.get(2).copied() {
        Some(0x0C) => DramType::Ddr4,
        Some(0x12) => DramType::Ddr5,
        Some(other) => DramType::Other(other),
        None => DramType::Other(0),
    }
}

#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn decode_blob(bytes: &[u8], source: &str, locator: String) -> Option<ProfileGroup> {
    let dram = dram_type(bytes);
    let dram_label = match dram {
        DramType::Ddr4 => "DDR4",
        DramType::Ddr5 => "DDR5",
        DramType::Other(_) => return None,
    };

    let mut group = ProfileGroup::new(
        Subsystem::Memory,
        format!("DIMM @ {} ({})", locator, dram_label),
        "SPD XMP/EXPO profiles",
        source.to_string(),
    );

    let mut any_profile = false;

    // XMP magic at byte 384: 0x0C 0x4A
    if bytes.len() >= 386 && bytes[384] == 0x0C && bytes[385] == 0x4A {
        any_profile = true;
        decode_xmp(&mut group, bytes, dram);
    }
    // EXPO magic at byte 768: "EXPO"
    if bytes.len() >= 772 && &bytes[768..772] == b"EXPO" {
        any_profile = true;
        decode_expo(&mut group, bytes);
    }

    if !any_profile {
        group.note(
            "No XMP or EXPO magic found in this SPD blob — this DIMM ships \
            with JEDEC profiles only.",
        );
    }
    Some(group)
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn decode_xmp(group: &mut ProfileGroup, bytes: &[u8], dram: DramType) {
    // Profile count byte 385 has bit-flags per slot. Each slot's data
    // location and stride depends on DDR generation. We expose count +
    // per-slot voltage / speed only — the full sub-timing decode varies
    // significantly between XMP 2.0 (DDR4) and XMP 3.0 (DDR5) and is left
    // for a future iteration.
    let enabled = bytes.get(385).copied().unwrap_or(0);
    let profile_count = enabled.count_ones() as u64;
    group.push(
        Setting::info(
            "xmp.version",
            "XMP Version",
            SettingValue::Text(match dram {
                DramType::Ddr5 => "XMP 3.0".into(),
                _ => "XMP 2.0".into(),
            }),
        )
        .with_source("SPD offset 384"),
    );
    group.push(
        Setting::info(
            "xmp.profile_count",
            "XMP Profiles Present",
            SettingValue::Uint(profile_count),
        )
        .with_description("Number of XMP profiles enabled in this DIMM's SPD.")
        .with_source("SPD offset 385"),
    );

    let (slot_stride, slot_base) = match dram {
        DramType::Ddr5 => (64usize, 393usize),
        _ => (35usize, 393usize),
    };
    for slot in 0..5 {
        if (enabled >> slot) & 1 == 0 {
            continue;
        }
        let base = slot_base + slot * slot_stride;
        if base + 4 >= bytes.len() {
            break;
        }
        let vdd_raw = bytes[base] as u32;
        // DDR4 XMP encodes Vdd in steps of 5 mV starting at 1.0 V; bit 7 indicates
        // "encoding present". DDR5 uses different encoding; expose raw for now.
        let voltage = match dram {
            DramType::Ddr4 => Some(1.0 + (vdd_raw & 0x7F) as f64 * 0.005),
            _ => None,
        };
        if let Some(v) = voltage {
            group.push(
                Setting::info(
                    format!("xmp.slot{}.vdd_v", slot),
                    format!("XMP Profile {} Vdd", slot + 1),
                    SettingValue::Float(v),
                )
                .with_unit("V")
                .with_risk(SettingRisk::Dangerous)
                .with_source(format!("SPD offset {}", base)),
            );
        } else {
            group.push(
                Setting::info(
                    format!("xmp.slot{}.vdd_raw", slot),
                    format!("XMP Profile {} Vdd (raw)", slot + 1),
                    SettingValue::Uint(vdd_raw as u64),
                )
                .with_source(format!("SPD offset {}", base)),
            );
        }
        // Min cycle time is two bytes (mtb units, 125 ps), at base+1 / base+2 for DDR4.
        let mtb_lo = bytes.get(base + 1).copied().unwrap_or(0) as u32;
        let mtb_hi = bytes.get(base + 2).copied().unwrap_or(0) as u32;
        let mtb = mtb_lo | (mtb_hi << 8);
        if mtb > 0 {
            let cycle_ps = mtb as f64 * 125.0; // mtb is in 125-ps units
            let mts = (2.0 * 1_000_000.0 / cycle_ps).round() as u64;
            group.push(
                Setting::info(
                    format!("xmp.slot{}.speed_mts", slot),
                    format!("XMP Profile {} Speed", slot + 1),
                    SettingValue::Uint(mts),
                )
                .with_unit("MT/s")
                .with_source(format!("SPD offset {}–{}", base + 1, base + 2)),
            );
        }
    }
    if profile_count == 0 {
        group.note("XMP magic present but no profiles flagged enabled in byte 385.");
    }
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn decode_expo(group: &mut ProfileGroup, bytes: &[u8]) {
    let enabled = bytes.get(772).copied().unwrap_or(0);
    let profile_count = enabled.count_ones() as u64;
    group.push(
        Setting::info(
            "expo.version",
            "EXPO Version",
            SettingValue::Text("AMD EXPO".into()),
        )
        .with_source("SPD offset 768"),
    );
    group.push(
        Setting::info(
            "expo.profile_count",
            "EXPO Profiles Present",
            SettingValue::Uint(profile_count),
        )
        .with_description("Number of AMD EXPO profiles enabled in this DIMM's SPD.")
        .with_source("SPD offset 772"),
    );
    // EXPO profile slots begin at offset 776 with 64 bytes per slot. Decode
    // voltage and min cycle time identically to XMP 3.0 layout.
    for slot in 0..2 {
        if (enabled >> slot) & 1 == 0 {
            continue;
        }
        let base = 776 + slot * 64;
        if base + 4 >= bytes.len() {
            break;
        }
        let vdd_raw = bytes[base] as u32;
        group.push(
            Setting::info(
                format!("expo.slot{}.vdd_raw", slot),
                format!("EXPO Profile {} Vdd (raw)", slot + 1),
                SettingValue::Uint(vdd_raw as u64),
            )
            .with_risk(SettingRisk::Dangerous)
            .with_source(format!("SPD offset {}", base)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ddr4_blob() -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[2] = 0x0C; // DDR4
        b[384] = 0x0C; // XMP magic
        b[385] = 0x4A;
        b[386] = 0b0000_0011; // two profiles enabled
                              // slot 0 at 393: vdd raw=0x28 → 1.0 + 40*0.005 = 1.2V
        b[393] = 0x28;
        b[394] = 0x07; // mtb lo (DDR4-3200: cycle = 625 ps → mtb = 5)
        b[395] = 0x00;
        b
    }

    #[test]
    fn detects_ddr4_xmp() {
        let blob = make_ddr4_blob();
        let g = decode_blob(&blob, "test", "0-0050".into()).unwrap();
        assert!(g.settings.iter().any(|s| s.id == "xmp.version"));
        let profile_count = g
            .settings
            .iter()
            .find(|s| s.id == "xmp.profile_count")
            .unwrap();
        assert!(matches!(profile_count.value, SettingValue::Uint(_)));
    }

    #[test]
    fn skips_unknown_dram_type() {
        let mut blob = vec![0u8; 512];
        blob[2] = 0xFF;
        assert!(decode_blob(&blob, "test", "x".into()).is_none());
    }

    #[test]
    fn detects_expo_magic() {
        let mut b = vec![0u8; 1024];
        b[2] = 0x12; // DDR5
        b[768..772].copy_from_slice(b"EXPO");
        b[772] = 0b0000_0001;
        b[776] = 0x50;
        let g = decode_blob(&b, "test", "y".into()).unwrap();
        assert!(g.settings.iter().any(|s| s.id == "expo.version"));
    }
}

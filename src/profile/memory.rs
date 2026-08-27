//! Memory profile provider — per-DIMM SPD / XMP / EXPO read-only view.
//!
//! Builds on the existing [`crate::memory_topology::MemoryTopologyMonitor`],
//! which already parses SMBIOS Type 17 records. We surface per-DIMM rated vs.
//! configured speed (the gap between them is the classic indicator that an
//! XMP/EXPO profile has *not* been activated), plus voltage, rank count, and
//! ECC status.
//!
//! True SPD byte-level XMP profile decode (Intel XMP 2.0/3.0 magic at SPD
//! offsets 384–509, AMD EXPO at 768–1023) requires direct SMBus / `i2c-dev`
//! access on Linux and an SMBus driver on Windows. The hooks are in place
//! for that next step but the bytes themselves are not read in this pass.

use super::{ProfileGroup, ProfileProvider, Setting, SettingRisk, SettingValue, Subsystem};

pub struct MemoryProfileProvider {
    _private: (),
}

impl MemoryProfileProvider {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for MemoryProfileProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileProvider for MemoryProfileProvider {
    fn subsystem(&self) -> Subsystem {
        Subsystem::Memory
    }

    fn snapshot(&mut self) -> Vec<ProfileGroup> {
        let mut groups = super::spd_xmp::scan_xmp_groups();
        let monitor = match crate::memory_topology::MemoryTopologyMonitor::new() {
            Ok(m) => m,
            Err(_) => return groups,
        };
        for dimm in monitor.populated_dimms() {
            let label = format!(
                "{} — {} {} ({:.2} GiB {:?})",
                dimm.locator,
                if dimm.manufacturer.is_empty() {
                    "Unknown"
                } else {
                    &dimm.manufacturer
                },
                if dimm.part_number.is_empty() {
                    ""
                } else {
                    &dimm.part_number
                },
                dimm.capacity_gib(),
                dimm.memory_type,
            );
            let mut g = ProfileGroup::new(
                Subsystem::Memory,
                label,
                "DIMM SPD / configured",
                "SMBIOS Type 17",
            );
            g.push(Setting::info(
                "bank",
                "Bank Locator",
                SettingValue::Text(dimm.bank.clone()),
            ));
            g.push(
                Setting::info(
                    "rated_speed_mts",
                    "Rated Speed",
                    SettingValue::Uint(dimm.speed_mts as u64),
                )
                .with_unit("MT/s")
                .with_description("Maximum speed the DIMM is rated for (SPD)."),
            );
            g.push(
                Setting::info(
                    "configured_speed_mts",
                    "Configured Speed",
                    SettingValue::Uint(dimm.configured_speed_mts as u64),
                )
                .with_unit("MT/s")
                .with_description(
                    "Speed the BIOS programmed at boot. If lower than Rated Speed, \
                    XMP/EXPO is likely disabled.",
                ),
            );
            if dimm.configured_speed_mts > 0
                && dimm.speed_mts > 0
                && dimm.configured_speed_mts < dimm.speed_mts
            {
                g.note(format!(
                    "DIMM is running at {} MT/s but is rated for {} MT/s — enable \
                    XMP/EXPO in BIOS to use the rated speed.",
                    dimm.configured_speed_mts, dimm.speed_mts
                ));
            }
            g.push(Setting::info(
                "memory_type",
                "Memory Type",
                SettingValue::Text(format!("{:?}", dimm.memory_type)),
            ));
            g.push(Setting::info(
                "form_factor",
                "Form Factor",
                SettingValue::Text(format!("{:?}", dimm.form_factor)),
            ));
            // SMBIOS Type 17 defines zero in the voltage fields as "unknown",
            // and this machine's firmware leaves them there --- so the row read
            // "Voltage = 0 V [dangerous]" for a DDR5 module that necessarily runs
            // near 1.1V. A zero volts no DIMM can have, marked dangerous so it
            // reads as significant, is worse than no row at all.
            if dimm.voltage > 0.0 {
                g.push(
                    Setting::info("voltage_v", "Voltage", SettingValue::Float(dimm.voltage))
                        .with_unit("V")
                        .with_risk(SettingRisk::Dangerous)
                        .with_description(
                            "DIMM operating voltage. Overriding via XMP/EXPO can affect stability.",
                        ),
                );
            }
            g.push(Setting::info(
                "ranks",
                "Ranks",
                SettingValue::Uint(dimm.ranks as u64),
            ));
            g.push(Setting::info(
                "data_width_bits",
                "Data Width",
                SettingValue::Uint(dimm.data_width_bits as u64),
            ));
            g.push(Setting::info(
                "total_width_bits",
                "Total Width",
                SettingValue::Uint(dimm.total_width_bits as u64),
            ));
            g.push(Setting::info(
                "ecc",
                "ECC",
                SettingValue::Bool(dimm.is_ecc()),
            ));
            if !dimm.serial_number.is_empty() {
                g.push(Setting::info(
                    "serial_number",
                    "Serial Number",
                    SettingValue::Text(dimm.serial_number.clone()),
                ));
            }
            g.note(
                "Raw XMP/EXPO profile blocks (full sub-timings, secondary/tertiary \
                straps, voltages per profile) require direct SPD/SMBus reads and \
                are not yet implemented.",
            );
            groups.push(g);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke() {
        let mut p = MemoryProfileProvider::new();
        let _ = p.snapshot();
    }
}

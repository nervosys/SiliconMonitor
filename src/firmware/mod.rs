//! Firmware inventory — BIOS, UEFI, EC, ME, NIC firmware, storage firmware.
//!
//! Collects firmware versions for all system components, security status,
//! and infers firmware age and update urgency.
//!
//! # Platform Support
//!
//! - **Linux**: DMI/SMBIOS (`/sys/class/dmi/id/`), `fwupdmgr`, `ethtool -i`
//! - **Windows**: WMI (`Win32_BIOS`), `MSFT_Firmware`, registry
//! - **macOS**: `system_profiler SPHardwareDataType`, `ioreg`
//!
//! ## Inference
//!
//! Firmware dates are parsed to estimate age. Known vulnerability databases
//! (e.g., BIOS date < 2023 = likely unpatched Spectre/Meltdown mitigations)
//! produce a risk score.
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::firmware::FirmwareInventory;
//!
//! let inventory = FirmwareInventory::new().unwrap();
//! for fw in inventory.items() {
//!     println!("{}: {} v{} ({})", fw.component, fw.vendor, fw.version, fw.date);
//! }
//! println!("Risk score: {}/100", inventory.risk_score());
//! ```

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

/// Firmware component type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FirmwareComponent {
    /// System BIOS / UEFI
    SystemBios,
    /// Intel Management Engine (ME) or AMD PSP
    ManagementEngine,
    /// Embedded Controller (EC)
    EmbeddedController,
    /// Trusted Platform Module (TPM)
    Tpm,
    /// Network adapter firmware
    Nic,
    /// Storage controller / disk firmware
    Storage,
    /// GPU VBIOS
    GpuVbios,
    /// Thunderbolt controller
    Thunderbolt,
    /// Bluetooth controller
    Bluetooth,
    /// WiFi adapter
    Wifi,
    /// Base Management Controller (BMC/IPMI)
    Bmc,
    /// CPU microcode
    CpuMicrocode,
    /// Other
    Other(String),
}

/// UEFI Secure Boot status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecureBootStatus {
    Enabled,
    Disabled,
    NotSupported,
    Unknown,
}

/// Boot mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootMode {
    UEFI,
    Legacy,
    Unknown,
}

/// A single firmware entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareEntry {
    /// Component type
    pub component: FirmwareComponent,
    /// Vendor / manufacturer
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Release date (YYYY-MM-DD or vendor format)
    pub date: String,
    /// Device path or identifier
    pub device: String,
    /// Updateable via OS mechanisms?
    pub updateable: bool,
    /// Estimated age in days (inferred from date)
    pub estimated_age_days: Option<u32>,
    /// Security risk level (0 = low, 100 = critical)
    pub inferred_risk_score: u8,
}

/// Firmware inventory for the system.
pub struct FirmwareInventory {
    entries: Vec<FirmwareEntry>,
    secure_boot: SecureBootStatus,
    boot_mode: BootMode,
    system_vendor: String,
    system_product: String,
    /// System serial number.
    pub system_serial: String,
}

impl FirmwareInventory {
    pub fn new() -> Result<Self, SimonError> {
        let mut inv = Self {
            entries: Vec::new(),
            secure_boot: SecureBootStatus::Unknown,
            boot_mode: BootMode::Unknown,
            system_vendor: String::new(),
            system_product: String::new(),
            system_serial: String::new(),
        };
        inv.refresh()?;
        Ok(inv)
    }

    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.entries.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        // Infer risk scores for all entries
        for entry in &mut self.entries {
            entry.estimated_age_days = Self::estimate_age(&entry.date);
            entry.inferred_risk_score = Self::infer_risk(entry);
        }

        Ok(())
    }

    pub fn items(&self) -> &[FirmwareEntry] {
        &self.entries
    }

    pub fn secure_boot_status(&self) -> &SecureBootStatus {
        &self.secure_boot
    }

    pub fn boot_mode(&self) -> &BootMode {
        &self.boot_mode
    }

    pub fn system_vendor(&self) -> &str {
        &self.system_vendor
    }

    pub fn system_product(&self) -> &str {
        &self.system_product
    }

    /// Overall firmware risk score (max across all components).
    pub fn risk_score(&self) -> u8 {
        self.entries.iter().map(|e| e.inferred_risk_score).max().unwrap_or(0)
    }

    /// Average firmware age in days.
    pub fn average_firmware_age_days(&self) -> Option<f64> {
        let ages: Vec<f64> = self.entries.iter()
            .filter_map(|e| e.estimated_age_days.map(|d| d as f64))
            .collect();
        if ages.is_empty() {
            None
        } else {
            Some(ages.iter().sum::<f64>() / ages.len() as f64)
        }
    }

    /// Firmware entries that need attention (risk > 50).
    pub fn high_risk_entries(&self) -> Vec<&FirmwareEntry> {
        self.entries.iter().filter(|e| e.inferred_risk_score > 50).collect()
    }

    /// Estimate age from date string.
    fn estimate_age(date: &str) -> Option<u32> {
        // Try common date formats: YYYY-MM-DD, MM/DD/YYYY, YYYYMMDD
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();

        // Very simple date parser
        let (year, month, day) = Self::parse_date(date)?;

        // Approximate days since date
        let fw_days = year as u64 * 365 + month as u64 * 30 + day as u64;
        let now_days = now / 86400;
        // Epoch offset: 1970-01-01
        let epoch_days = 1970 * 365;

        if fw_days > epoch_days {
            let age = now_days.saturating_sub(fw_days - epoch_days);
            Some(age as u32)
        } else {
            None
        }
    }

    fn parse_date(date: &str) -> Option<(u32, u32, u32)> {
        // YYYY-MM-DD
        if date.len() >= 10 && date.chars().nth(4) == Some('-') {
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() >= 3 {
                let y = parts[0].parse().ok()?;
                let m = parts[1].parse().ok()?;
                let d = parts[2].parse().ok()?;
                return Some((y, m, d));
            }
        }

        // MM/DD/YYYY
        if date.contains('/') {
            let parts: Vec<&str> = date.split('/').collect();
            if parts.len() >= 3 {
                let m = parts[0].parse().ok()?;
                let d = parts[1].parse().ok()?;
                let y = parts[2].parse().ok()?;
                return Some((y, m, d));
            }
        }

        // YYYYMMDD
        if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
            let y = date[0..4].parse().ok()?;
            let m = date[4..6].parse().ok()?;
            let d = date[6..8].parse().ok()?;
            return Some((y, m, d));
        }

        None
    }

    /// Infer security risk from firmware entry.
    fn infer_risk(entry: &FirmwareEntry) -> u8 {
        let mut risk: u8 = 0;

        // Age-based risk
        if let Some(age_days) = entry.estimated_age_days {
            risk = match age_days {
                0..=180 => 0,        // < 6 months: low risk
                181..=365 => 10,     // 6-12 months
                366..=730 => 25,     // 1-2 years
                731..=1095 => 45,    // 2-3 years
                1096..=1825 => 65,   // 3-5 years
                _ => 85,            // 5+ years: high risk
            };
        }

        // Component-specific risk amplifiers
        match &entry.component {
            FirmwareComponent::SystemBios => {
                // BIOS is critical — amplify risk
                risk = risk.saturating_add(10);
            }
            FirmwareComponent::ManagementEngine => {
                // ME has had many CVEs — highest risk
                risk = risk.saturating_add(15);
            }
            FirmwareComponent::CpuMicrocode => {
                // CPU microcode patches Spectre/Meltdown
                risk = risk.saturating_add(10);
            }
            FirmwareComponent::Bmc => {
                // BMC = remote management, high attack surface
                risk = risk.saturating_add(15);
            }
            FirmwareComponent::Tpm => {
                risk = risk.saturating_add(5);
            }
            _ => {}
        }

        risk.min(100)
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        let dmi = std::path::Path::new("/sys/class/dmi/id");

        // System info
        self.system_vendor = std::fs::read_to_string(dmi.join("sys_vendor"))
            .unwrap_or_default().trim().to_string();
        self.system_product = std::fs::read_to_string(dmi.join("product_name"))
            .unwrap_or_default().trim().to_string();
        self.system_serial = std::fs::read_to_string(dmi.join("product_serial"))
            .unwrap_or_default().trim().to_string();

        // BIOS
        let bios_vendor = std::fs::read_to_string(dmi.join("bios_vendor"))
            .unwrap_or_default().trim().to_string();
        let bios_version = std::fs::read_to_string(dmi.join("bios_version"))
            .unwrap_or_default().trim().to_string();
        let bios_date = std::fs::read_to_string(dmi.join("bios_date"))
            .unwrap_or_default().trim().to_string();

        if !bios_vendor.is_empty() {
            self.entries.push(FirmwareEntry {
                component: FirmwareComponent::SystemBios,
                vendor: bios_vendor,
                version: bios_version,
                date: bios_date,
                device: "System BIOS".into(),
                updateable: true,
                estimated_age_days: None,
                inferred_risk_score: 0,
            });
        }

        // Secure boot check
        if std::path::Path::new("/sys/firmware/efi").exists() {
            self.boot_mode = BootMode::UEFI;
            let sb_path = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
            if let Ok(data) = std::fs::read(sb_path) {
                // Last byte: 1 = enabled, 0 = disabled
                self.secure_boot = if data.last() == Some(&1) {
                    SecureBootStatus::Enabled
                } else {
                    SecureBootStatus::Disabled
                };
            }
        } else {
            self.boot_mode = BootMode::Legacy;
            self.secure_boot = SecureBootStatus::NotSupported;
        }

        // CPU microcode
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("microcode") {
                    if let Some(ver) = line.split(':').nth(1) {
                        self.entries.push(FirmwareEntry {
                            component: FirmwareComponent::CpuMicrocode,
                            vendor: self.system_vendor.clone(),
                            version: ver.trim().to_string(),
                            date: String::new(),
                            device: "CPU Microcode".into(),
                            updateable: true,
                            estimated_age_days: None,
                            inferred_risk_score: 0,
                        });
                        break;
                    }
                }
            }
        }

        // Network adapter firmware via ethtool
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let iface = entry.file_name().to_string_lossy().to_string();
                if iface == "lo" {
                    continue;
                }
                if let Ok(output) = std::process::Command::new("ethtool")
                    .args(["-i", &iface])
                    .output()
                {
                    let text = String::from_utf8(output.stdout).unwrap_or_default();
                    let mut version = String::new();
                    let mut driver = String::new();
                    for line in text.lines() {
                        if let Some(v) = line.strip_prefix("firmware-version: ") {
                            version = v.trim().to_string();
                        }
                        if let Some(d) = line.strip_prefix("driver: ") {
                            driver = d.trim().to_string();
                        }
                    }
                    if !version.is_empty() {
                        self.entries.push(FirmwareEntry {
                            component: FirmwareComponent::Nic,
                            vendor: driver,
                            version,
                            date: String::new(),
                            device: iface,
                            updateable: false,
                            estimated_age_days: None,
                            inferred_risk_score: 0,
                        });
                    }
                }
            }
        }

        // Try fwupdmgr for additional firmware
        if let Ok(output) = std::process::Command::new("fwupdmgr")
            .args(["get-devices", "--json"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(devices) = json.get("Devices").and_then(|d| d.as_array()) {
                    for dev in devices {
                        let name = dev.get("Name").and_then(|n| n.as_str()).unwrap_or("");
                        let vendor = dev.get("Vendor").and_then(|v| v.as_str()).unwrap_or("");
                        let version = dev.get("Version").and_then(|v| v.as_str()).unwrap_or("");

                        if !name.is_empty() && !version.is_empty() {
                            let component = if name.to_lowercase().contains("thunderbolt") {
                                FirmwareComponent::Thunderbolt
                            } else if name.to_lowercase().contains("tpm") {
                                FirmwareComponent::Tpm
                            } else if name.to_lowercase().contains("bmc") || name.to_lowercase().contains("ipmi") {
                                FirmwareComponent::Bmc
                            } else {
                                FirmwareComponent::Other(name.to_string())
                            };

                            self.entries.push(FirmwareEntry {
                                component,
                                vendor: vendor.to_string(),
                                version: version.to_string(),
                                date: String::new(),
                                device: name.to_string(),
                                updateable: dev.get("Flags")
                                    .and_then(|f| f.as_array())
                                    .map(|flags| flags.iter().any(|f| f.as_str() == Some("updatable")))
                                    .unwrap_or(false),
                                estimated_age_days: None,
                                inferred_risk_score: 0,
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        self.boot_mode = BootMode::UEFI; // Modern Windows is almost always UEFI

        // BIOS information
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_BIOS | Select-Object Manufacturer,SMBIOSBIOSVersion,ReleaseDate | ConvertTo-Json"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let vendor = json.get("Manufacturer").and_then(|v| v.as_str()).unwrap_or("");
                let version = json.get("SMBIOSBIOSVersion").and_then(|v| v.as_str()).unwrap_or("");
                let date = json.get("ReleaseDate").and_then(|v| v.as_str()).unwrap_or("");

                // Extract date from WMI format: /Date(1234567890000)/
                let clean_date = if date.contains("/Date(") {
                    // Parse WMI timestamp
                    date.trim_start_matches("/Date(")
                        .trim_end_matches(")/")
                        .split(')')
                        .next()
                        .and_then(|ts| ts.parse::<i64>().ok())
                        .map(|ts| {
                            let secs = ts / 1000;
                            let _d = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64);
                            format!("{}", secs) // simplified
                        })
                        .unwrap_or_default()
                } else {
                    date.to_string()
                };

                if !vendor.is_empty() {
                    self.entries.push(FirmwareEntry {
                        component: FirmwareComponent::SystemBios,
                        vendor: vendor.to_string(),
                        version: version.to_string(),
                        date: clean_date,
                        device: "System BIOS".into(),
                        updateable: true,
                        estimated_age_days: None,
                        inferred_risk_score: 0,
                    });
                }
            }
        }

        // System info
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_ComputerSystem | Select-Object Manufacturer,Model | ConvertTo-Json"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                self.system_vendor = json.get("Manufacturer")
                    .and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.system_product = json.get("Model")
                    .and_then(|v| v.as_str()).unwrap_or("").to_string();
            }
        }

        // Secure Boot status
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Confirm-SecureBootUEFI"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default().trim().to_string();
            self.secure_boot = match text.as_str() {
                "True" => SecureBootStatus::Enabled,
                "False" => SecureBootStatus::Disabled,
                _ => SecureBootStatus::Unknown,
            };
        }

        // Storage firmware
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-PhysicalDisk | Select-Object FriendlyName,Manufacturer,FirmwareVersion | ConvertTo-Json"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            // Handle both single object and array
            let disks: Vec<serde_json::Value> = if text.trim_start().starts_with('[') {
                serde_json::from_str(&text).unwrap_or_default()
            } else {
                serde_json::from_str::<serde_json::Value>(&text)
                    .map(|v| vec![v])
                    .unwrap_or_default()
            };

            for disk in disks {
                let name = disk.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or("");
                let vendor = disk.get("Manufacturer").and_then(|v| v.as_str()).unwrap_or("");
                let fw = disk.get("FirmwareVersion").and_then(|v| v.as_str()).unwrap_or("");

                if !fw.is_empty() {
                    self.entries.push(FirmwareEntry {
                        component: FirmwareComponent::Storage,
                        vendor: vendor.to_string(),
                        version: fw.to_string(),
                        date: String::new(),
                        device: name.to_string(),
                        updateable: false,
                        estimated_age_days: None,
                        inferred_risk_score: 0,
                    });
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.boot_mode = BootMode::UEFI;

        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with("Model Name:") {
                    self.system_product = line.split(':').nth(1).unwrap_or("").trim().to_string();
                }
                if line.starts_with("Boot ROM Version:") {
                    let version = line.split(':').nth(1).unwrap_or("").trim().to_string();
                    self.entries.push(FirmwareEntry {
                        component: FirmwareComponent::SystemBios,
                        vendor: "Apple".into(),
                        version,
                        date: String::new(),
                        device: "Boot ROM".into(),
                        updateable: true,
                        estimated_age_days: None,
                        inferred_risk_score: 0,
                    });
                }
            }
        }

        self.system_vendor = "Apple".to_string();

        // T2 / Secure Enclave
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPiBridgeDataType"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with("Firmware Version:") || line.starts_with("Build Version:") {
                    let version = line.split(':').nth(1).unwrap_or("").trim().to_string();
                    self.entries.push(FirmwareEntry {
                        component: FirmwareComponent::EmbeddedController,
                        vendor: "Apple".into(),
                        version,
                        date: String::new(),
                        device: "T2/Secure Enclave".into(),
                        updateable: true,
                        estimated_age_days: None,
                        inferred_risk_score: 0,
                    });
                    break;
                }
            }
        }
    }
}

impl Default for FirmwareInventory {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            entries: Vec::new(),
            secure_boot: SecureBootStatus::Unknown,
            boot_mode: BootMode::Unknown,
            system_vendor: String::new(),
            system_product: String::new(),
            system_serial: String::new(),
        })
    }
}

impl std::fmt::Display for FirmwareComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SystemBios => write!(f, "System BIOS/UEFI"),
            Self::ManagementEngine => write!(f, "Management Engine"),
            Self::EmbeddedController => write!(f, "Embedded Controller"),
            Self::Tpm => write!(f, "TPM"),
            Self::Nic => write!(f, "Network Adapter"),
            Self::Storage => write!(f, "Storage"),
            Self::GpuVbios => write!(f, "GPU VBIOS"),
            Self::Thunderbolt => write!(f, "Thunderbolt"),
            Self::Bluetooth => write!(f, "Bluetooth"),
            Self::Wifi => write!(f, "WiFi"),
            Self::Bmc => write!(f, "BMC/IPMI"),
            Self::CpuMicrocode => write!(f, "CPU Microcode"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firmware_inventory_creation() {
        let inv = FirmwareInventory::new();
        assert!(inv.is_ok());
    }

    #[test]
    fn test_firmware_inventory_default() {
        let inv = FirmwareInventory::default();
        let _ = inv.risk_score();
        let _ = inv.average_firmware_age_days();
        let _ = inv.high_risk_entries();
    }

    #[test]
    fn test_date_parsing() {
        assert_eq!(FirmwareInventory::parse_date("2024-01-15"), Some((2024, 1, 15)));
        assert_eq!(FirmwareInventory::parse_date("01/15/2024"), Some((2024, 1, 15)));
        assert_eq!(FirmwareInventory::parse_date("20240115"), Some((2024, 1, 15)));
    }

    #[test]
    fn test_risk_inference() {
        let mut entry = FirmwareEntry {
            component: FirmwareComponent::SystemBios,
            vendor: "Test".into(),
            version: "1.0".into(),
            date: String::new(),
            device: "BIOS".into(),
            updateable: true,
            estimated_age_days: Some(2000), // ~5.5 years
            inferred_risk_score: 0,
        };
        entry.inferred_risk_score = FirmwareInventory::infer_risk(&entry);
        assert!(entry.inferred_risk_score > 80); // Old BIOS = high risk
    }

    #[test]
    fn test_serialization() {
        let entry = FirmwareEntry {
            component: FirmwareComponent::SystemBios,
            vendor: "AMI".into(),
            version: "2.10".into(),
            date: "2024-01-15".into(),
            device: "System BIOS".into(),
            updateable: true,
            estimated_age_days: Some(365),
            inferred_risk_score: 20,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let _: FirmwareEntry = serde_json::from_str(&json).unwrap();
    }
}

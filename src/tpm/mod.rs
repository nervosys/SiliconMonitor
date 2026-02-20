//! Trusted Platform Module (TPM) monitoring.
//!
//! Detects TPM presence, version, manufacturer, and capabilities.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/class/tpm/`, `/dev/tpm*`
//! - **Windows**: Uses WMI (`Win32_Tpm`) and registry
//! - **macOS**: Checks Secure Enclave presence (Apple's TPM equivalent)
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::tpm::TpmMonitor;
//!
//! let monitor = TpmMonitor::new().unwrap();
//! if let Some(tpm) = monitor.tpm() {
//!     println!("TPM {} - v{} by {}", tpm.device, tpm.version, tpm.manufacturer);
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// TPM specification version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TpmVersion {
    /// TPM 1.2
    V1_2,
    /// TPM 2.0
    V2_0,
    /// Apple Secure Enclave (T1/T2/Apple Silicon)
    SecureEnclave,
    /// Unknown version
    Unknown,
}

/// TPM status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TpmStatus {
    /// TPM is enabled and ready
    Enabled,
    /// TPM exists but is disabled in BIOS/firmware
    Disabled,
    /// TPM is in a locked state
    Locked,
    /// Status unknown
    Unknown,
}

/// Information about the TPM device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpmInfo {
    /// Device name (e.g., "tpm0")
    pub device: String,
    /// TPM version
    pub version: TpmVersion,
    /// Current status
    pub status: TpmStatus,
    /// Manufacturer name
    pub manufacturer: String,
    /// Firmware version
    pub firmware_version: String,
    /// Device path
    pub device_path: String,
    /// Whether the TPM is the system's primary/active TPM
    pub is_primary: bool,
    /// Supported algorithms (SHA-1, SHA-256, RSA, ECC, etc.)
    pub algorithms: Vec<String>,
    /// PCR bank count
    pub pcr_banks: u32,
    /// Whether platform integrity measurements are active
    pub measured_boot: bool,
}

/// Monitor for TPM devices
pub struct TpmMonitor {
    tpm_info: Option<TpmInfo>,
}

impl TpmMonitor {
    /// Create a new TpmMonitor and detect TPM.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self { tpm_info: None };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh TPM detection.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.tpm_info = None;

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        Ok(())
    }

    /// Get the detected TPM info, if any.
    pub fn tpm(&self) -> Option<&TpmInfo> {
        self.tpm_info.as_ref()
    }

    /// Returns true if a TPM is present.
    pub fn has_tpm(&self) -> bool {
        self.tpm_info.is_some()
    }

    /// Returns true if TPM 2.0 is available.
    pub fn has_tpm2(&self) -> bool {
        self.tpm_info
            .as_ref()
            .map(|t| t.version == TpmVersion::V2_0)
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        let tpm_class = std::path::Path::new("/sys/class/tpm");
        if !tpm_class.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(tpm_class) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("tpm") {
                    continue;
                }

                let base = entry.path();

                // Read TPM version from tpm_version_major
                let version = if let Ok(major) =
                    std::fs::read_to_string(base.join("tpm_version_major"))
                {
                    match major.trim() {
                        "2" => TpmVersion::V2_0,
                        "1" => TpmVersion::V1_2,
                        _ => TpmVersion::Unknown,
                    }
                } else {
                    // Fallback: check caps or device path
                    if std::path::Path::new("/dev/tpmrm0").exists() {
                        TpmVersion::V2_0
                    } else if std::path::Path::new("/dev/tpm0").exists() {
                        TpmVersion::V1_2
                    } else {
                        TpmVersion::Unknown
                    }
                };

                // Read manufacturer from caps
                let caps = std::fs::read_to_string(base.join("caps")).unwrap_or_default();
                let mut manufacturer = String::new();
                let mut firmware_version = String::new();
                for line in caps.lines() {
                    if let Some(m) = line.strip_prefix("Manufacturer:") {
                        manufacturer = m.trim().to_string();
                    }
                    if let Some(v) = line.strip_prefix("Firmware version:") {
                        firmware_version = v.trim().to_string();
                    }
                }

                // Fallback vendor detection from device description
                if manufacturer.is_empty() {
                    manufacturer = std::fs::read_to_string(base.join("device/description"))
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                }

                // PCR banks
                let pcr_path = base.join("pcr-sha256");
                let pcr_banks = if pcr_path.exists() {
                    std::fs::read_dir(&pcr_path)
                        .map(|d| d.count() as u32)
                        .unwrap_or(0)
                } else {
                    0
                };

                // Algorithms
                let mut algorithms = Vec::new();
                for algo in &["sha1", "sha256", "sha384", "sha512", "sm3-256"] {
                    if base.join(format!("pcr-{}", algo)).exists() {
                        algorithms.push(algo.to_uppercase());
                    }
                }
                if algorithms.is_empty() {
                    // Default for TPM 2.0
                    if version == TpmVersion::V2_0 {
                        algorithms = vec!["SHA-1".into(), "SHA-256".into(), "RSA".into(), "ECC".into()];
                    } else {
                        algorithms = vec!["SHA-1".into(), "RSA".into()];
                    }
                }

                // Measured boot check (IMA or tpm_bios)
                let measured_boot = std::path::Path::new("/sys/kernel/security/ima").exists()
                    || std::path::Path::new("/sys/kernel/security/tpm0/binary_bios_measurements")
                        .exists();

                self.tpm_info = Some(TpmInfo {
                    device: name,
                    version,
                    status: TpmStatus::Enabled,
                    manufacturer,
                    firmware_version,
                    device_path: format!("/dev/{}", entry.file_name().to_string_lossy()),
                    is_primary: true,
                    algorithms,
                    pcr_banks,
                    measured_boot,
                });
                break; // Usually only one TPM
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // Try WMI Win32_Tpm (requires admin, but attempt anyway)
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance -Namespace 'root/cimv2/Security/MicrosoftTpm' -ClassName Win32_Tpm -ErrorAction SilentlyContinue | Select-Object IsActivated_InitialValue, IsEnabled_InitialValue, IsOwned_InitialValue, ManufacturerIdTxt, ManufacturerVersion, SpecVersion, PhysicalPresenceVersionInfo | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let spec = val["SpecVersion"].as_str().unwrap_or("");
                    let version = if spec.starts_with("2.0") || spec.contains("2.0") {
                        TpmVersion::V2_0
                    } else if spec.starts_with("1.2") || spec.contains("1.2") {
                        TpmVersion::V1_2
                    } else {
                        TpmVersion::Unknown
                    };

                    let enabled = val["IsEnabled_InitialValue"].as_bool().unwrap_or(false);
                    let activated = val["IsActivated_InitialValue"].as_bool().unwrap_or(false);
                    let status = if enabled && activated {
                        TpmStatus::Enabled
                    } else if enabled {
                        TpmStatus::Locked
                    } else {
                        TpmStatus::Disabled
                    };

                    let manufacturer = val["ManufacturerIdTxt"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let firmware_version = val["ManufacturerVersion"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    let mut algorithms = vec!["SHA-1".into(), "SHA-256".into()];
                    if version == TpmVersion::V2_0 {
                        algorithms.extend(["RSA".to_string(), "ECC".to_string()]);
                    }

                    self.tpm_info = Some(TpmInfo {
                        device: "tpm0".into(),
                        version,
                        status,
                        manufacturer,
                        firmware_version,
                        device_path: r"\\.\TPM".into(),
                        is_primary: true,
                        algorithms,
                        pcr_banks: 24,
                        measured_boot: true, // Windows with TPM implies measured boot
                    });
                    return;
                }
            }
        }

        // Fallback: check registry for TPM existence
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "if (Test-Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\TPM') { 'present' } else { 'absent' }"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if text.trim() == "present" {
                    self.tpm_info = Some(TpmInfo {
                        device: "tpm0".into(),
                        version: TpmVersion::Unknown,
                        status: TpmStatus::Unknown,
                        manufacturer: String::new(),
                        firmware_version: String::new(),
                        device_path: r"\\.\TPM".into(),
                        is_primary: true,
                        algorithms: Vec::new(),
                        pcr_banks: 0,
                        measured_boot: false,
                    });
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        // macOS uses Secure Enclave instead of discrete TPM
        // T1 chip (2016 MBP), T2 chip (2018+), Apple Silicon (M1+) all have SE
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                let has_se = text.contains("Apple M")
                    || text.contains("Apple T2")
                    || text.contains("Apple T1");
                if has_se {
                    self.tpm_info = Some(TpmInfo {
                        device: "sep0".into(),
                        version: TpmVersion::SecureEnclave,
                        status: TpmStatus::Enabled,
                        manufacturer: "Apple".into(),
                        firmware_version: String::new(),
                        device_path: String::new(),
                        is_primary: true,
                        algorithms: vec!["AES-256".into(), "SHA-256".into(), "ECC-P256".into()],
                        pcr_banks: 0,
                        measured_boot: true,
                    });
                }
            }
        }
    }
}

impl Default for TpmMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self { tpm_info: None })
    }
}

impl std::fmt::Display for TpmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1_2 => write!(f, "1.2"),
            Self::V2_0 => write!(f, "2.0"),
            Self::SecureEnclave => write!(f, "Secure Enclave"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tpm_monitor_creation() {
        let monitor = TpmMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_tpm_monitor_default() {
        let monitor = TpmMonitor::default();
        let _ = monitor.has_tpm();
        let _ = monitor.has_tpm2();
    }

    #[test]
    fn test_tpm_version_display() {
        assert_eq!(TpmVersion::V2_0.to_string(), "2.0");
        assert_eq!(TpmVersion::V1_2.to_string(), "1.2");
        assert_eq!(TpmVersion::SecureEnclave.to_string(), "Secure Enclave");
    }

    #[test]
    fn test_tpm_info_serialization() {
        let info = TpmInfo {
            device: "tpm0".into(),
            version: TpmVersion::V2_0,
            status: TpmStatus::Enabled,
            manufacturer: "IFX".into(),
            firmware_version: "7.85".into(),
            device_path: "/dev/tpm0".into(),
            is_primary: true,
            algorithms: vec!["SHA-256".into(), "RSA".into()],
            pcr_banks: 24,
            measured_boot: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("tpm0"));
        let _: TpmInfo = serde_json::from_str(&json).unwrap();
    }
}

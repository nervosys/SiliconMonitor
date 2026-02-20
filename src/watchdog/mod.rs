//! Hardware and software watchdog timer monitoring.
//!
//! Enumerates watchdog devices, reports timeout configuration, pre-timeout
//! governors, identity, firmware version, and status flags.
//!
//! ## Platform Support
//!
//! - **Linux**: `/sys/class/watchdog/`, `/dev/watchdog*`
//! - **Windows / macOS**: Basic detection only

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

/// Watchdog device type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchdogType {
    /// Hardware watchdog (e.g. iTCO, SP5100).
    Hardware,
    /// Software watchdog (softdog).
    Software,
    Unknown,
}

impl std::fmt::Display for WatchdogType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hardware => write!(f, "hardware"),
            Self::Software => write!(f, "software"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Watchdog status flags.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WatchdogStatus {
    /// Watchdog is active (currently ticking).
    pub active: bool,
    /// Watchdog has triggered at least once.
    pub triggered: bool,
    /// Boot status — did watchdog cause last reboot?
    pub boot_triggered: bool,
}

/// Pre-timeout governor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreTimeoutGovernor {
    /// No pre-timeout action.
    Noop,
    /// Panic on pre-timeout.
    Panic,
    /// Custom governor.
    Custom(String),
    /// No pre-timeout configured.
    None,
}

impl std::fmt::Display for PreTimeoutGovernor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Noop => write!(f, "noop"),
            Self::Panic => write!(f, "panic"),
            Self::Custom(s) => write!(f, "{}", s),
            Self::None => write!(f, "none"),
        }
    }
}

/// Information about a single watchdog device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogInfo {
    /// Device name (e.g. "watchdog0").
    pub name: String,
    /// Device identity (driver name, e.g. "iTCO_wdt").
    pub identity: String,
    /// Watchdog type.
    pub watchdog_type: WatchdogType,
    /// Timeout in seconds.
    pub timeout_secs: u32,
    /// Pre-timeout in seconds (0 = disabled).
    pub pretimeout_secs: u32,
    /// Pre-timeout governor.
    pub pretimeout_governor: PreTimeoutGovernor,
    /// Minimum timeout in seconds.
    pub min_timeout_secs: u32,
    /// Maximum timeout in seconds.
    pub max_timeout_secs: u32,
    /// Firmware version (if available).
    pub firmware_version: u32,
    /// Status.
    pub status: WatchdogStatus,
    /// Available pre-timeout governors.
    pub available_governors: Vec<String>,
}

impl WatchdogInfo {
    /// Whether timeout is at default value (usually 30-60 seconds).
    pub fn is_default_timeout(&self) -> bool {
        self.timeout_secs == 30 || self.timeout_secs == 60
    }
}

/// Overview of all watchdog devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogOverview {
    /// All watchdog devices.
    pub devices: Vec<WatchdogInfo>,
    /// Total count.
    pub total_count: u32,
    /// Active count.
    pub active_count: u32,
    /// Hardware watchdog count.
    pub hardware_count: u32,
    /// Recommendations.
    pub recommendations: Vec<String>,
}

/// Watchdog monitor.
pub struct WatchdogMonitor {
    overview: WatchdogOverview,
}

impl WatchdogMonitor {
    /// Create a new watchdog monitor.
    pub fn new() -> Result<Self, SimonError> {
        let overview = Self::scan()?;
        Ok(Self { overview })
    }

    /// Refresh.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.overview = Self::scan()?;
        Ok(())
    }

    /// Get overview.
    pub fn overview(&self) -> &WatchdogOverview {
        &self.overview
    }

    /// Get devices.
    pub fn devices(&self) -> &[WatchdogInfo] {
        &self.overview.devices
    }

    /// Find device by name.
    pub fn device(&self, name: &str) -> Option<&WatchdogInfo> {
        self.overview.devices.iter().find(|d| d.name == name)
    }

    #[cfg(target_os = "linux")]
    fn scan() -> Result<WatchdogOverview, SimonError> {
        let wdt_path = std::path::Path::new("/sys/class/watchdog");

        if !wdt_path.exists() {
            return Ok(Self::empty_overview());
        }

        let entries = std::fs::read_dir(wdt_path).map_err(SimonError::Io)?;
        let mut devices = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            let identity = Self::read_sysfs(&path.join("identity")).unwrap_or_else(|| "unknown".into());

            let watchdog_type = if identity.contains("soft") || identity.contains("Soft") {
                WatchdogType::Software
            } else if identity != "unknown" {
                WatchdogType::Hardware
            } else {
                WatchdogType::Unknown
            };

            let timeout_secs = Self::read_sysfs_u32(&path.join("timeout")).unwrap_or(0);
            let pretimeout_secs = Self::read_sysfs_u32(&path.join("pretimeout")).unwrap_or(0);
            let min_timeout_secs = Self::read_sysfs_u32(&path.join("min_timeout")).unwrap_or(0);
            let max_timeout_secs = Self::read_sysfs_u32(&path.join("max_timeout")).unwrap_or(0);
            let firmware_version = Self::read_sysfs_u32(&path.join("fw_version")).unwrap_or(0);

            let pretimeout_governor = match Self::read_sysfs(&path.join("pretimeout_governor")).as_deref() {
                Some("noop") => PreTimeoutGovernor::Noop,
                Some("panic") => PreTimeoutGovernor::Panic,
                Some("") | None => PreTimeoutGovernor::None,
                Some(other) => PreTimeoutGovernor::Custom(other.to_string()),
            };

            let available_governors = Self::read_sysfs(&path.join("pretimeout_available_governors"))
                .map(|s| s.split_whitespace().map(String::from).collect())
                .unwrap_or_default();

            // Status from state file
            let state_str = Self::read_sysfs(&path.join("state")).unwrap_or_default();
            let active = state_str == "active";

            let bootstatus = Self::read_sysfs_u32(&path.join("bootstatus")).unwrap_or(0);
            let boot_triggered = bootstatus != 0;

            let status = WatchdogStatus {
                active,
                triggered: false,
                boot_triggered,
            };

            devices.push(WatchdogInfo {
                name,
                identity,
                watchdog_type,
                timeout_secs,
                pretimeout_secs,
                pretimeout_governor,
                min_timeout_secs,
                max_timeout_secs,
                firmware_version,
                status,
                available_governors,
            });
        }

        devices.sort_by(|a, b| a.name.cmp(&b.name));

        let total = devices.len() as u32;
        let active = devices.iter().filter(|d| d.status.active).count() as u32;
        let hw = devices.iter().filter(|d| d.watchdog_type == WatchdogType::Hardware).count() as u32;

        let mut recs = Vec::new();
        if hw == 0 && total > 0 {
            recs.push("No hardware watchdog detected; software watchdog only".into());
        }
        for dev in &devices {
            if dev.status.boot_triggered {
                recs.push(format!("{}: watchdog-triggered reboot detected in boot status", dev.name));
            }
        }

        Ok(WatchdogOverview {
            devices,
            total_count: total,
            active_count: active,
            hardware_count: hw,
            recommendations: recs,
        })
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs(path: &std::path::Path) -> Option<String> {
        std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs_u32(path: &std::path::Path) -> Option<u32> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    #[cfg(not(target_os = "linux"))]
    fn scan() -> Result<WatchdogOverview, SimonError> {
        Ok(Self::empty_overview())
    }

    fn empty_overview() -> WatchdogOverview {
        WatchdogOverview {
            devices: Vec::new(),
            total_count: 0,
            active_count: 0,
            hardware_count: 0,
            recommendations: Vec::new(),
        }
    }
}

impl Default for WatchdogMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            overview: Self::empty_overview(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_display() {
        assert_eq!(WatchdogType::Hardware.to_string(), "hardware");
        assert_eq!(WatchdogType::Software.to_string(), "software");
    }

    #[test]
    fn test_governor_display() {
        assert_eq!(PreTimeoutGovernor::Panic.to_string(), "panic");
        assert_eq!(PreTimeoutGovernor::Noop.to_string(), "noop");
        assert_eq!(PreTimeoutGovernor::None.to_string(), "none");
    }

    #[test]
    fn test_default_timeout() {
        let info = WatchdogInfo {
            name: "watchdog0".into(),
            identity: "iTCO_wdt".into(),
            watchdog_type: WatchdogType::Hardware,
            timeout_secs: 30,
            pretimeout_secs: 0,
            pretimeout_governor: PreTimeoutGovernor::None,
            min_timeout_secs: 2,
            max_timeout_secs: 614,
            firmware_version: 0,
            status: WatchdogStatus { active: false, triggered: false, boot_triggered: false },
            available_governors: Vec::new(),
        };
        assert!(info.is_default_timeout());
    }

    #[test]
    fn test_monitor_default() {
        let monitor = WatchdogMonitor::default();
        let _overview = monitor.overview();
    }

    #[test]
    fn test_serialization() {
        let info = WatchdogInfo {
            name: "watchdog0".into(),
            identity: "softdog".into(),
            watchdog_type: WatchdogType::Software,
            timeout_secs: 60,
            pretimeout_secs: 10,
            pretimeout_governor: PreTimeoutGovernor::Panic,
            min_timeout_secs: 1,
            max_timeout_secs: 65535,
            firmware_version: 0,
            status: WatchdogStatus { active: true, triggered: false, boot_triggered: false },
            available_governors: vec!["noop".into(), "panic".into()],
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("softdog"));
        let _: WatchdogInfo = serde_json::from_str(&json).unwrap();
    }
}

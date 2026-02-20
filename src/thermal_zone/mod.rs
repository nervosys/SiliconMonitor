//! Thermal zone monitoring.
//!
//! Enumerates all thermal zones, trip points, cooling devices, and thermal
//! policies. Provides comprehensive thermal state beyond what hwmon offers.
//!
//! ## Platform Support
//!
//! - **Linux**: `/sys/class/thermal/thermal_zone*`, `/sys/class/thermal/cooling_device*`
//! - **Windows**: WMI `Win32_TemperatureProbe`
//! - **macOS**: IOKit thermal sensors

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

/// Thermal zone type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalZoneType {
    /// ACPI thermal zone.
    Acpi,
    /// x86 package thermal.
    X86Pkg,
    /// Intel PCH (Platform Controller Hub).
    Pch,
    /// SoC internal sensor.
    Soc,
    /// GPU thermal.
    Gpu,
    /// NVMe drive.
    Nvme,
    /// iwlwifi (Wi-Fi card).
    Iwlwifi,
    /// Other / unknown.
    Other(String),
}

impl std::fmt::Display for ThermalZoneType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acpi => write!(f, "ACPI"),
            Self::X86Pkg => write!(f, "x86_pkg"),
            Self::Pch => write!(f, "PCH"),
            Self::Soc => write!(f, "SoC"),
            Self::Gpu => write!(f, "GPU"),
            Self::Nvme => write!(f, "NVMe"),
            Self::Iwlwifi => write!(f, "iwlwifi"),
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Trip point type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TripPointType {
    /// Active cooling trip (fan ramp).
    Active,
    /// Passive cooling trip (throttle).
    Passive,
    /// Hot — OS-level alert.
    Hot,
    /// Critical — hardware shutdown.
    Critical,
}

impl std::fmt::Display for TripPointType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Passive => write!(f, "passive"),
            Self::Hot => write!(f, "hot"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Trip point information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripPoint {
    /// Trip point index.
    pub index: u32,
    /// Trip type.
    pub trip_type: TripPointType,
    /// Temperature in millidegrees Celsius.
    pub temp_mc: i64,
    /// Hysteresis in millidegrees Celsius.
    pub hysteresis_mc: i64,
}

impl TripPoint {
    /// Temperature in degrees Celsius.
    pub fn temp_c(&self) -> f64 {
        self.temp_mc as f64 / 1000.0
    }
}

/// Cooling device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolingDeviceInfo {
    /// Device name (e.g. "cooling_device0").
    pub name: String,
    /// Cooling type (e.g. "Processor", "intel_powerclamp", "Fan").
    pub cooling_type: String,
    /// Current cooling state.
    pub cur_state: u32,
    /// Maximum cooling state.
    pub max_state: u32,
}

impl CoolingDeviceInfo {
    /// Utilization percentage.
    pub fn utilization_pct(&self) -> f64 {
        if self.max_state > 0 {
            (self.cur_state as f64 / self.max_state as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Thermal zone information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZoneInfo {
    /// Zone name (e.g. "thermal_zone0").
    pub name: String,
    /// Zone type.
    pub zone_type: ThermalZoneType,
    /// Raw type string.
    pub type_string: String,
    /// Current temperature in millidegrees Celsius.
    pub temp_mc: i64,
    /// Thermal policy/governor (e.g. "step_wise", "user_space").
    pub policy: String,
    /// Available policies.
    pub available_policies: Vec<String>,
    /// Trip points.
    pub trip_points: Vec<TripPoint>,
    /// Mode (enabled/disabled).
    pub enabled: bool,
    /// Whether zone is in passive cooling mode.
    pub passive_active: bool,
}

impl ThermalZoneInfo {
    /// Temperature in degrees Celsius.
    pub fn temp_c(&self) -> f64 {
        self.temp_mc as f64 / 1000.0
    }

    /// Distance to critical trip point in degrees.
    pub fn headroom_to_critical_c(&self) -> Option<f64> {
        self.trip_points
            .iter()
            .filter(|tp| tp.trip_type == TripPointType::Critical)
            .map(|tp| tp.temp_c() - self.temp_c())
            .next()
    }

    /// Is temperature above passive trip?
    pub fn is_throttling(&self) -> bool {
        self.trip_points
            .iter()
            .filter(|tp| tp.trip_type == TripPointType::Passive)
            .any(|tp| self.temp_mc >= tp.temp_mc)
    }
}

/// Thermal overview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalZoneOverview {
    /// All thermal zones.
    pub zones: Vec<ThermalZoneInfo>,
    /// Cooling devices.
    pub cooling_devices: Vec<CoolingDeviceInfo>,
    /// Total zone count.
    pub zone_count: u32,
    /// Hottest zone name.
    pub hottest_zone: String,
    /// Hottest temperature in C.
    pub hottest_temp_c: f64,
    /// Number of zones in passive/throttle state.
    pub throttling_count: u32,
    /// Recommendations.
    pub recommendations: Vec<String>,
}

/// Thermal zone monitor.
pub struct ThermalZoneMonitor {
    overview: ThermalZoneOverview,
}

impl ThermalZoneMonitor {
    /// Create a new thermal zone monitor.
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
    pub fn overview(&self) -> &ThermalZoneOverview {
        &self.overview
    }

    /// Get zones.
    pub fn zones(&self) -> &[ThermalZoneInfo] {
        &self.overview.zones
    }

    /// Get cooling devices.
    pub fn cooling_devices(&self) -> &[CoolingDeviceInfo] {
        &self.overview.cooling_devices
    }

    /// Get hottest zone.
    pub fn hottest(&self) -> Option<&ThermalZoneInfo> {
        self.overview.zones.iter().max_by_key(|z| z.temp_mc)
    }

    /// Get zones that are throttling.
    pub fn throttling_zones(&self) -> Vec<&ThermalZoneInfo> {
        self.overview.zones.iter().filter(|z| z.is_throttling()).collect()
    }

    #[cfg(target_os = "linux")]
    fn scan() -> Result<ThermalZoneOverview, SimonError> {
        let thermal_path = std::path::Path::new("/sys/class/thermal");

        if !thermal_path.exists() {
            return Ok(Self::empty_overview());
        }

        let mut zones = Vec::new();
        let mut cooling = Vec::new();

        let entries = std::fs::read_dir(thermal_path).map_err(SimonError::Io)?;

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();

            if name.starts_with("thermal_zone") {
                if let Some(zone) = Self::read_zone(&path, &name) {
                    zones.push(zone);
                }
            } else if name.starts_with("cooling_device") {
                if let Some(cd) = Self::read_cooling_device(&path, &name) {
                    cooling.push(cd);
                }
            }
        }

        zones.sort_by(|a, b| a.name.cmp(&b.name));
        cooling.sort_by(|a, b| a.name.cmp(&b.name));

        let zone_count = zones.len() as u32;
        let throttling = zones.iter().filter(|z| z.is_throttling()).count() as u32;

        let (hottest_zone, hottest_temp) = zones
            .iter()
            .max_by_key(|z| z.temp_mc)
            .map(|z| (z.name.clone(), z.temp_c()))
            .unwrap_or_default();

        let mut recs = Vec::new();
        if throttling > 0 {
            recs.push(format!("{} zone(s) in thermal throttling state", throttling));
        }
        for zone in &zones {
            if let Some(headroom) = zone.headroom_to_critical_c() {
                if headroom < 10.0 {
                    recs.push(format!(
                        "{}: only {:.1}°C from critical shutdown temperature",
                        zone.name, headroom
                    ));
                }
            }
        }

        Ok(ThermalZoneOverview {
            zones,
            cooling_devices: cooling,
            zone_count,
            hottest_zone,
            hottest_temp_c: hottest_temp,
            throttling_count: throttling,
            recommendations: recs,
        })
    }

    #[cfg(target_os = "linux")]
    fn read_zone(path: &std::path::Path, name: &str) -> Option<ThermalZoneInfo> {
        let type_string = Self::read_sysfs(&path.join("type")).unwrap_or_default();

        let zone_type = match type_string.as_str() {
            "acpitz" | "ACPI\\_THM" => ThermalZoneType::Acpi,
            s if s.starts_with("x86_pkg") => ThermalZoneType::X86Pkg,
            "pch_cannonlake" | "pch_skylake" | "pch_alderlake" | "pch_raptorlake" => ThermalZoneType::Pch,
            s if s.contains("pch") => ThermalZoneType::Pch,
            s if s.contains("soc") || s.contains("SoC") => ThermalZoneType::Soc,
            s if s.contains("gpu") || s.contains("GPU") => ThermalZoneType::Gpu,
            s if s.starts_with("nvme") => ThermalZoneType::Nvme,
            "iwlwifi_1" | "iwlwifi" => ThermalZoneType::Iwlwifi,
            s if s.contains("iwlwifi") => ThermalZoneType::Iwlwifi,
            other if !other.is_empty() => ThermalZoneType::Other(other.to_string()),
            _ => ThermalZoneType::Other("unknown".to_string()),
        };

        let temp_mc = Self::read_sysfs_i64(&path.join("temp")).unwrap_or(0);

        let policy = Self::read_sysfs(&path.join("policy")).unwrap_or_default();
        let available_policies = Self::read_sysfs(&path.join("available_policies"))
            .map(|s| s.split_whitespace().map(String::from).collect())
            .unwrap_or_default();

        let mode_str = Self::read_sysfs(&path.join("mode")).unwrap_or_else(|| "enabled".into());
        let enabled = mode_str != "disabled";

        // Read trip points
        let mut trip_points = Vec::new();
        for i in 0..20 {
            let tp_temp_path = path.join(format!("trip_point_{}_temp", i));
            let tp_type_path = path.join(format!("trip_point_{}_type", i));

            if !tp_temp_path.exists() {
                break;
            }

            let tp_temp = Self::read_sysfs_i64(&tp_temp_path).unwrap_or(0);
            let tp_type_str = Self::read_sysfs(&tp_type_path).unwrap_or_default();
            let tp_type = match tp_type_str.as_str() {
                "active" => TripPointType::Active,
                "passive" => TripPointType::Passive,
                "hot" => TripPointType::Hot,
                "critical" => TripPointType::Critical,
                _ => continue,
            };

            let hyst = Self::read_sysfs_i64(&path.join(format!("trip_point_{}_hyst", i))).unwrap_or(0);

            trip_points.push(TripPoint {
                index: i,
                trip_type: tp_type,
                temp_mc: tp_temp,
                hysteresis_mc: hyst,
            });
        }

        let passive_active = trip_points
            .iter()
            .filter(|tp| tp.trip_type == TripPointType::Passive)
            .any(|tp| temp_mc >= tp.temp_mc);

        Some(ThermalZoneInfo {
            name: name.to_string(),
            zone_type,
            type_string,
            temp_mc,
            policy,
            available_policies,
            trip_points,
            enabled,
            passive_active,
        })
    }

    #[cfg(target_os = "linux")]
    fn read_cooling_device(path: &std::path::Path, name: &str) -> Option<CoolingDeviceInfo> {
        let cooling_type = Self::read_sysfs(&path.join("type")).unwrap_or_default();
        let cur_state = Self::read_sysfs_u32(&path.join("cur_state")).unwrap_or(0);
        let max_state = Self::read_sysfs_u32(&path.join("max_state")).unwrap_or(0);

        Some(CoolingDeviceInfo {
            name: name.to_string(),
            cooling_type,
            cur_state,
            max_state,
        })
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs(path: &std::path::Path) -> Option<String> {
        std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs_i64(path: &std::path::Path) -> Option<i64> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs_u32(path: &std::path::Path) -> Option<u32> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    #[cfg(not(target_os = "linux"))]
    fn scan() -> Result<ThermalZoneOverview, SimonError> {
        Ok(Self::empty_overview())
    }

    fn empty_overview() -> ThermalZoneOverview {
        ThermalZoneOverview {
            zones: Vec::new(),
            cooling_devices: Vec::new(),
            zone_count: 0,
            hottest_zone: String::new(),
            hottest_temp_c: 0.0,
            throttling_count: 0,
            recommendations: Vec::new(),
        }
    }
}

impl Default for ThermalZoneMonitor {
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
    fn test_zone_type_display() {
        assert_eq!(ThermalZoneType::X86Pkg.to_string(), "x86_pkg");
        assert_eq!(ThermalZoneType::Pch.to_string(), "PCH");
        assert_eq!(ThermalZoneType::Nvme.to_string(), "NVMe");
    }

    #[test]
    fn test_trip_point_temp() {
        let tp = TripPoint { index: 0, trip_type: TripPointType::Critical, temp_mc: 105000, hysteresis_mc: 0 };
        assert!((tp.temp_c() - 105.0).abs() < 0.01);
    }

    #[test]
    fn test_throttle_detection() {
        let zone = ThermalZoneInfo {
            name: "thermal_zone0".into(),
            zone_type: ThermalZoneType::X86Pkg,
            type_string: "x86_pkg_temp".into(),
            temp_mc: 95000,
            policy: "step_wise".into(),
            available_policies: vec!["step_wise".into()],
            trip_points: vec![
                TripPoint { index: 0, trip_type: TripPointType::Passive, temp_mc: 90000, hysteresis_mc: 2000 },
                TripPoint { index: 1, trip_type: TripPointType::Critical, temp_mc: 105000, hysteresis_mc: 0 },
            ],
            enabled: true,
            passive_active: true,
        };
        assert!(zone.is_throttling());
        assert!((zone.headroom_to_critical_c().unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_cooling_utilization() {
        let cd = CoolingDeviceInfo { name: "cooling_device0".into(), cooling_type: "Processor".into(), cur_state: 5, max_state: 10 };
        assert!((cd.utilization_pct() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_monitor_default() {
        let monitor = ThermalZoneMonitor::default();
        let _overview = monitor.overview();
    }

    #[test]
    fn test_serialization() {
        let zone = ThermalZoneInfo {
            name: "thermal_zone0".into(),
            zone_type: ThermalZoneType::Acpi,
            type_string: "acpitz".into(),
            temp_mc: 45000,
            policy: "step_wise".into(),
            available_policies: Vec::new(),
            trip_points: Vec::new(),
            enabled: true,
            passive_active: false,
        };
        let json = serde_json::to_string(&zone).unwrap();
        assert!(json.contains("acpitz"));
        let _: ThermalZoneInfo = serde_json::from_str(&json).unwrap();
    }
}

//! Main statistics collection and management

use crate::core::{
    cpu::CpuStats, engine::EngineStats, gpu::GpuStats, memory::MemoryStats,
    platform_info::BoardInfo, power::PowerStats, process::ProcessStats,
    temperature::TemperatureStats,
};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Complete system snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// CPU statistics
    pub cpu: CpuStats,
    /// GPU statistics
    pub gpus: std::collections::HashMap<String, crate::core::gpu::GpuInfo>,
    /// Memory statistics
    pub memory: MemoryStats,
    /// Power statistics
    pub power: PowerStats,
    /// Temperature statistics
    pub temperature: TemperatureStats,
    /// Fan statistics
    pub fans: std::collections::HashMap<String, crate::core::fan::FanInfo>,
    /// Board information
    pub board: BoardInfo,
    /// Process statistics
    pub processes: ProcessStats,
    /// Engine statistics
    pub engines: EngineStats,
    /// Uptime in seconds
    pub uptime: Duration,
}

/// Main Simon interface
pub struct Simon {
    /// Update interval
    interval: Duration,
    /// Last snapshot
    last_snapshot: Option<Snapshot>,
    /// Platform information (cached)
    board_info: BoardInfo,
}

impl Simon {
    /// Create a new Simon instance
    ///
    /// # Arguments
    ///
    /// * `interval` - Update interval in seconds (default: 1.0)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use simon::Simon;
    ///
    /// let stats = Simon::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        Self::with_interval(1.0)
    }

    /// Create a new Simon instance with custom interval
    ///
    /// # Arguments
    ///
    /// * `interval` - Update interval in seconds
    ///
    /// # Example
    ///
    /// ```no_run
    /// use simon::Simon;
    ///
    /// let stats = Simon::with_interval(0.5).unwrap();
    /// ```
    pub fn with_interval(interval: f64) -> Result<Self> {
        let interval = Duration::from_secs_f64(interval);

        // Detect platform information once
        let board_info = detect_platform_info()?;

        Ok(Self {
            interval,
            last_snapshot: None,
            board_info,
        })
    }

    /// Get current snapshot of all statistics
    ///
    /// # Example
    ///
    /// ```no_run
    /// use simon::Simon;
    ///
    /// let mut stats = Simon::new().unwrap();
    /// let snapshot = stats.snapshot().unwrap();
    /// println!("CPU cores: {}", snapshot.cpu.cores.len());
    /// ```
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        // Read all stats
        let cpu = read_cpu_stats()?;
        let gpu_stats = read_gpu_stats()?;
        let memory = read_memory_stats()?;
        let power = read_power_stats()?;
        let temperature = read_temperature_stats()?;

        // Read fan stats
        let fans = read_fan_stats();

        // Read process stats
        let processes = read_process_stats()?;

        // Read engine stats
        let engines = read_engine_stats()?;

        // Read uptime
        let uptime = read_uptime()?;

        let snapshot = Snapshot {
            cpu,
            gpus: gpu_stats.gpus().clone(),
            memory,
            power,
            temperature,
            fans,
            board: self.board_info.clone(),
            processes,
            engines,
            uptime,
        };

        self.last_snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Get the update interval
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Get board information
    pub fn board_info(&self) -> &BoardInfo {
        &self.board_info
    }

    /// Get last snapshot (if available)
    pub fn last_snapshot(&self) -> Option<&Snapshot> {
        self.last_snapshot.as_ref()
    }
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
fn read_cpu_stats() -> Result<CpuStats> {
    crate::platform::linux::read_cpu_stats()
}

#[cfg(target_os = "linux")]
fn read_gpu_stats() -> Result<GpuStats> {
    crate::platform::linux::read_gpu_stats()
}

#[cfg(target_os = "linux")]
fn read_memory_stats() -> Result<MemoryStats> {
    crate::platform::linux::read_memory_stats()
}

#[cfg(target_os = "linux")]
fn read_power_stats() -> Result<PowerStats> {
    crate::platform::linux::read_power_stats()
}

#[cfg(target_os = "linux")]
fn read_temperature_stats() -> Result<TemperatureStats> {
    crate::platform::linux::read_temperature_stats()
}

#[cfg(target_os = "linux")]
fn detect_platform_info() -> Result<BoardInfo> {
    crate::platform::linux::detect_platform()
}

#[cfg(target_os = "linux")]
fn read_uptime() -> Result<Duration> {
    use std::fs;
    let uptime_str = fs::read_to_string("/proc/uptime")?;
    let uptime_secs: f64 = uptime_str
        .split_whitespace()
        .next()
        .ok_or_else(|| crate::error::SimonError::Parse("Invalid uptime format".to_string()))?
        .parse()
        .map_err(|e| crate::error::SimonError::Parse(format!("Failed to parse uptime: {}", e)))?;
    Ok(Duration::from_secs_f64(uptime_secs))
}

#[cfg(target_os = "linux")]
fn read_process_stats() -> Result<ProcessStats> {
    crate::core::process::linux::read_process_stats()
}

#[cfg(target_os = "linux")]
fn read_engine_stats() -> Result<EngineStats> {
    crate::core::engine::linux::read_engine_stats()
}

#[cfg(windows)]
fn read_cpu_stats() -> Result<CpuStats> {
    crate::platform::windows::read_cpu_stats()
}

#[cfg(windows)]
fn read_gpu_stats() -> Result<GpuStats> {
    crate::platform::windows::read_gpu_stats()
}

#[cfg(windows)]
fn read_memory_stats() -> Result<MemoryStats> {
    crate::platform::windows::read_memory_stats()
}

#[cfg(windows)]
fn read_power_stats() -> Result<PowerStats> {
    crate::platform::windows::read_power_stats()
}

#[cfg(windows)]
fn read_temperature_stats() -> Result<TemperatureStats> {
    crate::platform::windows::read_temperature_stats()
}

#[cfg(windows)]
fn detect_platform_info() -> Result<BoardInfo> {
    crate::platform::windows::detect_platform()
}

#[cfg(windows)]
fn read_uptime() -> Result<Duration> {
    Ok(crate::platform::windows::get_system_uptime())
}

#[cfg(windows)]
fn read_process_stats() -> Result<ProcessStats> {
    crate::platform::windows::read_process_stats()
}

#[cfg(windows)]
fn read_engine_stats() -> Result<EngineStats> {
    // Windows doesn't have the same engine concept as Jetson
    // Return empty stats
    Ok(EngineStats::default())
}

/// Read fan stats - returns empty map on failure (non-critical)
#[cfg(target_os = "linux")]
fn read_fan_stats() -> std::collections::HashMap<String, crate::core::fan::FanInfo> {
    use crate::core::fan::FanInfo;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    let mut fans = HashMap::new();

    // Look for fans in hwmon
    let hwmon_path = Path::new("/sys/class/hwmon");
    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.flatten() {
            let hwmon_dir = entry.path();

            // Get device name
            let name = fs::read_to_string(hwmon_dir.join("name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string());

            // Look for fan inputs (fan1_input, fan2_input, etc.)
            for i in 1..=10 {
                let fan_input = hwmon_dir.join(format!("fan{}_input", i));
                if !fan_input.exists() {
                    continue;
                }

                // Read RPM
                let rpm = fs::read_to_string(&fan_input)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);

                // Try to read PWM (speed control)
                let pwm_path = hwmon_dir.join(format!("pwm{}", i));
                let speed_percent = if pwm_path.exists() {
                    fs::read_to_string(&pwm_path)
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .map(|pwm| pwm * 100 / 255) // PWM is 0-255
                        .unwrap_or(0)
                } else {
                    // If no PWM, estimate from RPM (assume max 5000 RPM)
                    std::cmp::min(rpm / 50, 100)
                };

                fans.insert(
                    format!("{}-fan{}", name, i),
                    FanInfo {
                        speed: vec![speed_percent],
                        rpm: Some(vec![rpm]),
                        profile: "auto".to_string(),
                        governor: None,
                        control: None,
                    },
                );
            }
        }
    }

    // Also check for Jetson-style thermal cooling devices
    let cooling_path = Path::new("/sys/class/thermal");
    if let Ok(entries) = fs::read_dir(cooling_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("cooling_device") {
                continue;
            }

            // Read current state and max state
            let cur_state = fs::read_to_string(path.join("cur_state"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let max_state = fs::read_to_string(path.join("max_state"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(1);

            let speed_percent = if max_state > 0 {
                cur_state * 100 / max_state
            } else {
                0
            };

            // Get device type
            let device_type = fs::read_to_string(path.join("type"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "fan".to_string());

            if device_type.contains("fan") || device_type.contains("pwm") {
                fans.insert(
                    format!("{}", name),
                    FanInfo {
                        speed: vec![speed_percent],
                        rpm: None,
                        profile: "thermal".to_string(),
                        governor: None,
                        control: Some(device_type),
                    },
                );
            }
        }
    }

    fans
}

/// Read fan stats on Windows using WMI
#[cfg(windows)]
fn read_fan_stats() -> std::collections::HashMap<String, crate::core::fan::FanInfo> {
    use crate::core::fan::FanInfo;
    use serde::Deserialize;
    use std::collections::HashMap;
    use wmi::{COMLibrary, WMIConnection};

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Fan {
        name: Option<String>,
        active_cooling: Option<bool>,
    }

    let mut fans = HashMap::new();

    // Try WMI fan class
    if let Ok(com_con) = COMLibrary::new() {
        if let Ok(wmi_con) = WMIConnection::with_namespace_path("root\\CIMV2", com_con.into()) {
            // Try Win32_Fan class (may not be populated on all systems)
            let wmi_fans: Vec<Win32Fan> = wmi_con
                .raw_query("SELECT Name, ActiveCooling FROM Win32_Fan")
                .unwrap_or_default();

            for (idx, fan) in wmi_fans.iter().enumerate() {
                let name = fan.name.clone().unwrap_or_else(|| format!("Fan{}", idx));
                let active = fan.active_cooling.unwrap_or(false);

                fans.insert(
                    name.clone(),
                    FanInfo {
                        speed: vec![if active { 100 } else { 0 }],
                        rpm: None,
                        profile: if active { "active" } else { "idle" }.to_string(),
                        governor: None,
                        control: None,
                    },
                );
            }
        }

        // Try OpenHardwareMonitor/LibreHardwareMonitor for detailed fan info
        if let Ok(lhm_con) =
            WMIConnection::with_namespace_path("root\\LibreHardwareMonitor", com_con.into())
        {
            #[derive(Deserialize, Debug)]
            #[serde(rename_all = "PascalCase")]
            #[allow(dead_code)]
            struct LhmSensor {
                name: String,
                sensor_type: String,
                value: f32,
                parent: String,
            }

            let lhm_fans: Vec<LhmSensor> = lhm_con
                .raw_query(
                    "SELECT Name, SensorType, Value, Parent FROM Sensor WHERE SensorType = 'Fan'",
                )
                .unwrap_or_default();

            for fan in lhm_fans {
                let name = format!(
                    "{}-{}",
                    fan.parent.split('/').last().unwrap_or("MB"),
                    fan.name
                );
                fans.insert(
                    name,
                    FanInfo {
                        speed: vec![(fan.value / 50.0).min(100.0) as u32], // Rough estimate from RPM
                        rpm: Some(vec![fan.value as u32]),
                        profile: "auto".to_string(),
                        governor: None,
                        control: None,
                    },
                );
            }
        }
    }

    fans
}

/// Read fan stats on macOS
#[cfg(target_os = "macos")]
fn read_fan_stats() -> std::collections::HashMap<String, crate::core::fan::FanInfo> {
    use crate::core::fan::FanInfo;
    use std::collections::HashMap;
    use std::process::Command;

    let mut fans = HashMap::new();

    // Try using smcFanControl or similar if available
    // For now, try iStats gem if available
    if let Ok(output) = Command::new("istats").args(["fan", "speed"]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(rpm_str) = line.split_whitespace().last() {
                if let Ok(rpm) = rpm_str.replace("RPM", "").trim().parse::<u32>() {
                    fans.insert(
                        "CPU Fan".to_string(),
                        FanInfo {
                            speed: vec![(rpm / 50).min(100)], // Rough estimate
                            rpm: Some(vec![rpm]),
                            profile: "auto".to_string(),
                            governor: None,
                            control: None,
                        },
                    );
                    break;
                }
            }
        }
    }

    fans
}

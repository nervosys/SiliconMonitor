//! Windows platform implementation
//!
//! Provides CPU, memory, and system monitoring for Windows using Windows APIs.

use crate::core::{
    cpu::{CpuCore, CpuFrequency, CpuStats, CpuTotal},
    gpu::GpuStats,
    memory::{MemoryStats, RamInfo, SwapInfo},
    platform_info::{BoardInfo, HardwareInfo, LibraryVersions, PlatformInfo},
    power::PowerStats,
    temperature::TemperatureStats,
};
use crate::error::{SimonError, Result};
use std::collections::HashMap;
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{
    GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};

/// Previous CPU times for utilization calculation
static PREV_IDLE_TIME: AtomicU64 = AtomicU64::new(0);
static PREV_KERNEL_TIME: AtomicU64 = AtomicU64::new(0);
static PREV_USER_TIME: AtomicU64 = AtomicU64::new(0);

/// Read CPU statistics on Windows
pub fn read_cpu_stats() -> Result<CpuStats> {
    let mut stats = CpuStats::new()?;

    // Get number of processors
    let mut sys_info: SYSTEM_INFO = unsafe { mem::zeroed() };
    unsafe { GetSystemInfo(&mut sys_info) };
    let cpu_count = sys_info.dwNumberOfProcessors as usize;

    // Get system times for overall CPU utilization
    let (user_percent, system_percent, idle_percent) = get_system_cpu_utilization()?;

    // Create cores (Windows doesn't provide per-core stats easily without PDH)
    for cpu_id in 0..cpu_count {
        let core = CpuCore {
            id: cpu_id,
            online: true,
            governor: "windows".to_string(),
            frequency: get_cpu_frequency(),
            user: Some(user_percent),
            nice: Some(0.0), // Windows doesn't have nice
            system: Some(system_percent),
            idle: Some(idle_percent),
            model: get_cpu_model_name(),
        };
        stats.cores.push(core);
    }

    // Set totals
    stats.total = CpuTotal {
        user: user_percent,
        nice: 0.0,
        system: system_percent,
        idle: idle_percent,
    };

    Ok(stats)
}

/// Get CPU utilization using kernel32 GetSystemTimes via FFI
fn get_system_cpu_utilization() -> Result<(f32, f32, f32)> {
    use windows::Win32::Foundation::FILETIME;

    // GetSystemTimes is in kernel32.dll - use raw FFI call
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(
            lpIdleTime: *mut FILETIME,
            lpKernelTime: *mut FILETIME,
            lpUserTime: *mut FILETIME,
        ) -> i32;
    }

    let mut idle_time: FILETIME = unsafe { mem::zeroed() };
    let mut kernel_time: FILETIME = unsafe { mem::zeroed() };
    let mut user_time: FILETIME = unsafe { mem::zeroed() };

    let result = unsafe { GetSystemTimes(&mut idle_time, &mut kernel_time, &mut user_time) };

    if result == 0 {
        return Err(SimonError::System("GetSystemTimes failed".to_string()));
    }

    // Convert FILETIME to u64
    let idle = filetime_to_u64(&idle_time);
    let kernel = filetime_to_u64(&kernel_time);
    let user = filetime_to_u64(&user_time);

    // Get previous values
    let prev_idle = PREV_IDLE_TIME.load(Ordering::Relaxed);
    let prev_kernel = PREV_KERNEL_TIME.load(Ordering::Relaxed);
    let prev_user = PREV_USER_TIME.load(Ordering::Relaxed);

    // Store current values for next calculation
    PREV_IDLE_TIME.store(idle, Ordering::Relaxed);
    PREV_KERNEL_TIME.store(kernel, Ordering::Relaxed);
    PREV_USER_TIME.store(user, Ordering::Relaxed);

    // Calculate deltas
    let idle_delta = idle.saturating_sub(prev_idle);
    let kernel_delta = kernel.saturating_sub(prev_kernel);
    let user_delta = user.saturating_sub(prev_user);

    // Kernel time includes idle time
    let system_delta = kernel_delta.saturating_sub(idle_delta);
    let total = idle_delta + system_delta + user_delta;

    if total == 0 || prev_idle == 0 {
        // First call or no change - return reasonable defaults
        return Ok((0.0, 0.0, 100.0));
    }

    let idle_percent = (idle_delta as f64 / total as f64 * 100.0) as f32;
    let system_percent = (system_delta as f64 / total as f64 * 100.0) as f32;
    let user_percent = (user_delta as f64 / total as f64 * 100.0) as f32;

    Ok((user_percent, system_percent, idle_percent))
}

fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

/// Get CPU frequency (approximate from registry or processor info)
fn get_cpu_frequency() -> Option<CpuFrequency> {
    use std::process::Command;

    // Try to get CPU frequency from wmic
    let output = Command::new("wmic")
        .args(["cpu", "get", "CurrentClockSpeed,MaxClockSpeed"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().collect();

    if lines.len() >= 2 {
        let values: Vec<&str> = lines[1].split_whitespace().collect();
        if values.len() >= 2 {
            let current = values[0].parse::<u32>().unwrap_or(0);
            let max = values[1].parse::<u32>().unwrap_or(current);
            return Some(CpuFrequency {
                current,
                min: 0, // Windows doesn't easily provide min freq
                max,
            });
        }
    }

    None
}

/// Get CPU model name from registry
fn get_cpu_model_name() -> String {
    use std::process::Command;

    let output = Command::new("wmic")
        .args(["cpu", "get", "Name"])
        .output()
        .ok();

    if let Some(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() >= 2 {
            return lines[1].trim().to_string();
        }
    }

    "Unknown CPU".to_string()
}

/// Read memory statistics on Windows
pub fn read_memory_stats() -> Result<MemoryStats> {
    let mut mem_status: MEMORYSTATUSEX = unsafe { mem::zeroed() };
    mem_status.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;

    unsafe {
        GlobalMemoryStatusEx(&mut mem_status)
            .map_err(|e| SimonError::System(format!("GlobalMemoryStatusEx failed: {}", e)))?;
    }

    // Get performance info for page file (swap) details
    let mut perf_info: PERFORMANCE_INFORMATION = unsafe { mem::zeroed() };
    perf_info.cb = mem::size_of::<PERFORMANCE_INFORMATION>() as u32;

    let swap_info = if unsafe { GetPerformanceInfo(&mut perf_info, perf_info.cb) }.is_ok() {
        let page_size = perf_info.PageSize as u64;
        let total_pages = perf_info.CommitLimit as u64;
        let used_pages = perf_info.CommitTotal as u64;

        SwapInfo {
            total: (total_pages * page_size) / 1024, // Convert to KB
            used: (used_pages * page_size) / 1024,
            cached: 0,
        }
    } else {
        // Fallback using MEMORYSTATUSEX pagefile info
        SwapInfo {
            total: (mem_status.ullTotalPageFile - mem_status.ullTotalPhys) / 1024,
            used: (mem_status.ullTotalPageFile - mem_status.ullAvailPageFile) / 1024,
            cached: 0,
        }
    };

    let total_kb = mem_status.ullTotalPhys / 1024;
    let avail_kb = mem_status.ullAvailPhys / 1024;
    let used_kb = total_kb - avail_kb;

    Ok(MemoryStats {
        ram: RamInfo {
            total: total_kb,
            used: used_kb,
            free: avail_kb,
            buffers: 0, // Windows doesn't expose this separately
            cached: 0,  // Could use GetPerformanceInfo for SystemCache
            shared: 0,
            lfb: None,
        },
        swap: swap_info,
        emc: None,  // Not applicable to Windows
        iram: None, // Not applicable to Windows
    })
}

/// Get system uptime on Windows
pub fn get_system_uptime() -> std::time::Duration {
    use windows::Win32::System::SystemInformation::GetTickCount64;

    let ticks = unsafe { GetTickCount64() };
    std::time::Duration::from_millis(ticks)
}

/// Read GPU stats - deferred to gpu module for proper NVML handling
pub fn read_gpu_stats() -> Result<GpuStats> {
    // Return empty GPU stats - the gpu module handles this properly with NVML
    // This avoids breaking snapshot() while keeping the proper recommendation
    Ok(GpuStats::new())
}

pub fn read_power_stats() -> Result<PowerStats> {
    use serde::Deserialize;
    use wmi::{COMLibrary, WMIConnection};

    // WMI battery structure
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct Win32Battery {
        name: Option<String>,
        estimated_charge_remaining: Option<u16>,
        battery_status: Option<u16>,
        design_capacity: Option<u32>,
        full_charge_capacity: Option<u32>,
        design_voltage: Option<u32>,
    }

    let mut power_stats = PowerStats::default();

    // Initialize COM library for WMI access
    if let Ok(com_con) = COMLibrary::new() {
        if let Ok(wmi_con) = WMIConnection::with_namespace_path("root\\CIMV2", com_con.into()) {
            // Query battery info (laptops)
            let batteries: Vec<Win32Battery> = wmi_con
                .raw_query("SELECT Name, EstimatedChargeRemaining, BatteryStatus, DesignCapacity, FullChargeCapacity, DesignVoltage FROM Win32_Battery")
                .unwrap_or_default();

            for (idx, battery) in batteries.iter().enumerate() {
                let name = battery
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Battery{}", idx));

                // Calculate power from design capacity and voltage (mWh to mW approximation)
                let power_mw = if let (Some(capacity), Some(voltage)) =
                    (battery.full_charge_capacity, battery.design_voltage)
                {
                    // Rough approximation: capacity in mWh, voltage in mV
                    // Power ~= (capacity * voltage / 1000) / discharge_hours
                    // Assume ~3 hour discharge for rough estimate
                    (capacity * voltage / 1000 / 3) as u32
                } else {
                    0
                };

                // Determine status from battery_status field
                // 1 = Discharging, 2 = AC Power, 3 = Fully Charged, etc.
                let _is_charging = battery.battery_status.map_or(false, |s| s == 2 || s == 3);

                power_stats.rails.insert(
                    name,
                    crate::core::power::PowerRail {
                        online: true,
                        sensor_type: "Battery".to_string(),
                        voltage: battery.design_voltage.unwrap_or(0),
                        current: 0, // Not available via WMI
                        power: power_mw,
                        average: power_mw,
                        warn: None,
                        crit: None,
                    },
                );

                power_stats.total.power += power_mw;
                power_stats.total.average += power_mw;
            }
        }
    }

    Ok(power_stats)
}

/// Read temperature stats from WMI thermal zones and optionally Open Hardware Monitor
pub fn read_temperature_stats() -> Result<TemperatureStats> {
    use serde::Deserialize;
    use std::collections::HashMap;
    use wmi::{COMLibrary, WMIConnection};

    // WMI thermal zone structure (in tenths of Kelvin)
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct ThermalZone {
        instance_name: String,
        current_temperature: u32, // In tenths of Kelvin
    }

    // Performance counter thermal zone (Kelvin)
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    struct PerfThermalZone {
        name: String,
        temperature: u32, // In Kelvin (not tenths!)
    }

    // Open Hardware Monitor sensor structure (if OHM is installed)
    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct OhmSensor {
        name: String,
        sensor_type: String,
        value: f32,
        parent: String,
    }

    let mut sensors = HashMap::new();

    // Initialize COM library
    let com_con = COMLibrary::new()
        .map_err(|e| SimonError::System(format!("Failed to initialize COM: {}", e)))?;

    // Try to get CPU temperature from Open Hardware Monitor if available
    // OHM exposes sensors via WMI in root\OpenHardwareMonitor namespace
    if let Ok(ohm_con) =
        WMIConnection::with_namespace_path("root\\OpenHardwareMonitor", com_con.into())
    {
        let ohm_sensors: Vec<OhmSensor> = ohm_con
            .raw_query("SELECT Name, SensorType, Value, Parent FROM Sensor WHERE SensorType = 'Temperature'")
            .unwrap_or_default();

        for sensor in ohm_sensors {
            // Filter to CPU and motherboard temperatures
            let sensor_name = if sensor.parent.contains("CPU") || sensor.name.contains("CPU") {
                format!("CPU-{}", sensor.name.replace(' ', "_"))
            } else if sensor.parent.contains("Motherboard") {
                format!("MB-{}", sensor.name.replace(' ', "_"))
            } else {
                continue; // Skip other sensors (GPU temps come from NVML)
            };

            sensors.insert(
                sensor_name,
                crate::core::temperature::TemperatureSensor {
                    online: true,
                    temp: sensor.value,
                    max: Some(95.0),
                    crit: Some(105.0),
                },
            );
        }
    }

    // Try LibreHardwareMonitor as well (fork of OHM with better modern hardware support)
    if let Ok(lhm_con) =
        WMIConnection::with_namespace_path("root\\LibreHardwareMonitor", com_con.into())
    {
        let lhm_sensors: Vec<OhmSensor> = lhm_con
            .raw_query("SELECT Name, SensorType, Value, Parent FROM Sensor WHERE SensorType = 'Temperature'")
            .unwrap_or_default();

        for sensor in lhm_sensors {
            // Filter to CPU and motherboard temperatures
            let sensor_name = if sensor.parent.contains("CPU") || sensor.name.contains("CPU") {
                format!("CPU-{}", sensor.name.replace(' ', "_"))
            } else if sensor.parent.contains("Motherboard") {
                format!("MB-{}", sensor.name.replace(' ', "_"))
            } else {
                continue;
            };

            // Only add if not already present from OHM
            if !sensors.contains_key(&sensor_name) {
                sensors.insert(
                    sensor_name,
                    crate::core::temperature::TemperatureSensor {
                        online: true,
                        temp: sensor.value,
                        max: Some(95.0),
                        crit: Some(105.0),
                    },
                );
            }
        }
    }

    // Try CIMV2 performance counters for thermal zone info (more widely available)
    if let Ok(cimv2_con) = WMIConnection::with_namespace_path("root\\CIMV2", com_con.into()) {
        let perf_zones: Vec<PerfThermalZone> = cimv2_con
            .raw_query("SELECT Name, Temperature FROM Win32_PerfFormattedData_Counters_ThermalZoneInformation")
            .unwrap_or_default();

        for zone in perf_zones {
            // Temperature is in Kelvin, convert to Celsius
            let temp_celsius = zone.temperature as f32 - 273.15;

            // Only add valid temperatures
            if temp_celsius > 0.0 && temp_celsius < 150.0 {
                let sensor_name = format!("TZ-{}", zone.name.replace("\\_TZ.", ""));
                if !sensors.contains_key(&sensor_name) {
                    sensors.insert(
                        sensor_name,
                        crate::core::temperature::TemperatureSensor {
                            online: true,
                            temp: temp_celsius,
                            max: Some(100.0),
                            crit: Some(105.0),
                        },
                    );
                }
            }
        }
    }

    // Try ACPI thermal zones in root\WMI (requires admin, but try anyway)
    if let Ok(wmi_con) = WMIConnection::with_namespace_path("root\\WMI", com_con.into()) {
        let thermal_zones: Vec<ThermalZone> = wmi_con
            .raw_query("SELECT InstanceName, CurrentTemperature FROM MSAcpi_ThermalZoneTemperature")
            .unwrap_or_default();

        for zone in thermal_zones {
            // Convert from tenths of Kelvin to Celsius
            let temp_celsius = (zone.current_temperature as f32 / 10.0) - 273.15;

            // Only add valid temperatures (ignore invalid readings)
            if temp_celsius > 0.0 && temp_celsius < 150.0 {
                let sensor_name = zone
                    .instance_name
                    .replace("ACPI\\ThermalZone\\", "")
                    .replace("_0", "");
                if !sensors.contains_key(&format!("TZ-{}", sensor_name)) {
                    sensors.insert(
                        format!("ACPI-{}", sensor_name),
                        crate::core::temperature::TemperatureSensor {
                            online: true,
                            temp: temp_celsius,
                            max: Some(100.0),
                            crit: Some(105.0),
                        },
                    );
                }
            }
        }
    }

    Ok(TemperatureStats { sensors })
}

pub fn detect_platform() -> Result<BoardInfo> {
    use std::process::Command;

    // Get manufacturer and model from wmic
    let manufacturer = Command::new("wmic")
        .args(["baseboard", "get", "Manufacturer"])
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().nth(1).map(|s| s.trim().to_string())
        });

    let model = Command::new("wmic")
        .args(["baseboard", "get", "Product"])
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().nth(1).map(|s| s.trim().to_string())
        });

    Ok(BoardInfo {
        platform: PlatformInfo {
            machine: std::env::consts::ARCH.to_string(),
            system: "Windows".to_string(),
            distribution: None,
            release: "NT".to_string(),
        },
        hardware: HardwareInfo {
            model: model.unwrap_or_else(|| "Unknown".to_string()),
            p_number: None,
            module: manufacturer,
            soc: None,
            cuda_arch: None,
            codename: None,
            serial_number: None,
            l4t: None,
            jetpack: None,
        },
        libraries: LibraryVersions {
            cuda: None,
            cudnn: None,
            tensorrt: None,
            other: HashMap::new(),
        },
    })
}

/// Read process statistics on Windows using CreateToolhelp32Snapshot
pub fn read_process_stats() -> Result<crate::core::process::ProcessStats> {
    use crate::core::process::{ProcessInfo, ProcessStats};
    use std::mem;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let mut stats = ProcessStats::new()?;

    unsafe {
        // Create snapshot of all processes
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| {
            SimonError::System(format!("Failed to create process snapshot: {}", e))
        })?;

        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

        // Iterate through processes
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let pid = entry.th32ProcessID;

                // Get process name from entry
                let name_len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);

                // Try to open process for more info
                let mut memory_kb = 0u64;
                let mut cpu_percent = 0.0f32;

                if let Ok(process_handle) =
                    OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
                {
                    // Get memory info
                    let mut mem_counters: PROCESS_MEMORY_COUNTERS = mem::zeroed();
                    mem_counters.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

                    if GetProcessMemoryInfo(
                        process_handle,
                        &mut mem_counters,
                        mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                    )
                    .is_ok()
                    {
                        memory_kb = mem_counters.WorkingSetSize as u64 / 1024;
                    }

                    // Get CPU times for utilization calculation
                    let mut creation_time = mem::zeroed();
                    let mut exit_time = mem::zeroed();
                    let mut kernel_time = mem::zeroed();
                    let mut user_time = mem::zeroed();

                    if GetProcessTimes(
                        process_handle,
                        &mut creation_time,
                        &mut exit_time,
                        &mut kernel_time,
                        &mut user_time,
                    )
                    .is_ok()
                    {
                        let kernel = (kernel_time.dwHighDateTime as u64) << 32
                            | kernel_time.dwLowDateTime as u64;
                        let user = (user_time.dwHighDateTime as u64) << 32
                            | user_time.dwLowDateTime as u64;

                        // Simple estimate: total CPU time / uptime
                        let total_time = kernel + user;
                        let uptime_100ns = get_system_uptime().as_nanos() as u64 / 100;
                        if uptime_100ns > 0 {
                            cpu_percent = (total_time as f64 / uptime_100ns as f64 * 100.0) as f32;
                            cpu_percent = cpu_percent.min(100.0);
                        }
                    }

                    let _ = CloseHandle(process_handle);
                }

                // Determine process state based on thread count
                let state = if entry.cntThreads > 0 { 'R' } else { 'S' };

                // Only include processes with significant memory usage (> 1MB)
                if memory_kb > 1024 {
                    stats.processes.push(ProcessInfo {
                        pid,
                        user: String::new(), // Would need additional API calls
                        gpu: String::new(),
                        process_type: "System".to_string(),
                        priority: entry.pcPriClassBase as i32,
                        state,
                        cpu_percent,
                        memory_kb,
                        gpu_memory_kb: 0, // Filled in by GPU module
                        name,
                    });
                }

                // Move to next process
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    // Sort by memory usage (descending)
    stats
        .processes
        .sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb));

    // Keep top 50 processes
    stats.processes.truncate(50);

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_cpu_stats() {
        let stats = read_cpu_stats();
        assert!(stats.is_ok());
        let stats = stats.unwrap();
        assert!(!stats.cores.is_empty());
    }

    #[test]
    fn test_read_memory_stats() {
        let stats = read_memory_stats();
        assert!(stats.is_ok());
        let stats = stats.unwrap();
        assert!(stats.ram.total > 0);
    }

    #[test]
    fn test_get_system_uptime() {
        let uptime = get_system_uptime();
        assert!(uptime.as_secs() > 0);
    }
}

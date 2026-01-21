//! Linux CPU monitoring

use crate::core::cpu::{CpuCore, CpuFrequency, CpuStats, CpuTotal};
use crate::error::{SimonError, Result};
use crate::platform::common::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Read CPU statistics
pub fn read_cpu_stats() -> Result<CpuStats> {
    let mut stats = CpuStats::new()?;

    // Read CPU times from /proc/stat
    let proc_stat = fs::read_to_string("/proc/stat")?;
    let cpu_times = parse_proc_stat(&proc_stat)?;

    // Get number of CPUs
    let cpu_count = get_cpu_count();

    // Read per-core information
    for cpu_id in 0..cpu_count {
        let core = read_cpu_core(cpu_id, &cpu_times)?;
        stats.cores.push(core);
    }

    // Calculate totals
    stats.total = calculate_total(&stats.cores);

    Ok(stats)
}

fn get_cpu_count() -> usize {
    let online = fs::read_to_string("/sys/devices/system/cpu/online")
        .ok()
        .and_then(|s| parse_cpu_range(&s));

    if let Some(count) = online {
        count
    } else {
        // Fallback to counting CPU directories
        fs::read_dir("/sys/devices/system/cpu")
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name().to_string_lossy().starts_with("cpu")
                            && e.file_name().to_string_lossy()[3..]
                                .chars()
                                .all(|c| c.is_ascii_digit())
                    })
                    .count()
            })
            .unwrap_or(1)
    }
}

fn parse_cpu_range(range: &str) -> Option<usize> {
    // Parse ranges like "0-7" or "0,2-5,7"
    let parts: Vec<&str> = range.trim().split(',').collect();
    let mut max_cpu = 0;

    for part in parts {
        if let Some(hyphen_pos) = part.find('-') {
            let end = part[hyphen_pos + 1..].parse::<usize>().ok()?;
            max_cpu = max_cpu.max(end);
        } else {
            let cpu = part.parse::<usize>().ok()?;
            max_cpu = max_cpu.max(cpu);
        }
    }

    Some(max_cpu + 1)
}

fn read_cpu_core(cpu_id: usize, cpu_times: &[(String, Vec<u64>)]) -> Result<CpuCore> {
    let cpu_path = format!("/sys/devices/system/cpu/cpu{}", cpu_id);
    let online_path = format!("{}/online", cpu_path);

    // Check if CPU is online
    let online = if Path::new(&online_path).exists() {
        read_file_u32(&online_path)? == 1
    } else {
        true // CPU0 doesn't have online file
    };

    // Read governor
    let governor_path = format!("{}/cpufreq/scaling_governor", cpu_path);
    let governor = read_file_string(&governor_path).unwrap_or_else(|_| "unknown".to_string());

    // Read frequency
    let frequency = if online {
        read_cpu_frequency(&cpu_path).ok()
    } else {
        None
    };

    // Get CPU times for this core
    let cpu_name = format!("cpu{}", cpu_id);
    let (user, nice, system, idle) = cpu_times
        .iter()
        .find(|(name, _)| name == &cpu_name)
        .map(|(_, times)| calculate_cpu_percentages(times))
        .unwrap_or((None, None, None, Some(100.0)));

    // Read CPU model
    let model = read_cpu_model();

    Ok(CpuCore {
        id: cpu_id,
        online,
        governor,
        frequency,
        user,
        nice,
        system,
        idle,
        model,
    })
}

/// Read CPU temperature for a specific core
/// Tries multiple sources: hwmon, thermal_zone, coretemp
pub fn read_cpu_temperature(cpu_id: usize) -> Option<i32> {
    // Try hwmon (modern systems)
    if let Some(temp) = read_hwmon_temperature(cpu_id) {
        return Some(temp);
    }

    // Try thermal_zone
    if let Some(temp) = read_thermal_zone_temperature(cpu_id) {
        return Some(temp);
    }

    // Try coretemp
    if let Some(temp) = read_coretemp_temperature(cpu_id) {
        return Some(temp);
    }

    None
}

fn read_hwmon_temperature(cpu_id: usize) -> Option<i32> {
    // Search /sys/class/hwmon for CPU temperature sensors
    let hwmon_dir = "/sys/class/hwmon";
    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.flatten() {
            let hwmon_path = entry.path();

            // Check if this is a CPU temperature sensor
            if let Ok(name) = fs::read_to_string(hwmon_path.join("name")) {
                let name = name.trim();
                if name.contains("coretemp")
                    || name.contains("k10temp")
                    || name.contains("zenpower")
                {
                    // Try to find the specific core's temperature
                    for i in 2..20 {
                        // temp1 is usually package, temp2+ are cores
                        let label_path = hwmon_path.join(format!("temp{}_label", i));
                        let input_path = hwmon_path.join(format!("temp{}_input", i));

                        if let Ok(label) = fs::read_to_string(&label_path) {
                            let label = label.trim();
                            if label.contains(&format!("Core {}", cpu_id)) {
                                if let Ok(temp_str) = fs::read_to_string(&input_path) {
                                    if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                                        return Some(temp_millic / 1000); // Convert to Celsius
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn read_thermal_zone_temperature(cpu_id: usize) -> Option<i32> {
    // Try /sys/class/thermal/thermal_zone*/temp
    let thermal_dir = "/sys/class/thermal";
    if let Ok(entries) = fs::read_dir(thermal_dir) {
        for entry in entries.flatten() {
            let zone_path = entry.path();

            if let Ok(zone_type) = fs::read_to_string(zone_path.join("type")) {
                let zone_type = zone_type.trim();
                if zone_type.contains(&format!("cpu{}", cpu_id))
                    || zone_type.contains(&format!("core{}", cpu_id))
                    || zone_type.contains(&format!("x86_pkg_temp"))
                {
                    if let Ok(temp_str) = fs::read_to_string(zone_path.join("temp")) {
                        if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                            return Some(temp_millic / 1000); // Convert to Celsius
                        }
                    }
                }
            }
        }
    }
    None
}

fn read_coretemp_temperature(cpu_id: usize) -> Option<i32> {
    // Fallback: try the package temperature as an approximation
    let paths = [
        "/sys/class/hwmon/hwmon0/temp1_input",
        "/sys/class/hwmon/hwmon1/temp1_input",
        "/sys/class/thermal/thermal_zone0/temp",
    ];

    for path in &paths {
        if let Ok(temp_str) = fs::read_to_string(path) {
            if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                return Some(temp_millic / 1000);
            }
        }
    }

    None
}

/// Get CPU temperatures for all cores
pub fn read_all_cpu_temperatures() -> HashMap<usize, i32> {
    let mut temperatures = HashMap::new();
    let cpu_count = get_cpu_count();

    for cpu_id in 0..cpu_count {
        if let Some(temp) = read_cpu_temperature(cpu_id) {
            temperatures.insert(cpu_id, temp);
        }
    }

    temperatures
}

fn read_cpu_frequency(cpu_path: &str) -> Result<CpuFrequency> {
    let cur_path = format!("{}/cpufreq/scaling_cur_freq", cpu_path);
    let min_path = format!("{}/cpufreq/scaling_min_freq", cpu_path);
    let max_path = format!("{}/cpufreq/scaling_max_freq", cpu_path);

    Ok(CpuFrequency {
        current: (read_file_u32(&cur_path)? / 1000), // Convert kHz to MHz
        min: (read_file_u32(&min_path)? / 1000),
        max: (read_file_u32(&max_path)? / 1000),
    })
}

fn read_cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            for line in content.lines() {
                if line.starts_with("model name") || line.starts_with("Processor") {
                    if let Some(pos) = line.find(':') {
                        return Some(line[pos + 1..].trim().to_string());
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

fn parse_proc_stat(content: &str) -> Result<Vec<(String, Vec<u64>)>> {
    let mut cpu_times = Vec::new();

    for line in content.lines() {
        if line.starts_with("cpu") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let cpu_name = parts[0].to_string();
            let times: Vec<u64> = parts[1..].iter().filter_map(|s| s.parse().ok()).collect();

            cpu_times.push((cpu_name, times));
        }
    }

    Ok(cpu_times)
}

fn calculate_cpu_percentages(
    times: &[u64],
) -> (Option<f32>, Option<f32>, Option<f32>, Option<f32>) {
    if times.len() < 4 {
        return (None, None, None, None);
    }

    let user = times[0];
    let nice = times[1];
    let system = times[2];
    let idle = times[3];

    let total = times.iter().sum::<u64>() as f32;

    if total == 0.0 {
        return (Some(0.0), Some(0.0), Some(0.0), Some(100.0));
    }

    (
        Some((user as f32 / total) * 100.0),
        Some((nice as f32 / total) * 100.0),
        Some((system as f32 / total) * 100.0),
        Some((idle as f32 / total) * 100.0),
    )
}

fn calculate_total(cores: &[CpuCore]) -> CpuTotal {
    let online_cores: Vec<&CpuCore> = cores.iter().filter(|c| c.online).collect();

    if online_cores.is_empty() {
        return CpuTotal {
            user: 0.0,
            nice: 0.0,
            system: 0.0,
            idle: 100.0,
        };
    }

    let count = online_cores.len() as f32;

    CpuTotal {
        user: online_cores.iter().filter_map(|c| c.user).sum::<f32>() / count,
        nice: online_cores.iter().filter_map(|c| c.nice).sum::<f32>() / count,
        system: online_cores.iter().filter_map(|c| c.system).sum::<f32>() / count,
        idle: online_cores.iter().filter_map(|c| c.idle).sum::<f32>() / count,
    }
}

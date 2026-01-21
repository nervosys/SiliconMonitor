// Linux-specific hardware monitoring implementations
//
// On Linux, hardware sensors are typically exposed via:
// - /sys/class/hwmon/* for temperatures, fans, voltages
// - /sys/devices/system/cpu/* for CPU info
// - /sys/class/drm/* for GPU info
// - /sys/class/block/*/device/* for storage

use super::{HwSensor, HwSensorType, HwType};
use std::fs;
use std::path::Path;

/// Read all hardware sensors from /sys/class/hwmon
pub fn read_all_hwmon_sensors() -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    let hwmon_path = Path::new("/sys/class/hwmon");
    if !hwmon_path.exists() {
        return sensors;
    }

    if let Ok(entries) = fs::read_dir(hwmon_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Get chip name
            let chip_name = fs::read_to_string(path.join("name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| entry.file_name().to_string_lossy().to_string());

            // Determine hardware type based on chip name
            let hw_type = classify_hwmon_chip(&chip_name);

            // Read temperature sensors
            sensors.extend(read_temperature_inputs(&path, &chip_name, hw_type));

            // Read fan sensors
            sensors.extend(read_fan_inputs(&path, &chip_name, hw_type));

            // Read voltage sensors
            sensors.extend(read_voltage_inputs(&path, &chip_name, hw_type));

            // Read power sensors
            sensors.extend(read_power_inputs(&path, &chip_name, hw_type));
        }
    }

    sensors
}

fn classify_hwmon_chip(name: &str) -> HwType {
    let name_lower = name.to_lowercase();

    if name_lower.contains("coretemp")
        || name_lower.contains("k10temp")
        || name_lower.contains("zenpower")
        || name_lower.contains("cpu")
    {
        HwType::Cpu
    } else if name_lower.contains("amdgpu")
        || name_lower.contains("nvidia")
        || name_lower.contains("radeon")
        || name_lower.contains("nouveau")
        || name_lower.contains("i915")
    {
        HwType::Gpu
    } else if name_lower.contains("nvme")
        || name_lower.contains("drivetemp")
        || name_lower.contains("ata")
    {
        HwType::Storage
    } else if name_lower.contains("nct")
        || name_lower.contains("it87")
        || name_lower.contains("f7")
        || name_lower.contains("nuvoton")
        || name_lower.contains("fintek")
        || name_lower.contains("smsc")
    {
        HwType::Motherboard
    } else {
        HwType::Other
    }
}

fn read_temperature_inputs(path: &Path, chip_name: &str, hw_type: HwType) -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    // Find all temp*_input files
    for i in 1..=16 {
        let input_file = path.join(format!("temp{}_input", i));
        if input_file.exists() {
            if let Ok(temp_str) = fs::read_to_string(&input_file) {
                if let Ok(temp_mc) = temp_str.trim().parse::<i32>() {
                    let temp_c = temp_mc as f32 / 1000.0;

                    // Get label if available
                    let label = fs::read_to_string(path.join(format!("temp{}_label", i)))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| format!("Temp {}", i));

                    let sensor_name = if label != format!("Temp {}", i) {
                        format!("{} {}", chip_name, label)
                    } else {
                        format!("{} Temp {}", chip_name, i)
                    };

                    // Get min/max if available
                    let min = fs::read_to_string(path.join(format!("temp{}_min", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .map(|v| v as f32 / 1000.0);

                    let max = fs::read_to_string(path.join(format!("temp{}_max", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .map(|v| v as f32 / 1000.0);

                    sensors.push(HwSensor {
                        name: sensor_name,
                        value: temp_c,
                        min,
                        max,
                        sensor_type: HwSensorType::Temperature,
                        hardware_type: hw_type,
                    });
                }
            }
        }
    }

    sensors
}

fn read_fan_inputs(path: &Path, chip_name: &str, hw_type: HwType) -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    // Find all fan*_input files (RPM)
    for i in 1..=8 {
        let input_file = path.join(format!("fan{}_input", i));
        if input_file.exists() {
            if let Ok(rpm_str) = fs::read_to_string(&input_file) {
                if let Ok(rpm) = rpm_str.trim().parse::<u32>() {
                    // Get label if available
                    let label = fs::read_to_string(path.join(format!("fan{}_label", i)))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| format!("Fan {}", i));

                    let sensor_name = if label != format!("Fan {}", i) {
                        format!("{} {}", chip_name, label)
                    } else {
                        format!("{} Fan {}", chip_name, i)
                    };

                    // Get min/max if available
                    let min = fs::read_to_string(path.join(format!("fan{}_min", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .map(|v| v as f32);

                    let max = fs::read_to_string(path.join(format!("fan{}_max", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .map(|v| v as f32);

                    sensors.push(HwSensor {
                        name: sensor_name,
                        value: rpm as f32,
                        min,
                        max,
                        sensor_type: HwSensorType::Fan,
                        hardware_type: hw_type,
                    });
                }
            }
        }

        // Also check pwm (fan speed control, 0-255)
        let pwm_file = path.join(format!("pwm{}", i));
        if pwm_file.exists() {
            if let Ok(pwm_str) = fs::read_to_string(&pwm_file) {
                if let Ok(pwm) = pwm_str.trim().parse::<u8>() {
                    // Convert 0-255 to percentage
                    let percent = (pwm as f32 / 255.0) * 100.0;

                    sensors.push(HwSensor {
                        name: format!("{} PWM {}", chip_name, i),
                        value: percent,
                        min: Some(0.0),
                        max: Some(100.0),
                        sensor_type: HwSensorType::Control,
                        hardware_type: hw_type,
                    });
                }
            }
        }
    }

    sensors
}

fn read_voltage_inputs(path: &Path, chip_name: &str, hw_type: HwType) -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    // Find all in*_input files (voltage in millivolts)
    for i in 0..=16 {
        let input_file = path.join(format!("in{}_input", i));
        if input_file.exists() {
            if let Ok(mv_str) = fs::read_to_string(&input_file) {
                if let Ok(mv) = mv_str.trim().parse::<i32>() {
                    let volts = mv as f32 / 1000.0;

                    // Get label if available
                    let label = fs::read_to_string(path.join(format!("in{}_label", i)))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| format!("Voltage {}", i));

                    let sensor_name = if label != format!("Voltage {}", i) {
                        format!("{} {}", chip_name, label)
                    } else {
                        format!("{} Voltage {}", chip_name, i)
                    };

                    // Get min/max if available
                    let min = fs::read_to_string(path.join(format!("in{}_min", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .map(|v| v as f32 / 1000.0);

                    let max = fs::read_to_string(path.join(format!("in{}_max", i)))
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok())
                        .map(|v| v as f32 / 1000.0);

                    sensors.push(HwSensor {
                        name: sensor_name,
                        value: volts,
                        min,
                        max,
                        sensor_type: HwSensorType::Voltage,
                        hardware_type: hw_type,
                    });
                }
            }
        }
    }

    sensors
}

fn read_power_inputs(path: &Path, chip_name: &str, hw_type: HwType) -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    // Find all power*_input files (power in microwatts)
    for i in 1..=4 {
        let input_file = path.join(format!("power{}_input", i));
        if input_file.exists() {
            if let Ok(uw_str) = fs::read_to_string(&input_file) {
                if let Ok(uw) = uw_str.trim().parse::<i64>() {
                    let watts = uw as f32 / 1_000_000.0;

                    // Get label if available
                    let label = fs::read_to_string(path.join(format!("power{}_label", i)))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| format!("Power {}", i));

                    let sensor_name = if label != format!("Power {}", i) {
                        format!("{} {}", chip_name, label)
                    } else {
                        format!("{} Power {}", chip_name, i)
                    };

                    sensors.push(HwSensor {
                        name: sensor_name,
                        value: watts,
                        min: None,
                        max: None,
                        sensor_type: HwSensorType::Power,
                        hardware_type: hw_type,
                    });
                }
            }
        }

        // Also check power*_average
        let avg_file = path.join(format!("power{}_average", i));
        if avg_file.exists() && !input_file.exists() {
            if let Ok(uw_str) = fs::read_to_string(&avg_file) {
                if let Ok(uw) = uw_str.trim().parse::<i64>() {
                    let watts = uw as f32 / 1_000_000.0;

                    sensors.push(HwSensor {
                        name: format!("{} Power {} Avg", chip_name, i),
                        value: watts,
                        min: None,
                        max: None,
                        sensor_type: HwSensorType::Power,
                        hardware_type: hw_type,
                    });
                }
            }
        }
    }

    // Check energy counters (energy*_input in microjoules)
    for i in 1..=4 {
        let energy_file = path.join(format!("energy{}_input", i));
        if energy_file.exists() {
            if let Ok(uj_str) = fs::read_to_string(&energy_file) {
                if let Ok(uj) = uj_str.trim().parse::<i64>() {
                    let joules = uj as f32 / 1_000_000.0;

                    sensors.push(HwSensor {
                        name: format!("{} Energy {}", chip_name, i),
                        value: joules,
                        min: None,
                        max: None,
                        sensor_type: HwSensorType::Energy,
                        hardware_type: hw_type,
                    });
                }
            }
        }
    }

    sensors
}

/// Read CPU frequency from /sys/devices/system/cpu
pub fn read_cpu_frequencies() -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    let cpu_path = Path::new("/sys/devices/system/cpu");
    if !cpu_path.exists() {
        return sensors;
    }

    if let Ok(entries) = fs::read_dir(cpu_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Match cpu0, cpu1, etc.
            if name_str.starts_with("cpu") && name_str[3..].chars().all(|c| c.is_ascii_digit()) {
                let freq_file = entry.path().join("cpufreq/scaling_cur_freq");
                if freq_file.exists() {
                    if let Ok(khz_str) = fs::read_to_string(&freq_file) {
                        if let Ok(khz) = khz_str.trim().parse::<u64>() {
                            let mhz = khz as f32 / 1000.0;

                            // Get max frequency
                            let max_mhz =
                                fs::read_to_string(entry.path().join("cpufreq/scaling_max_freq"))
                                    .ok()
                                    .and_then(|s| s.trim().parse::<u64>().ok())
                                    .map(|k| k as f32 / 1000.0);

                            sensors.push(HwSensor {
                                name: format!("CPU {} Clock", &name_str[3..]),
                                value: mhz,
                                min: None,
                                max: max_mhz,
                                sensor_type: HwSensorType::Clock,
                                hardware_type: HwType::Cpu,
                            });
                        }
                    }
                }
            }
        }
    }

    sensors
}

/// Read RAPL (Running Average Power Limit) energy/power data
pub fn read_rapl_power() -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    let rapl_path = Path::new("/sys/class/powercap");
    if !rapl_path.exists() {
        return sensors;
    }

    // Look for intel-rapl domains
    if let Ok(entries) = fs::read_dir(rapl_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.starts_with("intel-rapl") {
                // Read domain name
                let domain_name = fs::read_to_string(path.join("name"))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| name_str.to_string());

                // Energy in microjoules
                if let Ok(uj_str) = fs::read_to_string(path.join("energy_uj")) {
                    if let Ok(uj) = uj_str.trim().parse::<u64>() {
                        let joules = uj as f32 / 1_000_000.0;

                        sensors.push(HwSensor {
                            name: format!("RAPL {} Energy", domain_name),
                            value: joules,
                            min: None,
                            max: None,
                            sensor_type: HwSensorType::Energy,
                            hardware_type: HwType::Cpu,
                        });
                    }
                }

                // Check for constraint_0_power_limit_uw (TDP)
                if let Ok(uw_str) = fs::read_to_string(path.join("constraint_0_power_limit_uw")) {
                    if let Ok(uw) = uw_str.trim().parse::<u64>() {
                        let watts = uw as f32 / 1_000_000.0;

                        sensors.push(HwSensor {
                            name: format!("RAPL {} Power Limit", domain_name),
                            value: watts,
                            min: None,
                            max: None,
                            sensor_type: HwSensorType::Power,
                            hardware_type: HwType::Cpu,
                        });
                    }
                }
            }
        }
    }

    sensors
}

/// Get all Linux hardware sensors
pub fn read_all_linux_sensors() -> Vec<HwSensor> {
    let mut sensors = Vec::new();

    sensors.extend(read_all_hwmon_sensors());
    sensors.extend(read_cpu_frequencies());
    sensors.extend(read_rapl_power());

    sensors
}

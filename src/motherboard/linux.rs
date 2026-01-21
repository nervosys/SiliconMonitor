// Linux motherboard and system monitoring implementation
//
// Data sources:
// - /sys/class/hwmon/* - Hardware monitoring sensors
// - /sys/class/dmi/id/* - DMI/SMBIOS system information
// - /sys/firmware/efi - EFI/UEFI detection
// - /proc/cpuinfo - CPU information
// - /sys/module/*/version - Kernel module versions
// - lsmod - Loaded kernel modules

use super::traits::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Linux motherboard sensor device
pub struct LinuxSensor {
    name: String,
    hwmon_path: PathBuf,
    chip_name: String,
}

impl LinuxSensor {
    /// Create a new Linux sensor from a hwmon path
    pub fn new(hwmon_path: PathBuf) -> Result<Self, Error> {
        let name_path = hwmon_path.join("name");
        let chip_name = fs::read_to_string(&name_path)
            .map_err(|e| Error::InitializationFailed(format!("Failed to read sensor name: {}", e)))?
            .trim()
            .to_string();

        let name = hwmon_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            name,
            hwmon_path,
            chip_name,
        })
    }

    /// Read a sensor input file
    fn read_input(&self, pattern: &str, index: u32) -> Option<i64> {
        let path = self.hwmon_path.join(format!("{}{}_input", pattern, index));
        fs::read_to_string(path).ok()?.trim().parse::<i64>().ok()
    }

    /// Read a sensor label file
    fn read_label(&self, pattern: &str, index: u32) -> Option<String> {
        let path = self.hwmon_path.join(format!("{}{}_label", pattern, index));
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    }

    /// Read a sensor max file
    fn read_max(&self, pattern: &str, index: u32) -> Option<i64> {
        let path = self.hwmon_path.join(format!("{}{}_max", pattern, index));
        fs::read_to_string(path).ok()?.trim().parse::<i64>().ok()
    }

    /// Read a sensor critical file
    fn read_crit(&self, pattern: &str, index: u32) -> Option<i64> {
        let path = self.hwmon_path.join(format!("{}{}_crit", pattern, index));
        fs::read_to_string(path).ok()?.trim().parse::<i64>().ok()
    }

    /// Determine sensor type from label
    fn classify_sensor(label: &str) -> SensorType {
        let label_lower = label.to_lowercase();
        if label_lower.contains("cpu")
            || label_lower.contains("core")
            || label_lower.contains("package")
        {
            SensorType::Cpu
        } else if label_lower.contains("chipset") || label_lower.contains("pch") {
            SensorType::Chipset
        } else if label_lower.contains("vrm") || label_lower.contains("vcore") {
            SensorType::Vrm
        } else if label_lower.contains("ambient") || label_lower.contains("system") {
            SensorType::Ambient
        } else if label_lower.contains("m.2") || label_lower.contains("nvme") {
            SensorType::M2Slot
        } else {
            SensorType::Other
        }
    }
}

impl MotherboardDevice for LinuxSensor {
    fn name(&self) -> &str {
        &self.chip_name
    }

    fn device_path(&self) -> Option<String> {
        Some(self.hwmon_path.to_string_lossy().to_string())
    }

    fn temperature_sensors(&self) -> Result<Vec<TemperatureSensor>, Error> {
        let mut sensors = Vec::new();

        // Try temperature inputs (temp1_input through temp32_input)
        for i in 1..=32 {
            if let Some(temp_millic) = self.read_input("temp", i) {
                let label = self
                    .read_label("temp", i)
                    .unwrap_or_else(|| format!("temp{}", i));

                let temperature = temp_millic as f32 / 1000.0;
                let max = self.read_max("temp", i).map(|v| v as f32 / 1000.0);
                let critical = self.read_crit("temp", i).map(|v| v as f32 / 1000.0);
                let sensor_type = Self::classify_sensor(&label);

                sensors.push(TemperatureSensor {
                    label,
                    temperature,
                    max,
                    critical,
                    sensor_type,
                });
            }
        }

        Ok(sensors)
    }

    fn voltage_rails(&self) -> Result<Vec<VoltageRail>, Error> {
        let mut rails = Vec::new();

        // Try voltage inputs (in0_input through in32_input)
        for i in 0..=32 {
            if let Some(voltage_milliv) = self.read_input("in", i) {
                let label = self
                    .read_label("in", i)
                    .unwrap_or_else(|| format!("in{}", i));

                let voltage = voltage_milliv as f32 / 1000.0;
                let min = self.read_max("in", i).map(|v| v as f32 / 1000.0);
                let max = self.read_crit("in", i).map(|v| v as f32 / 1000.0);

                rails.push(VoltageRail {
                    label,
                    voltage,
                    min,
                    max,
                });
            }
        }

        Ok(rails)
    }

    fn fans(&self) -> Result<Vec<FanInfo>, Error> {
        let mut fans = Vec::new();

        // Try fan inputs (fan1_input through fan16_input)
        for i in 1..=16 {
            if let Some(rpm) = self.read_input("fan", i) {
                let label = self
                    .read_label("fan", i)
                    .unwrap_or_else(|| format!("fan{}", i));

                // Try to read PWM value
                let pwm_path = self.hwmon_path.join(format!("pwm{}", i));
                let pwm = fs::read_to_string(&pwm_path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u8>().ok());

                // Check if PWM is writable (controllable)
                let pwm_enable_path = self.hwmon_path.join(format!("pwm{}_enable", i));
                let controllable = pwm_enable_path.exists()
                    && fs::metadata(&pwm_enable_path)
                        .map(|m| !m.permissions().readonly())
                        .unwrap_or(false);

                let rpm_value = if rpm > 0 { Some(rpm as u32) } else { None };

                fans.push(FanInfo {
                    label,
                    rpm: rpm_value,
                    pwm,
                    min_rpm: None,
                    max_rpm: None,
                    controllable,
                });
            }
        }

        Ok(fans)
    }

    fn set_fan_speed(&self, fan_index: usize, speed: FanControl) -> Result<(), Error> {
        let pwm_path = self.hwmon_path.join(format!("pwm{}", fan_index + 1));
        let pwm_enable_path = self.hwmon_path.join(format!("pwm{}_enable", fan_index + 1));

        if !pwm_path.exists() {
            return Err(Error::FanControlError(format!(
                "Fan {} does not support PWM control",
                fan_index
            )));
        }

        match speed {
            FanControl::Manual(pwm_value) => {
                // Set to manual mode (pwm_enable = 1)
                fs::write(&pwm_enable_path, "1\n").map_err(|e| {
                    Error::PermissionDenied(format!("Failed to set fan mode: {}", e))
                })?;

                // Set PWM value
                fs::write(&pwm_path, format!("{}\n", pwm_value)).map_err(|e| {
                    Error::PermissionDenied(format!("Failed to set fan speed: {}", e))
                })?;
            }
            FanControl::Automatic => {
                // Set to automatic mode (pwm_enable = 2 or 5 depending on chip)
                fs::write(&pwm_enable_path, "2\n").map_err(|e| {
                    Error::PermissionDenied(format!("Failed to set fan mode: {}", e))
                })?;
            }
        }

        Ok(())
    }
}

/// Enumerate all hwmon sensors
pub fn enumerate() -> Result<Vec<Box<dyn MotherboardDevice>>, Error> {
    let hwmon_dir = Path::new("/sys/class/hwmon");

    if !hwmon_dir.exists() {
        return Err(Error::NoSensorsFound);
    }

    let mut devices: Vec<Box<dyn MotherboardDevice>> = Vec::new();

    for entry in fs::read_dir(hwmon_dir).map_err(|e| Error::IoError(e))? {
        let entry = entry.map_err(|e| Error::IoError(e))?;
        let path = entry.path();

        // Skip if not a directory
        if !path.is_dir() {
            continue;
        }

        // Try to create a sensor device
        if let Ok(sensor) = LinuxSensor::new(path) {
            devices.push(Box::new(sensor));
        }
    }

    if devices.is_empty() {
        Err(Error::NoSensorsFound)
    } else {
        Ok(devices)
    }
}

/// Read DMI/SMBIOS information
fn read_dmi(path: &str) -> Option<String> {
    fs::read_to_string(Path::new("/sys/class/dmi/id").join(path))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Detect firmware type (BIOS or UEFI)
fn detect_firmware_type() -> FirmwareType {
    if Path::new("/sys/firmware/efi").exists() {
        FirmwareType::Uefi
    } else {
        FirmwareType::Bios
    }
}

/// Get system information
pub fn get_system_info() -> Result<SystemInfo, Error> {
    // OS information
    let os_release = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();

    let mut os_info = HashMap::new();
    for line in os_release.lines() {
        if let Some((key, value)) = line.split_once('=') {
            os_info.insert(key.to_string(), value.trim_matches('"').to_string());
        }
    }

    let os_name = os_info
        .get("PRETTY_NAME")
        .or_else(|| os_info.get("NAME"))
        .cloned()
        .unwrap_or_else(|| "Linux".to_string());

    let os_version = os_info
        .get("VERSION")
        .or_else(|| os_info.get("VERSION_ID"))
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());

    // Kernel version
    let kernel_version = fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from));

    // Architecture
    let architecture = std::env::consts::ARCH.to_string();

    // Hostname
    let hostname = fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string());

    // BIOS information
    let bios = BiosInfo {
        vendor: read_dmi("bios_vendor"),
        version: read_dmi("bios_version"),
        release_date: read_dmi("bios_date"),
        revision: None,
        firmware_type: detect_firmware_type(),
        secure_boot: None, // Would need to parse /sys/firmware/efi/efivars/SecureBoot-*
    };

    // Hardware information
    let manufacturer = read_dmi("sys_vendor");
    let product_name = read_dmi("product_name");
    let serial_number = read_dmi("product_serial");
    let uuid = read_dmi("product_uuid");

    let board_vendor = read_dmi("board_vendor");
    let board_name = read_dmi("board_name");
    let board_version = read_dmi("board_version");

    // CPU information
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cpu_name = cpuinfo
        .lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());

    let cpu_cores = cpuinfo
        .lines()
        .find(|line| line.starts_with("cpu cores"))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|s| s.trim().parse::<u32>().ok());

    let cpu_threads = cpuinfo
        .lines()
        .filter(|line| line.starts_with("processor"))
        .count() as u32;

    let cpu_threads = if cpu_threads > 0 {
        Some(cpu_threads)
    } else {
        None
    };

    Ok(SystemInfo {
        os_name,
        os_version,
        kernel_version,
        architecture,
        hostname,
        bios,
        manufacturer,
        product_name,
        serial_number,
        uuid,
        board_vendor,
        board_name,
        board_version,
        cpu_name,
        cpu_cores,
        cpu_threads,
    })
}

/// Get driver/module versions
pub fn get_driver_versions() -> Result<Vec<DriverInfo>, Error> {
    let mut drivers = Vec::new();

    // GPU drivers
    if let Ok(version) = fs::read_to_string("/sys/module/nvidia/version") {
        drivers.push(DriverInfo {
            name: "nvidia".to_string(),
            version: version.trim().to_string(),
            driver_type: DriverType::Gpu,
            description: Some("NVIDIA GPU Driver".to_string()),
            vendor: Some("NVIDIA".to_string()),
            date: None,
        });
    }

    if let Ok(version) = fs::read_to_string("/sys/module/amdgpu/version") {
        drivers.push(DriverInfo {
            name: "amdgpu".to_string(),
            version: version.trim().to_string(),
            driver_type: DriverType::Gpu,
            description: Some("AMD GPU Driver".to_string()),
            vendor: Some("AMD".to_string()),
            date: None,
        });
    }

    if let Ok(version) = fs::read_to_string("/sys/module/i915/version") {
        drivers.push(DriverInfo {
            name: "i915".to_string(),
            version: version.trim().to_string(),
            driver_type: DriverType::Gpu,
            description: Some("Intel GPU Driver".to_string()),
            vendor: Some("Intel".to_string()),
            date: None,
        });
    }

    // Storage drivers
    for module in &["nvme", "ahci", "sata_nv", "megaraid_sas"] {
        let version_path = format!("/sys/module/{}/version", module);
        if let Ok(version) = fs::read_to_string(&version_path) {
            drivers.push(DriverInfo {
                name: module.to_string(),
                version: version.trim().to_string(),
                driver_type: DriverType::Storage,
                description: Some(format!("{} Storage Driver", module.to_uppercase())),
                vendor: None,
                date: None,
            });
        }
    }

    // Network drivers (common ones)
    for module in &["e1000e", "igb", "ixgbe", "r8169", "bnx2x"] {
        let version_path = format!("/sys/module/{}/version", module);
        if let Ok(version) = fs::read_to_string(&version_path) {
            drivers.push(DriverInfo {
                name: module.to_string(),
                version: version.trim().to_string(),
                driver_type: DriverType::Network,
                description: Some(format!("{} Network Driver", module.to_uppercase())),
                vendor: None,
                date: None,
            });
        }
    }

    Ok(drivers)
}

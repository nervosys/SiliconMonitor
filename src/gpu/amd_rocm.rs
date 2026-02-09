// SPDX-License-Identifier: MIT OR Apache-2.0
//! AMD GPU monitoring via sysfs
//!
//! This module provides AMD GPU monitoring through sysfs on Linux.

use super::traits::{
    Clocks, Device, Error, FanSpeed, GpuProcess, Memory, PciInfo, Power, Temperature,
    TemperatureThresholds, Utilization, Vendor,
};
use std::fs;
use std::path::PathBuf;

pub struct AmdGpu {
    index: u32,
    #[allow(dead_code)]
    card_path: PathBuf,
    device_path: PathBuf,
}

impl AmdGpu {
    pub fn new(index: u32, card_path: PathBuf) -> Result<Self, Error> {
        let device_path = card_path.join("device");
        if !device_path.exists() {
            return Err(Error::InitializationFailed(
                "Device path does not exist".to_string(),
            ));
        }
        Ok(Self {
            index,
            card_path,
            device_path,
        })
    }

    fn read_sysfs_string(&self, attr: &str) -> Option<String> {
        fs::read_to_string(self.device_path.join(attr))
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn read_sysfs_u64(&self, attr: &str) -> Option<u64> {
        self.read_sysfs_string(attr)?.parse::<u64>().ok()
    }
}

impl AmdGpu {
    fn read_hwmon_temp(&self, hwmon_path: &std::path::Path, sensor: &str) -> Option<f32> {
        fs::read_to_string(hwmon_path.join(sensor))
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .map(|millidegrees| millidegrees as f32 / 1000.0)
    }

    fn get_temperature_thresholds(
        &self,
        hwmon_path: &std::path::Path,
    ) -> Option<TemperatureThresholds> {
        // AMD GPUs expose critical thresholds via hwmon
        // temp1_crit = Edge critical temp
        // temp2_crit = Junction critical temp
        // temp3_crit = Memory critical temp
        // temp1_emergency = Emergency shutdown temp

        let critical = self
            .read_hwmon_temp(hwmon_path, "temp1_crit")
            .or_else(|| self.read_hwmon_temp(hwmon_path, "temp2_crit"));

        let shutdown = self
            .read_hwmon_temp(hwmon_path, "temp1_emergency")
            .or_else(|| self.read_hwmon_temp(hwmon_path, "temp2_emergency"));

        let memory_critical = self.read_hwmon_temp(hwmon_path, "temp3_crit");

        // AMD doesn't expose slowdown threshold directly
        // Estimate as 85-90% of critical temp if available
        let slowdown = critical.map(|c| c * 0.85);

        if critical.is_some() || shutdown.is_some() || slowdown.is_some() {
            Some(TemperatureThresholds {
                slowdown,
                shutdown,
                critical,
                memory_critical,
            })
        } else {
            None
        }
    }

    fn read_current_clock(&self, file: &str) -> Option<u32> {
        // Parse pp_dpm files which have format like:
        // 0: 500Mhz
        // 1: 800Mhz *
        // The * indicates the current level
        let content = self.read_sysfs_string(file)?;
        for line in content.lines() {
            if line.contains('*') {
                // Extract frequency value
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 {
                    let freq_str = parts[1]
                        .trim()
                        .replace("Mhz", "")
                        .replace("*", "")
                        .trim()
                        .to_string();
                    if let Ok(freq) = freq_str.parse::<u32>() {
                        return Some(freq);
                    }
                }
            }
        }
        None
    }
}

impl Device for AmdGpu {
    fn vendor(&self) -> Vendor {
        Vendor::Amd
    }
    fn index(&self) -> u32 {
        self.index
    }
    fn name(&self) -> Result<String, Error> {
        // Try to read product name, fallback to device ID
        if let Some(name) = self.read_sysfs_string("product_name") {
            return Ok(name);
        }

        // Read device ID if name not available
        if let Some(device_id) = self.read_sysfs_string("device") {
            return Ok(format!("AMD GPU {}", device_id));
        }

        Ok(format!("AMD GPU #{}", self.index))
    }
    fn uuid(&self) -> Result<String, Error> {
        // AMD GPUs don't typically expose a UUID via sysfs
        // Use a combination of vendor, device, and serial if available
        let vendor = self.read_sysfs_string("vendor").unwrap_or_default();
        let device = self.read_sysfs_string("device").unwrap_or_default();
        let serial = self
            .read_sysfs_string("serial")
            .unwrap_or_else(|| format!("{}", self.index));

        Ok(format!("GPU-{}-{}-{}", vendor, device, serial))
    }
    fn pci_info(&self) -> Result<PciInfo, Error> {
        // Parse PCI address from the sysfs path
        // Path looks like: /sys/class/drm/card0/device -> ../../devices/pci0000:00/0000:00:01.0/...
        let link = fs::read_link(&self.device_path)
            .map_err(|e| Error::QueryFailed(format!("Failed to read device link: {}", e)))?;

        let path_str = link.to_string_lossy();

        // Extract PCI address (format: 0000:00:00.0)
        // Find the last occurrence that matches the PCI address pattern
        for component in path_str.split('/').rev() {
            let parts: Vec<&str> = component.split(':').collect();
            if parts.len() == 3 {
                let domain_bus = parts[0];
                let device_func = parts[2].split('.').collect::<Vec<&str>>();

                if domain_bus.len() == 7 && device_func.len() == 2 {
                    if let (Ok(domain), Ok(bus), Ok(device), Ok(function)) = (
                        u16::from_str_radix(&domain_bus[0..4], 16),
                        u8::from_str_radix(&domain_bus[5..7], 16),
                        u8::from_str_radix(parts[1], 16),
                        u8::from_str_radix(device_func[1], 16),
                    ) {
                        let bus_id =
                            format!("{:04x}:{:02x}:{:02x}.{}", domain, bus, device, function);
                        return Ok(PciInfo {
                            domain: domain as u32,
                            bus,
                            device,
                            function,
                            bus_id,
                            pcie_generation: None,
                            pcie_link_width: None,
                        });
                    }
                }
            }
        }

        Err(Error::QueryFailed("Could not parse PCI info".to_string()))
    }
    fn driver_version(&self) -> Result<String, Error> {
        // Try to read from module version
        if let Ok(version) = fs::read_to_string("/sys/module/amdgpu/version") {
            return Ok(version.trim().to_string());
        }

        // Fallback to driver name
        if let Some(driver) = self.read_sysfs_string("driver") {
            return Ok(driver);
        }

        Ok("amdgpu".to_string())
    }
    fn temperature(&self) -> Result<Temperature, Error> {
        // AMD GPUs expose temperature via hwmon
        // Find hwmon directory
        let hwmon_path = self.device_path.join("hwmon");
        if !hwmon_path.exists() {
            return Err(Error::NotSupported);
        }

        let hwmon_dirs: Vec<_> = fs::read_dir(&hwmon_path)
            .map_err(|e| Error::QueryFailed(format!("Failed to read hwmon dir: {}", e)))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("hwmon"))
            .collect();

        if hwmon_dirs.is_empty() {
            return Err(Error::NotSupported);
        }

        let hwmon = hwmon_dirs[0].path();

        // Read temperature sensors (in millidegrees, convert to Celsius)
        let edge = self.read_hwmon_temp(&hwmon, "temp1_input");
        let junction = self.read_hwmon_temp(&hwmon, "temp2_input");
        let memory = self.read_hwmon_temp(&hwmon, "temp3_input");

        // Read temperature thresholds
        let thresholds = self.get_temperature_thresholds(&hwmon);

        Ok(Temperature {
            edge,
            junction,
            memory,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds,
        })
    }

    fn power(&self) -> Result<Power, Error> {
        // Find hwmon directory for power readings
        let hwmon_path = self.device_path.join("hwmon");
        if !hwmon_path.exists() {
            return Err(Error::NotSupported);
        }

        let hwmon_dirs: Vec<_> = fs::read_dir(&hwmon_path)
            .map_err(|e| Error::QueryFailed(format!("Failed to read hwmon dir: {}", e)))?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("hwmon"))
            .collect();

        if hwmon_dirs.is_empty() {
            return Err(Error::NotSupported);
        }

        let hwmon = hwmon_dirs[0].path();

        // Read power values (in microwatts, convert to watts)
        let read_power = |sensor: &str| -> f32 {
            fs::read_to_string(hwmon.join(sensor))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(|microwatts| microwatts as f32 / 1_000_000.0)
                .unwrap_or(0.0)
        };

        let current = read_power("power1_average");
        let limit = read_power("power1_cap");
        let max_limit = read_power("power1_cap_max");
        let min_limit = read_power("power1_cap_min");

        Ok(Power {
            current,
            average: if current > 0.0 { Some(current) } else { None },
            limit,
            default_limit: limit,
            min_limit,
            max_limit,
            enforced_limit: limit,
        })
    }
    fn clocks(&self) -> Result<Clocks, Error> {
        // Read clock frequencies from sysfs
        // AMD exposes current frequencies via pp_dpm_sclk (graphics) and pp_dpm_mclk (memory)
        let graphics = self.read_current_clock("pp_dpm_sclk").unwrap_or(0);
        let memory = self.read_current_clock("pp_dpm_mclk").unwrap_or(0);

        Ok(Clocks {
            graphics,
            memory,
            sm: None,
            video: None,
        })
    }
    fn utilization(&self) -> Result<Utilization, Error> {
        // Read GPU utilization from gpu_busy_percent
        let gpu = self
            .read_sysfs_string("gpu_busy_percent")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(0.0);

        // Memory utilization not directly available via sysfs
        // Would need to calculate from memory bandwidth counters
        let memory = 0.0;

        Ok(Utilization {
            gpu,
            memory,
            encoder: None,
            decoder: None,
            jpeg: None,
            ofa: None,
        })
    }
    fn memory(&self) -> Result<Memory, Error> {
        // Read VRAM info from mem_info_vram_* files
        let total = self.read_sysfs_u64("mem_info_vram_total").unwrap_or(0);
        let used = self.read_sysfs_u64("mem_info_vram_used").unwrap_or(0);
        let free = total.saturating_sub(used);

        Ok(Memory {
            total,
            used,
            free,
            bar1_total: None,
            bar1_used: None,
        })
    }
    fn fan_speed(&self) -> Result<Option<FanSpeed>, Error> {
        // Find hwmon directory for fan readings
        let hwmon_path = self.device_path.join("hwmon");
        if !hwmon_path.exists() {
            return Ok(None);
        }

        let hwmon_dir = fs::read_dir(&hwmon_path)
            .ok()
            .and_then(|rd| rd.filter_map(|e| e.ok()).nth(0))
            .map(|e| e.path());

        if let Some(hwmon) = hwmon_dir {
            // Try to read fan percentage from PWM first (nvtop parity)
            // PWM is 0-255, pwm1_max defines the maximum value
            if let Some(pwm) = fs::read_to_string(hwmon.join("pwm1"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                let pwm_max = fs::read_to_string(hwmon.join("pwm1_max"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(255); // Default to 255 if pwm1_max not available

                let percentage = (pwm * 100) / pwm_max;
                return Ok(Some(FanSpeed::Percent(percentage)));
            }

            // Fallback: Read fan speed in RPM
            if let Some(rpm) = fs::read_to_string(hwmon.join("fan1_input"))
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
            {
                return Ok(Some(FanSpeed::Rpm(rpm)));
            }
        }

        Ok(None)
    }
    fn performance_state(&self) -> Result<Option<String>, Error> {
        // Read performance level from power_dpm_force_performance_level
        Ok(self.read_sysfs_string("power_dpm_force_performance_level"))
    }
    fn processes(&self) -> Result<Vec<Box<dyn GpuProcess>>, Error> {
        Ok(Vec::new())
    }
}

pub fn enumerate() -> Result<Vec<Box<dyn Device>>, Error> {
    let mut devices: Vec<Box<dyn Device>> = Vec::new();

    // Scan /sys/class/drm for AMD GPU devices
    let drm_path = std::path::Path::new("/sys/class/drm");
    if !drm_path.exists() {
        return Ok(devices);
    }

    let entries = fs::read_dir(drm_path).map_err(|e| {
        Error::InitializationFailed(format!("Failed to read /sys/class/drm: {}", e))
    })?;

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Look for card devices (card0, card1, etc)
        if !name_str.starts_with("card") || name_str.contains('-') {
            continue;
        }

        // Extract card number
        let index = name_str.trim_start_matches("card").parse::<u32>().ok();

        let Some(index) = index else {
            continue;
        };

        let card_path = entry.path();
        let device_path = card_path.join("device");

        // Check if this is an AMD GPU (vendor ID 0x1002)
        if let Ok(vendor_id) = fs::read_to_string(device_path.join("vendor")) {
            if vendor_id.trim() == "0x1002" {
                // This is an AMD GPU
                match AmdGpu::new(index, card_path) {
                    Ok(gpu) => devices.push(Box::new(gpu)),
                    Err(_) => continue,
                }
            }
        }
    }

    if devices.is_empty() {
        return Err(Error::NoDevicesFound);
    }

    Ok(devices)
}

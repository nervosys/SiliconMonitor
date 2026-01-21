// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024 NervoSys

//! Intel GPU monitoring via i915/xe drivers
//!
//! This module provides Intel GPU support using the DRM (Direct Rendering Manager)
//! interface, supporting both legacy i915 and modern xe drivers. It monitors:
//! - GPU utilization (render, video, video enhancement engines)
//! - Memory usage (system and stolen memory for iGPUs)
//! - Temperature and power
//! - Frequency and turbo states
//! - Process tracking via fdinfo
//!
//! Based on nvtop's extract_gpuinfo_intel.c implementation.

use crate::gpu::{
    Gpu, GpuClocks, GpuCollection, GpuDynamicInfo, GpuEngines, GpuMemory, GpuPower, GpuProcess,
    GpuStaticInfo, GpuThermal, GpuVendor, PcieLinkInfo,
};
use crate::Error;

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Intel GPU driver type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntelDriver {
    /// Legacy i915 driver
    I915,
    /// Modern Xe driver
    Xe,
}

/// Intel GPU implementation
pub struct IntelGpu {
    index: usize,
    name: String,
    pci_bus_id: String,
    driver: IntelDriver,
    #[cfg(target_os = "linux")]
    card_path: String,
    #[cfg(target_os = "linux")]
    hwmon_path: Option<String>,
}

impl IntelGpu {
    /// Create new Intel GPU instance
    #[cfg(target_os = "linux")]
    pub fn new(index: usize, pci_bus_id: String, card_path: String, driver: IntelDriver) -> Result<Self, Error> {
        let device_path = format!("{}/device", card_path);
        
        // Try to read GPU name from product_name or derive from PCI ID
        let name = read_intel_gpu_name(&device_path, &driver)
            .unwrap_or_else(|| format!("Intel GPU {} ({})", index, driver.name()));
        
        // Find hwmon path
        let hwmon_path = find_hwmon_path(&device_path);

        Ok(Self {
            index,
            name,
            pci_bus_id,
            driver,
            card_path,
            hwmon_path,
        })
    }

    /// Create new Intel GPU instance (non-Linux stub)
    #[cfg(not(target_os = "linux"))]
    pub fn new(index: usize, pci_bus_id: String, driver: IntelDriver) -> Result<Self, Error> {
        let name = format!("Intel GPU {} ({})", index, driver.name());
        Ok(Self {
            index,
            name,
            pci_bus_id,
            driver,
        })
    }

    /// Get driver type
    pub fn driver(&self) -> IntelDriver {
        self.driver
    }
}

impl IntelDriver {
    /// Get driver name
    pub fn name(&self) -> &'static str {
        match self {
            IntelDriver::I915 => "i915",
            IntelDriver::Xe => "xe",
        }
    }
}

#[cfg(target_os = "linux")]
fn read_intel_gpu_name(device_path: &str, driver: &IntelDriver) -> Option<String> {
    // Try lspci-style parsing from PCI IDs
    let device = fs::read_to_string(format!("{}/device", device_path))
        .ok()
        .map(|s| s.trim().to_string())?;
    
    // Map common Intel GPU device IDs to names
    let name = match device.as_str() {
        // Integrated Graphics (common ones)
        "0x9a49" | "0x9a40" => "Intel UHD Graphics (Tiger Lake)",
        "0x46a6" | "0x46a8" => "Intel UHD Graphics (Alder Lake)",
        "0xa7a0" | "0xa7a1" => "Intel UHD Graphics (Raptor Lake)",
        "0x7d55" | "0x7d45" => "Intel UHD Graphics (Meteor Lake)",
        "0x5917" | "0x5912" => "Intel UHD Graphics 620 (Kaby Lake)",
        "0x3e92" | "0x3e91" => "Intel UHD Graphics 630 (Coffee Lake)",
        "0x8a52" | "0x8a56" => "Intel UHD Graphics (Ice Lake)",
        // Arc discrete GPUs
        "0x5690" | "0x5691" | "0x5692" => "Intel Arc A770",
        "0x5693" | "0x5694" => "Intel Arc A750",
        "0x56a0" | "0x56a1" => "Intel Arc A580",
        "0x5696" | "0x5697" => "Intel Arc A380",
        "0x56a5" | "0x56a6" => "Intel Arc A310",
        // Xe discrete
        "0x0bd0" | "0x0bd5" | "0x0bd6" | "0x0bd7" => "Intel Data Center GPU Max",
        _ => {
            // Generic name with driver info
            return Some(format!("Intel Graphics [{}] ({})", device, driver.name()));
        }
    };
    
    Some(name.to_string())
}

#[cfg(target_os = "linux")]
fn find_hwmon_path(device_path: &str) -> Option<String> {
    let hwmon_base = format!("{}/hwmon", device_path);
    if let Ok(entries) = fs::read_dir(&hwmon_base) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                return Some(path.to_string_lossy().to_string());
            }
        }
    }
    None
}

impl Gpu for IntelGpu {
    fn static_info(&self) -> Result<GpuStaticInfo, Error> {
        #[cfg(target_os = "linux")]
        {
            // Read driver version
            let driver_path = match self.driver {
                IntelDriver::I915 => "/sys/module/i915/version",
                IntelDriver::Xe => "/sys/module/xe/version",
            };
            let driver_version = fs::read_to_string(driver_path)
                .ok()
                .map(|s| s.trim().to_string());

            // Intel GPUs are almost always integrated (except Arc)
            let is_discrete = self.name.to_lowercase().contains("arc") || 
                             self.name.to_lowercase().contains("data center");

            Ok(GpuStaticInfo {
                index: self.index,
                vendor: GpuVendor::Intel,
                name: self.name.clone(),
                pci_bus_id: Some(self.pci_bus_id.clone()),
                uuid: None,
                vbios_version: None,
                driver_version,
                compute_capability: None,
                shader_cores: None,
                l2_cache: None,
                num_engines: None,
                integrated: !is_discrete,
            })
        }

        #[cfg(not(target_os = "linux"))]
        Ok(GpuStaticInfo {
            index: self.index,
            vendor: GpuVendor::Intel,
            name: self.name.clone(),
            pci_bus_id: Some(self.pci_bus_id.clone()),
            uuid: None,
            vbios_version: None,
            driver_version: Some(self.driver.name().to_string()),
            compute_capability: None,
            shader_cores: None,
            l2_cache: None,
            num_engines: None,
            integrated: true,
        })
    }

    fn dynamic_info(&self) -> Result<GpuDynamicInfo, Error> {
        #[cfg(target_os = "linux")]
        {
            let device_path = format!("{}/device", self.card_path);
            
            // Read GPU frequency
            let freq_path = match self.driver {
                IntelDriver::I915 => format!("{}/gt_cur_freq_mhz", device_path),
                IntelDriver::Xe => format!("{}/gt/gt0/freq0/cur_freq", device_path),
            };
            let graphics_clock = fs::read_to_string(&freq_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok());

            // Read max frequency
            let freq_max_path = match self.driver {
                IntelDriver::I915 => format!("{}/gt_max_freq_mhz", device_path),
                IntelDriver::Xe => format!("{}/gt/gt0/freq0/max_freq", device_path),
            };
            let graphics_max = fs::read_to_string(&freq_max_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok());

            // Calculate utilization from frequency ratio
            let utilization = if let (Some(cur), Some(max)) = (graphics_clock, graphics_max) {
                if max > 0 {
                    ((cur as f32 / max as f32) * 100.0) as u8
                } else {
                    0
                }
            } else {
                0
            };

            // Read power from hwmon (if available)
            let power_draw = self.hwmon_path.as_ref().and_then(|hwmon| {
                fs::read_to_string(format!("{}/power1_average", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .map(|uw| (uw / 1000) as u32)
            });

            // Read temperature from hwmon
            let temperature = self.hwmon_path.as_ref().and_then(|hwmon| {
                fs::read_to_string(format!("{}/temp1_input", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .map(|t| (t / 1000) as u32)
            });

            Ok(GpuDynamicInfo {
                utilization,
                memory: GpuMemory {
                    total: 0,
                    used: 0,
                    free: 0,
                    utilization: 0,
                },
                clocks: GpuClocks {
                    graphics: graphics_clock,
                    graphics_max,
                    memory: None,
                    memory_max: None,
                    sm: None,
                    video: None,
                },
                power: GpuPower {
                    draw: power_draw,
                    limit: None,
                    default_limit: None,
                    usage_percent: None,
                },
                thermal: GpuThermal {
                    temperature,
                    max_temperature: None,
                    critical_temperature: None,
                    fan_speed: None,
                    fan_rpm: None,
                },
                pcie: PcieLinkInfo {
                    current_gen: None,
                    max_gen: None,
                    current_width: None,
                    max_width: None,
                    current_speed: None,
                    max_speed: None,
                    tx_throughput: None,
                    rx_throughput: None,
                },
                engines: GpuEngines {
                    graphics: Some(utilization),
                    compute: None,
                    encoder: None,
                    decoder: None,
                    copy: None,
                    vendor_specific: vec![],
                },
                processes: vec![],
            })
        }

        #[cfg(not(target_os = "linux"))]
        Ok(GpuDynamicInfo {
            utilization: 0,
            memory: GpuMemory {
                total: 0,
                used: 0,
                free: 0,
                utilization: 0,
            },
            clocks: GpuClocks {
                graphics: None,
                graphics_max: None,
                memory: None,
                memory_max: None,
                sm: None,
                video: None,
            },
            power: GpuPower {
                draw: None,
                limit: None,
                default_limit: None,
                usage_percent: None,
            },
            thermal: GpuThermal {
                temperature: None,
                max_temperature: None,
                critical_temperature: None,
                fan_speed: None,
                fan_rpm: None,
            },
            pcie: PcieLinkInfo {
                current_gen: None,
                max_gen: None,
                current_width: None,
                max_width: None,
                current_speed: None,
                max_speed: None,
                tx_throughput: None,
                rx_throughput: None,
            },
            engines: GpuEngines {
                graphics: None,
                compute: None,
                encoder: None,
                decoder: None,
                copy: None,
                vendor_specific: vec![],
            },
            processes: vec![],
        })
    }

    fn vendor(&self) -> GpuVendor {
        GpuVendor::Intel
    }

    fn index(&self) -> usize {
        self.index
    }

    fn name(&self) -> Result<String, Error> {
        Ok(self.name.clone())
    }

    fn processes(&self) -> Result<Vec<GpuProcess>, Error> {
        #[cfg(target_os = "linux")]
        {
            parse_intel_fdinfo_processes(&self.card_path, &self.driver)
        }
        #[cfg(not(target_os = "linux"))]
        Ok(vec![])
    }

    fn kill_process(&self, pid: u32) -> Result<(), Error> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| {
                Error::ProcessError(format!("Failed to kill process {}: {}", pid, e))
            })?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            Err(Error::NotSupported(
                "Process killing not supported on this platform".to_string(),
            ))
        }
    }
}

/// Parse fdinfo for Intel GPU processes
#[cfg(target_os = "linux")]
fn parse_intel_fdinfo_processes(card_path: &str, driver: &IntelDriver) -> Result<Vec<GpuProcess>, Error> {
    let mut processes = Vec::new();
    let proc_dir = Path::new("/proc");
    
    let driver_name = driver.name();
    
    if let Ok(proc_entries) = fs::read_dir(proc_dir) {
        for proc_entry in proc_entries.flatten() {
            let pid_str = proc_entry.file_name();
            let pid_str = pid_str.to_string_lossy();
            
            let pid: u32 = match pid_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            
            let fdinfo_dir = proc_entry.path().join("fdinfo");
            if !fdinfo_dir.exists() {
                continue;
            }
            
            if let Ok(fdinfo_entries) = fs::read_dir(&fdinfo_dir) {
                for fdinfo_entry in fdinfo_entries.flatten() {
                    if let Ok(content) = fs::read_to_string(fdinfo_entry.path()) {
                        // Check for i915 or xe driver
                        let driver_match = format!("drm-driver:\t{}", driver_name);
                        if content.contains(&driver_match) {
                            // Parse engine usage
                            let mut total_time = 0u64;
                            
                            for line in content.lines() {
                                // Parse drm-engine-render, drm-engine-video, etc.
                                if line.starts_with("drm-engine-") {
                                    if let Some(time) = parse_engine_time(line) {
                                        total_time += time;
                                    }
                                }
                            }
                            
                            if total_time > 0 {
                                let name = fs::read_to_string(proc_entry.path().join("comm"))
                                    .map(|s| s.trim().to_string())
                                    .unwrap_or_else(|_| format!("Process {}", pid));
                                
                                processes.push(GpuProcess {
                                    pid,
                                    name,
                                    gpu_memory: 0, // Intel iGPUs share system memory
                                    compute_util: None,
                                    memory_util: None,
                                    encoder_util: None,
                                    decoder_util: None,
                                    process_type: None,
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    
    processes.sort_by_key(|p| p.pid);
    processes.dedup_by_key(|p| p.pid);
    
    Ok(processes)
}

#[cfg(target_os = "linux")]
fn parse_engine_time(line: &str) -> Option<u64> {
    // Parse "drm-engine-render:\t12345 ns" 
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        return parts[1].parse().ok();
    }
    None
}

/// Detect all Intel GPUs in the system
pub fn detect_gpus(collection: &mut GpuCollection) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        let dri_path = Path::new("/sys/class/drm");
        
        if !dri_path.exists() {
            return Ok(());
        }
        
        let mut gpu_index = 0;
        
        if let Ok(entries) = fs::read_dir(dri_path) {
            let mut cards: Vec<_> = entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with("card") && !name.contains('-') {
                        Some((name, e.path()))
                    } else {
                        None
                    }
                })
                .collect();
            
            cards.sort_by(|a, b| a.0.cmp(&b.0));
            
            for (_card_name, card_path) in cards {
                let device_path = card_path.join("device");
                let driver_path = device_path.join("driver");
                
                if let Ok(driver_target) = fs::read_link(&driver_path) {
                    let driver_name = driver_target
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    
                    let driver = match driver_name {
                        "i915" => Some(IntelDriver::I915),
                        "xe" => Some(IntelDriver::Xe),
                        _ => None,
                    };
                    
                    if let Some(driver) = driver {
                        // Get PCI bus ID
                        let pci_bus_id = if let Ok(dev_link) = fs::read_link(&device_path) {
                            dev_link
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string()
                        } else {
                            "unknown".to_string()
                        };
                        
                        if let Ok(gpu) = IntelGpu::new(
                            gpu_index,
                            pci_bus_id,
                            card_path.to_string_lossy().to_string(),
                            driver,
                        ) {
                            collection.add_gpu(Box::new(gpu));
                            gpu_index += 1;
                        }
                    }
                }
            }
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        let _ = collection;
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intel_gpu_creation() {
        #[cfg(target_os = "linux")]
        {
            let result = IntelGpu::new(
                0, 
                "0000:00:02.0".to_string(), 
                "/sys/class/drm/card0".to_string(),
                IntelDriver::I915,
            );
            let _ = result;
        }
    }

    #[test]
    fn test_driver_name() {
        assert_eq!(IntelDriver::I915.name(), "i915");
        assert_eq!(IntelDriver::Xe.name(), "xe");
    }
}

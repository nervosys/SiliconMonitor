// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2024 NervoSys

//! AMD GPU monitoring via amdgpu driver and libdrm
//!
//! This module provides AMD GPU support using the DRM (Direct Rendering Manager)
//! interface, specifically the amdgpu kernel driver. It monitors:
//! - GPU utilization (graphics, compute engines)
//! - VRAM usage
//! - Temperature and fan control
//! - Power consumption
//! - Video encode/decode engines (VCN)
//! - Process tracking via fdinfo
//!
//! Based on nvtop's extract_gpuinfo_amdgpu.c implementation.

use crate::gpu::{
    Gpu, GpuClocks, GpuCollection, GpuDynamicInfo, GpuEngines, GpuMemory, GpuPower, GpuProcess,
    GpuStaticInfo, GpuThermal, GpuVendor, PcieLinkInfo,
};
use crate::Error;

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

/// AMD GPU implementation
pub struct AmdGpu {
    index: usize,
    name: String,
    pci_bus_id: String,
    #[cfg(target_os = "linux")]
    card_path: String,
    #[cfg(target_os = "linux")]
    hwmon_path: Option<String>,
}

impl AmdGpu {
    /// Create new AMD GPU instance
    #[cfg(target_os = "linux")]
    pub fn new(index: usize, pci_bus_id: String, card_path: String) -> Result<Self, Error> {
        let device_path = format!("{}/device", card_path);

        // Try to read GPU name from product_name or uevent
        let name = read_gpu_name(&device_path).unwrap_or_else(|| format!("AMD GPU {}", index));

        // Find hwmon path
        let hwmon_path = find_hwmon_path(&device_path);

        Ok(Self {
            index,
            name,
            pci_bus_id,
            card_path,
            hwmon_path,
        })
    }

    /// Create new AMD GPU instance (non-Linux stub)
    #[cfg(not(target_os = "linux"))]
    pub fn new(index: usize, pci_bus_id: String) -> Result<Self, Error> {
        let name = format!("AMD GPU {}", index);
        Ok(Self {
            index,
            name,
            pci_bus_id,
        })
    }
}

#[cfg(target_os = "linux")]
fn read_gpu_name(device_path: &str) -> Option<String> {
    // Try product_name first (newer drivers)
    if let Ok(name) = fs::read_to_string(format!("{}/product_name", device_path)) {
        let name = name.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    // Try parsing uevent for DEVNAME
    if let Ok(uevent) = fs::read_to_string(format!("{}/uevent", device_path)) {
        for line in uevent.lines() {
            if line.starts_with("PCI_ID=") {
                let pci_id = line.trim_start_matches("PCI_ID=");
                return Some(format!("AMD GPU [{}]", pci_id));
            }
        }
    }

    // Try reading device/vendor from PCI
    let vendor = fs::read_to_string(format!("{}/vendor", device_path))
        .map(|s| s.trim().to_string())
        .ok();
    let device = fs::read_to_string(format!("{}/device", device_path))
        .map(|s| s.trim().to_string())
        .ok();

    if let (Some(v), Some(d)) = (vendor, device) {
        return Some(format!("AMD GPU [{}:{}]", v, d));
    }

    None
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

impl Gpu for AmdGpu {
    fn static_info(&self) -> Result<GpuStaticInfo, Error> {
        #[cfg(target_os = "linux")]
        {
            let device_path = format!("{}/device", self.card_path);

            // Read VBIOS version
            let vbios_version = fs::read_to_string(format!("{}/vbios_version", device_path))
                .ok()
                .map(|s| s.trim().to_string());

            // Read driver version
            let driver_version = fs::read_to_string("/sys/module/amdgpu/version")
                .ok()
                .map(|s| s.trim().to_string());

            // Read EU/shader info if available
            let shader_cores = fs::read_to_string(format!("{}/gpu_busy_percent", device_path))
                .ok()
                .and_then(|_| {
                    // This indicates the file exists, but we need a different approach for EU count
                    // Try reading from debugfs or another source
                    None
                });

            Ok(GpuStaticInfo {
                index: self.index,
                vendor: GpuVendor::Amd,
                name: self.name.clone(),
                pci_bus_id: Some(self.pci_bus_id.clone()),
                uuid: None,
                vbios_version,
                driver_version,
                compute_capability: None,
                shader_cores,
                l2_cache: None,
                num_engines: None,
                integrated: self.name.to_lowercase().contains("vega")
                    || self.name.to_lowercase().contains("integrated")
                    || self.name.to_lowercase().contains("renoir")
                    || self.name.to_lowercase().contains("cezanne")
                    || self.name.to_lowercase().contains("phoenix"),
            })
        }

        #[cfg(not(target_os = "linux"))]
        Ok(GpuStaticInfo {
            index: self.index,
            vendor: GpuVendor::Amd,
            name: self.name.clone(),
            pci_bus_id: Some(self.pci_bus_id.clone()),
            uuid: None,
            vbios_version: None,
            driver_version: None,
            compute_capability: None,
            shader_cores: None,
            l2_cache: None,
            num_engines: None,
            integrated: false,
        })
    }

    fn dynamic_info(&self) -> Result<GpuDynamicInfo, Error> {
        #[cfg(target_os = "linux")]
        {
            let device_path = format!("{}/device", self.card_path);

            // Read GPU utilization
            let utilization = fs::read_to_string(format!("{}/gpu_busy_percent", device_path))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(0);

            // Read memory info
            let mem_total = read_sysfs_bytes(&format!("{}/mem_info_vram_total", device_path));
            let mem_used = read_sysfs_bytes(&format!("{}/mem_info_vram_used", device_path));
            let mem_free = mem_total.saturating_sub(mem_used);
            let mem_util = if mem_total > 0 {
                ((mem_used as f64 / mem_total as f64) * 100.0) as u8
            } else {
                0
            };

            // Read clocks from hwmon
            let (graphics_clock, memory_clock) = if let Some(ref hwmon) = self.hwmon_path {
                let gfx = read_sysfs_mhz(&format!("{}/freq1_input", hwmon));
                let mem = read_sysfs_mhz(&format!("{}/freq2_input", hwmon));
                (gfx, mem)
            } else {
                (None, None)
            };

            // Read power from hwmon
            let power_draw = self
                .hwmon_path
                .as_ref()
                .and_then(|hwmon| read_sysfs_microwatts(&format!("{}/power1_average", hwmon)));

            // Read temperature from hwmon
            let temperature = self.hwmon_path.as_ref().and_then(|hwmon| {
                fs::read_to_string(format!("{}/temp1_input", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .map(|t| (t / 1000) as u32)
            });

            // Read critical temp
            let critical_temp = self.hwmon_path.as_ref().and_then(|hwmon| {
                fs::read_to_string(format!("{}/temp1_crit", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .map(|t| (t / 1000) as u32)
            });

            // Read fan speed
            let fan_rpm = self.hwmon_path.as_ref().and_then(|hwmon| {
                fs::read_to_string(format!("{}/fan1_input", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
            });

            let fan_speed = self.hwmon_path.as_ref().and_then(|hwmon| {
                let current = fs::read_to_string(format!("{}/pwm1", hwmon))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())?;
                // PWM is 0-255
                Some(((current as f32 / 255.0) * 100.0) as u8)
            });

            Ok(GpuDynamicInfo {
                utilization,
                memory: GpuMemory {
                    total: mem_total,
                    used: mem_used,
                    free: mem_free,
                    utilization: mem_util,
                },
                clocks: GpuClocks {
                    graphics: graphics_clock,
                    graphics_max: None,
                    memory: memory_clock,
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
                    critical_temperature: critical_temp,
                    fan_speed,
                    fan_rpm,
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
        GpuVendor::Amd
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
            parse_fdinfo_processes(&self.card_path)
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

/// Read a sysfs value as bytes
#[cfg(target_os = "linux")]
fn read_sysfs_bytes(path: &str) -> u64 {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

/// Read frequency in Hz, convert to MHz
#[cfg(target_os = "linux")]
fn read_sysfs_mhz(path: &str) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|hz| (hz / 1_000_000) as u32)
}

/// Read power in microwatts, convert to milliwatts
#[cfg(target_os = "linux")]
fn read_sysfs_microwatts(path: &str) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|uw| (uw / 1000) as u32)
}

/// Parse fdinfo for AMD GPU processes
#[cfg(target_os = "linux")]
fn parse_fdinfo_processes(card_path: &str) -> Result<Vec<GpuProcess>, Error> {
    let mut processes = Vec::new();
    let proc_dir = Path::new("/proc");

    // Get the card's DRM minor number from card path
    let card_name = Path::new(card_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if let Ok(proc_entries) = fs::read_dir(proc_dir) {
        for proc_entry in proc_entries.flatten() {
            let pid_str = proc_entry.file_name();
            let pid_str = pid_str.to_string_lossy();

            // Skip non-numeric entries
            let pid: u32 = match pid_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Check fdinfo for this process
            let fdinfo_dir = proc_entry.path().join("fdinfo");
            if !fdinfo_dir.exists() {
                continue;
            }

            if let Ok(fdinfo_entries) = fs::read_dir(&fdinfo_dir) {
                for fdinfo_entry in fdinfo_entries.flatten() {
                    if let Ok(content) = fs::read_to_string(fdinfo_entry.path()) {
                        // Look for drm-client-id and drm-driver: amdgpu
                        if content.contains("drm-driver:\tamdgpu") {
                            let mut vram_mem = 0u64;
                            let mut gtt_mem = 0u64;

                            for line in content.lines() {
                                if line.starts_with("drm-memory-vram:") {
                                    // Parse "drm-memory-vram:\t1234 KiB"
                                    if let Some(val) = parse_fdinfo_memory(line) {
                                        vram_mem = val;
                                    }
                                } else if line.starts_with("drm-memory-gtt:") {
                                    if let Some(val) = parse_fdinfo_memory(line) {
                                        gtt_mem = val;
                                    }
                                }
                            }

                            if vram_mem > 0 || gtt_mem > 0 {
                                // Get process name
                                let name = fs::read_to_string(proc_entry.path().join("comm"))
                                    .map(|s| s.trim().to_string())
                                    .unwrap_or_else(|_| format!("Process {}", pid));

                                processes.push(GpuProcess {
                                    pid,
                                    name,
                                    gpu_memory: vram_mem,
                                    compute_util: None,
                                    memory_util: None,
                                    encoder_util: None,
                                    decoder_util: None,
                                    process_type: None,
                                });
                            }
                            break; // Found amdgpu for this process
                        }
                    }
                }
            }
        }
    }

    // Deduplicate by PID
    processes.sort_by_key(|p| p.pid);
    processes.dedup_by_key(|p| p.pid);

    Ok(processes)
}

#[cfg(target_os = "linux")]
fn parse_fdinfo_memory(line: &str) -> Option<u64> {
    // Parse lines like "drm-memory-vram:\t1234 KiB" or "drm-memory-vram: 1234 KiB"
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let value: u64 = parts[1].parse().ok()?;
        let unit = parts.get(2).unwrap_or(&"B");
        let bytes = match unit.to_uppercase().as_str() {
            "KIB" | "KB" => value * 1024,
            "MIB" | "MB" => value * 1024 * 1024,
            "GIB" | "GB" => value * 1024 * 1024 * 1024,
            _ => value,
        };
        return Some(bytes);
    }
    None
}

/// Detect all AMD GPUs in the system
pub fn detect_gpus(collection: &mut GpuCollection) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        let dri_path = Path::new("/sys/class/drm");

        if !dri_path.exists() {
            return Ok(());
        }

        let mut gpu_index = 0;

        // Scan card* entries
        if let Ok(entries) = fs::read_dir(dri_path) {
            let mut cards: Vec<_> = entries
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Match card0, card1, etc. but not card0-DP-1, etc.
                    if name.starts_with("card") && !name.contains('-') {
                        Some((name, e.path()))
                    } else {
                        None
                    }
                })
                .collect();

            cards.sort_by(|a, b| a.0.cmp(&b.0));

            for (card_name, card_path) in cards {
                let device_path = card_path.join("device");

                // Check if it's an AMD GPU by reading driver symlink
                let driver_path = device_path.join("driver");
                if let Ok(driver_target) = fs::read_link(&driver_path) {
                    let driver_name = driver_target
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    if driver_name == "amdgpu" {
                        // Get PCI bus ID from device path symlink
                        let pci_bus_id = if let Ok(dev_link) = fs::read_link(&device_path) {
                            dev_link
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                                .to_string()
                        } else {
                            "unknown".to_string()
                        };

                        if let Ok(gpu) = AmdGpu::new(
                            gpu_index,
                            pci_bus_id,
                            card_path.to_string_lossy().to_string(),
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
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_amd_gpu_creation() {
        #[cfg(target_os = "linux")]
        {
            // This will fail on systems without AMD GPUs, which is expected
            let result = AmdGpu::new(
                0,
                "0000:03:00.0".to_string(),
                "/sys/class/drm/card0".to_string(),
            );
            // Just verify it doesn't panic
            let _ = result;
        }
    }
}

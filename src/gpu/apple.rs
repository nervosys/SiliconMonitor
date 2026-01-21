//! Apple GPU monitoring (M1/M2/M3/M4 integrated GPUs)
//!
//! This module provides GPU monitoring for Apple Silicon using powermetrics
//! and Metal Performance Shaders framework.

use super::*;
use crate::error::Error;

/// Apple GPU implementation
pub struct AppleGpu {
    index: u32,
    name: String,
    cores: u32,
    max_frequency: u32,
    max_power: u32,
}

impl AppleGpu {
    /// Create new Apple GPU instance
    pub fn new(index: u32, name: String, cores: u32) -> Self {
        // Estimate max frequency and power based on core count
        let (max_frequency, max_power) = match cores {
            0..=10 => (1400, 20000),  // M1/M2/M3/M4 base
            11..=20 => (1450, 40000), // M1/M2/M3/M4 Pro
            21..=40 => (1500, 75000), // M1/M2/M3/M4 Max
            _ => (1550, 130000),      // M1/M2 Ultra
        };

        Self {
            index,
            name,
            cores,
            max_frequency,
            max_power,
        }
    }

    /// Detect Apple GPUs
    pub fn detect_gpus() -> Result<Vec<AppleGpu>, Error> {
        // Use system_profiler to get GPU info
        let output = std::process::Command::new("system_profiler")
            .args(&["-detailLevel", "basic", "SPDisplaysDataType"])
            .output()
            .map_err(|e| Error::CommandExecutionFailed(format!("system_profiler: {}", e)))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut gpus = Vec::new();
        let mut cores = 8; // Default

        // Parse for GPU cores
        for line in text.lines() {
            if line.contains("Total Number of Cores") {
                if let Some(cores_str) = line.split(':').nth(1) {
                    cores = cores_str.trim().parse().unwrap_or(8);
                }
            }
        }

        // Get SOC name via sysctl
        let output = std::process::Command::new("sysctl")
            .args(&["-n", "machdep.cpu.brand_string"])
            .output()
            .map_err(|e| Error::CommandExecutionFailed(format!("sysctl: {}", e)))?;

        let soc_name = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Create GPU instance
        gpus.push(AppleGpu::new(0, format!("{} GPU", soc_name), cores));

        Ok(gpus)
    }
}

#[cfg(feature = "apple")]
impl Gpu for AppleGpu {
    fn vendor(&self) -> GpuVendor {
        GpuVendor::Apple
    }

    fn index(&self) -> usize {
        self.index as usize
    }

    fn name(&self) -> Result<String, crate::Error> {
        Ok(self.name.clone())
    }

    fn static_info(&self) -> Result<GpuStaticInfo, crate::Error> {
        Ok(GpuStaticInfo {
            index: self.index as usize,
            vendor: GpuVendor::Apple,
            name: self.name.clone(),
            pci_bus_id: None,
            uuid: None,
            vbios_version: None,
            driver_version: None,
            compute_capability: None,
            shader_cores: Some(self.cores),
            l2_cache: None,
            num_engines: None,
            integrated: true, // Apple Silicon GPUs are always integrated
        })
    }

    fn dynamic_info(&self) -> Result<GpuDynamicInfo, crate::Error> {
        // Get metrics from powermetrics via the Silicon monitor
        use crate::silicon::apple::AppleSiliconMonitor;

        let monitor = AppleSiliconMonitor::new()?;
        let data = monitor.parse_powermetrics()?;

        Ok(GpuDynamicInfo {
            utilization: data.gpu_active,
            memory: GpuMemory {
                used: 0, // Not available from powermetrics
                free: 0,
                total: 0,
                utilization: 0,
            },
            clocks: GpuClocks {
                graphics: Some(data.gpu_freq_mhz),
                graphics_max: Some(self.max_frequency),
                memory: None, // Unified memory, no separate memory clock
                memory_max: None,
                sm: None,
                video: None,
            },
            power: GpuPower {
                draw: Some(data.gpu_power_mw),
                limit: None, // Not available
                default_limit: None,
                usage_percent: if data.gpu_power_mw > 0 && self.max_power > 0 {
                    Some(((data.gpu_power_mw as f32 / self.max_power as f32) * 100.0) as u8)
                } else {
                    None
                },
            },
            thermal: GpuThermal {
                temperature: None,               // Not available from powermetrics
                max_temperature: Some(100),      // Conservative estimate
                critical_temperature: Some(110), // Conservative estimate
                fan_speed: None,
                fan_rpm: None,
            },
            pcie: PcieLinkInfo {
                current_gen: None, // Integrated GPU, no PCIe
                current_width: None,
                max_gen: None,
                max_width: None,
                current_speed: None,
                max_speed: None,
                tx_throughput: None,
                rx_throughput: None,
            },
            engines: GpuEngines {
                graphics: Some(data.gpu_active),
                compute: Some(data.gpu_active), // Same as graphics on Apple Silicon
                encoder: None,                  // Not available
                decoder: None,                  // Not available
                copy: None,                     // Not available
                vendor_specific: Vec::new(),
            },
            processes: Vec::new(), // Not available from powermetrics
        })
    }

    fn processes(&self) -> Result<Vec<GpuProcess>, crate::Error> {
        // Process tracking not available via powermetrics
        Ok(Vec::new())
    }

    fn kill_process(&self, _pid: u32) -> Result<(), crate::Error> {
        Err(crate::Error::Unsupported(
            "Process killing not supported on Apple GPUs".to_string(),
        ))
    }

    fn set_power_limit(&mut self, _limit_mw: u32) -> Result<(), crate::Error> {
        Err(crate::Error::Unsupported(
            "Power limit control not supported on Apple GPUs".to_string(),
        ))
    }
}

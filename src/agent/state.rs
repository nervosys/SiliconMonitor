//! System state extraction for AI agent context
//!
//! This module extracts relevant system state from the hardware monitor
//! to provide context for agent responses.

use crate::error::{Result, SimonError};
use crate::gpu::GpuInfo;
use crate::SiliconMonitor;
use serde::{Deserialize, Serialize};

use super::Query;

/// Condensed CPU state for agent context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuState {
    /// CPU model name
    pub name: String,
    /// Number of physical cores
    pub cores: usize,
    /// Number of threads
    pub threads: usize,
    /// Overall CPU utilization (0-100%)
    pub utilization: f32,
    /// CPU temperature (Celsius) if available
    pub temperature_c: Option<f32>,
    /// Current frequency (MHz) if available
    pub frequency_mhz: Option<u64>,
    /// Per-core utilization
    pub per_core_usage: Vec<f32>,
}

/// Condensed memory state for agent context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryState {
    /// Total RAM (MB)
    pub total_mb: u64,
    /// Used RAM (MB)
    pub used_mb: u64,
    /// Available RAM (MB)
    pub available_mb: u64,
    /// Memory utilization (0-100%)
    pub utilization: f32,
}

/// Condensed system state for agent context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    /// CPU information
    pub cpu: Option<CpuState>,

    /// Memory information
    pub memory: Option<MemoryState>,

    /// GPU information (only for queried GPUs)
    pub gpus: Vec<GpuState>,

    /// Timestamp of state capture
    pub timestamp: u64,
}

/// Condensed GPU state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuState {
    /// GPU index
    pub index: usize,

    /// GPU name
    pub name: String,

    /// GPU vendor
    pub vendor: String,

    /// Graphics utilization (0-100%)
    pub utilization: u32,

    /// Memory used (MB)
    pub memory_used_mb: u64,

    /// Memory total (MB)
    pub memory_total_mb: u64,

    /// GPU temperature (Celsius)
    pub temperature_c: u32,

    /// Power usage (Watts)
    pub power_w: f32,

    /// Power limit (Watts)
    pub power_limit_w: Option<f32>,

    /// GPU clock (MHz)
    pub clock_mhz: Option<u32>,

    /// Memory clock (MHz)
    pub memory_clock_mhz: Option<u32>,

    /// Fan speed (%)
    pub fan_speed_percent: Option<u32>,

    /// Number of processes using this GPU
    pub process_count: usize,
}

impl SystemState {
    /// Extract system state from monitor based on query
    pub fn from_monitor(monitor: &SiliconMonitor, query: &Query) -> Result<Self> {
        let gpu_infos = monitor
            .snapshot_gpus()
            .map_err(|e| SimonError::Other(format!("Failed to get GPU state: {}", e)))?;

        // Determine which GPUs to include
        let gpu_states: Vec<GpuState> = if query.all_gpus || query.gpu_indices.is_empty() {
            // Include all GPUs
            gpu_infos
                .into_iter()
                .enumerate()
                .map(|(idx, info)| Self::gpu_to_state(idx, info))
                .collect()
        } else {
            // Include only specified GPUs
            query
                .gpu_indices
                .iter()
                .filter_map(|&idx| {
                    gpu_infos
                        .get(idx)
                        .map(|info| Self::gpu_to_state(idx, info.clone()))
                })
                .collect()
        };

        // Get CPU state (platform-specific)
        let cpu = Self::get_cpu_state();

        // Get memory state (platform-specific)
        let memory = Self::get_memory_state();

        Ok(Self {
            cpu,
            memory,
            gpus: gpu_states,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Get CPU state from platform-specific APIs
    fn get_cpu_state() -> Option<CpuState> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_cpu_stats() {
                let utilization = 100.0 - stats.total.idle;
                let num_cpus = stats.cores.len();
                return Some(CpuState {
                    name: stats
                        .cores
                        .first()
                        .map(|c| c.model.clone())
                        .unwrap_or_else(|| "CPU".to_string()),
                    cores: num_cpus,
                    threads: num_cpus,
                    utilization: utilization as f32,
                    temperature_c: None, // Requires admin
                    frequency_mhz: stats
                        .cores
                        .first()
                        .and_then(|c| c.frequency.as_ref().map(|f| f.current as u64)),
                    per_core_usage: stats
                        .cores
                        .iter()
                        .map(|c| (100.0 - c.idle.unwrap_or(100.0)) as f32)
                        .collect(),
                });
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_cpu_stats() {
                let utilization = 100.0 - stats.total.idle;
                let num_cpus = stats.cores.len();
                return Some(CpuState {
                    name: stats
                        .cores
                        .first()
                        .map(|c| c.model.clone())
                        .unwrap_or_else(|| "CPU".to_string()),
                    cores: num_cpus,
                    threads: num_cpus,
                    utilization: utilization as f32,
                    temperature_c: None,
                    frequency_mhz: stats
                        .cores
                        .first()
                        .and_then(|c| c.frequency.as_ref().map(|f| f.current as u64)),
                    per_core_usage: stats
                        .cores
                        .iter()
                        .map(|c| (100.0 - c.idle.unwrap_or(100.0)) as f32)
                        .collect(),
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS fallback
            let num_cpus = num_cpus::get();
            return Some(CpuState {
                name: "Apple Silicon / Intel".to_string(),
                cores: num_cpus,
                threads: num_cpus,
                utilization: 0.0, // Would need IOKit for real value
                temperature_c: None,
                frequency_mhz: None,
                per_core_usage: vec![],
            });
        }

        #[allow(unreachable_code)]
        None
    }

    /// Get memory state from platform-specific APIs
    fn get_memory_state() -> Option<MemoryState> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_memory_stats() {
                let total_mb = stats.ram.total / 1024 / 1024;
                let used_mb = stats.ram.used / 1024 / 1024;
                let available_mb = stats.ram.free / 1024 / 1024;
                let utilization = if stats.ram.total > 0 {
                    (stats.ram.used as f64 / stats.ram.total as f64 * 100.0) as f32
                } else {
                    0.0
                };
                return Some(MemoryState {
                    total_mb,
                    used_mb,
                    available_mb,
                    utilization,
                });
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_memory_stats() {
                let total_mb = stats.ram.total / 1024 / 1024;
                let used_mb = stats.ram.used / 1024 / 1024;
                let available_mb = stats.ram.free / 1024 / 1024;
                let utilization = if stats.ram.total > 0 {
                    (stats.ram.used as f64 / stats.ram.total as f64 * 100.0) as f32
                } else {
                    0.0
                };
                return Some(MemoryState {
                    total_mb,
                    used_mb,
                    available_mb,
                    utilization,
                });
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS would need vm_statistics for real values
            return None;
        }

        #[allow(unreachable_code)]
        None
    }

    /// Convert GpuInfo to condensed GpuState
    fn gpu_to_state(index: usize, info: GpuInfo) -> GpuState {
        GpuState {
            index,
            name: info.static_info.name,
            vendor: format!("{:?}", info.static_info.vendor),
            utilization: info.dynamic_info.utilization as u32,
            memory_used_mb: info.dynamic_info.memory.used / 1024 / 1024,
            memory_total_mb: info.dynamic_info.memory.total / 1024 / 1024,
            temperature_c: info.dynamic_info.thermal.temperature.unwrap_or(0) as u32,
            power_w: info.dynamic_info.power.draw.unwrap_or(0) as f32 / 1000.0,
            power_limit_w: info.dynamic_info.power.limit.map(|l| l as f32 / 1000.0),
            clock_mhz: info.dynamic_info.clocks.graphics,
            memory_clock_mhz: info.dynamic_info.clocks.memory,
            fan_speed_percent: info.dynamic_info.thermal.fan_speed.map(|f| f as u32),
            process_count: info.dynamic_info.processes.len(),
        }
    }

    /// Format state as natural language text for agent context
    pub fn to_context_string(&self) -> String {
        let mut context = String::new();
        context.push_str("Current System State:\n");

        // CPU information
        if let Some(cpu) = &self.cpu {
            context.push_str(&format!("\nCPU: {} ({} cores)\n", cpu.name, cpu.cores));
            context.push_str(&format!("  Utilization: {:.1}%\n", cpu.utilization));
            if let Some(freq) = cpu.frequency_mhz {
                context.push_str(&format!("  Frequency: {} MHz\n", freq));
            }
            if let Some(temp) = cpu.temperature_c {
                context.push_str(&format!("  Temperature: {:.1}°C\n", temp));
            }
            if !cpu.per_core_usage.is_empty() && cpu.per_core_usage.len() <= 16 {
                // Show per-core for reasonable core counts
                let core_str: Vec<String> = cpu
                    .per_core_usage
                    .iter()
                    .enumerate()
                    .map(|(i, u)| format!("Core{}: {:.0}%", i, u))
                    .collect();
                context.push_str(&format!("  Per-Core: {}\n", core_str.join(", ")));
            }
        }

        // Memory information
        if let Some(mem) = &self.memory {
            context.push_str(&format!(
                "\nMemory: {} / {} MB ({:.1}% used)\n",
                mem.used_mb, mem.total_mb, mem.utilization
            ));
            context.push_str(&format!("  Available: {} MB\n", mem.available_mb));
        }

        // GPU information
        for gpu in &self.gpus {
            context.push_str(&format!(
                "\nGPU {}: {} ({})\n",
                gpu.index, gpu.name, gpu.vendor
            ));
            context.push_str(&format!("  Utilization: {}%\n", gpu.utilization));
            context.push_str(&format!(
                "  Memory: {} / {} MB ({:.1}%)\n",
                gpu.memory_used_mb,
                gpu.memory_total_mb,
                (gpu.memory_used_mb as f32 / gpu.memory_total_mb as f32) * 100.0
            ));
            context.push_str(&format!("  Temperature: {}°C\n", gpu.temperature_c));
            context.push_str(&format!("  Power: {:.1}W", gpu.power_w));
            if let Some(limit) = gpu.power_limit_w {
                context.push_str(&format!(" / {:.1}W", limit));
            }
            context.push('\n');

            if let Some(clock) = gpu.clock_mhz {
                context.push_str(&format!("  GPU Clock: {} MHz\n", clock));
            }
            if let Some(mem_clock) = gpu.memory_clock_mhz {
                context.push_str(&format!("  Memory Clock: {} MHz\n", mem_clock));
            }
            if gpu.process_count > 0 {
                context.push_str(&format!("  Active Processes: {}\n", gpu.process_count));
            }
        }

        context
    }

    /// Get GPU state by index
    pub fn get_gpu(&self, index: usize) -> Option<&GpuState> {
        self.gpus.iter().find(|g| g.index == index)
    }

    /// Get all GPU states
    pub fn all_gpus(&self) -> &[GpuState] {
        &self.gpus
    }

    /// Calculate total power consumption
    pub fn total_power_w(&self) -> f32 {
        self.gpus.iter().map(|g| g.power_w).sum()
    }

    /// Get average GPU utilization
    pub fn avg_utilization(&self) -> f32 {
        if self.gpus.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.gpus.iter().map(|g| g.utilization).sum();
        sum as f32 / self.gpus.len() as f32
    }

    /// Get average GPU temperature
    pub fn avg_temperature(&self) -> f32 {
        if self.gpus.is_empty() {
            return 0.0;
        }
        let sum: u32 = self.gpus.iter().map(|g| g.temperature_c).sum();
        sum as f32 / self.gpus.len() as f32
    }

    /// Get hottest GPU
    pub fn hottest_gpu(&self) -> Option<&GpuState> {
        self.gpus.iter().max_by_key(|g| g.temperature_c)
    }

    /// Get most utilized GPU
    pub fn most_utilized_gpu(&self) -> Option<&GpuState> {
        self.gpus.iter().max_by_key(|g| g.utilization)
    }
}

impl GpuState {
    /// Get memory usage percentage
    pub fn memory_usage_percent(&self) -> f32 {
        if self.memory_total_mb == 0 {
            return 0.0;
        }
        (self.memory_used_mb as f32 / self.memory_total_mb as f32) * 100.0
    }

    /// Get power usage percentage (if limit available)
    pub fn power_usage_percent(&self) -> Option<f32> {
        self.power_limit_w
            .map(|limit| (self.power_w / limit) * 100.0)
    }

    /// Check if GPU is thermally throttling (above 80°C)
    pub fn is_hot(&self) -> bool {
        self.temperature_c >= 80
    }

    /// Check if GPU is critically hot (above 90°C)
    pub fn is_critical(&self) -> bool {
        self.temperature_c >= 90
    }

    /// Check if GPU is heavily utilized (above 80%)
    pub fn is_busy(&self) -> bool {
        self.utilization >= 80
    }

    /// Check if GPU is idle (below 10%)
    pub fn is_idle(&self) -> bool {
        self.utilization < 10
    }

    /// Get health status summary
    pub fn health_status(&self) -> &str {
        if self.is_critical() {
            "CRITICAL: Temperature too high"
        } else if self.is_hot() {
            "WARNING: Temperature elevated"
        } else if self.memory_usage_percent() > 95.0 {
            "WARNING: Memory nearly full"
        } else if self.is_busy() {
            "BUSY: High utilization"
        } else if self.is_idle() {
            "IDLE: Low utilization"
        } else {
            "HEALTHY: Normal operation"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_state_calculations() {
        let gpu = GpuState {
            index: 0,
            name: "Test GPU".to_string(),
            vendor: "NVIDIA".to_string(),
            utilization: 75,
            memory_used_mb: 8000,
            memory_total_mb: 16000,
            temperature_c: 65,
            power_w: 150.0,
            power_limit_w: Some(200.0),
            clock_mhz: Some(1500),
            memory_clock_mhz: Some(6000),
            fan_speed_percent: Some(60),
            process_count: 3,
        };

        assert_eq!(gpu.memory_usage_percent(), 50.0);
        assert_eq!(gpu.power_usage_percent(), Some(75.0));
        assert!(!gpu.is_hot());
        assert!(!gpu.is_idle());
        assert!(!gpu.is_busy());
        assert_eq!(gpu.health_status(), "HEALTHY: Normal operation");
    }

    #[test]
    fn test_system_state_aggregations() {
        let state = SystemState {
            cpu: Some(CpuState {
                name: "Test CPU".to_string(),
                cores: 8,
                threads: 16,
                utilization: 45.0,
                temperature_c: Some(55.0),
                frequency_mhz: Some(3600),
                per_core_usage: vec![40.0, 50.0, 45.0, 42.0, 48.0, 46.0, 44.0, 50.0],
            }),
            memory: Some(MemoryState {
                total_mb: 32768,
                used_mb: 16384,
                available_mb: 16384,
                utilization: 50.0,
            }),
            gpus: vec![
                GpuState {
                    index: 0,
                    name: "GPU 0".to_string(),
                    vendor: "NVIDIA".to_string(),
                    utilization: 50,
                    memory_used_mb: 4000,
                    memory_total_mb: 8000,
                    temperature_c: 60,
                    power_w: 100.0,
                    power_limit_w: Some(150.0),
                    clock_mhz: None,
                    memory_clock_mhz: None,
                    fan_speed_percent: None,
                    process_count: 2,
                },
                GpuState {
                    index: 1,
                    name: "GPU 1".to_string(),
                    vendor: "AMD".to_string(),
                    utilization: 80,
                    memory_used_mb: 6000,
                    memory_total_mb: 8000,
                    temperature_c: 75,
                    power_w: 120.0,
                    power_limit_w: Some(180.0),
                    clock_mhz: None,
                    memory_clock_mhz: None,
                    fan_speed_percent: None,
                    process_count: 1,
                },
            ],
            timestamp: 0,
        };

        assert_eq!(state.total_power_w(), 220.0);
        assert_eq!(state.avg_utilization(), 65.0);
        assert_eq!(state.avg_temperature(), 67.5);
        assert_eq!(state.hottest_gpu().unwrap().index, 1);
        assert_eq!(state.most_utilized_gpu().unwrap().index, 1);

        // Test CPU state
        assert!(state.cpu.is_some());
        assert_eq!(state.cpu.as_ref().unwrap().utilization, 45.0);

        // Test memory state
        assert!(state.memory.is_some());
        assert_eq!(state.memory.as_ref().unwrap().utilization, 50.0);
    }
}

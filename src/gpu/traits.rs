//! Unified traits and types for GPU monitoring
//!
//! This module defines the common interface that all GPU backends must implement.

use serde::{Deserialize, Serialize};
use std::fmt;

/// GPU Vendor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
}

impl fmt::Display for Vendor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Vendor::Nvidia => write!(f, "NVIDIA"),
            Vendor::Amd => write!(f, "AMD"),
            Vendor::Intel => write!(f, "Intel"),
            Vendor::Apple => write!(f, "Apple"),
        }
    }
}

/// GPU Device trait - common interface for all GPUs
pub trait Device: Send + Sync {
    /// Get the GPU vendor
    fn vendor(&self) -> Vendor;

    /// Get the device index (0-based)
    fn index(&self) -> u32;

    /// Get the device name (e.g., "NVIDIA GeForce RTX 4090")
    fn name(&self) -> Result<String, Error>;

    /// Get the device UUID (unique identifier)
    fn uuid(&self) -> Result<String, Error>;

    /// Get PCI bus information
    fn pci_info(&self) -> Result<PciInfo, Error>;

    /// Get driver version
    fn driver_version(&self) -> Result<String, Error>;

    // === Real-time Metrics ===

    /// Get temperature readings from all available sensors
    fn temperature(&self) -> Result<Temperature, Error>;

    /// Get power consumption and limits
    fn power(&self) -> Result<Power, Error>;

    /// Get clock frequencies (graphics, memory, SM, video)
    fn clocks(&self) -> Result<Clocks, Error>;

    /// Get utilization percentages (GPU, memory, encoders, decoders)
    fn utilization(&self) -> Result<Utilization, Error>;

    /// Get memory usage (total, used, free)
    fn memory(&self) -> Result<Memory, Error>;

    /// Get fan speed (RPM or percentage)
    fn fan_speed(&self) -> Result<Option<FanSpeed>, Error>;

    /// Get performance state (P0-P12 for NVIDIA, power states for others)
    fn performance_state(&self) -> Result<Option<String>, Error>;

    // === Process Monitoring ===

    /// Get list of processes using this GPU
    fn processes(&self) -> Result<Vec<Box<dyn GpuProcess>>, Error>;

    // === Vendor-Specific Features (optional) ===

    /// Get NVLink status (NVIDIA only)
    fn nvlink_status(&self) -> Result<Vec<NvLinkStatus>, Error> {
        Err(Error::NotSupported)
    }

    /// Get MIG mode status (NVIDIA only)
    fn mig_mode(&self) -> Result<MigMode, Error> {
        Err(Error::NotSupported)
    }

    /// Get ECC error counts (NVIDIA, AMD)
    fn ecc_errors(&self) -> Result<EccErrors, Error> {
        Err(Error::NotSupported)
    }

    /// Get compute mode (NVIDIA)
    fn compute_mode(&self) -> Result<Option<ComputeMode>, Error> {
        Ok(None)
    }

    /// Get persistence mode (NVIDIA)
    fn persistence_mode(&self) -> Result<Option<bool>, Error> {
        Ok(None)
    }

    // === Control Functions (may require root/admin) ===

    /// Set power limit (Watts)
    fn set_power_limit(&mut self, _watts: f32) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    /// Lock GPU clocks to specified frequency (MHz)
    fn lock_gpu_clocks(&mut self, _min_mhz: u32, _max_mhz: u32) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    /// Reset GPU clocks to default
    fn reset_gpu_clocks(&mut self) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    /// Set persistence mode (NVIDIA)
    fn set_persistence_mode(&mut self, _enabled: bool) -> Result<(), Error> {
        Err(Error::NotSupported)
    }

    /// Set compute mode (NVIDIA)
    fn set_compute_mode(&mut self, _mode: ComputeMode) -> Result<(), Error> {
        Err(Error::NotSupported)
    }
}

/// GPU Process trait - information about a process using GPU
pub trait GpuProcess: Send + Sync {
    /// Process ID
    fn pid(&self) -> u32;

    /// Process name/command
    fn name(&self) -> Result<String, Error>;

    /// Process type (Compute, Graphics, Mixed)
    fn process_type(&self) -> ProcessType;

    /// GPU memory used by this process (bytes)
    fn gpu_memory_used(&self) -> Result<u64, Error>;

    /// SM utilization percentage (0-100)
    fn sm_utilization(&self) -> Result<Option<f32>, Error> {
        Ok(None)
    }

    /// Encoder utilization percentage (0-100)
    fn encoder_utilization(&self) -> Result<Option<f32>, Error> {
        Ok(None)
    }

    /// Decoder utilization percentage (0-100)
    fn decoder_utilization(&self) -> Result<Option<f32>, Error> {
        Ok(None)
    }

    /// Host CPU utilization percentage (0-100)
    fn cpu_utilization(&self) -> Result<Option<f32>, Error> {
        Ok(None)
    }

    /// Host memory used (bytes)
    fn host_memory_used(&self) -> Result<Option<u64>, Error> {
        Ok(None)
    }

    /// Process running time (seconds)
    fn running_time(&self) -> Result<Option<u64>, Error> {
        Ok(None)
    }
}

// === Data Structures ===

/// PCI Bus Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciInfo {
    pub domain: u32,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub bus_id: String, // Format: "0000:01:00.0"
    pub pcie_generation: Option<u32>,
    pub pcie_link_width: Option<u32>,
}

/// Temperature readings from various sensors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Temperature {
    /// Edge temperature (AMD) in Celsius
    pub edge: Option<f32>,
    /// Junction/GPU temperature (NVIDIA, Intel) in Celsius
    pub junction: Option<f32>,
    /// Memory temperature in Celsius
    pub memory: Option<f32>,
    /// Hotspot temperature (AMD) in Celsius
    pub hotspot: Option<f32>,
    /// VR GFX voltage regulator temperature (AMD) in Celsius
    pub vr_gfx: Option<f32>,
    /// VR SOC voltage regulator temperature (AMD) in Celsius
    pub vr_soc: Option<f32>,
    /// VR MEM voltage regulator temperature (AMD) in Celsius
    pub vr_mem: Option<f32>,
    /// HBM memory temperatures (AMD MI100/MI200/MI300) in Celsius
    pub hbm: Option<Vec<f32>>,
    /// Temperature thresholds (slowdown, shutdown, critical)
    pub thresholds: Option<TemperatureThresholds>,
}

/// Temperature thresholds for GPU thermal management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureThresholds {
    /// Slowdown threshold (°C) - GPU begins throttling performance
    pub slowdown: Option<f32>,
    /// Shutdown threshold (°C) - GPU will shut down to prevent damage
    pub shutdown: Option<f32>,
    /// Critical threshold (°C) - Maximum safe operating temperature
    pub critical: Option<f32>,
    /// Memory critical threshold (°C) - Maximum safe memory temperature
    pub memory_critical: Option<f32>,
}

impl Temperature {
    /// Get the primary/most important temperature reading
    pub fn primary(&self) -> Option<f32> {
        self.junction.or(self.hotspot).or(self.edge).or(self.memory)
    }

    /// Get the maximum temperature across all sensors
    pub fn max(&self) -> Option<f32> {
        let mut temps = Vec::new();
        if let Some(t) = self.edge {
            temps.push(t);
        }
        if let Some(t) = self.junction {
            temps.push(t);
        }
        if let Some(t) = self.memory {
            temps.push(t);
        }
        if let Some(t) = self.hotspot {
            temps.push(t);
        }
        if let Some(t) = self.vr_gfx {
            temps.push(t);
        }
        if let Some(t) = self.vr_soc {
            temps.push(t);
        }
        if let Some(t) = self.vr_mem {
            temps.push(t);
        }
        if let Some(hbm_temps) = &self.hbm {
            temps.extend(hbm_temps);
        }
        temps.into_iter().reduce(f32::max)
    }

    /// Get temperature status relative to thresholds
    pub fn status(&self) -> TemperatureStatus {
        let Some(temp) = self.primary() else {
            return TemperatureStatus::Unknown;
        };

        let Some(thresholds) = &self.thresholds else {
            return TemperatureStatus::Normal;
        };

        // Check critical first (most severe)
        if let Some(critical) = thresholds.critical {
            if temp >= critical {
                return TemperatureStatus::Critical;
            }
        }

        // Check shutdown threshold
        if let Some(shutdown) = thresholds.shutdown {
            if temp >= shutdown {
                return TemperatureStatus::Shutdown;
            }
        }

        // Check slowdown/throttle threshold
        if let Some(slowdown) = thresholds.slowdown {
            if temp >= slowdown {
                return TemperatureStatus::Throttling;
            }
        }

        // Normal if below all thresholds
        TemperatureStatus::Normal
    }
}

/// Temperature status based on thresholds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemperatureStatus {
    /// Temperature is normal
    Normal,
    /// Temperature is above slowdown threshold (throttling)
    Throttling,
    /// Temperature is approaching shutdown threshold
    Shutdown,
    /// Temperature is at or above critical threshold
    Critical,
    /// Temperature data not available or no thresholds defined
    Unknown,
}

impl fmt::Display for TemperatureStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemperatureStatus::Normal => write!(f, "Normal"),
            TemperatureStatus::Throttling => write!(f, "Throttling"),
            TemperatureStatus::Shutdown => write!(f, "Shutdown Warning"),
            TemperatureStatus::Critical => write!(f, "CRITICAL"),
            TemperatureStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Power consumption and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Power {
    /// Current power draw in Watts
    pub current: f32,
    /// Average power draw in Watts (if available)
    pub average: Option<f32>,
    /// Power limit in Watts
    pub limit: f32,
    /// Default power limit in Watts
    pub default_limit: f32,
    /// Minimum allowed power limit in Watts
    pub min_limit: f32,
    /// Maximum allowed power limit in Watts
    pub max_limit: f32,
    /// Enforced power limit (may differ from limit if capped by system)
    pub enforced_limit: f32,
}

/// Clock frequencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clocks {
    /// Graphics/shader clock in MHz
    pub graphics: u32,
    /// Memory clock in MHz
    pub memory: u32,
    /// SM (Streaming Multiprocessor) clock in MHz (NVIDIA)
    pub sm: Option<u32>,
    /// Video clock in MHz (NVIDIA)
    pub video: Option<u32>,
}

/// Utilization percentages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Utilization {
    /// GPU utilization (0-100%)
    pub gpu: f32,
    /// Memory controller utilization (0-100%)
    pub memory: f32,
    /// Video encoder utilization (0-100%)
    pub encoder: Option<f32>,
    /// Video decoder utilization (0-100%)
    pub decoder: Option<f32>,
    /// JPEG encoder utilization (0-100%, NVIDIA only)
    pub jpeg: Option<f32>,
    /// Optical Flow Accelerator utilization (0-100%, NVIDIA only)
    pub ofa: Option<f32>,
}

/// Memory usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Total memory in bytes
    pub total: u64,
    /// Used memory in bytes
    pub used: u64,
    /// Free memory in bytes
    pub free: u64,
    /// BAR1 total memory (NVIDIA)
    pub bar1_total: Option<u64>,
    /// BAR1 used memory (NVIDIA)
    pub bar1_used: Option<u64>,
}

impl Memory {
    /// Get memory utilization percentage
    pub fn utilization_percent(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64 * 100.0) as f32
        }
    }
}

/// Fan speed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FanSpeed {
    Rpm(u32),
    Percent(u32),
}

/// Process type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessType {
    Compute,
    Graphics,
    Mixed,
}

/// Compute mode (NVIDIA)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComputeMode {
    Default,
    ExclusiveThread,
    Prohibited,
    ExclusiveProcess,
}

/// NVLink status (NVIDIA)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvLinkStatus {
    pub link_id: u32,
    pub state: LinkState,
    pub version: u32,
    pub remote_device_type: String,
    pub remote_pci_bus_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
    Active,
    Inactive,
    Unknown,
}

/// MIG mode (NVIDIA Multi-Instance GPU)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigMode {
    pub current: bool,
    pub pending: bool,
}

/// ECC error counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EccErrors {
    /// Volatile single-bit errors (since last reboot)
    pub volatile_single_bit: u64,
    /// Volatile double-bit errors (since last reboot)
    pub volatile_double_bit: u64,
    /// Aggregate single-bit errors (total)
    pub aggregate_single_bit: u64,
    /// Aggregate double-bit errors (total)
    pub aggregate_double_bit: u64,
}

// === Error Types ===

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Operation not supported on this device")]
    NotSupported,

    #[error("No GPU devices found")]
    NoDevicesFound,

    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Failed to query device: {0}")]
    QueryFailed(String),

    #[error("Failed to control device: {0}")]
    ControlFailed(String),

    #[error("Insufficient permissions: {0}")]
    PermissionDenied(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[cfg(feature = "nvidia")]
    #[error("NVML error: {0}")]
    NvmlError(#[from] nvml_wrapper::error::NvmlError),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Temperature tests ===

    #[test]
    fn test_temperature_primary_junction_first() {
        let temp = Temperature {
            edge: Some(55.0),
            junction: Some(65.0),
            memory: Some(50.0),
            hotspot: Some(70.0),
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.primary(), Some(65.0));
    }

    #[test]
    fn test_temperature_primary_fallback_hotspot() {
        let temp = Temperature {
            edge: Some(55.0),
            junction: None,
            memory: None,
            hotspot: Some(70.0),
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.primary(), Some(70.0));
    }

    #[test]
    fn test_temperature_primary_fallback_edge() {
        let temp = Temperature {
            edge: Some(55.0),
            junction: None,
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.primary(), Some(55.0));
    }

    #[test]
    fn test_temperature_primary_none() {
        let temp = Temperature {
            edge: None,
            junction: None,
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.primary(), None);
    }

    #[test]
    fn test_temperature_max() {
        let temp = Temperature {
            edge: Some(55.0),
            junction: Some(65.0),
            memory: Some(50.0),
            hotspot: Some(80.0),
            vr_gfx: Some(72.0),
            vr_soc: None,
            vr_mem: None,
            hbm: Some(vec![60.0, 62.0, 85.0]),
            thresholds: None,
        };
        assert_eq!(temp.max(), Some(85.0));
    }

    #[test]
    fn test_temperature_max_none_when_empty() {
        let temp = Temperature {
            edge: None,
            junction: None,
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.max(), None);
    }

    #[test]
    fn test_temperature_status_normal() {
        let temp = Temperature {
            edge: None,
            junction: Some(50.0),
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: Some(TemperatureThresholds {
                slowdown: Some(80.0),
                shutdown: Some(95.0),
                critical: Some(100.0),
                memory_critical: None,
            }),
        };
        assert_eq!(temp.status(), TemperatureStatus::Normal);
    }

    #[test]
    fn test_temperature_status_throttling() {
        let temp = Temperature {
            edge: None,
            junction: Some(85.0),
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: Some(TemperatureThresholds {
                slowdown: Some(80.0),
                shutdown: Some(95.0),
                critical: Some(100.0),
                memory_critical: None,
            }),
        };
        assert_eq!(temp.status(), TemperatureStatus::Throttling);
    }

    #[test]
    fn test_temperature_status_shutdown() {
        let temp = Temperature {
            edge: None,
            junction: Some(96.0),
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: Some(TemperatureThresholds {
                slowdown: Some(80.0),
                shutdown: Some(95.0),
                critical: Some(100.0),
                memory_critical: None,
            }),
        };
        assert_eq!(temp.status(), TemperatureStatus::Shutdown);
    }

    #[test]
    fn test_temperature_status_critical() {
        let temp = Temperature {
            edge: None,
            junction: Some(105.0),
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: Some(TemperatureThresholds {
                slowdown: Some(80.0),
                shutdown: Some(95.0),
                critical: Some(100.0),
                memory_critical: None,
            }),
        };
        assert_eq!(temp.status(), TemperatureStatus::Critical);
    }

    #[test]
    fn test_temperature_status_unknown_when_no_primary() {
        let temp = Temperature {
            edge: None,
            junction: None,
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: Some(TemperatureThresholds {
                slowdown: Some(80.0),
                shutdown: Some(95.0),
                critical: Some(100.0),
                memory_critical: None,
            }),
        };
        assert_eq!(temp.status(), TemperatureStatus::Unknown);
    }

    #[test]
    fn test_temperature_status_normal_when_no_thresholds() {
        let temp = Temperature {
            edge: None,
            junction: Some(90.0),
            memory: None,
            hotspot: None,
            vr_gfx: None,
            vr_soc: None,
            vr_mem: None,
            hbm: None,
            thresholds: None,
        };
        assert_eq!(temp.status(), TemperatureStatus::Normal);
    }

    // === Memory tests ===

    #[test]
    fn test_memory_utilization_percent() {
        let mem = Memory {
            total: 8 * 1024 * 1024 * 1024, // 8GB
            used: 4 * 1024 * 1024 * 1024,  // 4GB
            free: 4 * 1024 * 1024 * 1024,
            bar1_total: None,
            bar1_used: None,
        };
        assert!((mem.utilization_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_memory_utilization_zero_total() {
        let mem = Memory {
            total: 0,
            used: 0,
            free: 0,
            bar1_total: None,
            bar1_used: None,
        };
        assert_eq!(mem.utilization_percent(), 0.0);
    }

    #[test]
    fn test_memory_utilization_full() {
        let mem = Memory {
            total: 1024,
            used: 1024,
            free: 0,
            bar1_total: None,
            bar1_used: None,
        };
        assert!((mem.utilization_percent() - 100.0).abs() < 0.01);
    }

    // === Vendor tests ===

    #[test]
    fn test_vendor_display() {
        assert_eq!(Vendor::Nvidia.to_string(), "NVIDIA");
        assert_eq!(Vendor::Amd.to_string(), "AMD");
        assert_eq!(Vendor::Intel.to_string(), "Intel");
        assert_eq!(Vendor::Apple.to_string(), "Apple");
    }

    // === TemperatureStatus tests ===

    #[test]
    fn test_temperature_status_display() {
        assert_eq!(TemperatureStatus::Normal.to_string(), "Normal");
        assert_eq!(TemperatureStatus::Throttling.to_string(), "Throttling");
        assert_eq!(TemperatureStatus::Shutdown.to_string(), "Shutdown Warning");
        assert_eq!(TemperatureStatus::Critical.to_string(), "CRITICAL");
        assert_eq!(TemperatureStatus::Unknown.to_string(), "Unknown");
    }
}

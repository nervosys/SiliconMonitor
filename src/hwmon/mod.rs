// Hardware monitoring module - Rust implementation of low-level sensor reading
//
// This module provides direct hardware access for sensor readings similar to
// LibreHardwareMonitor, but implemented in pure Rust.
//
// Supported sensors:
// - CPU temperature via MSR (Model-Specific Registers), Performance Counters, Registry
// - Storage temperature via S.M.A.R.T.
// - GPU temperature via NVML (NVIDIA) or vendor APIs
// - Motherboard/Super I/O sensors (requires kernel driver - currently limited)
//
// Note: Some sensors require elevated privileges (admin) to read.

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

mod cpu_temp;
mod smart;

pub use cpu_temp::read_cpu_temperatures;
pub use smart::read_storage_temperatures;

use serde::{Deserialize, Serialize};

/// Hardware sensor reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwSensor {
    pub name: String,
    pub value: f32,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub sensor_type: HwSensorType,
    pub hardware_type: HwType,
}

/// Type of sensor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HwSensorType {
    Temperature,
    Voltage,
    Fan,
    Power,
    Clock,
    Load,
    Data,
    SmallData,
    Throughput,
    Control, // PWM control values
    Energy,  // Energy counters (Joules)
}

/// Type of hardware component
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HwType {
    Cpu,
    Gpu,
    Motherboard,
    Storage,
    Memory,
    Network,
    Psu,
    Other,
}

/// Hardware monitor that aggregates all sensor sources
pub struct HardwareMonitor {
    pub sensors: Vec<HwSensor>,
}

impl HardwareMonitor {
    /// Create a new hardware monitor and read all sensors
    pub fn new() -> Self {
        let mut monitor = Self {
            sensors: Vec::new(),
        };
        monitor.refresh();
        monitor
    }

    /// Refresh all sensor readings
    pub fn refresh(&mut self) {
        self.sensors.clear();

        // CPU temperatures
        self.sensors.extend(cpu_temp::read_cpu_temperatures());

        // Storage temperatures
        self.sensors.extend(smart::read_storage_temperatures());

        #[cfg(target_os = "windows")]
        {
            // GPU temperatures and other sensors
            self.sensors.extend(windows::read_gpu_temperatures());

            // WMI thermal zones as fallback
            self.sensors.extend(windows::read_wmi_temperatures());

            // NVIDIA-specific sensors if available
            #[cfg(feature = "nvidia")]
            {
                self.sensors.extend(windows::read_nvidia_fan_speeds());
                self.sensors.extend(windows::read_nvidia_power());
                self.sensors.extend(windows::read_nvidia_clocks());
                self.sensors.extend(windows::read_nvidia_utilization());
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux has comprehensive hwmon support
            self.sensors.extend(linux::read_all_linux_sensors());
        }
    }

    /// Get all sensors
    pub fn all_sensors(&self) -> &[HwSensor] {
        &self.sensors
    }

    /// Get sensors by type
    pub fn sensors_by_type(&self, sensor_type: HwSensorType) -> Vec<&HwSensor> {
        self.sensors
            .iter()
            .filter(|s| s.sensor_type == sensor_type)
            .collect()
    }

    /// Get sensors by hardware type
    pub fn sensors_by_hardware(&self, hw_type: HwType) -> Vec<&HwSensor> {
        self.sensors
            .iter()
            .filter(|s| s.hardware_type == hw_type)
            .collect()
    }

    /// Get all temperature sensors
    pub fn temperatures(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Temperature)
    }

    /// Get all voltage sensors
    pub fn voltages(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Voltage)
    }

    /// Get all fan sensors
    pub fn fans(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Fan)
    }

    /// Get all power sensors
    pub fn power_sensors(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Power)
    }

    /// Get all clock sensors
    pub fn clocks(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Clock)
    }

    /// Get all load/utilization sensors
    pub fn loads(&self) -> Vec<&HwSensor> {
        self.sensors_by_type(HwSensorType::Load)
    }

    /// Get CPU sensors
    pub fn cpu_sensors(&self) -> Vec<&HwSensor> {
        self.sensors_by_hardware(HwType::Cpu)
    }

    /// Get GPU sensors
    pub fn gpu_sensors(&self) -> Vec<&HwSensor> {
        self.sensors_by_hardware(HwType::Gpu)
    }

    /// Get storage sensors
    pub fn storage_sensors(&self) -> Vec<&HwSensor> {
        self.sensors_by_hardware(HwType::Storage)
    }

    /// Get motherboard sensors
    pub fn motherboard_sensors(&self) -> Vec<&HwSensor> {
        self.sensors_by_hardware(HwType::Motherboard)
    }
}

impl Default for HardwareMonitor {
    fn default() -> Self {
        Self::new()
    }
}

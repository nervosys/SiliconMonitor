//! Temperature monitoring

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Temperature sensor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureSensor {
    /// Sensor online status
    pub online: bool,
    /// Current temperature in Celsius
    pub temp: f32,
    /// Maximum temperature threshold in Celsius (optional)
    pub max: Option<f32>,
    /// Critical temperature threshold in Celsius (optional)
    pub crit: Option<f32>,
}

/// Temperature statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureStats {
    /// Temperature sensors by name
    pub sensors: HashMap<String, TemperatureSensor>,
}

impl TemperatureStats {
    /// Create a new temperature stats instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            sensors: HashMap::new(),
        })
    }

    /// Get sensor by name
    pub fn get_sensor(&self, name: &str) -> Option<&TemperatureSensor> {
        self.sensors.get(name)
    }

    /// Get maximum temperature across all sensors
    pub fn max_temp(&self) -> Option<f32> {
        self.sensors
            .values()
            .filter(|s| s.online)
            .map(|s| s.temp)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}

impl Default for TemperatureStats {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            sensors: HashMap::new(),
        })
    }
}

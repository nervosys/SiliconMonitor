//! Power monitoring

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Power rail information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerRail {
    /// Rail online status
    pub online: bool,
    /// Sensor type (e.g., INA3221)
    pub sensor_type: String,
    /// Voltage in millivolts
    pub voltage: u32,
    /// Current in milliamperes
    pub current: u32,
    /// Power in milliwatts
    pub power: u32,
    /// Average power in milliwatts
    pub average: u32,
    /// Warning current limit in milliamperes (optional)
    pub warn: Option<u32>,
    /// Critical current limit in milliamperes (optional)
    pub crit: Option<u32>,
}

/// Total power information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalPower {
    /// Total power in milliwatts
    pub power: u32,
    /// Average total power in milliwatts
    pub average: u32,
}

/// Exponential Moving Average calculator for power readings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerEma {
    /// Current EMA value
    value: f64,
    /// Smoothing factor (0.0 - 1.0, higher = more responsive)
    alpha: f64,
    /// Number of samples seen
    samples: u64,
}

impl PowerEma {
    /// Create a new EMA calculator with given alpha (smoothing factor)
    /// Alpha of 0.1 gives ~10 sample smoothing, 0.2 gives ~5 sample smoothing
    pub fn new(alpha: f64) -> Self {
        Self {
            value: 0.0,
            alpha: alpha.clamp(0.01, 1.0),
            samples: 0,
        }
    }

    /// Create EMA with default smoothing (alpha = 0.1 for ~10 sample window)
    pub fn default_smoothing() -> Self {
        Self::new(0.1)
    }

    /// Update with new sample and return new average
    pub fn update(&mut self, sample: u32) -> u32 {
        let sample_f = sample as f64;
        if self.samples == 0 {
            self.value = sample_f;
        } else {
            self.value = self.alpha * sample_f + (1.0 - self.alpha) * self.value;
        }
        self.samples += 1;
        self.value.round() as u32
    }

    /// Get current average value
    pub fn average(&self) -> u32 {
        self.value.round() as u32
    }

    /// Get number of samples processed
    pub fn sample_count(&self) -> u64 {
        self.samples
    }

    /// Reset the EMA calculator
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.samples = 0;
    }
}

impl Default for PowerEma {
    fn default() -> Self {
        Self::default_smoothing()
    }
}

/// Power average tracker for multiple rails
#[derive(Debug, Clone, Default)]
pub struct PowerAverageTracker {
    /// EMA calculators for each rail
    rail_emas: HashMap<String, PowerEma>,
    /// EMA calculator for total power
    total_ema: PowerEma,
}

impl PowerAverageTracker {
    /// Create a new power average tracker
    pub fn new() -> Self {
        Self {
            rail_emas: HashMap::new(),
            total_ema: PowerEma::default_smoothing(),
        }
    }

    /// Create tracker with custom smoothing factor
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            rail_emas: HashMap::new(),
            total_ema: PowerEma::new(alpha),
        }
    }

    /// Update a rail's power reading and return the new average
    pub fn update_rail(&mut self, name: &str, power: u32) -> u32 {
        self.rail_emas
            .entry(name.to_string())
            .or_insert_with(PowerEma::default_smoothing)
            .update(power)
    }

    /// Update total power and return the new average
    pub fn update_total(&mut self, power: u32) -> u32 {
        self.total_ema.update(power)
    }

    /// Get rail average without updating
    pub fn get_rail_average(&self, name: &str) -> Option<u32> {
        self.rail_emas.get(name).map(|ema| ema.average())
    }

    /// Get total average without updating
    pub fn get_total_average(&self) -> u32 {
        self.total_ema.average()
    }

    /// Update PowerStats with tracked averages
    pub fn update_stats(&mut self, stats: &mut PowerStats) {
        // Update each rail's average
        for (name, rail) in stats.rails.iter_mut() {
            rail.average = self.update_rail(name, rail.power);
        }

        // Update total average
        stats.total.average = self.update_total(stats.total.power);
    }

    /// Reset all tracking
    pub fn reset(&mut self) {
        self.rail_emas.clear();
        self.total_ema.reset();
    }
}

/// Power statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStats {
    /// Individual power rails
    pub rails: HashMap<String, PowerRail>,
    /// Total power
    pub total: TotalPower,
}

impl PowerStats {
    /// Create a new power stats instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            rails: HashMap::new(),
            total: TotalPower {
                power: 0,
                average: 0,
            },
        })
    }

    /// Get power rail by name
    pub fn get_rail(&self, name: &str) -> Option<&PowerRail> {
        self.rails.get(name)
    }

    /// Get total power in watts
    pub fn total_watts(&self) -> f32 {
        self.total.power as f32 / 1000.0
    }
}

impl Default for PowerStats {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

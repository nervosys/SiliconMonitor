//! Battery monitoring module
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Battery charging state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargingState { Charging, Discharging, Full, NotCharging, Unknown }

/// Battery health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryHealth { Good, Fair, Poor, Unknown }

/// Battery information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryInfo {
    pub name: String,
    pub manufacturer: Option<String>,
    pub charge_percent: f32,
    pub state: ChargingState,
    pub health: BatteryHealth,
    pub voltage_mv: Option<u32>,
    pub time_to_empty: Option<Duration>,
    pub time_to_full: Option<Duration>,
    pub cycle_count: Option<u32>,
    pub temperature_c: Option<f32>,
}

/// Battery monitor
pub struct BatteryMonitor {
    batteries: Vec<BatteryInfo>,
    ac_connected: bool,
}

impl BatteryMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        Ok(Self { batteries: Vec::new(), ac_connected: true })
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> { Ok(()) }
    pub fn batteries(&self) -> &[BatteryInfo] { &self.batteries }
    pub fn ac_connected(&self) -> bool { self.ac_connected }
}

impl Default for BatteryMonitor {
    fn default() -> Self { Self { batteries: Vec::new(), ac_connected: true } }
}
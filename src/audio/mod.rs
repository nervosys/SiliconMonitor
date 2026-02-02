//! Audio device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioDeviceType { Output, Input, Duplex }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioState { Active, Idle, Suspended, Unavailable }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub device_type: AudioDeviceType,
    pub state: AudioState,
    pub is_default: bool,
    pub volume: f32,
    pub muted: bool,
}

pub struct AudioMonitor {
    devices: Vec<AudioDevice>,
    default_output: Option<String>,
    default_input: Option<String>,
}

impl AudioMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        Ok(Self { devices: Vec::new(), default_output: None, default_input: None })
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> { Ok(()) }
    pub fn devices(&self) -> &[AudioDevice] { &self.devices }
    pub fn master_volume(&self) -> f32 { 1.0 }
    pub fn is_muted(&self) -> bool { false }
}

impl Default for AudioMonitor {
    fn default() -> Self { Self { devices: Vec::new(), default_output: None, default_input: None } }
}
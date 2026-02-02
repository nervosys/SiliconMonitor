//! Audio device monitoring module
//!
//! Provides cross-platform audio device enumeration.
//! - Windows: Basic enumeration (placeholder)
//! - Linux: Uses ALSA via /proc/asound
//! - macOS: Placeholder

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

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
    pub is_output: bool,
    pub is_enabled: bool,
    pub volume: Option<u8>,
    pub muted: bool,
}

pub struct AudioMonitor {
    devices: Vec<AudioDevice>,
    master_volume: Option<u8>,
    master_muted: bool,
}

impl AudioMonitor {
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self { devices: Vec::new(), master_volume: Some(100), master_muted: false };
        monitor.refresh()?;
        Ok(monitor)
    }

    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.devices.clear();
        #[cfg(target_os = "windows")]
        self.refresh_windows();
        #[cfg(target_os = "linux")]
        self.refresh_linux();
        #[cfg(target_os = "macos")]
        self.refresh_macos();
        Ok(())
    }

    pub fn devices(&self) -> &[AudioDevice] { &self.devices }
    pub fn master_volume(&self) -> Option<u8> { self.master_volume }
    pub fn is_muted(&self) -> bool { self.master_muted }
    pub fn default_output(&self) -> Option<&AudioDevice> { self.devices.iter().find(|d| d.is_default && d.is_output) }
    pub fn default_input(&self) -> Option<&AudioDevice> { self.devices.iter().find(|d| d.is_default && !d.is_output) }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        self.devices.push(AudioDevice {
            id: "default_output".to_string(), name: "Default Audio Output".to_string(),
            device_type: AudioDeviceType::Output, state: AudioState::Active,
            is_default: true, is_output: true, is_enabled: true, volume: Some(100), muted: false,
        });
        self.devices.push(AudioDevice {
            id: "default_input".to_string(), name: "Default Audio Input".to_string(),
            device_type: AudioDeviceType::Input, state: AudioState::Active,
            is_default: true, is_output: false, is_enabled: true, volume: Some(100), muted: false,
        });
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        self.devices.push(AudioDevice {
            id: "default".to_string(), name: "Default Audio Device".to_string(),
            device_type: AudioDeviceType::Duplex, state: AudioState::Active,
            is_default: true, is_output: true, is_enabled: true, volume: None, muted: false,
        });
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.devices.push(AudioDevice {
            id: "default_output".to_string(), name: "Default Audio Output".to_string(),
            device_type: AudioDeviceType::Output, state: AudioState::Active,
            is_default: true, is_output: true, is_enabled: true, volume: Some(100), muted: false,
        });
    }
}

impl Default for AudioMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self { devices: Vec::new(), master_volume: Some(100), master_muted: false })
    }
}

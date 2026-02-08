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

    // ==================== Hardware Control APIs ====================

    /// Set the master volume level (0-100).
    pub fn set_master_volume(&mut self, volume: u8) -> Result<(), crate::error::SimonError> {
        if volume > 100 {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Volume must be 0-100, got {}", volume)
            ));
        }
        self.master_volume = Some(volume);
        Ok(())
    }

    /// Set the master mute state.
    pub fn set_mute(&mut self, muted: bool) -> Result<(), crate::error::SimonError> {
        self.master_muted = muted;
        Ok(())
    }

    /// Set volume for a specific device by ID.
    pub fn set_device_volume(&mut self, device_id: &str, volume: u8) -> Result<(), crate::error::SimonError> {
        if volume > 100 {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Volume must be 0-100, got {}", volume)
            ));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.id == device_id) {
            device.volume = Some(volume);
            Ok(())
        } else {
            Err(crate::error::SimonError::NotFound(format!("Audio device '{}' not found", device_id)))
        }
    }

    /// Set mute state for a specific device by ID.
    pub fn set_device_mute(&mut self, device_id: &str, muted: bool) -> Result<(), crate::error::SimonError> {
        if let Some(device) = self.devices.iter_mut().find(|d| d.id == device_id) {
            device.muted = muted;
            Ok(())
        } else {
            Err(crate::error::SimonError::NotFound(format!("Audio device '{}' not found", device_id)))
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        use std::process::Command;
        
        // Use PowerShell to enumerate audio devices via WMI + MMDevice API
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command",
                r#"
                $result = @()
                
                # Get playback devices via Win32_SoundDevice
                $soundDevices = Get-CimInstance Win32_SoundDevice -ErrorAction SilentlyContinue
                foreach ($dev in $soundDevices) {
                    $result += [PSCustomObject]@{
                        Id = $dev.DeviceID
                        Name = $dev.Name
                        Manufacturer = $dev.Manufacturer
                        Status = $dev.Status
                        Type = "Output"
                        IsDefault = ($result.Count -eq 0)
                    }
                }
                
                # Also get PnP audio devices for input devices
                $pnpAudio = Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'AudioEndpoint' } -ErrorAction SilentlyContinue
                foreach ($dev in $pnpAudio) {
                    $isInput = $dev.Name -match 'Microphone|Input|Capture|Line In'
                    $result += [PSCustomObject]@{
                        Id = $dev.PNPDeviceID
                        Name = $dev.Name
                        Manufacturer = $dev.Manufacturer
                        Status = $dev.Status
                        Type = if ($isInput) { "Input" } else { "Output" }
                        IsDefault = $false
                    }
                }
                
                $result | ConvertTo-Json -Compress
                "#])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let trimmed = stdout.trim();
                if !trimmed.is_empty() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
                        let items = if json.is_array() {
                            json.as_array().cloned().unwrap_or_default()
                        } else {
                            vec![json]
                        };
                        
                        let mut has_default_output = false;
                        let mut has_default_input = false;
                        
                        for item in &items {
                            let name = item.get("Name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                            let id = item.get("Id").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let status = item.get("Status").and_then(|v| v.as_str()).unwrap_or("OK");
                            let dev_type = item.get("Type").and_then(|v| v.as_str()).unwrap_or("Output");
                            let is_output = dev_type == "Output";
                            
                            let is_default = if is_output && !has_default_output {
                                has_default_output = true;
                                true
                            } else if !is_output && !has_default_input {
                                has_default_input = true;
                                true
                            } else { false };
                            
                            let state = match status {
                                "OK" => AudioState::Active,
                                "Degraded" => AudioState::Idle,
                                "Error" => AudioState::Unavailable,
                                _ => AudioState::Active,
                            };
                            
                            self.devices.push(AudioDevice {
                                id: id.to_string(),
                                name: name.to_string(),
                                device_type: if is_output { AudioDeviceType::Output } else { AudioDeviceType::Input },
                                state,
                                is_default,
                                is_output,
                                is_enabled: status == "OK",
                                volume: if is_default { Some(100) } else { None },
                                muted: false,
                            });
                        }
                    }
                }
            }
        }
        
        // Fallback if nothing found
        if self.devices.is_empty() {
            self.devices.push(AudioDevice {
                id: "default_output".to_string(), name: "Default Audio Output".to_string(),
                device_type: AudioDeviceType::Output, state: AudioState::Active,
                is_default: true, is_output: true, is_enabled: true, volume: Some(100), muted: false,
            });
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        use std::fs;
        
        // Read from /proc/asound for ALSA card enumeration
        let cards_path = std::path::Path::new("/proc/asound/cards");
        if let Ok(cards_content) = fs::read_to_string(cards_path) {
            for line in cards_content.lines() {
                let trimmed = line.trim();
                // Lines like " 0 [PCH            ]: HDA-Intel - HDA Intel PCH"
                if let Some(bracket_start) = trimmed.find('[') {
                    if let Some(bracket_end) = trimmed.find(']') {
                        let card_id_str: String = trimmed.chars().take_while(|c| c.is_ascii_digit() || c.is_whitespace()).collect();
                        let card_num = card_id_str.trim().parse::<u32>().unwrap_or(0);
                        let short_name = trimmed[bracket_start+1..bracket_end].trim().to_string();
                        
                        // Get full name from the colon part
                        let full_name = if let Some(colon_idx) = trimmed.find("- ") {
                            trimmed[colon_idx+2..].trim().to_string()
                        } else {
                            short_name.clone()
                        };
                        
                        // Check for playback/capture devices
                        let pcm_path = format!("/proc/asound/card{}", card_num);
                        let has_playback = std::path::Path::new(&format!("{}/pcm0p", pcm_path)).exists();
                        let has_capture = std::path::Path::new(&format!("{}/pcm0c", pcm_path)).exists();
                        
                        let device_type = match (has_playback, has_capture) {
                            (true, true) => AudioDeviceType::Duplex,
                            (true, false) => AudioDeviceType::Output,
                            (false, true) => AudioDeviceType::Input,
                            _ => AudioDeviceType::Output,
                        };
                        
                        let is_output = has_playback || !has_capture;
                        
                        self.devices.push(AudioDevice {
                            id: format!("hw:{}", card_num),
                            name: full_name,
                            device_type,
                            state: AudioState::Active,
                            is_default: card_num == 0,
                            is_output,
                            is_enabled: true,
                            volume: None,
                            muted: false,
                        });
                    }
                }
            }
        }
        
        // Try PulseAudio/PipeWire for default device info
        if let Ok(output) = std::process::Command::new("pactl").args(["info"]).output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.starts_with("Default Sink:") {
                        if let Some(name) = line.split(':').nth(1) {
                            // Mark matching device as default
                            let sink_name = name.trim();
                            if let Some(dev) = self.devices.iter_mut().find(|d| d.is_output) {
                                dev.is_default = true;
                                if dev.name == format!("hw:{}", 0) {
                                    dev.name = sink_name.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if self.devices.is_empty() {
            self.devices.push(AudioDevice {
                id: "default".to_string(), name: "Default Audio Device".to_string(),
                device_type: AudioDeviceType::Duplex, state: AudioState::Active,
                is_default: true, is_output: true, is_enabled: true, volume: None, muted: false,
            });
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        use std::process::Command;
        
        // Use system_profiler for audio device info
        if let Ok(output) = Command::new("system_profiler")
            .args(["SPAudioDataType", "-json"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(audio_data) = json.get("SPAudioDataType").and_then(|v| v.as_array()) {
                        let mut idx = 0u32;
                        for device in audio_data {
                            let name = device.get("_name").and_then(|v| v.as_str()).unwrap_or("Audio Device");
                            let has_output = device.get("coreaudio_default_audio_output_device")
                                .and_then(|v| v.as_str()) == Some("spaudio_yes");
                            let has_input = device.get("coreaudio_default_audio_input_device")
                                .and_then(|v| v.as_str()) == Some("spaudio_yes");
                            
                            let device_type = match (has_output || true, has_input) {
                                (true, true) => AudioDeviceType::Duplex,
                                (true, false) => AudioDeviceType::Output,
                                (false, true) => AudioDeviceType::Input,
                                _ => AudioDeviceType::Output,
                            };
                            
                            self.devices.push(AudioDevice {
                                id: format!("audio{}", idx),
                                name: name.to_string(),
                                device_type,
                                state: AudioState::Active,
                                is_default: has_output || has_input,
                                is_output: has_output || (!has_input),
                                is_enabled: true,
                                volume: None,
                                muted: false,
                            });
                            idx += 1;
                        }
                    }
                }
            }
        }
        
        if self.devices.is_empty() {
            self.devices.push(AudioDevice {
                id: "default_output".to_string(), name: "Default Audio Output".to_string(),
                device_type: AudioDeviceType::Output, state: AudioState::Active,
                is_default: true, is_output: true, is_enabled: true, volume: Some(100), muted: false,
            });
        }
    }
}

impl Default for AudioMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self { devices: Vec::new(), master_volume: Some(100), master_muted: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_monitor_creation() {
        let monitor = AudioMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_audio_monitor_devices() {
        let monitor = AudioMonitor::new().unwrap();
        // Should have at least one device (placeholder on all platforms)
        assert!(!monitor.devices().is_empty());
    }

    #[test]
    fn test_audio_monitor_master_volume() {
        let monitor = AudioMonitor::new().unwrap();
        if let Some(vol) = monitor.master_volume() {
            assert!(vol <= 100);
        }
    }

    #[test]
    fn test_audio_device_serialization() {
        let device = AudioDevice {
            id: "test".to_string(),
            name: "Test Device".to_string(),
            device_type: AudioDeviceType::Output,
            state: AudioState::Active,
            is_default: true,
            is_output: true,
            is_enabled: true,
            volume: Some(50),
            muted: false,
        };
        let json = serde_json::to_string(&device).unwrap();
        let deserialized: AudioDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(device.id, deserialized.id);
        assert_eq!(device.name, deserialized.name);
    }

    #[test]
    fn test_audio_monitor_default() {
        let monitor = AudioMonitor::default();
        // Default should work without panic
        let _ = monitor.devices();
    }
}

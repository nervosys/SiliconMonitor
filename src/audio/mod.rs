//! Audio device monitoring module
//!
//! Provides cross-platform audio device enumeration.
//! - Windows: `Win32_SoundDevice` via PowerShell/CIM
//! - Linux: ALSA via /proc/asound
//! - macOS: `system_profiler SPAudioDataType`
//!
//! Note that `system_profiler` reports which device is *default* for playback and
//! capture, not what each device is capable of, so macOS device types are inferred
//! rather than read directly.

use crate::error::SimonError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioDeviceType {
    Output,
    Input,
    Duplex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioState {
    Active,
    Idle,
    Suspended,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub device_type: AudioDeviceType,
    pub state: AudioState,
    pub is_default: bool,
    pub is_output: bool,
    pub is_enabled: bool,
    /// Device volume, if it has been read. Nothing reads it: the default
    /// device was given `Some(100)` and every other device `None`, so the one
    /// device a user looks at was the one carrying an invented figure.
    pub volume: Option<u8>,
    /// Device mute state, if it has been read. Nothing reads it either.
    pub muted: Option<bool>,
}

pub struct AudioMonitor {
    devices: Vec<AudioDevice>,
    /// System master volume, if it has been read.
    ///
    /// It never has been. This was initialised to `Some(100)` in the
    /// constructor and no `refresh_*` path on any platform assigns it, so
    /// `simon cli audio` reported "Master Volume: 100%" on every machine, the
    /// TUI printed the same, and the agent tool surface published
    /// `"master_volume": 100`. None of those was a reading.
    master_volume: Option<u8>,
    /// System mute state, if it has been read — see [`Self::master_volume`].
    /// It was a constant `false`, reported as "Muted: No".
    master_muted: Option<bool>,
}

impl AudioMonitor {
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            devices: Vec::new(),
            // Nothing reads these yet. Reading the system mixer needs
            // IAudioEndpointVolume on Windows, PulseAudio or ALSA on Linux and
            // CoreAudio on macOS; until one of those is wired up the honest
            // value is "not read", not "100%".
            master_volume: None,
            master_muted: None,
        };
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

    pub fn devices(&self) -> &[AudioDevice] {
        &self.devices
    }
    pub fn master_volume(&self) -> Option<u8> {
        self.master_volume
    }
    /// Whether the system is muted, or `None` because nothing reads it.
    pub fn is_muted(&self) -> Option<bool> {
        self.master_muted
    }
    pub fn default_output(&self) -> Option<&AudioDevice> {
        self.devices.iter().find(|d| d.is_default && d.is_output)
    }
    pub fn default_input(&self) -> Option<&AudioDevice> {
        self.devices.iter().find(|d| d.is_default && !d.is_output)
    }

    // ==================== Hardware Control APIs ====================

    /// Set the master volume level (0-100).
    ///
    /// # Not implemented
    ///
    /// This assigned the field and returned `Ok(())`, touching no audio API on
    /// any platform. A caller set the volume to 20, was told it had worked, and
    /// the machine did not change — while `master_volume()` then returned the
    /// 20 it had just stored, so even reading it back appeared to confirm it.
    ///
    /// It returns an error until a real mixer call is behind it. A control that
    /// reports success without acting is worse than one that is absent.
    pub fn set_master_volume(&mut self, volume: u8) -> Result<(), crate::error::SimonError> {
        if volume > 100 {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Volume must be 0-100, got {volume}"
            )));
        }
        Err(crate::error::SimonError::NotImplemented(
            concat!(
                "setting the master volume: simon has no mixer binding on this ",
                "platform, and this call previously changed only its own copy of ",
                "the value"
            )
            .to_string(),
        ))
    }

    /// Set the master mute state.
    ///
    /// # Not implemented
    ///
    /// See [`Self::set_master_volume`]. This changed only the field.
    pub fn set_mute(&mut self, _muted: bool) -> Result<(), crate::error::SimonError> {
        Err(crate::error::SimonError::NotImplemented(
            "setting the mute state: simon has no mixer binding on this platform".to_string(),
        ))
    }

    /// Set volume for a specific device by ID.
    pub fn set_device_volume(
        &mut self,
        device_id: &str,
        volume: u8,
    ) -> Result<(), crate::error::SimonError> {
        if volume > 100 {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Volume must be 0-100, got {}",
                volume
            )));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.id == device_id) {
            device.volume = Some(volume);
            Ok(())
        } else {
            Err(crate::error::SimonError::NotFound(format!(
                "Audio device '{}' not found",
                device_id
            )))
        }
    }

    /// Set mute state for a specific device by ID.
    ///
    /// # Not implemented
    ///
    /// See [`Self::set_master_volume`]. This changed only the field.
    pub fn set_device_mute(
        &mut self,
        device_id: &str,
        _muted: bool,
    ) -> Result<(), crate::error::SimonError> {
        if self.devices.iter().any(|d| d.id == device_id) {
            Err(crate::error::SimonError::NotImplemented(
                concat!(
                    "setting a device's mute state: simon has no mixer binding ",
                    "on this platform"
                )
                .to_string(),
            ))
        } else {
            Err(crate::error::SimonError::NotFound(format!(
                "Audio device '{}' not found",
                device_id
            )))
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
                            let name = item
                                .get("Name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown");
                            let id = item.get("Id").and_then(|v| v.as_str()).unwrap_or("unknown");
                            let status =
                                item.get("Status").and_then(|v| v.as_str()).unwrap_or("OK");
                            let dev_type = item
                                .get("Type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Output");
                            let is_output = dev_type == "Output";

                            let is_default = if is_output && !has_default_output {
                                has_default_output = true;
                                true
                            } else if !is_output && !has_default_input {
                                has_default_input = true;
                                true
                            } else {
                                false
                            };

                            let state = match status {
                                "OK" => AudioState::Active,
                                "Degraded" => AudioState::Idle,
                                "Error" => AudioState::Unavailable,
                                _ => AudioState::Active,
                            };

                            self.devices.push(AudioDevice {
                                id: id.to_string(),
                                name: name.to_string(),
                                device_type: if is_output {
                                    AudioDeviceType::Output
                                } else {
                                    AudioDeviceType::Input
                                },
                                state,
                                is_default,
                                is_output,
                                is_enabled: status == "OK",
                                // Not read. `Some(100)` for the default
                                // device was a guess wearing a measurement.
                                volume: None,
                                muted: None,
                            });
                        }
                    }
                }
            }
        }

        // There is deliberately no fallback device here.
        //
        // This invented one when the enumeration found nothing: "Default Audio
        // Output", active, enabled, unmuted, at 100% volume — none of it read
        // from anything. A machine with no enumerable audio reported one
        // working output, indistinguishable from a machine that has one. The
        // same shape as the Intel root hub the USB reader used to invent.
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
                        let card_id_str: String = trimmed
                            .chars()
                            .take_while(|c| c.is_ascii_digit() || c.is_whitespace())
                            .collect();
                        let card_num = card_id_str.trim().parse::<u32>().unwrap_or(0);
                        let short_name = trimmed[bracket_start + 1..bracket_end].trim().to_string();

                        // Get full name from the colon part
                        let full_name = if let Some(colon_idx) = trimmed.find("- ") {
                            trimmed[colon_idx + 2..].trim().to_string()
                        } else {
                            short_name.clone()
                        };

                        // Check for playback/capture devices
                        let pcm_path = format!("/proc/asound/card{}", card_num);
                        let has_playback =
                            std::path::Path::new(&format!("{}/pcm0p", pcm_path)).exists();
                        let has_capture =
                            std::path::Path::new(&format!("{}/pcm0c", pcm_path)).exists();

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
                            muted: None,
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

        // No fallback device. An empty list means the ALSA cards file told us
        // nothing, which is not the same as one working duplex device.
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
                    if let Some(audio_data) = json.get("SPAudioDataType").and_then(|v| v.as_array())
                    {
                        let mut idx = 0u32;
                        for device in audio_data {
                            let name = device
                                .get("_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Audio Device");
                            let has_output = device
                                .get("coreaudio_default_audio_output_device")
                                .and_then(|v| v.as_str())
                                == Some("spaudio_yes");
                            let has_input = device
                                .get("coreaudio_default_audio_input_device")
                                .and_then(|v| v.as_str())
                                == Some("spaudio_yes");

                            // These keys mark which device is *default* for each
                            // direction, not what a device supports, so capability is
                            // inferred.
                            //
                            // The previous expression was `(has_output || true, ...)`,
                            // whose first element is unconditionally true — so
                            // `has_output` was dead and no device was ever classified
                            // as Input, including the default microphone.
                            let device_type = if has_input && has_output {
                                AudioDeviceType::Duplex
                            } else if has_input {
                                AudioDeviceType::Input
                            } else {
                                // Capability is unknown; output is the right default
                                // for the overwhelming majority of audio devices.
                                AudioDeviceType::Output
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
                                muted: None,
                            });
                            idx += 1;
                        }
                    }
                }
            }
        }

        // No fallback device, and no invented 100% volume with it. See the
        // Windows path above.
    }
}

impl Default for AudioMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            devices: Vec::new(),
            master_volume: None,
            master_muted: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing in simon reads the system mixer, so nothing may report one.
    ///
    /// `master_volume` was initialised to `Some(100)` in the constructor and
    /// assigned by no `refresh_*` path on any platform, so `simon cli audio`
    /// printed "Master Volume: 100%" on every machine, the TUI printed the
    /// same, and the agent tool surface published `"master_volume": 100`. This
    /// fails the moment a real reader lands, which is when the wording on
    /// those three surfaces needs revisiting.
    #[test]
    fn an_unread_mixer_reports_nothing_rather_than_full_volume() {
        let monitor = AudioMonitor::new().expect("monitor");

        assert_eq!(
            monitor.master_volume(),
            None,
            "no platform reads the master volume; 100% was a constructor default"
        );
        assert_eq!(monitor.is_muted(), None, "no platform reads the mute state");

        for device in monitor.devices() {
            assert_eq!(
                device.volume, None,
                "{}: device volume is not read on any platform",
                device.name
            );
            assert_eq!(
                device.muted, None,
                "{}: mute state is not read",
                device.name
            );
        }
    }

    /// A control that reports success without acting is worse than one that is
    /// absent, because the caller has no way to find out.
    #[test]
    fn the_mixer_setters_decline_rather_than_pretend() {
        let mut monitor = AudioMonitor::new().expect("monitor");

        assert!(
            monitor.set_master_volume(75).is_err(),
            "setting the volume touches no audio API and must not return Ok"
        );
        assert!(monitor.set_mute(true).is_err());

        // Still unread: the failed call must not have left its argument behind
        // as though it were a reading.
        assert_eq!(monitor.master_volume(), None);
        assert_eq!(monitor.is_muted(), None);

        // Argument validation still comes first.
        assert!(monitor.set_master_volume(101).is_err());
    }

    #[test]
    fn test_audio_monitor_creation() {
        let monitor = AudioMonitor::new();
        assert!(monitor.is_ok());
    }

    /// Whatever is enumerated must be identifiable. How many there are is the
    /// machine's business.
    ///
    /// This asserted `!devices().is_empty()`, under a comment reading
    /// "Should have at least one device (placeholder on all platforms)" — it
    /// existed to assert that the invented fallback device was present, and it
    /// failed on the Linux and Windows CI runners the moment that device was
    /// removed, because a headless runner genuinely has no audio endpoint.
    ///
    /// A test that pins a placeholder makes the placeholder the contract. The
    /// honest assertion is about the shape of what is reported, not about a
    /// count the hardware decides.
    #[test]
    fn test_audio_monitor_devices() {
        let monitor = AudioMonitor::new().unwrap();
        for device in monitor.devices() {
            assert!(
                !device.id.is_empty(),
                "an audio device was reported with no id"
            );
            assert!(
                !device.name.is_empty(),
                "audio device {} was reported with no name",
                device.id
            );
        }
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
            muted: Some(false),
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

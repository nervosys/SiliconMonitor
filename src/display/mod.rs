//! Display/Monitor monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayConnection {
    Hdmi,
    DisplayPort,
    Dvi,
    Vga,
    Internal,
    Edp,
    UsbC,
    Usb,
    Wireless,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrMode {
    Off,
    Hdr10,
    Hdr10Plus,
    DolbyVision,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub connection: DisplayConnection,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f32,
    pub brightness: Option<f32>,
    pub hdr: HdrMode,
    pub scale_factor: Option<f64>,
    pub physical_width_mm: Option<u32>,
    pub physical_height_mm: Option<u32>,
    pub bits_per_pixel: Option<u8>,
}

impl DisplayInfo {
    pub fn aspect_ratio(&self) -> String {
        fn gcd(a: u32, b: u32) -> u32 {
            if b == 0 {
                a
            } else {
                gcd(b, a % b)
            }
        }
        let g = gcd(self.width, self.height);
        format!("{}:{}", self.width / g, self.height / g)
    }
}

pub struct DisplayMonitor {
    displays: Vec<DisplayInfo>,
}

impl DisplayMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        let mut monitor = Self {
            displays: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> {
        self.displays.clear();
        #[cfg(target_os = "windows")]
        self.refresh_windows();
        #[cfg(target_os = "linux")]
        self.refresh_linux();
        #[cfg(target_os = "macos")]
        self.refresh_macos();
        Ok(())
    }
    pub fn displays(&self) -> &[DisplayInfo] {
        &self.displays
    }
    pub fn primary(&self) -> Option<&DisplayInfo> {
        self.displays.iter().find(|d| d.is_primary)
    }
    pub fn count(&self) -> usize {
        self.displays.len()
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        use std::process::Command;

        // Use PowerShell + WMI to enumerate monitors and video controllers
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command",
                r#"
                $monitors = @()
                $idx = 0
                
                # Get video controllers for GPU-linked display info
                $controllers = Get-CimInstance Win32_VideoController | Select-Object Name, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate, CurrentBitsPerPixel, AdapterCompatibility, VideoModeDescription
                
                # Get monitor details
                $monitorDetails = Get-CimInstance WmiMonitorID -Namespace root\wmi -ErrorAction SilentlyContinue
                $brightness = Get-CimInstance WmiMonitorBrightness -Namespace root\wmi -ErrorAction SilentlyContinue
                $connInfo = Get-CimInstance WmiMonitorConnectionParams -Namespace root\wmi -ErrorAction SilentlyContinue
                
                foreach ($ctrl in $controllers) {
                    $mon = [PSCustomObject]@{
                        Id = "display$idx"
                        Name = $ctrl.Name
                        Manufacturer = $ctrl.AdapterCompatibility
                        Width = if ($ctrl.CurrentHorizontalResolution) { $ctrl.CurrentHorizontalResolution } else { 0 }
                        Height = if ($ctrl.CurrentVerticalResolution) { $ctrl.CurrentVerticalResolution } else { 0 }
                        RefreshRate = if ($ctrl.CurrentRefreshRate) { $ctrl.CurrentRefreshRate } else { 0 }
                        BitsPerPixel = if ($ctrl.CurrentBitsPerPixel) { $ctrl.CurrentBitsPerPixel } else { 32 }
                        IsPrimary = ($idx -eq 0)
                        Connection = "Unknown"
                        Brightness = -1
                    }
                    
                    # Try to get brightness for this display
                    if ($brightness -and $idx -lt $brightness.Count) {
                        $mon.Brightness = $brightness[$idx].CurrentBrightness
                    } elseif ($brightness -and $brightness.CurrentBrightness) {
                        $mon.Brightness = $brightness.CurrentBrightness
                    }
                    
                    # Try to get connection type
                    if ($connInfo -and $idx -lt @($connInfo).Count) {
                        $ctype = @($connInfo)[$idx].VideoOutputTechnology
                        $mon.Connection = switch ($ctype) {
                            0  { "VGA" }
                            4  { "DVI" }
                            5  { "HDMI" }
                            6  { "LVDS" }
                            9  { "DisplayPort" }
                            10 { "DisplayPort" }
                            11 { "DisplayPort" }
                            14 { "eDP" }
                            default { "Unknown" }
                        }
                    }
                    
                    # Try to get monitor name/manufacturer from WMI
                    if ($monitorDetails -and $idx -lt @($monitorDetails).Count) {
                        $mdet = @($monitorDetails)[$idx]
                        $uname = ($mdet.UserFriendlyName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
                        $umfr  = ($mdet.ManufacturerName | Where-Object { $_ -ne 0 } | ForEach-Object { [char]$_ }) -join ''
                        if ($uname) { $mon.Name = $uname }
                        if ($umfr)  { $mon.Manufacturer = $umfr }
                    }
                    
                    $monitors += $mon
                    $idx++
                }
                
                $monitors | ConvertTo-Json -Compress
                "#])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
                    let items = if json.is_array() {
                        json.as_array().cloned().unwrap_or_default()
                    } else {
                        vec![json]
                    };

                    for item in &items {
                        let connection = match item
                            .get("Connection")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                        {
                            "HDMI" => DisplayConnection::Hdmi,
                            "DisplayPort" => DisplayConnection::DisplayPort,
                            "DVI" => DisplayConnection::Dvi,
                            "VGA" => DisplayConnection::Vga,
                            "eDP" | "LVDS" => DisplayConnection::Internal,
                            _ => DisplayConnection::Unknown,
                        };

                        let brightness = item
                            .get("Brightness")
                            .and_then(|v| v.as_f64())
                            .filter(|&b| b >= 0.0)
                            .map(|b| (b / 100.0) as f32);

                        self.displays.push(DisplayInfo {
                            id: item
                                .get("Id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("display0")
                                .to_string(),
                            name: item
                                .get("Name")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            manufacturer: item
                                .get("Manufacturer")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            connection,
                            is_primary: item
                                .get("IsPrimary")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                            width: item.get("Width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            height: item.get("Height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            refresh_rate: item
                                .get("RefreshRate")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0) as f32,
                            brightness,
                            hdr: HdrMode::Off,
                            scale_factor: None,
                            physical_width_mm: None,
                            physical_height_mm: None,
                            bits_per_pixel: item
                                .get("BitsPerPixel")
                                .and_then(|v| v.as_u64())
                                .map(|b| b as u8),
                        });
                    }
                }
            }
        }

        // Fallback if WMI returned nothing
        if self.displays.is_empty() {
            self.displays.push(DisplayInfo {
                id: "display0".to_string(),
                name: Some("Primary Display".to_string()),
                manufacturer: None,
                connection: DisplayConnection::Unknown,
                is_primary: true,
                width: 0,
                height: 0,
                refresh_rate: 0.0,
                brightness: None,
                hdr: HdrMode::Off,
                scale_factor: None,
                physical_width_mm: None,
                physical_height_mm: None,
                bits_per_pixel: None,
            });
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        use std::fs;
        use std::process::Command;

        // Try DRM/sysfs first (works without X11)
        let drm_path = std::path::Path::new("/sys/class/drm");
        if drm_path.exists() {
            if let Ok(entries) = fs::read_dir(drm_path) {
                let mut idx = 0u32;
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Look for card*-* entries (e.g. card0-HDMI-A-1, card0-eDP-1)
                    if !name.starts_with("card") || !name.contains('-') {
                        continue;
                    }
                    let path = entry.path();

                    // Check if connected
                    let status = fs::read_to_string(path.join("status")).unwrap_or_default();
                    if status.trim() != "connected" {
                        continue;
                    }

                    // Parse connection type from name
                    let connector = name.split('-').skip(1).collect::<Vec<_>>().join("-");
                    let connection = if connector.starts_with("HDMI") {
                        DisplayConnection::Hdmi
                    } else if connector.starts_with("DP") || connector.starts_with("DisplayPort") {
                        DisplayConnection::DisplayPort
                    } else if connector.starts_with("DVI") {
                        DisplayConnection::Dvi
                    } else if connector.starts_with("VGA") {
                        DisplayConnection::Vga
                    } else if connector.starts_with("eDP") {
                        DisplayConnection::Edp
                    } else {
                        DisplayConnection::Unknown
                    };

                    // Read EDID for monitor name/manufacturer
                    let (mon_name, manufacturer, phys_w, phys_h) =
                        if let Ok(edid) = fs::read(path.join("edid")) {
                            parse_edid_basic(&edid)
                        } else {
                            (None, None, None, None)
                        };

                    // Try to get current mode from "modes" file
                    let (width, height) = if let Ok(modes) = fs::read_to_string(path.join("modes"))
                    {
                        if let Some(first_mode) = modes.lines().next() {
                            let parts: Vec<&str> = first_mode.split('x').collect();
                            if parts.len() == 2 {
                                (
                                    parts[0].trim().parse::<u32>().unwrap_or(0),
                                    parts[1].trim().parse::<u32>().unwrap_or(0),
                                )
                            } else {
                                (0, 0)
                            }
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    };

                    self.displays.push(DisplayInfo {
                        id: format!("display{}", idx),
                        name: mon_name.or(Some(connector.clone())),
                        manufacturer,
                        connection,
                        is_primary: idx == 0,
                        width,
                        height,
                        refresh_rate: 0.0, // DRM modes file doesn't always include rate
                        brightness: read_backlight_brightness(),
                        hdr: HdrMode::Off,
                        scale_factor: None,
                        physical_width_mm: phys_w,
                        physical_height_mm: phys_h,
                        bits_per_pixel: None,
                    });
                    idx += 1;
                }
            }
        }

        // Fallback: try xrandr
        if self.displays.is_empty() {
            if let Ok(output) = Command::new("xrandr").args(["--current"]).output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let mut idx = 0u32;
                    for line in stdout.lines() {
                        if line.contains(" connected") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            let conn_name = parts.first().unwrap_or(&"Unknown");
                            let is_primary = line.contains("primary");

                            let connection = if conn_name.starts_with("HDMI") {
                                DisplayConnection::Hdmi
                            } else if conn_name.starts_with("DP") {
                                DisplayConnection::DisplayPort
                            } else if conn_name.starts_with("DVI") {
                                DisplayConnection::Dvi
                            } else if conn_name.starts_with("VGA") {
                                DisplayConnection::Vga
                            } else if conn_name.starts_with("eDP") {
                                DisplayConnection::Edp
                            } else {
                                DisplayConnection::Unknown
                            };

                            // Parse resolution from something like "1920x1080+0+0"
                            let (w, h) = parts
                                .iter()
                                .find(|p| p.contains('x') && p.contains('+'))
                                .and_then(|mode| {
                                    let res_part = mode.split('+').next()?;
                                    let dims: Vec<&str> = res_part.split('x').collect();
                                    if dims.len() == 2 {
                                        Some((
                                            dims[0].parse::<u32>().unwrap_or(0),
                                            dims[1].parse::<u32>().unwrap_or(0),
                                        ))
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or((0, 0));

                            // Parse physical dimensions from "520mm x 290mm"
                            let (phys_w, phys_h) = if let Some(mm_idx) = line.find("mm x ") {
                                let before = &line[..mm_idx];
                                let pw = before
                                    .rsplit_once(' ')
                                    .and_then(|(_, n)| n.parse::<u32>().ok());
                                let after = &line[mm_idx + 5..];
                                let ph = after
                                    .split("mm")
                                    .next()
                                    .and_then(|n| n.trim().parse::<u32>().ok());
                                (pw, ph)
                            } else {
                                (None, None)
                            };

                            self.displays.push(DisplayInfo {
                                id: format!("display{}", idx),
                                name: Some(conn_name.to_string()),
                                manufacturer: None,
                                connection,
                                is_primary,
                                width: w,
                                height: h,
                                refresh_rate: 0.0,
                                brightness: read_backlight_brightness(),
                                hdr: HdrMode::Off,
                                scale_factor: None,
                                physical_width_mm: phys_w,
                                physical_height_mm: phys_h,
                                bits_per_pixel: None,
                            });
                            idx += 1;
                        }
                    }
                }
            }
        }

        if self.displays.is_empty() {
            self.displays.push(DisplayInfo {
                id: "display0".to_string(),
                name: Some("Unknown Display".to_string()),
                manufacturer: None,
                connection: DisplayConnection::Unknown,
                is_primary: true,
                width: 0,
                height: 0,
                refresh_rate: 0.0,
                brightness: None,
                hdr: HdrMode::Off,
                scale_factor: None,
                physical_width_mm: None,
                physical_height_mm: None,
                bits_per_pixel: None,
            });
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        use std::process::Command;

        // Use system_profiler for display info
        if let Ok(output) = Command::new("system_profiler")
            .args(["SPDisplaysDataType", "-json"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(displays_data) =
                        json.get("SPDisplaysDataType").and_then(|v| v.as_array())
                    {
                        let mut idx = 0u32;
                        for gpu in displays_data {
                            if let Some(ndrvs) =
                                gpu.get("spdisplays_ndrvs").and_then(|v| v.as_array())
                            {
                                for display in ndrvs {
                                    let name = display
                                        .get("_name")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    let res = display
                                        .get("_spdisplays_resolution")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let (w, h) = if let Some(x_idx) = res.find(" x ") {
                                        let w = res[..x_idx]
                                            .trim()
                                            .replace(" ", "")
                                            .parse::<u32>()
                                            .unwrap_or(0);
                                        let rest = &res[x_idx + 3..];
                                        let h_str: String = rest
                                            .chars()
                                            .take_while(|c| c.is_ascii_digit())
                                            .collect();
                                        (w, h_str.parse::<u32>().unwrap_or(0))
                                    } else {
                                        (0, 0)
                                    };

                                    let connection = if display
                                        .get("spdisplays_connection_type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .contains("Internal")
                                    {
                                        DisplayConnection::Internal
                                    } else {
                                        DisplayConnection::Unknown
                                    };

                                    self.displays.push(DisplayInfo {
                                        id: format!("display{}", idx),
                                        name,
                                        manufacturer: display
                                            .get("_spdisplays_display-vendor-id")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string()),
                                        connection,
                                        is_primary: display
                                            .get("spdisplays_main")
                                            .and_then(|v| v.as_str())
                                            == Some("spdisplays_yes"),
                                        width: w,
                                        height: h,
                                        refresh_rate: 0.0,
                                        brightness: None,
                                        hdr: HdrMode::Off,
                                        scale_factor: None,
                                        physical_width_mm: None,
                                        physical_height_mm: None,
                                        bits_per_pixel: display
                                            .get("spdisplays_pixelresolution")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| {
                                                if s.contains("32") {
                                                    Some(32u8)
                                                } else if s.contains("30") {
                                                    Some(30)
                                                } else {
                                                    None
                                                }
                                            }),
                                    });
                                    idx += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.displays.is_empty() {
            self.displays.push(DisplayInfo {
                id: "display0".to_string(),
                name: Some("Display".to_string()),
                manufacturer: None,
                connection: DisplayConnection::Internal,
                is_primary: true,
                width: 0,
                height: 0,
                refresh_rate: 0.0,
                brightness: None,
                hdr: HdrMode::Off,
                scale_factor: None,
                physical_width_mm: None,
                physical_height_mm: None,
                bits_per_pixel: None,
            });
        }
    }
}

/// Parse basic EDID data for monitor name and manufacturer
#[cfg(target_os = "linux")]
fn parse_edid_basic(edid: &[u8]) -> (Option<String>, Option<String>, Option<u32>, Option<u32>) {
    if edid.len() < 128 {
        return (None, None, None, None);
    }

    // Manufacturer ID from bytes 8-9 (3 letters encoded)
    let mfr = if edid.len() > 9 {
        let mfr_code = ((edid[8] as u16) << 8) | edid[9] as u16;
        let c1 = ((mfr_code >> 10) & 0x1F) as u8 + b'A' - 1;
        let c2 = ((mfr_code >> 5) & 0x1F) as u8 + b'A' - 1;
        let c3 = (mfr_code & 0x1F) as u8 + b'A' - 1;
        if c1.is_ascii_alphabetic() && c2.is_ascii_alphabetic() && c3.is_ascii_alphabetic() {
            Some(format!("{}{}{}", c1 as char, c2 as char, c3 as char))
        } else {
            None
        }
    } else {
        None
    };

    // Physical dimensions from bytes 21-22 (cm)
    let phys_w = if edid[21] > 0 {
        Some(edid[21] as u32 * 10)
    } else {
        None
    };
    let phys_h = if edid[22] > 0 {
        Some(edid[22] as u32 * 10)
    } else {
        None
    };

    // Monitor name from descriptor blocks (bytes 54-125, 18 bytes each)
    let mut name = None;
    for block_start in (54..=108).step_by(18) {
        if block_start + 17 < edid.len()
            && edid[block_start] == 0
            && edid[block_start + 1] == 0
            && edid[block_start + 3] == 0xFC
        {
            // Monitor name descriptor
            let name_bytes: Vec<u8> = edid[block_start + 5..block_start + 18]
                .iter()
                .copied()
                .take_while(|&b| b != 0x0A && b != 0x00)
                .collect();
            if let Ok(n) = String::from_utf8(name_bytes) {
                let trimmed = n.trim().to_string();
                if !trimmed.is_empty() {
                    name = Some(trimmed);
                }
            }
            break;
        }
    }

    (name, mfr, phys_w, phys_h)
}

/// Read backlight brightness from sysfs (0.0 - 1.0)
#[cfg(target_os = "linux")]
fn read_backlight_brightness() -> Option<f32> {
    use std::fs;
    let bl_path = std::path::Path::new("/sys/class/backlight");
    if let Ok(entries) = fs::read_dir(bl_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let brightness = fs::read_to_string(path.join("brightness"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())?;
            let max = fs::read_to_string(path.join("max_brightness"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())?;
            if max > 0.0 {
                return Some(brightness / max);
            }
        }
    }
    None
}

impl Default for DisplayMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            displays: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_monitor_creation() {
        let monitor = DisplayMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_display_monitor_count() {
        let monitor = DisplayMonitor::new().unwrap();
        assert!(monitor.count() >= 1); // Placeholder always adds one
    }

    #[test]
    fn test_display_aspect_ratio() {
        let display = DisplayInfo {
            id: "test".to_string(),
            name: Some("Test".to_string()),
            manufacturer: None,
            connection: DisplayConnection::Hdmi,
            is_primary: true,
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            brightness: None,
            hdr: HdrMode::Off,
            scale_factor: Some(1.0),
            physical_width_mm: None,
            physical_height_mm: None,
            bits_per_pixel: Some(32),
        };
        assert_eq!(display.aspect_ratio(), "16:9");
    }

    #[test]
    fn test_display_4k_aspect_ratio() {
        let display = DisplayInfo {
            id: "test".to_string(),
            name: None,
            manufacturer: None,
            connection: DisplayConnection::DisplayPort,
            is_primary: false,
            width: 3840,
            height: 2160,
            refresh_rate: 144.0,
            brightness: Some(0.8),
            hdr: HdrMode::Hdr10,
            scale_factor: Some(1.5),
            physical_width_mm: Some(600),
            physical_height_mm: Some(340),
            bits_per_pixel: Some(30),
        };
        assert_eq!(display.aspect_ratio(), "16:9");
    }

    #[test]
    fn test_display_info_serialization() {
        let display = DisplayInfo {
            id: "test".to_string(),
            name: Some("Test Display".to_string()),
            manufacturer: Some("Acme".to_string()),
            connection: DisplayConnection::Hdmi,
            is_primary: true,
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            brightness: Some(0.5),
            hdr: HdrMode::Off,
            scale_factor: Some(1.0),
            physical_width_mm: Some(530),
            physical_height_mm: Some(300),
            bits_per_pixel: Some(32),
        };
        let json = serde_json::to_string(&display).unwrap();
        let deserialized: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(display.id, deserialized.id);
        assert_eq!(display.width, deserialized.width);
    }
}

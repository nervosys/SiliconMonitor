//! Display/Monitor monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayConnection { Hdmi, DisplayPort, Dvi, Vga, Internal, Edp, UsbC, Usb, Wireless, Virtual, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrMode { Off, Hdr10, Hdr10Plus, DolbyVision, Unknown }

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
        fn gcd(a: u32, b: u32) -> u32 { if b == 0 { a } else { gcd(b, a % b) } }
        let g = gcd(self.width, self.height);
        format!("{}:{}", self.width / g, self.height / g)
    }
}

pub struct DisplayMonitor { displays: Vec<DisplayInfo> }

impl DisplayMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        let mut monitor = Self { displays: Vec::new() };
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
    pub fn displays(&self) -> &[DisplayInfo] { &self.displays }
    pub fn primary(&self) -> Option<&DisplayInfo> { self.displays.iter().find(|d| d.is_primary) }
    pub fn count(&self) -> usize { self.displays.len() }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        self.displays.push(DisplayInfo {
            id: "display0".to_string(),
            name: Some("Primary Display".to_string()),
            manufacturer: None,
            connection: DisplayConnection::Unknown,
            is_primary: true,
            width: 1920, height: 1080, refresh_rate: 60.0,
            brightness: None, hdr: HdrMode::Off,
            scale_factor: Some(1.0), physical_width_mm: None, physical_height_mm: None, bits_per_pixel: Some(32),
        });
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        self.displays.push(DisplayInfo {
            id: "display0".to_string(),
            name: Some("Primary Display".to_string()),
            manufacturer: None,
            connection: DisplayConnection::Unknown,
            is_primary: true,
            width: 1920, height: 1080, refresh_rate: 60.0,
            brightness: None, hdr: HdrMode::Off,
            scale_factor: Some(1.0), physical_width_mm: None, physical_height_mm: None, bits_per_pixel: Some(32),
        });
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.displays.push(DisplayInfo {
            id: "display0".to_string(),
            name: Some("Primary Display".to_string()),
            manufacturer: None,
            connection: DisplayConnection::Unknown,
            is_primary: true,
            width: 1920, height: 1080, refresh_rate: 60.0,
            brightness: None, hdr: HdrMode::Off,
            scale_factor: Some(1.0), physical_width_mm: None, physical_height_mm: None, bits_per_pixel: Some(32),
        });
    }
}

impl Default for DisplayMonitor {
    fn default() -> Self { Self::new().unwrap_or(Self { displays: Vec::new() }) }
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

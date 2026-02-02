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

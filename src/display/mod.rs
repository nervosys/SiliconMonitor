//! Display/Monitor monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayConnection { Hdmi, DisplayPort, UsbC, Dvi, Vga, Internal, Virtual, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrMode { Off, Hdr10, Hdr10Plus, DolbyVision, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub connection: DisplayConnection,
    pub is_primary: bool,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: f32,
    pub brightness: Option<f32>,
    pub hdr: HdrMode,
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
        Ok(Self { displays: Vec::new() })
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> { Ok(()) }
    pub fn displays(&self) -> &[DisplayInfo] { &self.displays }
    pub fn primary(&self) -> Option<&DisplayInfo> { self.displays.iter().find(|d| d.is_primary) }
    pub fn count(&self) -> usize { self.displays.len() }
}

impl Default for DisplayMonitor {
    fn default() -> Self { Self { displays: Vec::new() } }
}
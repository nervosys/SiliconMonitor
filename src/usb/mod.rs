//! USB device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbSpeed { Low, Full, High, Super, SuperPlus, Usb4, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbClass { Audio, Hid, MassStorage, Hub, Video, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub bus: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub vendor_name: Option<String>,
    pub product_name: Option<String>,
    pub class: UsbClass,
    pub speed: UsbSpeed,
}

pub struct UsbMonitor { devices: Vec<UsbDevice> }

impl UsbMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        Ok(Self { devices: Vec::new() })
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> { Ok(()) }
    pub fn devices(&self) -> &[UsbDevice] { &self.devices }
}

impl Default for UsbMonitor {
    fn default() -> Self { Self { devices: Vec::new() } }
}
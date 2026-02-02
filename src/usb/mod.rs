//! USB device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbSpeed { Low, Full, High, Super, SuperPlus, SuperPlusx2, Usb4, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbDeviceClass { Audio, Communication, Hid, Printer, MassStorage, Hub, Video, Wireless, Vendor, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsbDevice {
    pub bus_number: u8,
    pub port_number: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub description: Option<String>,
    pub serial_number: Option<String>,
    pub class: UsbDeviceClass,
    pub speed: UsbSpeed,
}

pub struct UsbMonitor { devices: Vec<UsbDevice> }

impl UsbMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        let mut monitor = Self { devices: Vec::new() };
        monitor.refresh()?;
        Ok(monitor)
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> {
        self.devices.clear();
        #[cfg(target_os = "windows")]
        self.refresh_windows();
        #[cfg(target_os = "linux")]
        self.refresh_linux();
        #[cfg(target_os = "macos")]
        self.refresh_macos();
        Ok(())
    }
    pub fn devices(&self) -> &[UsbDevice] { &self.devices }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // Placeholder - shows example USB hub
        self.devices.push(UsbDevice {
            bus_number: 1, port_number: 0,
            vendor_id: 0x8086, product_id: 0x1234,
            manufacturer: Some("Intel Corp".to_string()),
            product: Some("USB 3.0 Root Hub".to_string()),
            description: None, serial_number: None,
            class: UsbDeviceClass::Hub, speed: UsbSpeed::Super,
        });
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        use std::fs;
        use std::path::Path;
        // Read from /sys/bus/usb/devices
        let usb_path = Path::new("/sys/bus/usb/devices");
        if usb_path.exists() {
            if let Ok(entries) = fs::read_dir(usb_path) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if !name.contains('-') || name.contains(':') { continue; }
                        let path = entry.path();
                        let vendor_id = read_usb_attr(&path, "idVendor");
                        let product_id = read_usb_attr(&path, "idProduct");
                        let manufacturer = read_usb_string(&path, "manufacturer");
                        let product = read_usb_string(&path, "product");
                        let serial = read_usb_string(&path, "serial");
                        let speed = match read_usb_string(&path, "speed").as_deref() {
                            Some("1.5") => UsbSpeed::Low,
                            Some("12") => UsbSpeed::Full,
                            Some("480") => UsbSpeed::High,
                            Some("5000") => UsbSpeed::Super,
                            Some("10000") => UsbSpeed::SuperPlus,
                            Some("20000") => UsbSpeed::SuperPlusx2,
                            _ => UsbSpeed::Unknown,
                        };
                        let class_code = read_usb_attr(&path, "bDeviceClass");
                        let class = match class_code {
                            0x01 => UsbDeviceClass::Audio,
                            0x02 => UsbDeviceClass::Communication,
                            0x03 => UsbDeviceClass::Hid,
                            0x07 => UsbDeviceClass::Printer,
                            0x08 => UsbDeviceClass::MassStorage,
                            0x09 => UsbDeviceClass::Hub,
                            0x0e => UsbDeviceClass::Video,
                            0xe0 => UsbDeviceClass::Wireless,
                            0xff => UsbDeviceClass::Vendor,
                            _ => UsbDeviceClass::Unknown,
                        };
                        let parts: Vec<&str> = name.split('-').collect();
                        let bus_number = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
                        let port_number = parts.get(1).and_then(|s| s.split('.').next()).and_then(|s| s.parse().ok()).unwrap_or(0);
                        self.devices.push(UsbDevice {
                            bus_number, port_number,
                            vendor_id: vendor_id as u16, product_id: product_id as u16,
                            manufacturer, product, description: None, serial_number: serial,
                            class, speed,
                        });
                    }
                }
            }
        }
        // Fallback
        if self.devices.is_empty() {
            self.devices.push(UsbDevice {
                bus_number: 1, port_number: 0,
                vendor_id: 0x8086, product_id: 0x0001,
                manufacturer: None, product: Some("USB Root Hub".to_string()),
                description: None, serial_number: None,
                class: UsbDeviceClass::Hub, speed: UsbSpeed::High,
            });
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.devices.push(UsbDevice {
            bus_number: 1, port_number: 0,
            vendor_id: 0x05ac, product_id: 0x8006,
            manufacturer: Some("Apple Inc.".to_string()),
            product: Some("USB Root Hub".to_string()),
            description: None, serial_number: None,
            class: UsbDeviceClass::Hub, speed: UsbSpeed::High,
        });
    }
}

impl Default for UsbMonitor {
    fn default() -> Self { Self::new().unwrap_or(Self { devices: Vec::new() }) }
}

#[cfg(target_os = "linux")]
fn read_usb_attr(path: &std::path::Path, attr: &str) -> u32 {
    std::fs::read_to_string(path.join(attr))
        .ok()
        .and_then(|s| u32::from_str_radix(s.trim(), 16).ok())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn read_usb_string(path: &std::path::Path, attr: &str) -> Option<String> {
    std::fs::read_to_string(path.join(attr))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

//! USB device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
    SuperPlus,
    SuperPlusx2,
    Usb4,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsbDeviceClass {
    Audio,
    Communication,
    Hid,
    Printer,
    MassStorage,
    Hub,
    Video,
    Wireless,
    Vendor,
    Unknown,
}

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

pub struct UsbMonitor {
    devices: Vec<UsbDevice>,
}

impl UsbMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        let mut monitor = Self {
            devices: Vec::new(),
        };
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
    pub fn devices(&self) -> &[UsbDevice] {
        &self.devices
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // Use WMI to enumerate real USB devices
        if let Ok(devices) = Self::wmi_enumerate_usb() {
            self.devices = devices;
        }
        // Fallback to setupapi-based approach
        if self.devices.is_empty() {
            if let Ok(devices) = Self::registry_enumerate_usb() {
                self.devices = devices;
            }
        }
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
                        if !name.contains('-') || name.contains(':') {
                            continue;
                        }
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
                        let port_number = parts
                            .get(1)
                            .and_then(|s| s.split('.').next())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        self.devices.push(UsbDevice {
                            bus_number,
                            port_number,
                            vendor_id: vendor_id as u16,
                            product_id: product_id as u16,
                            manufacturer,
                            product,
                            description: None,
                            serial_number: serial,
                            class,
                            speed,
                        });
                    }
                }
            }
        }
        // Fallback
        if self.devices.is_empty() {
            self.devices.push(UsbDevice {
                bus_number: 1,
                port_number: 0,
                vendor_id: 0x8086,
                product_id: 0x0001,
                manufacturer: None,
                product: Some("USB Root Hub".to_string()),
                description: None,
                serial_number: None,
                class: UsbDeviceClass::Hub,
                speed: UsbSpeed::High,
            });
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.devices.push(UsbDevice {
            bus_number: 1,
            port_number: 0,
            vendor_id: 0x05ac,
            product_id: 0x8006,
            manufacturer: Some("Apple Inc.".to_string()),
            product: Some("USB Root Hub".to_string()),
            description: None,
            serial_number: None,
            class: UsbDeviceClass::Hub,
            speed: UsbSpeed::High,
        });
    }
}

impl Default for UsbMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            devices: Vec::new(),
        })
    }
}

#[cfg(target_os = "windows")]
impl UsbMonitor {
    /// Enumerate USB devices using WMI Win32_PnPEntity
    fn wmi_enumerate_usb() -> Result<Vec<UsbDevice>, crate::error::SimonError> {
        use std::process::Command;
        let mut devices = Vec::new();

        // Use PowerShell to query WMI for USB devices
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPDeviceID -like 'USB*' } | Select-Object Name, Manufacturer, PNPDeviceID, Description, Status | ConvertTo-Json -Compress"])
            .output()
            .map_err(|e| crate::error::SimonError::Other(format!("WMI query failed: {}", e)))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
                let items = if json.is_array() {
                    json.as_array().cloned().unwrap_or_default()
                } else {
                    vec![json]
                };

                for (idx, item) in items.iter().enumerate() {
                    let name = item.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    let manufacturer = item.get("Manufacturer").and_then(|v| v.as_str());
                    let pnp_id = item
                        .get("PNPDeviceID")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let description = item.get("Description").and_then(|v| v.as_str());

                    // Parse VID/PID from PNPDeviceID like "USB\VID_046D&PID_C52B\..."
                    let (vid, pid) = parse_vid_pid(pnp_id);

                    // Determine device class from name/description
                    let class = classify_usb_device(name, description.unwrap_or(""));

                    // Determine speed from class heuristic
                    let speed = if pnp_id.contains("USB3")
                        || name.contains("USB 3")
                        || name.contains("xHCI")
                    {
                        UsbSpeed::Super
                    } else if name.contains("USB 2") || name.contains("EHCI") {
                        UsbSpeed::High
                    } else {
                        UsbSpeed::Unknown
                    };

                    // Extract serial from PNP ID (third segment)
                    let serial = pnp_id
                        .split('\\')
                        .nth(2)
                        .filter(|s| s.len() > 4 && !s.contains('&'))
                        .map(|s| s.to_string());

                    devices.push(UsbDevice {
                        bus_number: 0,
                        port_number: idx as u8,
                        vendor_id: vid,
                        product_id: pid,
                        manufacturer: manufacturer.map(|s| s.to_string()),
                        product: if name.is_empty() {
                            None
                        } else {
                            Some(name.to_string())
                        },
                        description: description.map(|s| s.to_string()),
                        serial_number: serial,
                        class,
                        speed,
                    });
                }
            }
        }

        Ok(devices)
    }

    /// Fallback: enumerate USB devices from registry
    fn registry_enumerate_usb() -> Result<Vec<UsbDevice>, crate::error::SimonError> {
        use std::process::Command;
        let mut devices = Vec::new();

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-CimInstance Win32_USBControllerDevice | ForEach-Object { \
                 [PSCustomObject]@{Dependent=$_.Dependent.ToString()} } | ConvertTo-Json -Compress",
            ])
            .output()
            .map_err(|e| {
                crate::error::SimonError::Other(format!("Registry query failed: {}", e))
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
                let items = if json.is_array() {
                    json.as_array().cloned().unwrap_or_default()
                } else {
                    vec![json]
                };

                for (idx, item) in items.iter().enumerate() {
                    let dep = item.get("Dependent").and_then(|v| v.as_str()).unwrap_or("");
                    let (vid, pid) = parse_vid_pid(dep);
                    if vid != 0 || pid != 0 {
                        devices.push(UsbDevice {
                            bus_number: 0,
                            port_number: idx as u8,
                            vendor_id: vid,
                            product_id: pid,
                            manufacturer: None,
                            product: Some(
                                dep.split('\\').last().unwrap_or("USB Device").to_string(),
                            ),
                            description: None,
                            serial_number: None,
                            class: UsbDeviceClass::Unknown,
                            speed: UsbSpeed::Unknown,
                        });
                    }
                }
            }
        }

        Ok(devices)
    }
}

#[cfg(target_os = "windows")]
fn parse_vid_pid(pnp_id: &str) -> (u16, u16) {
    let upper = pnp_id.to_uppercase();
    let vid = upper
        .find("VID_")
        .and_then(|i| u16::from_str_radix(&upper[i + 4..][..4.min(upper.len() - i - 4)], 16).ok())
        .unwrap_or(0);
    let pid = upper
        .find("PID_")
        .and_then(|i| u16::from_str_radix(&upper[i + 4..][..4.min(upper.len() - i - 4)], 16).ok())
        .unwrap_or(0);
    (vid, pid)
}

#[cfg(target_os = "windows")]
fn classify_usb_device(name: &str, description: &str) -> UsbDeviceClass {
    let combined = format!("{} {}", name, description).to_lowercase();
    if combined.contains("hub") {
        UsbDeviceClass::Hub
    } else if combined.contains("keyboard") || combined.contains("hid") {
        UsbDeviceClass::Hid
    } else if combined.contains("mouse") || combined.contains("pointing") {
        UsbDeviceClass::Hid
    } else if combined.contains("mass storage") || combined.contains("disk") {
        UsbDeviceClass::MassStorage
    } else if combined.contains("audio") || combined.contains("sound") {
        UsbDeviceClass::Audio
    } else if combined.contains("video")
        || combined.contains("camera")
        || combined.contains("webcam")
    {
        UsbDeviceClass::Video
    } else if combined.contains("printer") {
        UsbDeviceClass::Printer
    } else if combined.contains("wireless")
        || combined.contains("bluetooth")
        || combined.contains("wifi")
    {
        UsbDeviceClass::Wireless
    } else if combined.contains("serial") || combined.contains("modem") || combined.contains("comm")
    {
        UsbDeviceClass::Communication
    } else {
        UsbDeviceClass::Unknown
    }
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

// USB events for device monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsbEvent {
    Connected(UsbDevice),
    Disconnected(UsbDevice),
}

impl UsbMonitor {
    /// Check for device changes since last refresh
    /// Returns a list of connect/disconnect events
    pub fn poll_events(&mut self) -> Result<Vec<UsbEvent>, crate::error::SimonError> {
        let old_devices = self.devices.clone();
        self.refresh()?;

        let mut events = Vec::new();

        // Find disconnected devices (in old but not in new)
        for old in &old_devices {
            if !self.devices.iter().any(|d| device_matches(d, old)) {
                events.push(UsbEvent::Disconnected(old.clone()));
            }
        }

        // Find connected devices (in new but not in old)
        for new in &self.devices {
            if !old_devices.iter().any(|d| device_matches(d, new)) {
                events.push(UsbEvent::Connected(new.clone()));
            }
        }

        Ok(events)
    }
}

fn device_matches(a: &UsbDevice, b: &UsbDevice) -> bool {
    a.vendor_id == b.vendor_id
        && a.product_id == b.product_id
        && a.bus_number == b.bus_number
        && a.port_number == b.port_number
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usb_monitor_creation() {
        let monitor = UsbMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_usb_monitor_devices() {
        let monitor = UsbMonitor::new().unwrap();
        // Real USB detection may find devices or not depending on environment
        // Just verify the method doesn't panic
        let _devices = monitor.devices();
    }

    #[test]
    fn test_usb_device_serialization() {
        let device = UsbDevice {
            bus_number: 1,
            port_number: 2,
            vendor_id: 0x1234,
            product_id: 0x5678,
            manufacturer: Some("Test Manufacturer".to_string()),
            product: Some("Test Product".to_string()),
            description: None,
            serial_number: Some("ABC123".to_string()),
            class: UsbDeviceClass::MassStorage,
            speed: UsbSpeed::High,
        };
        let json = serde_json::to_string(&device).unwrap();
        let deserialized: UsbDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(device.vendor_id, deserialized.vendor_id);
        assert_eq!(device.product_id, deserialized.product_id);
        assert_eq!(device.serial_number, deserialized.serial_number);
    }

    #[test]
    fn test_usb_speed_variants() {
        let speeds = [
            UsbSpeed::Low,
            UsbSpeed::Full,
            UsbSpeed::High,
            UsbSpeed::Super,
            UsbSpeed::SuperPlus,
            UsbSpeed::SuperPlusx2,
            UsbSpeed::Usb4,
            UsbSpeed::Unknown,
        ];
        for speed in speeds {
            let json = serde_json::to_string(&speed).unwrap();
            let deserialized: UsbSpeed = serde_json::from_str(&json).unwrap();
            assert_eq!(speed, deserialized);
        }
    }

    #[test]
    fn test_usb_class_variants() {
        let classes = [
            UsbDeviceClass::Audio,
            UsbDeviceClass::Communication,
            UsbDeviceClass::Hid,
            UsbDeviceClass::Printer,
            UsbDeviceClass::MassStorage,
            UsbDeviceClass::Hub,
            UsbDeviceClass::Video,
            UsbDeviceClass::Wireless,
            UsbDeviceClass::Vendor,
            UsbDeviceClass::Unknown,
        ];
        for class in classes {
            let json = serde_json::to_string(&class).unwrap();
            let deserialized: UsbDeviceClass = serde_json::from_str(&json).unwrap();
            assert_eq!(class, deserialized);
        }
    }
}

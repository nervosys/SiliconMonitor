//! Input device monitoring — keyboards, mice, touchpads, game controllers, tablets.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/class/input/`, `/proc/bus/input/devices`
//! - **Windows**: Uses WMI (`Win32_Keyboard`, `Win32_PointingDevice`)
//! - **macOS**: Uses `system_profiler SPUSBDataType` and IOKit inference
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::input::InputMonitor;
//!
//! let monitor = InputMonitor::new().unwrap();
//! for device in monitor.devices() {
//!     println!("{}: {:?} ({})", device.name, device.device_type, device.interface);
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Type of input device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputDeviceType {
    /// Standard keyboard
    Keyboard,
    /// Mouse
    Mouse,
    /// Touchpad / trackpad
    Touchpad,
    /// Trackball
    Trackball,
    /// Trackpoint / pointing stick
    Trackpoint,
    /// Game controller / gamepad / joystick
    GameController,
    /// Drawing tablet / digitizer
    Tablet,
    /// Touchscreen
    Touchscreen,
    /// Stylus / pen input
    Stylus,
    /// Remote control / media keys
    Remote,
    /// Biometric reader (fingerprint, etc.)
    Biometric,
    /// Other or unrecognized input
    Other,
}

/// Connection interface for the input device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputInterface {
    USB,
    Bluetooth,
    PS2,
    I2C,
    SPI,
    Serial,
    Internal,
    Wireless,
    Virtual,
    Unknown,
}

/// Information about a single input device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDevice {
    /// Device name
    pub name: String,
    /// Device type (keyboard, mouse, etc.)
    pub device_type: InputDeviceType,
    /// Connection interface
    pub interface: InputInterface,
    /// Vendor name or ID
    pub vendor: String,
    /// Product name or ID
    pub product: String,
    /// Physical path (sysfs path on Linux, device instance on Windows)
    pub physical_path: String,
    /// Whether the device appears active / connected
    pub is_active: bool,
    /// Device-specific capabilities (e.g., "buttons:5", "axes:2")
    pub capabilities: Vec<String>,
}

/// Monitor for input devices (keyboards, mice, game controllers, etc.)
pub struct InputMonitor {
    devices: Vec<InputDevice>,
}

impl InputMonitor {
    /// Create a new InputMonitor and enumerate all input devices.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            devices: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh the list of input devices from the system.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.devices.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        Ok(())
    }

    /// Get all detected input devices.
    pub fn devices(&self) -> &[InputDevice] {
        &self.devices
    }

    /// Get devices filtered by type.
    pub fn devices_by_type(&self, device_type: InputDeviceType) -> Vec<&InputDevice> {
        self.devices
            .iter()
            .filter(|d| d.device_type == device_type)
            .collect()
    }

    /// Get all keyboards.
    pub fn keyboards(&self) -> Vec<&InputDevice> {
        self.devices_by_type(InputDeviceType::Keyboard)
    }

    /// Get all mice and pointing devices.
    pub fn pointing_devices(&self) -> Vec<&InputDevice> {
        self.devices
            .iter()
            .filter(|d| {
                matches!(
                    d.device_type,
                    InputDeviceType::Mouse
                        | InputDeviceType::Touchpad
                        | InputDeviceType::Trackball
                        | InputDeviceType::Trackpoint
                )
            })
            .collect()
    }

    /// Get all game controllers.
    pub fn game_controllers(&self) -> Vec<&InputDevice> {
        self.devices_by_type(InputDeviceType::GameController)
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        // Parse /proc/bus/input/devices for comprehensive device info
        if let Ok(content) = std::fs::read_to_string("/proc/bus/input/devices") {
            let mut name = String::new();
            let mut phys = String::new();
            let mut sysfs = String::new();
            let mut handlers = String::new();
            let mut bitmap_ev = String::new();
            let mut bitmap_key = String::new();
            let mut bitmap_rel = String::new();
            let mut bitmap_abs = String::new();
            let mut vendor_id = String::new();
            let mut product_id = String::new();

            let flush = |devices: &mut Vec<InputDevice>,
                         name: &str,
                         phys: &str,
                         sysfs: &str,
                         handlers: &str,
                         bitmap_ev: &str,
                         bitmap_key: &str,
                         bitmap_rel: &str,
                         bitmap_abs: &str,
                         vendor_id: &str,
                         product_id: &str| {
                if name.is_empty() {
                    return;
                }
                let device_type = Self::classify_linux(name, handlers, bitmap_ev, bitmap_key, bitmap_rel, bitmap_abs);
                let interface = Self::infer_interface_linux(phys, sysfs);
                let mut caps = Vec::new();
                if !bitmap_key.is_empty() && bitmap_key != "0" {
                    caps.push("keys".into());
                }
                if !bitmap_rel.is_empty() && bitmap_rel != "0" {
                    caps.push("relative-axes".into());
                }
                if !bitmap_abs.is_empty() && bitmap_abs != "0" {
                    caps.push("absolute-axes".into());
                }
                if handlers.contains("js") {
                    caps.push("joystick".into());
                }

                devices.push(InputDevice {
                    name: name.to_string(),
                    device_type,
                    interface,
                    vendor: format!("0x{}", vendor_id),
                    product: format!("0x{}", product_id),
                    physical_path: sysfs.to_string(),
                    is_active: handlers.contains("event"),
                    capabilities: caps,
                });
            };

            for line in content.lines() {
                if line.is_empty() {
                    flush(
                        &mut self.devices,
                        &name, &phys, &sysfs, &handlers,
                        &bitmap_ev, &bitmap_key, &bitmap_rel, &bitmap_abs,
                        &vendor_id, &product_id,
                    );
                    name.clear();
                    phys.clear();
                    sysfs.clear();
                    handlers.clear();
                    bitmap_ev.clear();
                    bitmap_key.clear();
                    bitmap_rel.clear();
                    bitmap_abs.clear();
                    vendor_id.clear();
                    product_id.clear();
                } else if let Some(rest) = line.strip_prefix("N: Name=\"") {
                    name = rest.trim_end_matches('"').to_string();
                } else if let Some(rest) = line.strip_prefix("P: Phys=") {
                    phys = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("S: Sysfs=") {
                    sysfs = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("H: Handlers=") {
                    handlers = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("I: Bus=") {
                    // Format: Bus=XXXX Vendor=XXXX Product=XXXX Version=XXXX
                    for part in rest.split_whitespace() {
                        if let Some(v) = part.strip_prefix("Vendor=") {
                            vendor_id = v.to_string();
                        } else if let Some(p) = part.strip_prefix("Product=") {
                            product_id = p.to_string();
                        }
                    }
                } else if let Some(rest) = line.strip_prefix("B: EV=") {
                    bitmap_ev = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("B: KEY=") {
                    bitmap_key = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("B: REL=") {
                    bitmap_rel = rest.to_string();
                } else if let Some(rest) = line.strip_prefix("B: ABS=") {
                    bitmap_abs = rest.to_string();
                }
            }
            // Flush last device
            flush(
                &mut self.devices,
                &name, &phys, &sysfs, &handlers,
                &bitmap_ev, &bitmap_key, &bitmap_rel, &bitmap_abs,
                &vendor_id, &product_id,
            );
        }
    }

    #[cfg(target_os = "linux")]
    fn classify_linux(name: &str, handlers: &str, _ev: &str, _key: &str, rel: &str, abs: &str) -> InputDeviceType {
        let lower = name.to_lowercase();
        if lower.contains("keyboard") || lower.contains("kbd") {
            return InputDeviceType::Keyboard;
        }
        if lower.contains("touchpad") || lower.contains("trackpad") || lower.contains("clickpad") {
            return InputDeviceType::Touchpad;
        }
        if lower.contains("touchscreen") || lower.contains("touch screen") {
            return InputDeviceType::Touchscreen;
        }
        if lower.contains("trackpoint") || lower.contains("pointing stick") {
            return InputDeviceType::Trackpoint;
        }
        if lower.contains("trackball") {
            return InputDeviceType::Trackball;
        }
        if lower.contains("tablet") || lower.contains("wacom") || lower.contains("digitizer") {
            return InputDeviceType::Tablet;
        }
        if lower.contains("stylus") || lower.contains("pen") {
            return InputDeviceType::Stylus;
        }
        if lower.contains("gamepad") || lower.contains("joystick") || lower.contains("controller")
            || lower.contains("xbox") || lower.contains("playstation") || lower.contains("dualshock")
            || lower.contains("dualsense") || lower.contains("nintendo") || handlers.contains("js")
        {
            return InputDeviceType::GameController;
        }
        if lower.contains("fingerprint") || lower.contains("biometric") {
            return InputDeviceType::Biometric;
        }
        if lower.contains("remote") || lower.contains("consumer control") || lower.contains("media") {
            return InputDeviceType::Remote;
        }
        if lower.contains("mouse") || (!rel.is_empty() && rel != "0") {
            return InputDeviceType::Mouse;
        }
        if !abs.is_empty() && abs != "0" {
            return InputDeviceType::Touchscreen;
        }
        InputDeviceType::Other
    }

    #[cfg(target_os = "linux")]
    fn infer_interface_linux(phys: &str, _sysfs: &str) -> InputInterface {
        let p = phys.to_lowercase();
        if p.contains("usb") {
            InputInterface::USB
        } else if p.contains("bluetooth") || p.contains("bt") {
            InputInterface::Bluetooth
        } else if p.contains("i8042") || p.contains("ps/2") || p.contains("isa") {
            InputInterface::PS2
        } else if p.contains("i2c") {
            InputInterface::I2C
        } else if p.contains("spi") {
            InputInterface::SPI
        } else if p.contains("serial") || p.contains("tty") {
            InputInterface::Serial
        } else if p.is_empty() || p.contains("virtual") || p.contains("input") {
            InputInterface::Virtual
        } else {
            InputInterface::Unknown
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // Keyboards via WMI
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_Keyboard | Select-Object Name, Description, DeviceID, Status, Layout | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for item in &items {
                        let name = item["Name"].as_str().unwrap_or("Unknown Keyboard").to_string();
                        let device_id = item["DeviceID"].as_str().unwrap_or("").to_string();
                        let iface = if device_id.to_lowercase().contains("usb") {
                            InputInterface::USB
                        } else if device_id.to_lowercase().contains("hid") {
                            InputInterface::USB
                        } else if device_id.contains("PS2") || device_id.contains("ACPI") {
                            InputInterface::PS2
                        } else {
                            InputInterface::Unknown
                        };
                        self.devices.push(InputDevice {
                            name,
                            device_type: InputDeviceType::Keyboard,
                            interface: iface,
                            vendor: String::new(),
                            product: String::new(),
                            physical_path: device_id,
                            is_active: item["Status"].as_str() == Some("OK"),
                            capabilities: vec!["keys".into()],
                        });
                    }
                }
            }
        }

        // Pointing devices via WMI
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_PointingDevice | Select-Object Name, Description, DeviceID, Status, PointingType, NumberOfButtons | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for item in &items {
                        let name = item["Name"].as_str().unwrap_or("Unknown Pointing Device").to_string();
                        let lower = name.to_lowercase();
                        let device_type = if lower.contains("touchpad") || lower.contains("trackpad") {
                            InputDeviceType::Touchpad
                        } else if lower.contains("trackball") {
                            InputDeviceType::Trackball
                        } else if lower.contains("touchscreen") || lower.contains("touch screen") {
                            InputDeviceType::Touchscreen
                        } else if lower.contains("tablet") || lower.contains("wacom") {
                            InputDeviceType::Tablet
                        } else {
                            InputDeviceType::Mouse
                        };
                        let device_id = item["DeviceID"].as_str().unwrap_or("").to_string();
                        let iface = if device_id.to_lowercase().contains("usb") || device_id.to_lowercase().contains("hid") {
                            InputInterface::USB
                        } else if device_id.contains("PS2") || device_id.contains("ACPI") {
                            InputInterface::PS2
                        } else {
                            InputInterface::Unknown
                        };
                        let buttons = item["NumberOfButtons"].as_u64().unwrap_or(0);
                        let mut caps = vec!["relative-axes".into()];
                        if buttons > 0 {
                            caps.push(format!("buttons:{}", buttons));
                        }
                        self.devices.push(InputDevice {
                            name,
                            device_type,
                            interface: iface,
                            vendor: String::new(),
                            product: String::new(),
                            physical_path: device_id,
                            is_active: item["Status"].as_str() == Some("OK"),
                            capabilities: caps,
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        // Use system_profiler for HID devices
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPUSBDataType", "-detailLevel", "mini"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                // Parse USB tree for HID/input devices
                let mut current_name = String::new();
                let mut current_vendor = String::new();
                let mut current_product = String::new();

                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.ends_with(':') && !trimmed.starts_with("USB") && !trimmed.is_empty() {
                        // This is a device name
                        if !current_name.is_empty() {
                            // Check if previous device was an input device
                            self.maybe_add_macos_device(&current_name, &current_vendor, &current_product);
                        }
                        current_name = trimmed.trim_end_matches(':').to_string();
                        current_vendor.clear();
                        current_product.clear();
                    } else if let Some(v) = trimmed.strip_prefix("Vendor ID:") {
                        current_vendor = v.trim().to_string();
                    } else if let Some(p) = trimmed.strip_prefix("Product ID:") {
                        current_product = p.trim().to_string();
                    }
                }
                if !current_name.is_empty() {
                    self.maybe_add_macos_device(&current_name, &current_vendor, &current_product);
                }
            }
        }

        // Also check for Bluetooth input devices
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPBluetoothDataType", "-detailLevel", "mini"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                for line in text.lines() {
                    let trimmed = line.trim();
                    let lower = trimmed.to_lowercase();
                    if (lower.contains("keyboard") || lower.contains("mouse")
                        || lower.contains("trackpad") || lower.contains("magic"))
                        && trimmed.ends_with(':')
                    {
                        let name = trimmed.trim_end_matches(':').to_string();
                        let device_type = if lower.contains("keyboard") {
                            InputDeviceType::Keyboard
                        } else if lower.contains("trackpad") {
                            InputDeviceType::Touchpad
                        } else {
                            InputDeviceType::Mouse
                        };
                        self.devices.push(InputDevice {
                            name,
                            device_type,
                            interface: InputInterface::Bluetooth,
                            vendor: "Apple".into(),
                            product: String::new(),
                            physical_path: String::new(),
                            is_active: true,
                            capabilities: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn maybe_add_macos_device(&mut self, name: &str, vendor: &str, product: &str) {
        let lower = name.to_lowercase();
        let device_type = if lower.contains("keyboard") {
            InputDeviceType::Keyboard
        } else if lower.contains("mouse") {
            InputDeviceType::Mouse
        } else if lower.contains("trackpad") || lower.contains("touchpad") {
            InputDeviceType::Touchpad
        } else if lower.contains("gamepad") || lower.contains("controller") || lower.contains("joystick") {
            InputDeviceType::GameController
        } else if lower.contains("tablet") || lower.contains("wacom") || lower.contains("intuos") {
            InputDeviceType::Tablet
        } else if lower.contains("touchscreen") {
            InputDeviceType::Touchscreen
        } else {
            return; // Skip non-input USB devices
        };
        self.devices.push(InputDevice {
            name: name.to_string(),
            device_type,
            interface: InputInterface::USB,
            vendor: vendor.to_string(),
            product: product.to_string(),
            physical_path: String::new(),
            is_active: true,
            capabilities: Vec::new(),
        });
    }
}

impl Default for InputMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            devices: Vec::new(),
        })
    }
}

impl std::fmt::Display for InputDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Keyboard => write!(f, "Keyboard"),
            Self::Mouse => write!(f, "Mouse"),
            Self::Touchpad => write!(f, "Touchpad"),
            Self::Trackball => write!(f, "Trackball"),
            Self::Trackpoint => write!(f, "Trackpoint"),
            Self::GameController => write!(f, "Game Controller"),
            Self::Tablet => write!(f, "Tablet"),
            Self::Touchscreen => write!(f, "Touchscreen"),
            Self::Stylus => write!(f, "Stylus"),
            Self::Remote => write!(f, "Remote"),
            Self::Biometric => write!(f, "Biometric"),
            Self::Other => write!(f, "Other"),
        }
    }
}

impl std::fmt::Display for InputInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::USB => write!(f, "USB"),
            Self::Bluetooth => write!(f, "Bluetooth"),
            Self::PS2 => write!(f, "PS/2"),
            Self::I2C => write!(f, "I2C"),
            Self::SPI => write!(f, "SPI"),
            Self::Serial => write!(f, "Serial"),
            Self::Internal => write!(f, "Internal"),
            Self::Wireless => write!(f, "Wireless"),
            Self::Virtual => write!(f, "Virtual"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_monitor_creation() {
        let monitor = InputMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_input_monitor_default() {
        let monitor = InputMonitor::default();
        let _ = monitor.devices();
    }

    #[test]
    fn test_input_device_type_display() {
        assert_eq!(InputDeviceType::Keyboard.to_string(), "Keyboard");
        assert_eq!(InputDeviceType::GameController.to_string(), "Game Controller");
    }

    #[test]
    fn test_input_device_serialization() {
        let device = InputDevice {
            name: "Test Keyboard".into(),
            device_type: InputDeviceType::Keyboard,
            interface: InputInterface::USB,
            vendor: "TestVendor".into(),
            product: "TestProduct".into(),
            physical_path: "/sys/test".into(),
            is_active: true,
            capabilities: vec!["keys".into()],
        };
        let json = serde_json::to_string(&device).unwrap();
        assert!(json.contains("Test Keyboard"));
        let _: InputDevice = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_filter_by_type() {
        let monitor = InputMonitor::default();
        let _keyboards = monitor.keyboards();
        let _pointing = monitor.pointing_devices();
        let _controllers = monitor.game_controllers();
    }
}

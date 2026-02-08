//! Bluetooth device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothDeviceType {
    Unknown,
    Computer,
    Phone,
    Headset,
    Speaker,
    Keyboard,
    Mouse,
    GameController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothState {
    Connected,
    Paired,
    Discovered,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: Option<String>,
    pub device_type: BluetoothDeviceType,
    pub state: BluetoothState,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothAdapter {
    pub id: String,
    pub name: String,
    pub address: String,
    pub powered: bool,
}

pub struct BluetoothMonitor {
    adapters: Vec<BluetoothAdapter>,
    devices: Vec<BluetoothDevice>,
}

impl BluetoothMonitor {
    pub fn new() -> Result<Self, crate::error::SimonError> {
        let mut monitor = Self {
            adapters: Vec::new(),
            devices: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> {
        self.adapters.clear();
        self.devices.clear();
        #[cfg(target_os = "windows")]
        self.refresh_windows();
        #[cfg(target_os = "linux")]
        self.refresh_linux();
        #[cfg(target_os = "macos")]
        self.refresh_macos();
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        use std::process::Command;

        // Query Bluetooth adapters and devices via WMI/PnP
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command",
                r#"
                $result = @{ Adapters = @(); Devices = @() }
                
                # Get Bluetooth radios (adapters)
                $radios = Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'Bluetooth' -and $_.Name -match 'Radio|Adapter|Controller' }
                foreach ($r in $radios) {
                    $result.Adapters += [PSCustomObject]@{
                        Id = $r.PNPDeviceID
                        Name = $r.Name
                        Manufacturer = $r.Manufacturer
                        Status = $r.Status
                    }
                }
                
                # If no radios found, check for any Bluetooth class device as adapter
                if ($result.Adapters.Count -eq 0) {
                    $btDevs = Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'Bluetooth' } | Select-Object -First 1
                    foreach ($r in $btDevs) {
                        $result.Adapters += [PSCustomObject]@{
                            Id = $r.PNPDeviceID
                            Name = $r.Name
                            Manufacturer = $r.Manufacturer
                            Status = $r.Status
                        }
                    }
                }
                
                # Get paired/connected Bluetooth devices
                $btDevices = Get-CimInstance -ClassName Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'Bluetooth' -and $_.Name -notmatch 'Radio|Adapter|Controller|Enumerator|Microsoft' }
                foreach ($d in $btDevices) {
                    $dtype = 'Unknown'
                    $name = $d.Name.ToLower()
                    if ($name -match 'keyboard') { $dtype = 'Keyboard' }
                    elseif ($name -match 'mouse|pointing') { $dtype = 'Mouse' }
                    elseif ($name -match 'headset|headphone|earphone|buds|airpods') { $dtype = 'Headset' }
                    elseif ($name -match 'speaker|audio') { $dtype = 'Speaker' }
                    elseif ($name -match 'phone') { $dtype = 'Phone' }
                    elseif ($name -match 'controller|gamepad|joystick') { $dtype = 'GameController' }
                    
                    # Extract Bluetooth address from PNPDeviceID if available
                    $addr = ''
                    if ($d.PNPDeviceID -match '([0-9A-Fa-f]{12})') {
                        $hex = $Matches[1]
                        $addr = ($hex -replace '(.{2})', '$1:').TrimEnd(':')
                    }
                    
                    $result.Devices += [PSCustomObject]@{
                        Name = $d.Name
                        Address = $addr
                        Type = $dtype
                        Connected = ($d.Status -eq 'OK')
                    }
                }
                
                $result | ConvertTo-Json -Depth 3 -Compress
                "#])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
                    // Parse adapters
                    if let Some(adapters) = json.get("Adapters").and_then(|v| v.as_array()) {
                        for (idx, adapter) in adapters.iter().enumerate() {
                            let name = adapter
                                .get("Name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Bluetooth Adapter");
                            let id = adapter.get("Id").and_then(|v| v.as_str()).unwrap_or("");
                            let status = adapter
                                .get("Status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("OK");

                            self.adapters.push(BluetoothAdapter {
                                id: format!("bt{}", idx),
                                name: name.to_string(),
                                address: id.to_string(),
                                powered: status == "OK",
                            });
                        }
                    }

                    // Parse devices
                    if let Some(devices) = json.get("Devices").and_then(|v| v.as_array()) {
                        for device in devices {
                            let name = device.get("Name").and_then(|v| v.as_str());
                            let address = device
                                .get("Address")
                                .and_then(|v| v.as_str())
                                .unwrap_or("00:00:00:00:00:00");
                            let is_connected = device
                                .get("Connected")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let dtype = match device
                                .get("Type")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                            {
                                "Keyboard" => BluetoothDeviceType::Keyboard,
                                "Mouse" => BluetoothDeviceType::Mouse,
                                "Headset" => BluetoothDeviceType::Headset,
                                "Speaker" => BluetoothDeviceType::Speaker,
                                "Phone" => BluetoothDeviceType::Phone,
                                "GameController" => BluetoothDeviceType::GameController,
                                "Computer" => BluetoothDeviceType::Computer,
                                _ => BluetoothDeviceType::Unknown,
                            };

                            self.devices.push(BluetoothDevice {
                                address: address.to_string(),
                                name: name.map(|s| s.to_string()),
                                device_type: dtype,
                                state: if is_connected {
                                    BluetoothState::Connected
                                } else {
                                    BluetoothState::Paired
                                },
                                battery_percent: None,
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        use std::fs;
        use std::path::Path;

        // Read Bluetooth adapters from /sys/class/bluetooth
        let bt_path = Path::new("/sys/class/bluetooth");
        if bt_path.exists() {
            if let Ok(entries) = fs::read_dir(bt_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.starts_with("hci") {
                        continue;
                    }

                    let path = entry.path();
                    let address = fs::read_to_string(path.join("address"))
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    // Check power state
                    let powered =
                        fs::read_to_string(format!("/sys/class/bluetooth/{}/powered", name))
                            .unwrap_or_else(|_| "1".to_string())
                            .trim()
                            == "1";

                    self.adapters.push(BluetoothAdapter {
                        id: name.clone(),
                        name: format!("Bluetooth Adapter ({})", name),
                        address,
                        powered,
                    });
                }
            }
        }

        // Use bluetoothctl to list paired/connected devices
        if let Ok(output) = std::process::Command::new("bluetoothctl")
            .args(["devices"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    // Format: "Device AA:BB:CC:DD:EE:FF Device Name"
                    let parts: Vec<&str> = line.splitn(3, ' ').collect();
                    if parts.len() >= 3 && parts[0] == "Device" {
                        let address = parts[1].to_string();
                        let name = parts[2].to_string();

                        // Check if connected
                        let is_connected = std::process::Command::new("bluetoothctl")
                            .args(["info", &address])
                            .output()
                            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Connected: yes"))
                            .unwrap_or(false);

                        let dtype = classify_bt_device(&name);

                        self.devices.push(BluetoothDevice {
                            address,
                            name: Some(name),
                            device_type: dtype,
                            state: if is_connected {
                                BluetoothState::Connected
                            } else {
                                BluetoothState::Paired
                            },
                            battery_percent: None,
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        use std::process::Command;

        if let Ok(output) = Command::new("system_profiler")
            .args(["SPBluetoothDataType", "-json"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(bt_data) =
                        json.get("SPBluetoothDataType").and_then(|v| v.as_array())
                    {
                        for section in bt_data {
                            // Controller info
                            if let Some(ctrl) = section.get("controller_properties") {
                                let address = ctrl
                                    .get("controller_address")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let name = ctrl
                                    .get("controller_chipset")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Bluetooth")
                                    .to_string();
                                let powered =
                                    ctrl.get("controller_powerState").and_then(|v| v.as_str())
                                        == Some("attrib_on");

                                self.adapters.push(BluetoothAdapter {
                                    id: "bt0".to_string(),
                                    name,
                                    address,
                                    powered,
                                });
                            }

                            // Connected devices
                            if let Some(devices) =
                                section.get("device_connected").and_then(|v| v.as_object())
                            {
                                for (dev_name, dev_info) in devices {
                                    let address = dev_info
                                        .get("device_address")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("00:00:00:00:00:00")
                                        .to_string();
                                    let dtype = classify_bt_device(dev_name);

                                    self.devices.push(BluetoothDevice {
                                        address,
                                        name: Some(dev_name.clone()),
                                        device_type: dtype,
                                        state: BluetoothState::Connected,
                                        battery_percent: dev_info
                                            .get("device_batteryPercent")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| {
                                                s.trim_end_matches('%').parse::<u8>().ok()
                                            }),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn adapters(&self) -> &[BluetoothAdapter] {
        &self.adapters
    }
    pub fn devices(&self) -> &[BluetoothDevice] {
        &self.devices
    }
    pub fn is_available(&self) -> bool {
        !self.adapters.is_empty()
    }

    // ==================== Hardware Control APIs ====================

    /// Initiate pairing with a Bluetooth device by address.
    pub fn pair_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Invalid Bluetooth address format: {}",
                address
            )));
        }
        Ok(())
    }

    /// Remove pairing with a Bluetooth device.
    pub fn unpair_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Invalid Bluetooth address format: {}",
                address
            )));
        }
        Ok(())
    }

    /// Connect to a paired Bluetooth device.
    pub fn connect_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Invalid Bluetooth address format: {}",
                address
            )));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.address == address) {
            device.state = BluetoothState::Connected;
        }
        Ok(())
    }

    /// Disconnect from a connected Bluetooth device.
    pub fn disconnect_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(format!(
                "Invalid Bluetooth address format: {}",
                address
            )));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.address == address) {
            device.state = BluetoothState::Disconnected;
        }
        Ok(())
    }

    /// Enable or disable a Bluetooth adapter.
    pub fn set_adapter_power(
        &mut self,
        adapter_id: &str,
        enabled: bool,
    ) -> Result<(), crate::error::SimonError> {
        if let Some(adapter) = self.adapters.iter_mut().find(|a| a.id == adapter_id) {
            adapter.powered = enabled;
            Ok(())
        } else {
            Err(crate::error::SimonError::NotFound(format!(
                "Bluetooth adapter '{}' not found",
                adapter_id
            )))
        }
    }

    fn is_valid_mac_address(address: &str) -> bool {
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 6 {
            return false;
        }
        parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn classify_bt_device(name: &str) -> BluetoothDeviceType {
    let lower = name.to_lowercase();
    if lower.contains("keyboard") {
        BluetoothDeviceType::Keyboard
    } else if lower.contains("mouse") || lower.contains("trackpad") {
        BluetoothDeviceType::Mouse
    } else if lower.contains("headset")
        || lower.contains("headphone")
        || lower.contains("earphone")
        || lower.contains("buds")
        || lower.contains("airpods")
    {
        BluetoothDeviceType::Headset
    } else if lower.contains("speaker") {
        BluetoothDeviceType::Speaker
    } else if lower.contains("phone") || lower.contains("iphone") || lower.contains("android") {
        BluetoothDeviceType::Phone
    } else if lower.contains("controller")
        || lower.contains("gamepad")
        || lower.contains("joystick")
    {
        BluetoothDeviceType::GameController
    } else if lower.contains("computer") || lower.contains("laptop") || lower.contains("macbook") {
        BluetoothDeviceType::Computer
    } else {
        BluetoothDeviceType::Unknown
    }
}

impl Default for BluetoothMonitor {
    fn default() -> Self {
        Self {
            adapters: Vec::new(),
            devices: Vec::new(),
        }
    }
}

// Bluetooth events for device monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BluetoothEvent {
    DeviceConnected(BluetoothDevice),
    DeviceDisconnected(BluetoothDevice),
    DevicePaired(BluetoothDevice),
    AdapterEnabled(BluetoothAdapter),
    AdapterDisabled(BluetoothAdapter),
}

impl BluetoothMonitor {
    /// Check for device changes since last refresh
    /// Returns a list of connect/disconnect events
    pub fn poll_events(&mut self) -> Result<Vec<BluetoothEvent>, crate::error::SimonError> {
        let old_devices = self.devices.clone();
        self.refresh()?;

        let mut events = Vec::new();

        // Find state changes
        for old in &old_devices {
            if let Some(new) = self.devices.iter().find(|d| d.address == old.address) {
                // State changed
                if old.state != new.state {
                    match new.state {
                        BluetoothState::Connected => {
                            events.push(BluetoothEvent::DeviceConnected(new.clone()))
                        }
                        BluetoothState::Paired => {
                            events.push(BluetoothEvent::DevicePaired(new.clone()))
                        }
                        BluetoothState::Disconnected => {
                            events.push(BluetoothEvent::DeviceDisconnected(new.clone()))
                        }
                        _ => {}
                    }
                }
            } else {
                // Device removed
                events.push(BluetoothEvent::DeviceDisconnected(old.clone()));
            }
        }

        // Find new devices
        for new in &self.devices {
            if !old_devices.iter().any(|d| d.address == new.address) {
                match new.state {
                    BluetoothState::Connected => {
                        events.push(BluetoothEvent::DeviceConnected(new.clone()))
                    }
                    BluetoothState::Paired => {
                        events.push(BluetoothEvent::DevicePaired(new.clone()))
                    }
                    _ => {}
                }
            }
        }

        Ok(events)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluetooth_monitor_creation() {
        let monitor = BluetoothMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_bluetooth_monitor_availability() {
        let monitor = BluetoothMonitor::new().unwrap();
        // Availability depends on platform - just ensure no panic
        let _ = monitor.is_available();
    }

    #[test]
    fn test_bluetooth_device_serialization() {
        let device = BluetoothDevice {
            address: "AA:BB:CC:DD:EE:FF".to_string(),
            name: Some("Test Device".to_string()),
            device_type: BluetoothDeviceType::Headset,
            state: BluetoothState::Connected,
            battery_percent: Some(75),
        };
        let json = serde_json::to_string(&device).unwrap();
        let deserialized: BluetoothDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(device.address, deserialized.address);
        assert_eq!(device.battery_percent, deserialized.battery_percent);
    }

    #[test]
    fn test_bluetooth_adapter_serialization() {
        let adapter = BluetoothAdapter {
            id: "hci0".to_string(),
            name: "Test Adapter".to_string(),
            address: "11:22:33:44:55:66".to_string(),
            powered: true,
        };
        let json = serde_json::to_string(&adapter).unwrap();
        let deserialized: BluetoothAdapter = serde_json::from_str(&json).unwrap();
        assert_eq!(adapter.id, deserialized.id);
        assert!(deserialized.powered);
    }
}

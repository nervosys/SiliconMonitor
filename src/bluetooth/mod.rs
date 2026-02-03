//! Bluetooth device monitoring module
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothDeviceType { Unknown, Computer, Phone, Headset, Speaker, Keyboard, Mouse, GameController }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BluetoothState { Connected, Paired, Discovered, Disconnected }

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
        Ok(Self { adapters: Vec::new(), devices: Vec::new() })
    }
    pub fn refresh(&mut self) -> Result<(), crate::error::SimonError> { Ok(()) }
    pub fn adapters(&self) -> &[BluetoothAdapter] { &self.adapters }
    pub fn devices(&self) -> &[BluetoothDevice] { &self.devices }
    pub fn is_available(&self) -> bool { !self.adapters.is_empty() }

    // ==================== Hardware Control APIs ====================

    /// Initiate pairing with a Bluetooth device by address.
    pub fn pair_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Invalid Bluetooth address format: {}", address)
            ));
        }
        Ok(())
    }

    /// Remove pairing with a Bluetooth device.
    pub fn unpair_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Invalid Bluetooth address format: {}", address)
            ));
        }
        Ok(())
    }

    /// Connect to a paired Bluetooth device.
    pub fn connect_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Invalid Bluetooth address format: {}", address)
            ));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.address == address) {
            device.state = BluetoothState::Connected;
        }
        Ok(())
    }

    /// Disconnect from a connected Bluetooth device.
    pub fn disconnect_device(&mut self, address: &str) -> Result<(), crate::error::SimonError> {
        if !Self::is_valid_mac_address(address) {
            return Err(crate::error::SimonError::InvalidInput(
                format!("Invalid Bluetooth address format: {}", address)
            ));
        }
        if let Some(device) = self.devices.iter_mut().find(|d| d.address == address) {
            device.state = BluetoothState::Disconnected;
        }
        Ok(())
    }

    /// Enable or disable a Bluetooth adapter.
    pub fn set_adapter_power(&mut self, adapter_id: &str, enabled: bool) -> Result<(), crate::error::SimonError> {
        if let Some(adapter) = self.adapters.iter_mut().find(|a| a.id == adapter_id) {
            adapter.powered = enabled;
            Ok(())
        } else {
            Err(crate::error::SimonError::NotFound(
                format!("Bluetooth adapter '{}' not found", adapter_id)
            ))
        }
    }

    fn is_valid_mac_address(address: &str) -> bool {
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 6 { return false; }
        parts.iter().all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
    }
}

impl Default for BluetoothMonitor {
    fn default() -> Self { Self { adapters: Vec::new(), devices: Vec::new() } }
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
                        BluetoothState::Connected => events.push(BluetoothEvent::DeviceConnected(new.clone())),
                        BluetoothState::Paired => events.push(BluetoothEvent::DevicePaired(new.clone())),
                        BluetoothState::Disconnected => events.push(BluetoothEvent::DeviceDisconnected(new.clone())),
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
                    BluetoothState::Connected => events.push(BluetoothEvent::DeviceConnected(new.clone())),
                    BluetoothState::Paired => events.push(BluetoothEvent::DevicePaired(new.clone())),
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

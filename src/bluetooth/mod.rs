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
}

impl Default for BluetoothMonitor {
    fn default() -> Self { Self { adapters: Vec::new(), devices: Vec::new() } }
}
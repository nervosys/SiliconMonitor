//! PCI device enumeration — all PCI/PCIe devices with class, vendor, driver info.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/bus/pci/devices/`
//! - **Windows**: Uses WMI (`Win32_PnPEntity` with PCI bus)
//! - **macOS**: Uses `system_profiler SPPCIDataType`
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::pci_devices::PciDeviceMonitor;
//!
//! let monitor = PciDeviceMonitor::new().unwrap();
//! for dev in monitor.devices() {
//!     println!("[{}] {} {} (driver: {})",
//!         dev.address, dev.vendor_name, dev.device_name, dev.driver);
//! }
//! println!("GPU devices: {}", monitor.devices_by_class(PciClass::DisplayController).len());
//! ```

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

/// PCI device class (major categories)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PciClass {
    /// 00: Unclassified
    Unclassified,
    /// 01: Mass storage controller (SATA, NVMe, RAID, etc.)
    MassStorage,
    /// 02: Network controller (Ethernet, WiFi, etc.)
    NetworkController,
    /// 03: Display controller (GPU, VGA)
    DisplayController,
    /// 04: Multimedia controller (audio, video)
    MultimediaController,
    /// 05: Memory controller
    MemoryController,
    /// 06: Bridge (PCI-to-PCI, host bridge, ISA bridge)
    Bridge,
    /// 07: Communication controller (serial, modem)
    CommunicationController,
    /// 08: System peripheral (DMA, timer, PIC)
    SystemPeripheral,
    /// 09: Input device controller
    InputDevice,
    /// 0A: Docking station
    DockingStation,
    /// 0B: Processor
    Processor,
    /// 0C: Serial bus controller (USB, FireWire, SMBus)
    SerialBusController,
    /// 0D: Wireless controller (Bluetooth, WiFi, etc.)
    WirelessController,
    /// 0E: Intelligent controller (I2O)
    IntelligentController,
    /// 0F: Satellite communication
    SatelliteComm,
    /// 10: Encryption controller
    EncryptionController,
    /// 11: Signal processing controller
    SignalProcessing,
    /// 12: Processing accelerator (NPU, FPGA)
    ProcessingAccelerator,
    /// 13: Non-essential instrumentation
    Instrumentation,
    /// 40: Co-processor
    CoProcessor,
    /// Other/unknown class
    Other(u8),
}

/// PCIe link information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciLinkInfo {
    /// Current link speed (e.g., "8.0 GT/s", "16.0 GT/s")
    pub speed: String,
    /// Current link width (e.g., "x1", "x4", "x16")
    pub width: String,
    /// Maximum supported speed
    pub max_speed: String,
    /// Maximum supported width
    pub max_width: String,
    /// PCIe generation (1-5)
    pub generation: u8,
}

/// Information about a single PCI device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciDeviceInfo {
    /// BDF address (e.g., "0000:00:1f.3")
    pub address: String,
    /// Vendor ID (hex, e.g., "8086")
    pub vendor_id: String,
    /// Device ID (hex, e.g., "a170")
    pub device_id: String,
    /// Subsystem vendor ID
    pub subsystem_vendor_id: String,
    /// Subsystem device ID
    pub subsystem_device_id: String,
    /// PCI class code (2-digit hex)
    pub class_code: String,
    /// Decoded device class
    pub class: PciClass,
    /// Vendor name (human readable)
    pub vendor_name: String,
    /// Device name (human readable)
    pub device_name: String,
    /// Kernel driver in use
    pub driver: String,
    /// Kernel module loaded for this device
    pub kernel_module: String,
    /// PCI revision
    pub revision: String,
    /// IOMMU group (for passthrough/vfio)
    pub iommu_group: Option<u32>,
    /// PCIe link information (if PCIe device)
    pub link_info: Option<PciLinkInfo>,
    /// Whether the device supports SR-IOV
    pub sriov_capable: bool,
    /// Number of SR-IOV virtual functions
    pub sriov_vfs: u32,
    /// NUMA node affinity (-1 if not applicable)
    pub numa_node: i32,
    /// Power state (e.g., "D0", "D3hot")
    pub power_state: String,
}

/// Monitor for PCI devices
pub struct PciDeviceMonitor {
    devices: Vec<PciDeviceInfo>,
}

impl PciDeviceMonitor {
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self { devices: Vec::new() };
        monitor.refresh()?;
        Ok(monitor)
    }

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

    pub fn devices(&self) -> &[PciDeviceInfo] {
        &self.devices
    }

    /// Get devices by class.
    pub fn devices_by_class(&self, class: PciClass) -> Vec<&PciDeviceInfo> {
        self.devices.iter().filter(|d| d.class == class).collect()
    }

    /// Get devices by vendor ID.
    pub fn devices_by_vendor(&self, vendor_id: &str) -> Vec<&PciDeviceInfo> {
        self.devices.iter().filter(|d| d.vendor_id == vendor_id).collect()
    }

    /// Get all GPU devices.
    pub fn gpus(&self) -> Vec<&PciDeviceInfo> {
        self.devices_by_class(PciClass::DisplayController)
    }

    /// Get all network devices.
    pub fn network_devices(&self) -> Vec<&PciDeviceInfo> {
        let mut devs = self.devices_by_class(PciClass::NetworkController);
        devs.extend(self.devices_by_class(PciClass::WirelessController));
        devs
    }

    /// Get all storage controllers.
    pub fn storage_devices(&self) -> Vec<&PciDeviceInfo> {
        self.devices_by_class(PciClass::MassStorage)
    }

    /// Get SR-IOV capable devices.
    pub fn sriov_devices(&self) -> Vec<&PciDeviceInfo> {
        self.devices.iter().filter(|d| d.sriov_capable).collect()
    }

    /// Classify a PCI device by its class byte.
    pub fn classify_pci(class_byte: u8) -> PciClass {
        match class_byte {
            0x00 => PciClass::Unclassified,
            0x01 => PciClass::MassStorage,
            0x02 => PciClass::NetworkController,
            0x03 => PciClass::DisplayController,
            0x04 => PciClass::MultimediaController,
            0x05 => PciClass::MemoryController,
            0x06 => PciClass::Bridge,
            0x07 => PciClass::CommunicationController,
            0x08 => PciClass::SystemPeripheral,
            0x09 => PciClass::InputDevice,
            0x0A => PciClass::DockingStation,
            0x0B => PciClass::Processor,
            0x0C => PciClass::SerialBusController,
            0x0D => PciClass::WirelessController,
            0x0E => PciClass::IntelligentController,
            0x0F => PciClass::SatelliteComm,
            0x10 => PciClass::EncryptionController,
            0x11 => PciClass::SignalProcessing,
            0x12 => PciClass::ProcessingAccelerator,
            0x13 => PciClass::Instrumentation,
            0x40 => PciClass::CoProcessor,
            other => PciClass::Other(other),
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        let pci_base = std::path::Path::new("/sys/bus/pci/devices");
        if !pci_base.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(pci_base) {
            for entry in entries.flatten() {
                let address = entry.file_name().to_string_lossy().to_string();
                let base = entry.path();

                let vendor_id = Self::read_hex_id(&base.join("vendor"));
                let device_id = Self::read_hex_id(&base.join("device"));
                let subsystem_vendor_id = Self::read_hex_id(&base.join("subsystem_vendor"));
                let subsystem_device_id = Self::read_hex_id(&base.join("subsystem_device"));
                let revision = Self::read_hex_id(&base.join("revision"));

                let class_hex = Self::read_trimmed(&base.join("class"));
                let class_byte = u8::from_str_radix(
                    &class_hex.trim_start_matches("0x").get(..2).unwrap_or("00"),
                    16,
                )
                .unwrap_or(0);
                let class = Self::classify_pci(class_byte);
                let class_code = format!("{:02x}", class_byte);

                // Driver
                let driver = std::fs::read_link(base.join("driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_default();

                // IOMMU group
                let iommu_group = std::fs::read_link(base.join("iommu_group"))
                    .ok()
                    .and_then(|p| {
                        p.file_name()
                            .and_then(|n| n.to_string_lossy().parse().ok())
                    });

                // NUMA node
                let numa_node: i32 = Self::read_trimmed(&base.join("numa_node"))
                    .parse()
                    .unwrap_or(-1);

                // Power state
                let power_state = Self::read_trimmed(&base.join("power_state"));

                // SR-IOV
                let sriov_capable = base.join("sriov_totalvfs").exists();
                let sriov_vfs: u32 = if sriov_capable {
                    Self::read_trimmed(&base.join("sriov_numvfs"))
                        .parse()
                        .unwrap_or(0)
                } else {
                    0
                };

                // PCIe link info
                let link_info = {
                    let speed = Self::read_trimmed(&base.join("current_link_speed"));
                    let width = Self::read_trimmed(&base.join("current_link_width"));
                    let max_speed = Self::read_trimmed(&base.join("max_link_speed"));
                    let max_width = Self::read_trimmed(&base.join("max_link_width"));
                    if !speed.is_empty() || !max_speed.is_empty() {
                        let gen = if max_speed.contains("32") { 5 }
                            else if max_speed.contains("16") { 4 }
                            else if max_speed.contains("8") { 3 }
                            else if max_speed.contains("5") { 2 }
                            else if max_speed.contains("2.5") { 1 }
                            else { 0 };
                        Some(PciLinkInfo {
                            speed,
                            width,
                            max_speed,
                            max_width,
                            generation: gen,
                        })
                    } else {
                        None
                    }
                };

                // Vendor/device name from lspci or /usr/share/hwdata
                let (vendor_name, device_name) = Self::lookup_pci_names(&vendor_id, &device_id);

                self.devices.push(PciDeviceInfo {
                    address,
                    vendor_id,
                    device_id,
                    subsystem_vendor_id,
                    subsystem_device_id,
                    class_code,
                    class,
                    vendor_name,
                    device_name,
                    driver,
                    kernel_module: String::new(),
                    revision,
                    iommu_group,
                    link_info,
                    sriov_capable,
                    sriov_vfs,
                    numa_node,
                    power_state,
                });
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn lookup_pci_names(vendor_id: &str, _device_id: &str) -> (String, String) {
        // Well-known vendor IDs
        let vendor = match vendor_id {
            "10de" => "NVIDIA Corporation",
            "1002" => "Advanced Micro Devices [AMD/ATI]",
            "8086" => "Intel Corporation",
            "14e4" => "Broadcom Inc.",
            "1b36" => "Red Hat (Virtio)",
            "15b3" => "Mellanox Technologies",
            "1af4" => "Virtio",
            "144d" => "Samsung Electronics",
            "1987" => "Phison Electronics",
            "126f" => "Silicon Motion",
            "1179" => "Toshiba/Kioxia",
            "1c5c" => "SK Hynix",
            "c0a9" => "Micron Technology",
            "106b" => "Apple Inc.",
            "1022" => "Advanced Micro Devices [AMD]",
            _ => "",
        };
        (vendor.to_string(), String::new())
    }

    #[cfg(target_os = "linux")]
    fn read_trimmed(path: &std::path::Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[cfg(target_os = "linux")]
    fn read_hex_id(path: &std::path::Path) -> String {
        Self::read_trimmed(path)
            .trim_start_matches("0x")
            .to_lowercase()
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                r#"Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPDeviceID -like 'PCI\*' } | Select-Object Name, Manufacturer, PNPDeviceID, Status, ConfigManagerErrorCode -First 500 | ConvertTo-Json -Compress"#])
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
                        let pnp_id = item["PNPDeviceID"].as_str().unwrap_or("");
                        let name = item["Name"].as_str().unwrap_or("").to_string();
                        let vendor_name = item["Manufacturer"].as_str().unwrap_or("").to_string();

                        // Parse VEN_XXXX&DEV_XXXX from PNP ID
                        let vendor_id = Self::extract_pnp_field(pnp_id, "VEN_");
                        let device_id = Self::extract_pnp_field(pnp_id, "DEV_");
                        let subsys_id = Self::extract_pnp_field(pnp_id, "SUBSYS_");

                        // Infer class from name
                        let class = Self::infer_class_from_name(&name);

                        self.devices.push(PciDeviceInfo {
                            address: pnp_id.to_string(),
                            vendor_id,
                            device_id,
                            subsystem_vendor_id: subsys_id.clone(),
                            subsystem_device_id: subsys_id,
                            class_code: String::new(),
                            class,
                            vendor_name,
                            device_name: name,
                            driver: String::new(),
                            kernel_module: String::new(),
                            revision: String::new(),
                            iommu_group: None,
                            link_info: None,
                            sriov_capable: false,
                            sriov_vfs: 0,
                            numa_node: -1,
                            power_state: item["Status"].as_str().unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn extract_pnp_field(pnp_id: &str, prefix: &str) -> String {
        pnp_id
            .find(prefix)
            .map(|pos| {
                let start = pos + prefix.len();
                pnp_id[start..]
                    .chars()
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect::<String>()
                    .to_lowercase()
            })
            .unwrap_or_default()
    }

    #[cfg(target_os = "windows")]
    fn infer_class_from_name(name: &str) -> PciClass {
        let lower = name.to_lowercase();
        if lower.contains("display") || lower.contains("video") || lower.contains("graphics") || lower.contains("gpu") || lower.contains("vga") {
            PciClass::DisplayController
        } else if lower.contains("ethernet") || lower.contains("network") || lower.contains("wi-fi") || lower.contains("wifi") {
            PciClass::NetworkController
        } else if lower.contains("storage") || lower.contains("sata") || lower.contains("ahci") || lower.contains("nvme") || lower.contains("raid") {
            PciClass::MassStorage
        } else if lower.contains("audio") || lower.contains("sound") || lower.contains("multimedia") {
            PciClass::MultimediaController
        } else if lower.contains("usb") || lower.contains("xhci") || lower.contains("smbus") {
            PciClass::SerialBusController
        } else if lower.contains("bridge") || lower.contains("pci-to-pci") || lower.contains("host") {
            PciClass::Bridge
        } else if lower.contains("bluetooth") || lower.contains("wireless") {
            PciClass::WirelessController
        } else if lower.contains("encryption") || lower.contains("tpm") {
            PciClass::EncryptionController
        } else if lower.contains("signal") || lower.contains("sensor") {
            PciClass::SignalProcessing
        } else {
            PciClass::Other(0xff)
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPPCIDataType", "-json"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(items) = val["SPPCIDataType"].as_array() {
                        for item in items {
                            let name = item["_name"].as_str().unwrap_or("").to_string();
                            let vendor_id = item["sppci_vendor-id"]
                                .as_str()
                                .unwrap_or("")
                                .trim_start_matches("0x")
                                .to_lowercase();
                            let device_id = item["sppci_device-id"]
                                .as_str()
                                .unwrap_or("")
                                .trim_start_matches("0x")
                                .to_lowercase();
                            let slot = item["sppci_slot_name"].as_str().unwrap_or("").to_string();
                            let driver = item["sppci_driver_installed"].as_str().unwrap_or("").to_string();
                            let link_speed = item["sppci_link-speed"].as_str().unwrap_or("").to_string();
                            let link_width = item["sppci_link-width"].as_str().unwrap_or("").to_string();

                            let link_info = if !link_speed.is_empty() {
                                Some(PciLinkInfo {
                                    speed: link_speed.clone(),
                                    width: link_width.clone(),
                                    max_speed: link_speed,
                                    max_width: link_width,
                                    generation: 0,
                                })
                            } else {
                                None
                            };

                            let class = Self::infer_class_from_name_mac(&name);

                            self.devices.push(PciDeviceInfo {
                                address: slot,
                                vendor_id,
                                device_id,
                                subsystem_vendor_id: String::new(),
                                subsystem_device_id: String::new(),
                                class_code: String::new(),
                                class,
                                vendor_name: String::new(),
                                device_name: name,
                                driver,
                                kernel_module: String::new(),
                                revision: String::new(),
                                iommu_group: None,
                                link_info,
                                sriov_capable: false,
                                sriov_vfs: 0,
                                numa_node: -1,
                                power_state: String::new(),
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn infer_class_from_name_mac(name: &str) -> PciClass {
        let lower = name.to_lowercase();
        if lower.contains("gpu") || lower.contains("display") || lower.contains("graphics") {
            PciClass::DisplayController
        } else if lower.contains("ethernet") || lower.contains("wifi") || lower.contains("network") {
            PciClass::NetworkController
        } else if lower.contains("nvme") || lower.contains("ahci") || lower.contains("storage") {
            PciClass::MassStorage
        } else if lower.contains("audio") || lower.contains("thunderbolt") {
            PciClass::MultimediaController
        } else if lower.contains("usb") || lower.contains("xhci") {
            PciClass::SerialBusController
        } else if lower.contains("bridge") {
            PciClass::Bridge
        } else {
            PciClass::Other(0xff)
        }
    }
}

impl Default for PciDeviceMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self { devices: Vec::new() })
    }
}

impl std::fmt::Display for PciClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unclassified => write!(f, "Unclassified"),
            Self::MassStorage => write!(f, "Mass Storage"),
            Self::NetworkController => write!(f, "Network Controller"),
            Self::DisplayController => write!(f, "Display Controller"),
            Self::MultimediaController => write!(f, "Multimedia"),
            Self::MemoryController => write!(f, "Memory Controller"),
            Self::Bridge => write!(f, "Bridge"),
            Self::CommunicationController => write!(f, "Communication"),
            Self::SystemPeripheral => write!(f, "System Peripheral"),
            Self::InputDevice => write!(f, "Input Device"),
            Self::DockingStation => write!(f, "Docking Station"),
            Self::Processor => write!(f, "Processor"),
            Self::SerialBusController => write!(f, "Serial Bus"),
            Self::WirelessController => write!(f, "Wireless"),
            Self::IntelligentController => write!(f, "Intelligent Controller"),
            Self::SatelliteComm => write!(f, "Satellite"),
            Self::EncryptionController => write!(f, "Encryption"),
            Self::SignalProcessing => write!(f, "Signal Processing"),
            Self::ProcessingAccelerator => write!(f, "Processing Accelerator"),
            Self::Instrumentation => write!(f, "Instrumentation"),
            Self::CoProcessor => write!(f, "Co-Processor"),
            Self::Other(c) => write!(f, "Other(0x{:02x})", c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let monitor = PciDeviceMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_monitor_default() {
        let monitor = PciDeviceMonitor::default();
        let _ = monitor.devices();
        let _ = monitor.gpus();
        let _ = monitor.network_devices();
        let _ = monitor.storage_devices();
    }

    #[test]
    fn test_class_display() {
        assert_eq!(PciClass::DisplayController.to_string(), "Display Controller");
        assert_eq!(PciClass::MassStorage.to_string(), "Mass Storage");
    }

    #[test]
    fn test_classify_pci() {
        assert_eq!(PciDeviceMonitor::classify_pci(0x03), PciClass::DisplayController);
        assert_eq!(PciDeviceMonitor::classify_pci(0x01), PciClass::MassStorage);
        assert_eq!(PciDeviceMonitor::classify_pci(0x02), PciClass::NetworkController);
    }

    #[test]
    fn test_serialization() {
        let dev = PciDeviceInfo {
            address: "0000:00:02.0".into(),
            vendor_id: "8086".into(),
            device_id: "a780".into(),
            subsystem_vendor_id: String::new(),
            subsystem_device_id: String::new(),
            class_code: "03".into(),
            class: PciClass::DisplayController,
            vendor_name: "Intel Corporation".into(),
            device_name: "UHD Graphics".into(),
            driver: "i915".into(),
            kernel_module: String::new(),
            revision: "00".into(),
            iommu_group: Some(1),
            link_info: None,
            sriov_capable: false,
            sriov_vfs: 0,
            numa_node: 0,
            power_state: "D0".into(),
        };
        let json = serde_json::to_string(&dev).unwrap();
        assert!(json.contains("Intel"));
        let _: PciDeviceInfo = serde_json::from_str(&json).unwrap();
    }
}

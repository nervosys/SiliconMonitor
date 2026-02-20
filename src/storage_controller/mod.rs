//! Storage controller monitoring — RAID arrays, HBAs, NVMe controllers.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/class/scsi_host/`, `/sys/class/nvme/`, `/proc/mdstat`
//! - **Windows**: Uses WMI (`Win32_SCSIController`, `Win32_IDEController`, `MSFT_StorageSubSystem`)
//! - **macOS**: Uses `system_profiler SPStorageDataType`, `SPNVMeDataType`
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::storage_controller::StorageControllerMonitor;
//!
//! let monitor = StorageControllerMonitor::new().unwrap();
//! for ctrl in monitor.controllers() {
//!     println!("{}: {} ({:?})", ctrl.name, ctrl.driver, ctrl.interface);
//! }
//! for array in monitor.raid_arrays() {
//!     println!("RAID {}: {} - {:?}", array.level, array.name, array.status);
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Storage interface type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageInterface {
    /// NVMe (Non-Volatile Memory Express)
    NVMe,
    /// AHCI / SATA controller
    AHCI,
    /// SAS (Serial Attached SCSI)
    SAS,
    /// SCSI (legacy parallel)
    SCSI,
    /// IDE / PATA (legacy)
    IDE,
    /// USB mass storage
    USB,
    /// Fibre Channel
    FibreChannel,
    /// iSCSI (network)
    ISCSI,
    /// Virtio (virtualized)
    Virtio,
    /// Unknown interface
    Unknown,
}

/// RAID level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaidLevel {
    Raid0,
    Raid1,
    Raid5,
    Raid6,
    Raid10,
    Raid50,
    Raid60,
    JBOD,
    Linear,
    Unknown(String),
}

/// RAID array status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaidStatus {
    /// Healthy / active
    Active,
    /// Degraded (missing member)
    Degraded,
    /// Rebuilding / resyncing
    Rebuilding,
    /// Failed / inactive
    Failed,
    /// Unknown status
    Unknown,
}

/// NVMe controller details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmeControllerInfo {
    /// Controller name (e.g., "nvme0")
    pub name: String,
    /// Model name
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Firmware revision
    pub firmware: String,
    /// PCIe link speed (e.g., "8 GT/s")
    pub pcie_speed: String,
    /// PCIe link width (e.g., "x4")
    pub pcie_width: String,
    /// Number of namespaces
    pub namespace_count: u32,
    /// Transport type (e.g., "pcie", "tcp")
    pub transport: String,
}

/// Storage controller information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageControllerInfo {
    /// Controller name or identifier
    pub name: String,
    /// Vendor or manufacturer
    pub vendor: String,
    /// Model or product name
    pub model: String,
    /// Driver in use
    pub driver: String,
    /// Controller interface type
    pub interface: StorageInterface,
    /// PCI address (if applicable)
    pub pci_address: String,
    /// Number of ports
    pub ports: u32,
    /// NVMe-specific info (if NVMe controller)
    pub nvme_info: Option<NvmeControllerInfo>,
}

/// Software RAID array information (mdadm, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaidArrayInfo {
    /// Array name (e.g., "md0")
    pub name: String,
    /// RAID level
    pub level: RaidLevel,
    /// Current status
    pub status: RaidStatus,
    /// Number of active devices
    pub active_devices: u32,
    /// Total expected devices
    pub total_devices: u32,
    /// Failed devices count
    pub failed_devices: u32,
    /// Spare devices count
    pub spare_devices: u32,
    /// Array size in bytes
    pub size_bytes: u64,
    /// Member device names
    pub members: Vec<String>,
    /// Resync progress percentage (0-100, None if not resyncing)
    pub resync_progress: Option<f32>,
}

/// Monitor for storage controllers and RAID arrays
pub struct StorageControllerMonitor {
    controllers: Vec<StorageControllerInfo>,
    raid_arrays: Vec<RaidArrayInfo>,
}

impl StorageControllerMonitor {
    /// Create a new StorageControllerMonitor and detect controllers.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            controllers: Vec::new(),
            raid_arrays: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh storage controller detection.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.controllers.clear();
        self.raid_arrays.clear();

        #[cfg(target_os = "linux")]
        {
            self.refresh_nvme_linux();
            self.refresh_scsi_linux();
            self.refresh_mdstat_linux();
        }

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        Ok(())
    }

    /// All detected storage controllers.
    pub fn controllers(&self) -> &[StorageControllerInfo] {
        &self.controllers
    }

    /// All detected RAID arrays.
    pub fn raid_arrays(&self) -> &[RaidArrayInfo] {
        &self.raid_arrays
    }

    /// Get NVMe controllers only.
    pub fn nvme_controllers(&self) -> Vec<&StorageControllerInfo> {
        self.controllers
            .iter()
            .filter(|c| c.interface == StorageInterface::NVMe)
            .collect()
    }

    /// Get controllers by interface type.
    pub fn controllers_by_interface(
        &self,
        iface: StorageInterface,
    ) -> Vec<&StorageControllerInfo> {
        self.controllers
            .iter()
            .filter(|c| c.interface == iface)
            .collect()
    }

    /// Check if any RAID arrays are degraded.
    pub fn has_degraded_raids(&self) -> bool {
        self.raid_arrays
            .iter()
            .any(|a| a.status == RaidStatus::Degraded || a.status == RaidStatus::Failed)
    }

    // ── Linux implementations ──

    #[cfg(target_os = "linux")]
    fn refresh_nvme_linux(&mut self) {
        let nvme_base = std::path::Path::new("/sys/class/nvme");
        if !nvme_base.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(nvme_base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("nvme") {
                    continue;
                }
                let base = entry.path();
                let model = Self::read_trimmed(&base.join("model"));
                let serial = Self::read_trimmed(&base.join("serial"));
                let firmware = Self::read_trimmed(&base.join("firmware_rev"));
                let transport = Self::read_trimmed(&base.join("transport"));

                // Count namespaces
                let ns_count = std::fs::read_dir(&base)
                    .map(|rd| {
                        rd.flatten()
                            .filter(|e| {
                                e.file_name()
                                    .to_string_lossy()
                                    .starts_with(&format!("{}n", name))
                            })
                            .count() as u32
                    })
                    .unwrap_or(0);

                // PCIe info from device/
                let pcie_speed = Self::read_trimmed(&base.join("device/current_link_speed"));
                let pcie_width = Self::read_trimmed(&base.join("device/current_link_width"));

                let nvme_info = NvmeControllerInfo {
                    name: name.clone(),
                    model: model.clone(),
                    serial: serial.clone(),
                    firmware,
                    pcie_speed,
                    pcie_width,
                    namespace_count: ns_count,
                    transport: if transport.is_empty() {
                        "pcie".to_string()
                    } else {
                        transport
                    },
                };

                self.controllers.push(StorageControllerInfo {
                    name: name.clone(),
                    vendor: String::new(),
                    model,
                    driver: "nvme".to_string(),
                    interface: StorageInterface::NVMe,
                    pci_address: Self::read_pci_address(&base),
                    ports: ns_count,
                    nvme_info: Some(nvme_info),
                });
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_scsi_linux(&mut self) {
        let scsi_base = std::path::Path::new("/sys/class/scsi_host");
        if !scsi_base.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(scsi_base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let base = entry.path();
                let proc_name = Self::read_trimmed(&base.join("proc_name"));

                let interface = match proc_name.as_str() {
                    "ahci" => StorageInterface::AHCI,
                    "mpt3sas" | "mpt2sas" | "megaraid_sas" => StorageInterface::SAS,
                    "uas" | "usb-storage" => StorageInterface::USB,
                    "virtio_scsi" => StorageInterface::Virtio,
                    _ => {
                        if proc_name.contains("iscsi") {
                            StorageInterface::ISCSI
                        } else {
                            StorageInterface::SCSI
                        }
                    }
                };

                let model_name = Self::read_trimmed(&base.join("model_name"));

                self.controllers.push(StorageControllerInfo {
                    name,
                    vendor: String::new(),
                    model: model_name,
                    driver: proc_name,
                    interface,
                    pci_address: String::new(),
                    ports: 0,
                    nvme_info: None,
                });
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_mdstat_linux(&mut self) {
        let content = match std::fs::read_to_string("/proc/mdstat") {
            Ok(c) => c,
            Err(_) => return,
        };

        let mut current: Option<RaidArrayInfo> = None;

        for line in content.lines() {
            let line = line.trim();

            // "md0 : active raid1 sda1[0] sdb1[1]"
            if line.starts_with("md") && line.contains(" : ") {
                // Save previous array
                if let Some(array) = current.take() {
                    self.raid_arrays.push(array);
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    continue;
                }

                let name = parts[0].to_string();
                let status_str = parts[2]; // "active" or "inactive"
                let level_str = if parts.len() > 3 { parts[3] } else { "" };

                let status = match status_str {
                    "active" => RaidStatus::Active,
                    "inactive" => RaidStatus::Failed,
                    _ => RaidStatus::Unknown,
                };

                let level = match level_str {
                    "raid0" => RaidLevel::Raid0,
                    "raid1" => RaidLevel::Raid1,
                    "raid5" => RaidLevel::Raid5,
                    "raid6" => RaidLevel::Raid6,
                    "raid10" => RaidLevel::Raid10,
                    "linear" => RaidLevel::Linear,
                    other => RaidLevel::Unknown(other.to_string()),
                };

                // Extract member devices (e.g., "sda1[0]", "sdb1[1]")
                let members: Vec<String> = parts[4..]
                    .iter()
                    .filter_map(|p| p.split('[').next().map(|s| s.to_string()))
                    .collect();

                current = Some(RaidArrayInfo {
                    name,
                    level,
                    status,
                    active_devices: members.len() as u32,
                    total_devices: members.len() as u32,
                    failed_devices: 0,
                    spare_devices: 0,
                    size_bytes: 0,
                    members,
                    resync_progress: None,
                });
            } else if let Some(ref mut array) = current {
                // "123456 blocks super 1.2 [2/2] [UU]"
                if line.contains("blocks") {
                    if let Some(blocks_str) = line.split_whitespace().next() {
                        if let Ok(blocks) = blocks_str.parse::<u64>() {
                            array.size_bytes = blocks * 1024;
                        }
                    }
                    // Parse [U_] or [UU] status
                    if let Some(bracket_pos) = line.rfind('[') {
                        let remainder = &line[bracket_pos + 1..];
                        if let Some(end) = remainder.find(']') {
                            let status_chars = &remainder[..end];
                            let failed = status_chars.chars().filter(|c| *c == '_').count() as u32;
                            if failed > 0 {
                                array.status = RaidStatus::Degraded;
                                array.failed_devices = failed;
                            }
                        }
                    }
                }
                // "recovery = 45.2% ..."
                if line.contains("recovery") || line.contains("resync") {
                    if let Some(pct_str) = line
                        .split('=')
                        .nth(1)
                        .and_then(|s| s.trim().split('%').next())
                    {
                        if let Ok(pct) = pct_str.trim().parse::<f32>() {
                            array.resync_progress = Some(pct);
                            array.status = RaidStatus::Rebuilding;
                        }
                    }
                }
            }
        }

        if let Some(array) = current {
            self.raid_arrays.push(array);
        }
    }

    #[cfg(target_os = "linux")]
    fn read_trimmed(path: &std::path::Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[cfg(target_os = "linux")]
    fn read_pci_address(base: &std::path::Path) -> String {
        // Resolve the symlink to get PCI BDF
        let device_link = base.join("device");
        std::fs::read_link(&device_link)
            .ok()
            .and_then(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_default()
    }

    // ── Windows implementation ──

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // SCSI controllers
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_SCSIController | Select-Object Name, Manufacturer, DriverName, DeviceID, Status | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                self.parse_windows_controllers(&text, StorageInterface::SCSI);
            }
        }

        // IDE controllers
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_IDEController | Select-Object Name, Manufacturer, DriverName, DeviceID, Status | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                self.parse_windows_controllers(&text, StorageInterface::AHCI);
            }
        }

        // NVMe detection via Get-PhysicalDisk
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-PhysicalDisk | Where-Object BusType -eq 'NVMe' | Select-Object FriendlyName, Manufacturer, Model, SerialNumber, FirmwareVersion | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for (i, item) in items.iter().enumerate() {
                        let model = item["Model"].as_str().unwrap_or("").trim().to_string();
                        let serial = item["SerialNumber"].as_str().unwrap_or("").trim().to_string();
                        let firmware = item["FirmwareVersion"].as_str().unwrap_or("").trim().to_string();
                        let friendly = item["FriendlyName"].as_str().unwrap_or(&model).to_string();

                        self.controllers.push(StorageControllerInfo {
                            name: format!("nvme{}", i),
                            vendor: item["Manufacturer"].as_str().unwrap_or("").trim().to_string(),
                            model: friendly,
                            driver: "nvme".to_string(),
                            interface: StorageInterface::NVMe,
                            pci_address: String::new(),
                            ports: 1,
                            nvme_info: Some(NvmeControllerInfo {
                                name: format!("nvme{}", i),
                                model,
                                serial,
                                firmware,
                                pcie_speed: String::new(),
                                pcie_width: String::new(),
                                namespace_count: 1,
                                transport: "pcie".to_string(),
                            }),
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn parse_windows_controllers(&mut self, json_text: &str, default_iface: StorageInterface) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_text) {
            let items = match &val {
                serde_json::Value::Array(arr) => arr.clone(),
                obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                _ => return,
            };
            for item in &items {
                let name = item["Name"].as_str().unwrap_or("").to_string();
                let vendor = item["Manufacturer"].as_str().unwrap_or("").to_string();
                let driver = item["DriverName"].as_str().unwrap_or("").to_string();

                let interface = if name.to_lowercase().contains("nvme") {
                    StorageInterface::NVMe
                } else if name.to_lowercase().contains("sas") {
                    StorageInterface::SAS
                } else if name.to_lowercase().contains("usb") {
                    StorageInterface::USB
                } else {
                    default_iface.clone()
                };

                self.controllers.push(StorageControllerInfo {
                    name: name.clone(),
                    vendor,
                    model: name,
                    driver,
                    interface,
                    pci_address: item["DeviceID"].as_str().unwrap_or("").to_string(),
                    ports: 0,
                    nvme_info: None,
                });
            }
        }
    }

    // ── macOS implementation ──

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        // NVMe controllers
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPNVMeDataType", "-json"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(items) = val["SPNVMeDataType"].as_array() {
                        for (i, item) in items.iter().enumerate() {
                            let name = item["_name"].as_str().unwrap_or("NVMe").to_string();
                            let model = item["device_model"].as_str().unwrap_or("").to_string();
                            let serial = item["device_serial"].as_str().unwrap_or("").to_string();
                            let firmware = item["device_revision"].as_str().unwrap_or("").to_string();

                            let nvme_info = NvmeControllerInfo {
                                name: format!("nvme{}", i),
                                model: model.clone(),
                                serial,
                                firmware,
                                pcie_speed: item["spnvme_linkspeed"].as_str().unwrap_or("").to_string(),
                                pcie_width: item["spnvme_linkwidth"].as_str().unwrap_or("").to_string(),
                                namespace_count: 1,
                                transport: "pcie".to_string(),
                            };

                            self.controllers.push(StorageControllerInfo {
                                name,
                                vendor: String::new(),
                                model,
                                driver: "nvme".to_string(),
                                interface: StorageInterface::NVMe,
                                pci_address: String::new(),
                                ports: 1,
                                nvme_info: Some(nvme_info),
                            });
                        }
                    }
                }
            }
        }

        // SATA controllers
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPSerialATADataType", "-json"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(items) = val["SPSerialATADataType"].as_array() {
                        for item in items {
                            let name = item["_name"].as_str().unwrap_or("SATA").to_string();
                            let vendor = item["spsata_vendor"].as_str().unwrap_or("").to_string();
                            let port_count = item["_items"]
                                .as_array()
                                .map(|a| a.len() as u32)
                                .unwrap_or(0);

                            self.controllers.push(StorageControllerInfo {
                                name,
                                vendor,
                                model: String::new(),
                                driver: "ahci".to_string(),
                                interface: StorageInterface::AHCI,
                                pci_address: String::new(),
                                ports: port_count,
                                nvme_info: None,
                            });
                        }
                    }
                }
            }
        }
    }
}

impl Default for StorageControllerMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            controllers: Vec::new(),
            raid_arrays: Vec::new(),
        })
    }
}

impl std::fmt::Display for StorageInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NVMe => write!(f, "NVMe"),
            Self::AHCI => write!(f, "AHCI/SATA"),
            Self::SAS => write!(f, "SAS"),
            Self::SCSI => write!(f, "SCSI"),
            Self::IDE => write!(f, "IDE"),
            Self::USB => write!(f, "USB"),
            Self::FibreChannel => write!(f, "Fibre Channel"),
            Self::ISCSI => write!(f, "iSCSI"),
            Self::Virtio => write!(f, "Virtio"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for RaidLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Raid0 => write!(f, "RAID 0"),
            Self::Raid1 => write!(f, "RAID 1"),
            Self::Raid5 => write!(f, "RAID 5"),
            Self::Raid6 => write!(f, "RAID 6"),
            Self::Raid10 => write!(f, "RAID 10"),
            Self::Raid50 => write!(f, "RAID 50"),
            Self::Raid60 => write!(f, "RAID 60"),
            Self::JBOD => write!(f, "JBOD"),
            Self::Linear => write!(f, "Linear"),
            Self::Unknown(s) => write!(f, "{}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_controller_creation() {
        let monitor = StorageControllerMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_storage_controller_default() {
        let monitor = StorageControllerMonitor::default();
        let _ = monitor.controllers();
        let _ = monitor.raid_arrays();
        let _ = monitor.nvme_controllers();
        let _ = monitor.has_degraded_raids();
    }

    #[test]
    fn test_nvme_info_serialization() {
        let info = NvmeControllerInfo {
            name: "nvme0".into(),
            model: "Samsung 980 PRO".into(),
            serial: "ABC123".into(),
            firmware: "5B2QGXA7".into(),
            pcie_speed: "8 GT/s".into(),
            pcie_width: "x4".into(),
            namespace_count: 1,
            transport: "pcie".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Samsung"));
        let _: NvmeControllerInfo = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_raid_serialization() {
        let array = RaidArrayInfo {
            name: "md0".into(),
            level: RaidLevel::Raid1,
            status: RaidStatus::Active,
            active_devices: 2,
            total_devices: 2,
            failed_devices: 0,
            spare_devices: 0,
            size_bytes: 1024 * 1024 * 1024,
            members: vec!["sda1".into(), "sdb1".into()],
            resync_progress: None,
        };
        let json = serde_json::to_string(&array).unwrap();
        assert!(json.contains("md0"));
        let _: RaidArrayInfo = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_interface_display() {
        assert_eq!(StorageInterface::NVMe.to_string(), "NVMe");
        assert_eq!(StorageInterface::AHCI.to_string(), "AHCI/SATA");
        assert_eq!(RaidLevel::Raid1.to_string(), "RAID 1");
    }
}

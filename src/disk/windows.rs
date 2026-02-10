// SPDX-License-Identifier: OCL-1.0
// Copyright (c) 2024 NervoSys

//! Windows disk monitoring via WMI and Windows Storage APIs
//!
//! This module provides disk monitoring for Windows using:
//! - WMI Win32_DiskDrive for device information
//! - Performance Counters for I/O statistics
//! - DeviceIoControl for SMART data

use crate::disk::traits::*;
use std::collections::HashMap;
use std::path::PathBuf;
use wmi::{COMLibrary, WMIConnection};

/// Create WMI connection with robust COM initialization
/// Handles cases where COM is already initialized by the GUI framework
fn create_wmi_connection() -> Result<WMIConnection, Error> {
    // Strategy 1: Fresh COM initialization (works best in background threads)
    if let Ok(com) = COMLibrary::new() {
        if let Ok(conn) = WMIConnection::with_namespace_path("root\\CIMV2", com) {
            return Ok(conn);
        }
    }

    // Strategy 2: COM without security init
    if let Ok(com) = COMLibrary::without_security() {
        if let Ok(conn) = WMIConnection::with_namespace_path("root\\CIMV2", com) {
            return Ok(conn);
        }
    }

    // Strategy 3: Assume COM is already initialized by the runtime (e.g., GUI apps)
    let com = unsafe { COMLibrary::assume_initialized() };
    WMIConnection::with_namespace_path("root\\CIMV2", com)
        .map_err(|e| Error::InitializationFailed(e.to_string()))
}

/// Windows disk device implementation
pub struct WindowsDisk {
    name: String,
    device_path: PathBuf,
    disk_type: DiskType,
    model: Option<String>,
    serial: Option<String>,
    size: u64,
    disk_index: u32,
    interface_type: Option<String>,
}

impl WindowsDisk {
    pub fn new(
        name: String,
        device_path: PathBuf,
        disk_type: DiskType,
        model: Option<String>,
        serial: Option<String>,
        size: u64,
        disk_index: u32,
        interface_type: Option<String>,
    ) -> Self {
        Self {
            name,
            device_path,
            disk_type,
            model,
            serial,
            size,
            disk_index,
            interface_type,
        }
    }

    /// Create from device index (e.g., 0 for PhysicalDrive0)
    pub fn from_index(index: u32) -> Result<Self, Error> {
        let name = format!("PhysicalDrive{}", index);
        let device_path = PathBuf::from(format!("\\\\.\\PhysicalDrive{}", index));

        // Try to detect disk type
        let disk_type = Self::detect_disk_type(index);

        Ok(Self {
            name,
            device_path,
            disk_type,
            model: None,
            serial: None,
            size: 0,
            disk_index: index,
            interface_type: None,
        })
    }

    /// Detect disk type from Windows APIs
    fn detect_disk_type(_index: u32) -> DiskType {
        // Would use IOCTL_STORAGE_QUERY_PROPERTY with StorageDeviceProperty
        // For now, default to unknown
        DiskType::Unknown
    }

    /// Read I/O statistics from WMI Performance Counters
    fn read_io_counters(&self) -> Result<(u64, u64, u64, u64), Error> {
        use serde::Deserialize;

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        #[allow(dead_code)]
        struct DiskPerf {
            name: String,
            disk_read_bytes_per_sec: u64,
            disk_write_bytes_per_sec: u64,
            disk_reads_per_sec: u32,
            disk_writes_per_sec: u32,
        }

        // Use robust WMI connection
        let wmi_con = create_wmi_connection()?;

        // Query disk performance data
        let query = format!(
            "SELECT Name, DiskReadBytesPerSec, DiskWriteBytesPerSec, DiskReadsPerSec, DiskWritesPerSec FROM Win32_PerfFormattedData_PerfDisk_PhysicalDisk WHERE Name LIKE '%{}'",
            self.name.replace("PhysicalDrive", "")
        );

        let perfs: Vec<DiskPerf> = wmi_con.raw_query(&query).unwrap_or_default();

        if let Some(perf) = perfs.first() {
            Ok((
                perf.disk_read_bytes_per_sec,
                perf.disk_write_bytes_per_sec,
                perf.disk_reads_per_sec as u64,
                perf.disk_writes_per_sec as u64,
            ))
        } else {
            // Try querying the "_Total" instance
            let total_perfs: Vec<DiskPerf> = wmi_con
                .raw_query("SELECT Name, DiskReadBytesPerSec, DiskWriteBytesPerSec, DiskReadsPerSec, DiskWritesPerSec FROM Win32_PerfFormattedData_PerfDisk_PhysicalDisk WHERE Name = '_Total'")
                .unwrap_or_default();

            if let Some(perf) = total_perfs.first() {
                Ok((
                    perf.disk_read_bytes_per_sec,
                    perf.disk_write_bytes_per_sec,
                    perf.disk_reads_per_sec as u64,
                    perf.disk_writes_per_sec as u64,
                ))
            } else {
                Ok((0, 0, 0, 0))
            }
        }
    }
}

impl DiskDevice for WindowsDisk {
    fn name(&self) -> &str {
        &self.name
    }

    fn disk_type(&self) -> DiskType {
        self.disk_type
    }

    fn info(&self) -> Result<DiskInfo, Error> {
        Ok(DiskInfo {
            name: self.name.clone(),
            model: self.model.clone().unwrap_or_else(|| "Unknown".to_string()),
            serial: self.serial.clone(),
            firmware: None,
            capacity: self.size,
            block_size: 512, // Most common
            disk_type: self.disk_type,
            interface_type: self.interface_type.clone(),
            physical_sector_size: Some(512),
            logical_sector_size: Some(512),
            rotation_rate: if matches!(self.disk_type, DiskType::NvmeSsd | DiskType::SataSsd) {
                Some(0)
            } else {
                Some(7200) // Common HDD speed
            },
            vendor: None,
        })
    }

    fn io_stats(&self) -> Result<DiskIoStats, Error> {
        let (read_bytes, write_bytes, read_ops, write_ops) = self.read_io_counters()?;

        Ok(DiskIoStats {
            read_bytes,
            write_bytes,
            read_ops,
            write_ops,
            read_time_ms: Some(0),
            write_time_ms: Some(0),
            queue_depth: Some(0),
            avg_latency_us: None,
            read_throughput: None,
            write_throughput: None,
        })
    }

    fn health(&self) -> Result<DiskHealth, Error> {
        // Would use DeviceIoControl with SMART_RCV_DRIVE_DATA
        // or WMI MSStorageDriver_FailurePredictStatus
        // For basic implementation, return Unknown
        Ok(DiskHealth::Unknown)
    }

    fn device_path(&self) -> PathBuf {
        self.device_path.clone()
    }

    fn filesystem_info(&self) -> Result<Vec<FilesystemInfo>, Error> {
        use serde::Deserialize;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
        };

        // Drive type constants
        const DRIVE_FIXED: u32 = 3;
        const DRIVE_REMOVABLE: u32 = 2;

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        #[allow(dead_code)]
        struct Win32DiskPartition {
            device_i_d: String,
            disk_index: u32,
            index: u32,
            size: Option<u64>,
        }

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        #[allow(dead_code)]
        struct Win32LogicalDiskToPartition {
            antecedent: String,
            dependent: String,
        }

        #[derive(Deserialize, Debug)]
        #[serde(rename_all = "PascalCase")]
        #[allow(dead_code)]
        struct Win32LogicalDisk {
            device_i_d: String,
            file_system: Option<String>,
            size: Option<u64>,
            free_space: Option<u64>,
            volume_name: Option<String>,
        }

        let mut filesystems = Vec::new();

        // Try WMI approach first to get partition-to-logical disk mapping
        if let Ok(wmi_con) = create_wmi_connection() {
            // Get partitions for this disk
            let partition_query = format!(
                "SELECT DeviceID, DiskIndex, Index, Size FROM Win32_DiskPartition WHERE DiskIndex = {}",
                self.disk_index
            );
            let partitions: Vec<Win32DiskPartition> =
                wmi_con.raw_query(&partition_query).unwrap_or_default();

            // Get logical disk to partition mappings
            let mappings: Vec<Win32LogicalDiskToPartition> = wmi_con
                .raw_query("SELECT Antecedent, Dependent FROM Win32_LogicalDiskToPartition")
                .unwrap_or_default();

            // Get all logical disks
            let logical_disks: Vec<Win32LogicalDisk> = wmi_con
                .raw_query("SELECT DeviceID, FileSystem, Size, FreeSpace, VolumeName FROM Win32_LogicalDisk")
                .unwrap_or_default();

            // Match partitions to logical disks
            for partition in &partitions {
                // Find mapping for this partition
                for mapping in &mappings {
                    if mapping.antecedent.contains(&partition.device_i_d) {
                        // Extract drive letter from dependent (e.g., "Win32_LogicalDisk.DeviceID=\"C:\"")
                        if let Some(start) = mapping.dependent.find("DeviceID=\"") {
                            let start = start + 10;
                            if let Some(end) = mapping.dependent[start..].find('"') {
                                let drive_letter = &mapping.dependent[start..start + end];

                                // Find the logical disk info
                                if let Some(ld) =
                                    logical_disks.iter().find(|d| d.device_i_d == drive_letter)
                                {
                                    let total_size = ld.size.unwrap_or(0);
                                    let free_space = ld.free_space.unwrap_or(0);
                                    let used_size = total_size.saturating_sub(free_space);

                                    filesystems.push(FilesystemInfo {
                                        mount_point: PathBuf::from(format!("{}\\", drive_letter)),
                                        fs_type: ld
                                            .file_system
                                            .clone()
                                            .unwrap_or_else(|| "Unknown".to_string()),
                                        total_size,
                                        used_size,
                                        available_size: free_space,
                                        total_inodes: None,
                                        used_inodes: None,
                                        read_only: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: if WMI didn't work, try Windows API directly for all drives
        // (this won't correctly map to physical disks but at least shows something)
        if filesystems.is_empty() {
            unsafe {
                let drives = GetLogicalDrives();
                for i in 0..26u32 {
                    if drives & (1 << i) != 0 {
                        let drive_letter = (b'A' + i as u8) as char;
                        let root_path: Vec<u16> = format!("{}:\\", drive_letter)
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();

                        let drive_type = GetDriveTypeW(PCWSTR(root_path.as_ptr()));

                        // Only include fixed and removable drives
                        if drive_type == DRIVE_FIXED || drive_type == DRIVE_REMOVABLE {
                            let mut free_bytes_available: u64 = 0;
                            let mut total_bytes: u64 = 0;
                            let mut total_free_bytes: u64 = 0;

                            if GetDiskFreeSpaceExW(
                                PCWSTR(root_path.as_ptr()),
                                Some(&mut free_bytes_available),
                                Some(&mut total_bytes),
                                Some(&mut total_free_bytes),
                            )
                            .is_ok()
                            {
                                // Get filesystem type
                                let mut fs_name: [u16; 256] = [0; 256];
                                let mut volume_name: [u16; 256] = [0; 256];

                                let _ = GetVolumeInformationW(
                                    PCWSTR(root_path.as_ptr()),
                                    Some(&mut volume_name),
                                    None,
                                    None,
                                    None,
                                    Some(&mut fs_name),
                                );

                                let fs_type = String::from_utf16_lossy(&fs_name)
                                    .trim_end_matches('\0')
                                    .to_string();

                                if !fs_type.is_empty() {
                                    filesystems.push(FilesystemInfo {
                                        mount_point: PathBuf::from(format!("{}:\\", drive_letter)),
                                        fs_type,
                                        total_size: total_bytes,
                                        used_size: total_bytes.saturating_sub(total_free_bytes),
                                        available_size: free_bytes_available,
                                        total_inodes: None,
                                        used_inodes: None,
                                        read_only: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(filesystems)
    }
}

/// Enumerate Windows disk devices using WMI
pub fn enumerate() -> Result<Vec<Box<dyn DiskDevice>>, Error> {
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "PascalCase")]
    #[allow(dead_code)]
    struct Win32DiskDrive {
        device_i_d: String,
        model: Option<String>,
        serial_number: Option<String>,
        size: Option<u64>,
        media_type: Option<String>,
        interface_type: Option<String>,
        index: u32,
    }

    let mut disks: Vec<Box<dyn DiskDevice>> = Vec::new();

    // Use robust WMI connection that handles GUI context
    if let Ok(wmi_con) = create_wmi_connection() {
        let wmi_disks: Vec<Win32DiskDrive> = wmi_con
            .raw_query(
                "SELECT DeviceID, Model, SerialNumber, Size, MediaType, InterfaceType, Index FROM Win32_DiskDrive",
            )
            .unwrap_or_default();

        for wmi_disk in wmi_disks {
            // Determine disk type - check multiple sources for best accuracy
            let disk_type = {
                let model_upper = wmi_disk.model.as_ref().map(|m| m.to_uppercase());
                let interface_upper = wmi_disk.interface_type.as_ref().map(|i| i.to_uppercase());

                // Check model name first (most reliable for SSDs)
                if let Some(ref model) = model_upper {
                    if model.contains("NVME")
                        || model.contains("990 PRO")
                        || model.contains("9100 PRO")
                        || model.contains("980 PRO")
                        || model.contains("970 EVO PLUS")
                    {
                        DiskType::NvmeSsd
                    } else if model.contains("SSD") || model.contains("970 EVO") {
                        // Could be SATA or NVMe SSD - check interface
                        if interface_upper.as_deref() == Some("SCSI") {
                            DiskType::NvmeSsd // SCSI interface on SSD = NVMe
                        } else {
                            DiskType::SataSsd
                        }
                    } else {
                        // Model doesn't clearly indicate SSD type
                        // Check if interface is SCSI (modern NVMe) vs IDE (SATA)
                        if interface_upper.as_deref() == Some("SCSI") {
                            // SCSI interface on modern systems usually means NVMe
                            // Check media type for additional hints
                            match wmi_disk.media_type.as_deref() {
                                Some(media) if media.contains("Fixed") => {
                                    // Fixed disk on SCSI - likely NVMe SSD
                                    DiskType::NvmeSsd
                                }
                                Some(media) if media.contains("Removable") => DiskType::Usb,
                                _ => DiskType::NvmeSsd, // Default SCSI to NVMe
                            }
                        } else {
                            // IDE interface or other
                            match wmi_disk.media_type.as_deref() {
                                Some(media) if media.contains("SSD") || media.contains("Solid") => {
                                    DiskType::SataSsd
                                }
                                Some(media) if media.contains("NVMe") => DiskType::NvmeSsd,
                                Some(media) if media.contains("Removable") => DiskType::Usb,
                                Some(media) if media.contains("Fixed") => DiskType::SataHdd,
                                _ => DiskType::Unknown,
                            }
                        }
                    }
                } else {
                    // No model - use interface and media type
                    if interface_upper.as_deref() == Some("SCSI") {
                        DiskType::NvmeSsd // SCSI without model = likely NVMe
                    } else {
                        match wmi_disk.media_type.as_deref() {
                            Some(media) if media.contains("SSD") => DiskType::SataSsd,
                            Some(media) if media.contains("NVMe") => DiskType::NvmeSsd,
                            Some(media) if media.contains("Removable") => DiskType::Usb,
                            Some(media) if media.contains("Fixed") => DiskType::SataHdd,
                            _ => DiskType::Unknown,
                        }
                    }
                }
            };

            // Format interface type for display
            let interface_type = wmi_disk.interface_type.map(|iface| {
                match iface.to_uppercase().as_str() {
                    "SCSI" => {
                        // SCSI often means NVMe on modern systems
                        if matches!(disk_type, DiskType::NvmeSsd) {
                            "NVMe (PCIe)".to_string()
                        } else {
                            "SCSI".to_string()
                        }
                    }
                    "IDE" => "SATA".to_string(),
                    "USB" => "USB".to_string(),
                    "1394" => "FireWire".to_string(),
                    other => other.to_string(),
                }
            });

            let disk = WindowsDisk::new(
                format!("PhysicalDrive{}", wmi_disk.index),
                PathBuf::from(format!("\\\\.\\PhysicalDrive{}", wmi_disk.index)),
                disk_type,
                wmi_disk.model,
                wmi_disk.serial_number.map(|s| s.trim().to_string()),
                wmi_disk.size.unwrap_or(0),
                wmi_disk.index,
                interface_type,
            );

            disks.push(Box::new(disk));
        }
    }

    // Fallback: try to enumerate physical drives directly
    if disks.is_empty() {
        for index in 0..8 {
            match WindowsDisk::from_index(index) {
                Ok(disk) => {
                    use std::fs::OpenOptions;
                    use std::os::windows::fs::OpenOptionsExt;

                    const FILE_FLAG_NO_BUFFERING: u32 = 0x20000000;
                    const FILE_SHARE_READ: u32 = 0x00000001;
                    const FILE_SHARE_WRITE: u32 = 0x00000002;

                    let device_path = format!("\\\\.\\PhysicalDrive{}", index);

                    if let Ok(_file) = OpenOptions::new()
                        .read(true)
                        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                        .custom_flags(FILE_FLAG_NO_BUFFERING)
                        .open(&device_path)
                    {
                        disks.push(Box::new(disk));
                    }
                }
                Err(_) => continue,
            }
        }
    }

    // If no disks found, return error
    if disks.is_empty() {
        return Err(Error::NotFound);
    }

    Ok(disks)
}

/// Enhanced disk monitor with caching
pub struct WindowsDiskMonitor {
    disks: HashMap<String, WindowsDisk>,
}

impl WindowsDiskMonitor {
    pub fn new() -> Result<Self, Error> {
        let disks_vec = enumerate()?;
        let mut disks = HashMap::new();

        for (i, disk) in disks_vec.iter().enumerate() {
            disks.insert(
                disk.name().to_string(),
                WindowsDisk {
                    name: disk.name().to_string(),
                    device_path: disk.device_path(),
                    disk_type: disk.disk_type(),
                    model: None,
                    serial: None,
                    size: 0,
                    disk_index: i as u32,
                    interface_type: None,
                },
            );
        }

        Ok(Self { disks })
    }

    pub fn disks(&self) -> Vec<&WindowsDisk> {
        self.disks.values().collect()
    }

    pub fn disk_by_name(&self, name: &str) -> Option<&WindowsDisk> {
        self.disks.get(name)
    }
}

impl Default for WindowsDiskMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            disks: HashMap::new(),
        })
    }
}

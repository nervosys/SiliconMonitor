//! Unified traits and types for disk monitoring

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Disk device trait - common interface for all storage devices
pub trait DiskDevice: Send + Sync {
    /// Get device name (e.g., "nvme0n1", "sda", "PhysicalDrive0")
    fn name(&self) -> &str;

    /// Get disk type (NVMe, SATA SSD, HDD, etc.)
    fn disk_type(&self) -> DiskType;

    /// Get static device information
    fn info(&self) -> Result<DiskInfo, Error>;

    /// Get current I/O statistics
    fn io_stats(&self) -> Result<DiskIoStats, Error>;

    /// Get SMART attributes (if supported)
    fn smart_info(&self) -> Result<SmartInfo, Error> {
        Err(Error::NotSupported)
    }

    /// Get NVMe-specific information (if applicable)
    fn nvme_info(&self) -> Result<NvmeInfo, Error> {
        Err(Error::NotSupported)
    }

    /// Get filesystem information for mounted devices
    fn filesystem_info(&self) -> Result<Vec<FilesystemInfo>, Error> {
        Ok(Vec::new())
    }

    /// Get current temperature in Celsius (if available)
    fn temperature(&self) -> Result<Option<f32>, Error> {
        Ok(None)
    }

    /// Get overall health status
    fn health(&self) -> Result<DiskHealth, Error>;

    /// Get device path (e.g., "/dev/nvme0n1", "\\\\.\\PhysicalDrive0")
    fn device_path(&self) -> PathBuf;
}

/// Disk type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiskType {
    /// NVMe SSD
    NvmeSsd,
    /// SATA SSD
    SataSsd,
    /// SATA HDD
    SataHdd,
    /// SCSI device
    Scsi,
    /// USB-attached storage
    Usb,
    /// Virtual disk (VM, cloud)
    Virtual,
    /// Unknown type
    Unknown,
}

/// Static disk information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    /// Device name
    pub name: String,
    /// Device model
    pub model: String,
    /// Serial number
    pub serial: Option<String>,
    /// Firmware version
    pub firmware: Option<String>,
    /// Total capacity in bytes
    pub capacity: u64,
    /// Block size in bytes
    pub block_size: u32,
    /// Disk type
    pub disk_type: DiskType,
    /// Interface type (e.g., "NVMe", "SATA", "USB", "SCSI", "PCIe")
    pub interface_type: Option<String>,
    /// Physical sector size in bytes
    pub physical_sector_size: Option<u32>,
    /// Logical sector size in bytes
    pub logical_sector_size: Option<u32>,
    /// Rotation speed (RPM) for HDDs
    pub rotation_rate: Option<u32>,
    /// Vendor
    pub vendor: Option<String>,
}

/// I/O Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoStats {
    /// Total bytes read since boot
    pub read_bytes: u64,
    /// Total bytes written since boot
    pub write_bytes: u64,
    /// Total read operations
    pub read_ops: u64,
    /// Total write operations
    pub write_ops: u64,
    /// Time spent reading (milliseconds)
    pub read_time_ms: Option<u64>,
    /// Time spent writing (milliseconds)
    pub write_time_ms: Option<u64>,
    /// Current queue depth
    pub queue_depth: Option<u32>,
    /// Average I/O latency (microseconds)
    pub avg_latency_us: Option<f64>,
    /// Read throughput (bytes/sec) - calculated from recent samples
    pub read_throughput: Option<u64>,
    /// Write throughput (bytes/sec) - calculated from recent samples
    pub write_throughput: Option<u64>,
}

impl DiskIoStats {
    /// Calculate IOPS (operations per second) - requires delta calculation
    pub fn total_ops(&self) -> u64 {
        self.read_ops + self.write_ops
    }

    /// Calculate total bytes transferred
    pub fn total_bytes(&self) -> u64 {
        self.read_bytes + self.write_bytes
    }
}

/// SMART Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartInfo {
    /// Overall SMART health status
    pub passed: bool,
    /// Individual SMART attributes
    pub attributes: Vec<SmartAttribute>,
    /// Temperature from SMART (Celsius)
    pub temperature: Option<f32>,
    /// Power-on hours
    pub power_on_hours: Option<u64>,
    /// Power cycle count
    pub power_cycle_count: Option<u64>,
    /// Reallocated sectors count
    pub reallocated_sectors: Option<u64>,
    /// Pending sector count
    pub pending_sectors: Option<u64>,
    /// Uncorrectable sector count
    pub uncorrectable_sectors: Option<u64>,
}

/// Individual SMART attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAttribute {
    /// Attribute ID
    pub id: u8,
    /// Attribute name
    pub name: String,
    /// Current value (0-255)
    pub value: u8,
    /// Worst value seen (0-255)
    pub worst: u8,
    /// Threshold value
    pub threshold: u8,
    /// Raw value (interpretation varies by attribute)
    pub raw_value: u64,
    /// Whether this attribute is critical
    pub critical: bool,
}

/// NVMe-specific information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmeInfo {
    /// Controller model
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Firmware revision
    pub firmware: String,
    /// NVMe version (e.g., "1.4")
    pub nvme_version: String,
    /// Total NVM capacity (bytes)
    pub total_capacity: u64,
    /// Unallocated capacity (bytes)
    pub unallocated_capacity: u64,
    /// Controller ID
    pub controller_id: u16,
    /// Number of namespaces
    pub num_namespaces: u32,
    /// Temperature sensors (Celsius)
    pub temperature_sensors: Vec<f32>,
    /// Current power state
    pub power_state: u8,
    /// Available power states
    pub available_power_states: Vec<NvmePowerState>,
    /// Percentage used (wear indicator, 0-100)
    pub percentage_used: Option<u8>,
    /// Data units read (512-byte units)
    pub data_units_read: Option<u64>,
    /// Data units written (512-byte units)
    pub data_units_written: Option<u64>,
    /// Host read commands
    pub host_read_commands: Option<u64>,
    /// Host write commands
    pub host_write_commands: Option<u64>,
    /// Critical warnings (bit flags)
    pub critical_warnings: u8,
}

/// NVMe power state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmePowerState {
    /// Power state number
    pub state: u8,
    /// Maximum power in watts
    pub max_power_watts: f32,
    /// Entry latency in microseconds
    pub entry_latency_us: u32,
    /// Exit latency in microseconds
    pub exit_latency_us: u32,
}

/// Filesystem information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemInfo {
    /// Mount point
    pub mount_point: PathBuf,
    /// Filesystem type (ext4, ntfs, apfs, etc.)
    pub fs_type: String,
    /// Total size in bytes
    pub total_size: u64,
    /// Used space in bytes
    pub used_size: u64,
    /// Available space in bytes
    pub available_size: u64,
    /// Total inodes (Unix-like systems)
    pub total_inodes: Option<u64>,
    /// Used inodes
    pub used_inodes: Option<u64>,
    /// Read-only flag
    pub read_only: bool,
}

impl FilesystemInfo {
    /// Calculate usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.total_size == 0 {
            0.0
        } else {
            (self.used_size as f64 / self.total_size as f64 * 100.0) as f32
        }
    }
}

/// Overall disk health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskHealth {
    /// Healthy, no issues detected
    Healthy,
    /// Warning - some metrics are concerning
    Warning,
    /// Critical - imminent failure likely
    Critical,
    /// Failed - disk has failed
    Failed,
    /// Unknown - cannot determine health
    Unknown,
}

/// Per-process disk I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDiskIo {
    /// Process ID
    pub pid: u32,
    /// Bytes read
    pub read_bytes: u64,
    /// Bytes written
    pub write_bytes: u64,
    /// Read syscalls
    pub read_syscalls: u64,
    /// Write syscalls
    pub write_syscalls: u64,
    /// Cancelled write bytes (Linux)
    pub cancelled_write_bytes: Option<u64>,
}

// === Error Types ===

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Operation not supported on this device")]
    NotSupported,

    #[error("No disk devices found")]
    NoDevicesFound,

    #[error("Device not found")]
    NotFound,

    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Failed to query device: {0}")]
    QueryFailed(String),

    #[error("Insufficient permissions: {0}")]
    PermissionDenied(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// SMART attribute IDs (common across vendors)
pub mod smart_ids {
    /// Raw read error rate
    pub const READ_ERROR_RATE: u8 = 0x01;
    /// Throughput performance
    pub const THROUGHPUT_PERFORMANCE: u8 = 0x02;
    /// Spin-up time
    pub const SPIN_UP_TIME: u8 = 0x03;
    /// Start/Stop count
    pub const START_STOP_COUNT: u8 = 0x04;
    /// Reallocated sectors count
    pub const REALLOCATED_SECTORS: u8 = 0x05;
    /// Seek error rate
    pub const SEEK_ERROR_RATE: u8 = 0x07;
    /// Seek time performance
    pub const SEEK_TIME_PERFORMANCE: u8 = 0x08;
    /// Power-on hours
    pub const POWER_ON_HOURS: u8 = 0x09;
    /// Spin retry count
    pub const SPIN_RETRY_COUNT: u8 = 0x0A;
    /// Recalibration retries
    pub const CALIBRATION_RETRY_COUNT: u8 = 0x0B;
    /// Power cycle count
    pub const POWER_CYCLE_COUNT: u8 = 0x0C;
    /// Current pending sector count
    pub const PENDING_SECTORS: u8 = 0xC5;
    /// Offline uncorrectable sector count
    pub const UNCORRECTABLE_SECTORS: u8 = 0xC6;
    /// UltraDMA CRC error count
    pub const UDMA_CRC_ERROR: u8 = 0xC7;
    /// Temperature (Celsius)
    pub const TEMPERATURE: u8 = 0xC2;
    /// Hardware ECC recovered
    pub const HARDWARE_ECC_RECOVERED: u8 = 0xC3;
    /// Reallocation event count
    pub const REALLOCATION_EVENTS: u8 = 0xC4;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // === DiskIoStats tests ===

    #[test]
    fn test_disk_io_total_ops() {
        let stats = DiskIoStats {
            read_bytes: 1000,
            write_bytes: 2000,
            read_ops: 100,
            write_ops: 200,
            read_time_ms: None,
            write_time_ms: None,
            queue_depth: None,
            avg_latency_us: None,
            read_throughput: None,
            write_throughput: None,
        };
        assert_eq!(stats.total_ops(), 300);
    }

    #[test]
    fn test_disk_io_total_bytes() {
        let stats = DiskIoStats {
            read_bytes: 1_000_000,
            write_bytes: 2_000_000,
            read_ops: 0,
            write_ops: 0,
            read_time_ms: None,
            write_time_ms: None,
            queue_depth: None,
            avg_latency_us: None,
            read_throughput: None,
            write_throughput: None,
        };
        assert_eq!(stats.total_bytes(), 3_000_000);
    }

    #[test]
    fn test_disk_io_zero() {
        let stats = DiskIoStats {
            read_bytes: 0,
            write_bytes: 0,
            read_ops: 0,
            write_ops: 0,
            read_time_ms: None,
            write_time_ms: None,
            queue_depth: None,
            avg_latency_us: None,
            read_throughput: None,
            write_throughput: None,
        };
        assert_eq!(stats.total_ops(), 0);
        assert_eq!(stats.total_bytes(), 0);
    }

    // === FilesystemInfo tests ===

    #[test]
    fn test_filesystem_usage_percent() {
        let fs = FilesystemInfo {
            mount_point: PathBuf::from("/"),
            fs_type: "ext4".to_string(),
            total_size: 1_000_000_000,
            used_size: 500_000_000,
            available_size: 500_000_000,
            total_inodes: None,
            used_inodes: None,
            read_only: false,
        };
        assert!((fs.usage_percent() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_filesystem_usage_percent_zero_total() {
        let fs = FilesystemInfo {
            mount_point: PathBuf::from("/"),
            fs_type: "tmpfs".to_string(),
            total_size: 0,
            used_size: 0,
            available_size: 0,
            total_inodes: None,
            used_inodes: None,
            read_only: false,
        };
        assert_eq!(fs.usage_percent(), 0.0);
    }

    #[test]
    fn test_filesystem_usage_percent_full() {
        let fs = FilesystemInfo {
            mount_point: PathBuf::from("/data"),
            fs_type: "ntfs".to_string(),
            total_size: 1_000_000,
            used_size: 1_000_000,
            available_size: 0,
            total_inodes: None,
            used_inodes: None,
            read_only: true,
        };
        assert!((fs.usage_percent() - 100.0).abs() < 0.01);
    }

    // === DiskHealth tests ===

    #[test]
    fn test_disk_health_equality() {
        assert_eq!(DiskHealth::Healthy, DiskHealth::Healthy);
        assert_ne!(DiskHealth::Healthy, DiskHealth::Warning);
        assert_ne!(DiskHealth::Warning, DiskHealth::Critical);
    }

    // === DiskType tests ===

    #[test]
    fn test_disk_type_equality() {
        assert_eq!(DiskType::NvmeSsd, DiskType::NvmeSsd);
        assert_ne!(DiskType::NvmeSsd, DiskType::SataHdd);
    }

    // === smart_ids constants ===

    #[test]
    fn test_smart_ids_values() {
        assert_eq!(smart_ids::POWER_ON_HOURS, 0x09);
        assert_eq!(smart_ids::TEMPERATURE, 0xC2);
        assert_eq!(smart_ids::REALLOCATED_SECTORS, 0x05);
        assert_eq!(smart_ids::PENDING_SECTORS, 0xC5);
    }
}

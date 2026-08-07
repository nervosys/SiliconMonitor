//! Disk Monitoring Module
//!
//! Provides unified disk monitoring across multiple platforms and storage types:
//! - NVMe SSDs (temperature, endurance, power states)
//! - SATA SSDs/HDDs (SMART attributes, temperature)
//! - SCSI devices
//! - Virtual/cloud storage
//!
//! Platform support:
//! - Linux: sysfs, ioctl, nvme-cli integration
//! - Windows: WMI, DeviceIoControl
//! - macOS: IOKit, diskutil

pub mod traits;

// Not target-gated. The parsers are pure byte arithmetic over structures the NVMe
// specification defines identically everywhere, so gating them to Windows would
// mean their tests only ever ran on one of the three platforms CI covers.
pub mod nvme_log;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
mod windows_nvme;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-export commonly used types
pub use traits::{
    DiskDevice, DiskHealth, DiskInfo, DiskIoStats, DiskType, Error, FilesystemInfo, NvmeInfo,
    SmartAttribute, SmartInfo,
};

/// Enumerate all disk devices in the system
pub fn enumerate_disks() -> Result<Vec<Box<dyn DiskDevice>>, Error> {
    #[cfg(target_os = "linux")]
    {
        linux::enumerate()
    }

    #[cfg(target_os = "windows")]
    {
        windows::enumerate()
    }

    #[cfg(target_os = "macos")]
    {
        macos::enumerate()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "Disk monitoring not supported on this platform".to_string(),
        ))
    }
}

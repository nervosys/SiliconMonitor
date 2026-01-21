//! Linux disk monitoring via sysfs, ioctl, and procfs

use crate::disk::traits::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Linux disk device implementation
pub struct LinuxDisk {
    name: String,
    device_path: PathBuf,
    disk_type: DiskType,
    major: u32,
    minor: u32,
}

impl LinuxDisk {
    /// Create a new Linux disk device
    pub fn new(name: String) -> Result<Self, Error> {
        let device_path = PathBuf::from(format!("/dev/{}", name));

        // Read device major/minor numbers
        let dev_path = format!("/sys/block/{}/dev", name);
        let dev_content = fs::read_to_string(&dev_path)
            .map_err(|e| Error::QueryFailed(format!("Failed to read {}: {}", dev_path, e)))?;

        let parts: Vec<&str> = dev_content.trim().split(':').collect();
        let major = parts[0].parse().unwrap_or(0);
        let minor = parts[1].parse().unwrap_or(0);

        // Determine disk type
        let disk_type = Self::detect_disk_type(&name)?;

        Ok(Self {
            name,
            device_path,
            disk_type,
            major,
            minor,
        })
    }

    fn detect_disk_type(name: &str) -> Result<DiskType, Error> {
        // NVMe devices: nvme*
        if name.starts_with("nvme") {
            return Ok(DiskType::NvmeSsd);
        }

        // Check if it's a rotational device (HDD)
        let rotational_path = format!("/sys/block/{}/queue/rotational", name);
        if let Ok(content) = fs::read_to_string(&rotational_path) {
            if content.trim() == "1" {
                return Ok(DiskType::SataHdd);
            } else {
                return Ok(DiskType::SataSsd);
            }
        }

        // SCSI devices
        if name.starts_with("sd") {
            // Could be SSD or HDD, default to SSD if we can't determine
            return Ok(DiskType::SataSsd);
        }

        Ok(DiskType::Unknown)
    }

    fn read_sysfs_string(&self, attribute: &str) -> Result<String, Error> {
        let path = format!("/sys/block/{}/{}", self.name, attribute);
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| Error::QueryFailed(format!("Failed to read {}: {}", path, e)))
    }

    fn read_sysfs_u64(&self, attribute: &str) -> Result<u64, Error> {
        self.read_sysfs_string(attribute)?
            .parse()
            .map_err(|e| Error::ParseError(format!("Failed to parse {}: {}", attribute, e)))
    }
}

impl DiskDevice for LinuxDisk {
    fn name(&self) -> &str {
        &self.name
    }

    fn disk_type(&self) -> DiskType {
        self.disk_type
    }

    fn info(&self) -> Result<DiskInfo, Error> {
        // Read model
        let model = self
            .read_sysfs_string("device/model")
            .unwrap_or_else(|_| "Unknown".to_string());

        // Read vendor
        let vendor = self.read_sysfs_string("device/vendor").ok();

        // Read firmware
        let firmware = self.read_sysfs_string("device/rev").ok();

        // Read capacity (in 512-byte sectors)
        let sectors = self.read_sysfs_u64("size")?;
        let capacity = sectors * 512;

        // Read queue info
        let logical_block_size = self
            .read_sysfs_u64("queue/logical_block_size")
            .unwrap_or(512) as u32;
        let physical_block_size = self
            .read_sysfs_u64("queue/physical_block_size")
            .unwrap_or(512) as u32;

        // Read rotation rate (0 = SSD, >0 = HDD RPM)
        let rotation_rate = self.read_sysfs_u64("queue/rotational").ok().and_then(|r| {
            if r > 0 {
                Some(r as u32)
            } else {
                None
            }
        });

        Ok(DiskInfo {
            name: self.name.clone(),
            model,
            serial: None, // Would need ioctl or smartctl
            firmware,
            capacity,
            block_size: logical_block_size,
            disk_type: self.disk_type,
            interface_type: Some(match self.disk_type {
                DiskType::NvmeSsd => "NVMe (PCIe)".to_string(),
                DiskType::SataSsd | DiskType::SataHdd => "SATA".to_string(),
                DiskType::Usb => "USB".to_string(),
                DiskType::Scsi => "SCSI".to_string(),
                DiskType::Virtual => "Virtual".to_string(),
                DiskType::Unknown => "Unknown".to_string(),
            }),
            physical_sector_size: Some(physical_block_size),
            logical_sector_size: Some(logical_block_size),
            rotation_rate,
            vendor,
        })
    }

    fn io_stats(&self) -> Result<DiskIoStats, Error> {
        let stat_path = format!("/sys/block/{}/stat", self.name);
        let stat_content = fs::read_to_string(&stat_path)
            .map_err(|e| Error::QueryFailed(format!("Failed to read {}: {}", stat_path, e)))?;

        // Format: read_ios read_merges read_sectors read_ticks write_ios write_merges write_sectors write_ticks in_flight io_ticks time_in_queue
        let parts: Vec<&str> = stat_content.split_whitespace().collect();
        if parts.len() < 11 {
            return Err(Error::ParseError("Invalid stat format".to_string()));
        }

        let read_ops: u64 = parts[0].parse().unwrap_or(0);
        let read_sectors: u64 = parts[2].parse().unwrap_or(0);
        let read_time_ms: u64 = parts[3].parse().unwrap_or(0);
        let write_ops: u64 = parts[4].parse().unwrap_or(0);
        let write_sectors: u64 = parts[6].parse().unwrap_or(0);
        let write_time_ms: u64 = parts[7].parse().unwrap_or(0);
        let in_flight: u32 = parts[8].parse().unwrap_or(0);

        Ok(DiskIoStats {
            read_bytes: read_sectors * 512,
            write_bytes: write_sectors * 512,
            read_ops,
            write_ops,
            read_time_ms: Some(read_time_ms),
            write_time_ms: Some(write_time_ms),
            queue_depth: Some(in_flight),
            avg_latency_us: None,  // Would need to calculate from deltas
            read_throughput: None, // Would need historical data
            write_throughput: None,
        })
    }

    fn temperature(&self) -> Result<Option<f32>, Error> {
        // For NVMe devices, check hwmon
        if self.disk_type == DiskType::NvmeSsd {
            let hwmon_path = format!("/sys/block/{}/device/hwmon", self.name);
            if let Ok(entries) = fs::read_dir(&hwmon_path) {
                for entry in entries.flatten() {
                    let temp_path = entry.path().join("temp1_input");
                    if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                        if let Ok(temp_millicelsius) = temp_str.trim().parse::<i32>() {
                            return Ok(Some(temp_millicelsius as f32 / 1000.0));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn health(&self) -> Result<DiskHealth, Error> {
        // Basic health check - would need SMART data for comprehensive check
        // For now, just check if device is accessible
        if self.device_path.exists() {
            Ok(DiskHealth::Healthy)
        } else {
            Ok(DiskHealth::Unknown)
        }
    }

    fn device_path(&self) -> PathBuf {
        self.device_path.clone()
    }

    fn filesystem_info(&self) -> Result<Vec<FilesystemInfo>, Error> {
        let mut filesystems = Vec::new();

        // Read /proc/mounts
        let mounts = fs::read_to_string("/proc/mounts")
            .map_err(|e| Error::QueryFailed(format!("Failed to read /proc/mounts: {}", e)))?;

        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let device = parts[0];
            let mount_point = parts[1];
            let fs_type = parts[2];

            // Check if this mount is for our device or a partition
            if device.contains(&self.name) || device == self.device_path.to_str().unwrap_or("") {
                // Get filesystem stats using statvfs
                if let Ok(stat) = nix::sys::statvfs::statvfs(mount_point) {
                    let total_size = stat.blocks() * stat.block_size();
                    let available_size = stat.blocks_available() * stat.block_size();
                    let free_size = stat.blocks_free() * stat.block_size();
                    let used_size = total_size - free_size;

                    filesystems.push(FilesystemInfo {
                        mount_point: PathBuf::from(mount_point),
                        fs_type: fs_type.to_string(),
                        total_size,
                        used_size,
                        available_size,
                        total_inodes: Some(stat.files()),
                        used_inodes: Some(stat.files() - stat.files_free()),
                        read_only: stat.flags().contains(nix::sys::statvfs::FsFlags::ST_RDONLY),
                    });
                }
            }
        }

        Ok(filesystems)
    }
}

/// Enumerate all block devices
pub fn enumerate() -> Result<Vec<Box<dyn DiskDevice>>, Error> {
    let mut devices = Vec::new();

    // Read /sys/block for all block devices
    let sys_block = Path::new("/sys/block");
    if !sys_block.exists() {
        return Err(Error::NoDevicesFound);
    }

    for entry in fs::read_dir(sys_block)
        .map_err(|e| Error::QueryFailed(format!("Failed to read /sys/block: {}", e)))?
    {
        let entry =
            entry.map_err(|e| Error::QueryFailed(format!("Failed to read entry: {}", e)))?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip loop devices, ram disks, etc.
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
            continue;
        }

        // Create device
        match LinuxDisk::new(name.clone()) {
            Ok(disk) => devices.push(Box::new(disk) as Box<dyn DiskDevice>),
            Err(e) => eprintln!("Warning: Failed to initialize disk {}: {}", name, e),
        }
    }

    if devices.is_empty() {
        return Err(Error::NoDevicesFound);
    }

    Ok(devices)
}

/// Get per-process I/O stats from /proc/[pid]/io
pub fn get_process_io(pid: u32) -> Result<ProcessDiskIo, Error> {
    let io_path = format!("/proc/{}/io", pid);
    let content = fs::read_to_string(&io_path)
        .map_err(|e| Error::QueryFailed(format!("Failed to read {}: {}", io_path, e)))?;

    let mut read_bytes = 0;
    let mut write_bytes = 0;
    let mut read_syscalls = 0;
    let mut write_syscalls = 0;
    let mut cancelled_write_bytes = None;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let value: u64 = parts[1].parse().unwrap_or(0);
        match parts[0] {
            "rchar:" => read_bytes = value,
            "wchar:" => write_bytes = value,
            "syscr:" => read_syscalls = value,
            "syscw:" => write_syscalls = value,
            "cancelled_write_bytes:" => cancelled_write_bytes = Some(value),
            _ => {}
        }
    }

    Ok(ProcessDiskIo {
        pid,
        read_bytes,
        write_bytes,
        read_syscalls,
        write_syscalls,
        cancelled_write_bytes,
    })
}

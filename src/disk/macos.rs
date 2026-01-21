//! macOS disk monitoring via IOKit and diskutil

use crate::disk::traits::*;
use std::path::PathBuf;
use std::process::Command;

/// macOS disk device implementation
pub struct MacDisk {
    name: String,
    device_path: PathBuf,
    disk_type: DiskType,
    model: Option<String>,
    serial: Option<String>,
    size_bytes: Option<u64>,
}

impl MacDisk {
    pub fn new(name: String, device_path: PathBuf, disk_type: DiskType) -> Self {
        Self {
            name,
            device_path,
            disk_type,
            model: None,
            serial: None,
            size_bytes: None,
        }
    }

    pub fn with_info(
        mut self,
        model: Option<String>,
        serial: Option<String>,
        size: Option<u64>,
    ) -> Self {
        self.model = model;
        self.serial = serial;
        self.size_bytes = size;
        self
    }
}

impl DiskDevice for MacDisk {
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
            capacity: self.size_bytes.unwrap_or(0),
            block_size: 512,
            disk_type: self.disk_type,
            interface_type: Some(match self.disk_type {
                DiskType::NvmeSsd => "NVMe (PCIe)".to_string(),
                DiskType::SataSsd | DiskType::SataHdd => "SATA".to_string(),
                DiskType::Usb => "USB".to_string(),
                DiskType::Scsi => "SCSI".to_string(),
                DiskType::Virtual => "Virtual".to_string(),
                DiskType::Unknown => "Unknown".to_string(),
            }),
            physical_sector_size: Some(4096),
            logical_sector_size: Some(512),
            rotation_rate: if self.disk_type == DiskType::SataSsd
                || self.disk_type == DiskType::NvmeSsd
            {
                Some(0)
            } else {
                None
            },
            vendor: None,
        })
    }

    fn io_stats(&self) -> Result<DiskIoStats, Error> {
        // Use iostat to get disk statistics
        let output = Command::new("iostat")
            .args(["-d", "-K", &self.name, "1", "1"])
            .output()
            .map_err(|_| Error::ReadFailed)?;

        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = text.lines().collect();

        // iostat output format:
        // disk0           KB/t tps  MB/s
        //                 X.XX XXX  X.XX
        if lines.len() >= 3 {
            let values: Vec<&str> = lines[2].split_whitespace().collect();
            if values.len() >= 3 {
                let _kb_per_t: f64 = values[0].parse().unwrap_or(0.0);
                let tps: f64 = values[1].parse().unwrap_or(0.0);
                let mb_per_s: f64 = values[2].parse().unwrap_or(0.0);

                return Ok(DiskIoStats {
                    read_ios: 0, // Not available from basic iostat
                    write_ios: 0,
                    read_bytes: 0,
                    write_bytes: 0,
                    read_bytes_per_sec: (mb_per_s * 1024.0 * 1024.0 / 2.0) as u64, // Approximate
                    write_bytes_per_sec: (mb_per_s * 1024.0 * 1024.0 / 2.0) as u64,
                    iops: tps as u64,
                    avg_latency_us: None,
                    queue_depth: None,
                });
            }
        }

        Err(Error::ReadFailed)
    }

    fn health(&self) -> Result<DiskHealth, Error> {
        // Use diskutil to check SMART status
        let output = Command::new("diskutil")
            .args(["info", self.device_path.to_str().unwrap_or("")])
            .output()
            .map_err(|_| Error::ReadFailed)?;

        let text = String::from_utf8_lossy(&output.stdout);

        // Check for SMART Status line
        for line in text.lines() {
            if line.contains("SMART Status:") {
                if line.contains("Verified") || line.contains("OK") {
                    return Ok(DiskHealth::Good);
                } else if line.contains("Failing") {
                    return Ok(DiskHealth::Critical);
                }
            }
        }

        Ok(DiskHealth::Unknown)
    }

    fn device_path(&self) -> PathBuf {
        self.device_path.clone()
    }
}

/// Enumerate macOS disk devices using diskutil
pub fn enumerate() -> Result<Vec<Box<dyn DiskDevice>>, Error> {
    let mut disks: Vec<Box<dyn DiskDevice>> = Vec::new();

    // Use diskutil list to get disk information
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|_| Error::EnumerationFailed)?;

    // Parse simple format (non-plist) as fallback
    let output = Command::new("diskutil")
        .args(["list"])
        .output()
        .map_err(|_| Error::EnumerationFailed)?;

    let text = String::from_utf8_lossy(&output.stdout);

    // Parse diskutil list output
    // /dev/disk0 (internal):
    //    #:                       TYPE NAME                    SIZE       IDENTIFIER
    //    0:      GUID_partition_scheme                        *500.1 GB   disk0
    for line in text.lines() {
        if line.starts_with("/dev/disk")
            && !line.contains("synthesized")
            && !line.contains("disk image")
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(disk_path) = parts.first() {
                let disk_name = disk_path.trim_start_matches("/dev/");

                // Skip partitions (they have 's' suffix like disk0s1)
                if disk_name.contains('s')
                    && disk_name
                        .chars()
                        .last()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                {
                    let has_s_partition = disk_name.chars().any(|c| c == 's');
                    let after_s: String = disk_name
                        .chars()
                        .skip_while(|&c| c != 's')
                        .skip(1)
                        .collect();
                    if has_s_partition
                        && !after_s.is_empty()
                        && after_s.chars().all(|c| c.is_ascii_digit())
                    {
                        continue;
                    }
                }

                // Determine disk type from context
                let disk_type = if line.contains("internal") {
                    if line.contains("SSD") || line.contains("NVMe") || line.contains("Apple SSD") {
                        DiskType::Ssd
                    } else {
                        DiskType::Unknown
                    }
                } else if line.contains("external") {
                    DiskType::Unknown
                } else {
                    DiskType::Unknown
                };

                // Get detailed info for this disk
                let (model, serial, size) = get_disk_details(disk_path);

                let mac_disk =
                    MacDisk::new(disk_name.to_string(), PathBuf::from(disk_path), disk_type)
                        .with_info(model, serial, size);

                disks.push(Box::new(mac_disk));
            }
        }
    }

    // If no disks found via parsing, add at least disk0
    if disks.is_empty() {
        let (model, serial, size) = get_disk_details("/dev/disk0");
        let disk = MacDisk::new(
            "disk0".to_string(),
            PathBuf::from("/dev/disk0"),
            DiskType::Unknown,
        )
        .with_info(model, serial, size);
        disks.push(Box::new(disk));
    }

    Ok(disks)
}

fn get_disk_details(disk_path: &str) -> (Option<String>, Option<String>, Option<u64>) {
    let output = Command::new("diskutil")
        .args(["info", disk_path])
        .output()
        .ok();

    let mut model = None;
    let mut serial = None;
    let mut size = None;

    if let Some(output) = output {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("Device / Media Name:") {
                model = line.split(':').nth(1).map(|s| s.trim().to_string());
            } else if line.starts_with("Disk Size:") {
                // Parse "Disk Size:   500.1 GB (500107862016 Bytes)"
                if let Some(bytes_part) = line.split('(').nth(1) {
                    if let Some(bytes_str) = bytes_part.split_whitespace().next() {
                        size = bytes_str.parse().ok();
                    }
                }
            }
            // Serial number often requires root access via system_profiler
        }
    }

    (model, serial, size)
}

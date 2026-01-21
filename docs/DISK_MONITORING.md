# Disk Monitoring Implementation Guide

## Overview

Silicon Monitor (simon) provides comprehensive disk and storage monitoring across all major platforms, supporting NVMe SSDs, SATA SSDs/HDDs, and other storage devices.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│         Disk Monitoring API (Rust)                  │
│  - DiskDevice trait (unified interface)             │
│  - Platform-specific implementations                │
│  - SMART, NVMe, I/O statistics                      │
└──────────────┬──────────────────────────────────────┘
               │
       ┌───────┴────────────────────┐
       │                            │
┌──────▼──────┐            ┌────────▼────────┐
│   Linux     │            │  Windows/macOS  │
├─────────────┤            ├─────────────────┤
│ - sysfs     │            │ - WMI           │
│ - procfs    │            │ - IOKit         │
│ - ioctl     │            │ - diskutil      │
│ - nvme-cli  │            │ - DeviceIoCtl   │
└─────────────┘            └─────────────────┘
```

## Features

### ✅ Implemented (Linux)

1. **Device Enumeration**
   - Automatic detection via `/sys/block`
   - Filtering of virtual devices (loop, ram, dm-*)
   - Device type classification (NVMe, SATA SSD, SATA HDD)

2. **Device Information**
   - Model, vendor, firmware version
   - Capacity (total size)
   - Block sizes (logical, physical)
   - Rotation rate detection (HDD vs SSD)
   - Device path

3. **I/O Statistics**
   - Read/write bytes (lifetime counters)
   - Read/write operations count
   - Read/write time (milliseconds)
   - Current queue depth
   - Data from `/sys/block/*/stat`

4. **Temperature Monitoring**
   - NVMe temperature via hwmon (`/sys/block/*/device/hwmon/hwmon*/temp1_input`)
   - Returns temperature in Celsius

5. **Filesystem Information**
   - Mount point detection from `/proc/mounts`
   - Filesystem type (ext4, xfs, btrfs, etc.)
   - Space usage (total, used, available)
   - Inode statistics
   - Read-only flag

6. **Per-Process I/O**
   - Read/write bytes per process
   - Syscall counts
   - Cancelled write bytes
   - Data from `/proc/[pid]/io`

### 🚧 Partially Implemented

- **Health Status**: Basic presence check (Linux), needs SMART integration
- **SMART Attributes**: Trait defined, not yet implemented
- **NVMe Specific Info**: Trait defined, not yet implemented

### ❌ Not Yet Implemented

1. **Windows Support**
   - WMI `Win32_DiskDrive` enumeration
   - Performance counters for I/O stats
   - `DeviceIoControl` for SMART data
   - NVMe passthrough commands

2. **macOS Support**
   - IOKit `IOBlockStorageDevice` enumeration
   - IOKit statistics gathering
   - diskutil integration for SMART
   - NVMe temperature from IOKit

3. **SMART Integration**
   - SMART attribute parsing
   - Health prediction
   - Temperature from SMART
   - Wear indicators
   - Error counters

4. **NVMe Advanced Features**
   - NVMe SMART log parsing
   - Power state management
   - Namespace enumeration
   - Controller information
   - Endurance metrics

## Data Structures

### DiskInfo
Static device information:
- `name`: Device name (nvme0n1, sda, etc.)
- `model`: Device model string
- `serial`: Serial number (optional)
- `firmware`: Firmware version (optional)
- `capacity`: Total capacity in bytes
- `block_size`: Block size in bytes
- `disk_type`: NVMe SSD, SATA SSD, SATA HDD, etc.
- `physical_sector_size`: Physical sector size (optional)
- `logical_sector_size`: Logical sector size (optional)
- `rotation_rate`: RPM for HDDs (optional)
- `vendor`: Vendor name (optional)

### DiskIoStats
Real-time I/O statistics:
- `read_bytes`: Total bytes read since boot
- `write_bytes`: Total bytes written since boot
- `read_ops`: Total read operations
- `write_ops`: Total write operations
- `read_time_ms`: Time spent reading (optional)
- `write_time_ms`: Time spent writing (optional)
- `queue_depth`: Current I/O queue depth (optional)
- `avg_latency_us`: Average I/O latency (optional)
- `read_throughput`: Current read throughput (optional)
- `write_throughput`: Current write throughput (optional)

Helper methods:
- `total_ops()`: Sum of read and write operations
- `total_bytes()`: Sum of read and write bytes

### FilesystemInfo
Mounted filesystem information:
- `mount_point`: Path where filesystem is mounted
- `fs_type`: Filesystem type (ext4, ntfs, apfs, etc.)
- `total_size`: Total filesystem size in bytes
- `used_size`: Used space in bytes
- `available_size`: Available space in bytes
- `total_inodes`: Total inodes (Unix-like, optional)
- `used_inodes`: Used inodes (optional)
- `read_only`: Read-only mount flag

Helper methods:
- `usage_percent()`: Calculate usage percentage

### SmartInfo (Future)
SMART health and attributes:
- `passed`: Overall SMART health status
- `attributes`: Array of individual SMART attributes
- `temperature`: Temperature from SMART (optional)
- `power_on_hours`: Total power-on hours (optional)
- `power_cycle_count`: Power cycle count (optional)
- `reallocated_sectors`: Reallocated sector count (optional)
- `pending_sectors`: Pending sector count (optional)
- `uncorrectable_sectors`: Uncorrectable sector count (optional)

### NvmeInfo (Future)
NVMe-specific information:
- `model`: Controller model
- `serial`: Serial number
- `firmware`: Firmware revision
- `nvme_version`: NVMe protocol version
- `total_capacity`: Total NVM capacity
- `unallocated_capacity`: Unallocated capacity
- `controller_id`: Controller identifier
- `num_namespaces`: Number of namespaces
- `temperature_sensors`: Array of temperature readings
- `power_state`: Current power state
- `available_power_states`: Available power states with latency/power info
- `percentage_used`: Wear indicator (0-100%)
- `data_units_read`: Lifetime data read (512-byte units)
- `data_units_written`: Lifetime data written (512-byte units)
- `host_read_commands`: Total host read commands
- `host_write_commands`: Total host write commands
- `critical_warnings`: Critical warning flags

## Usage Examples

### Basic Disk Enumeration

```rust
use simon::disk;

// Enumerate all disks
let disks = disk::enumerate_disks()?;

for disk in disks {
    println!("{}: {} ({:?})", 
        disk.name(), 
        disk.info()?.model,
        disk.disk_type()
    );
}
```

### I/O Statistics Monitoring

```rust
use simon::disk;
use std::time::Duration;

let disks = disk::enumerate_disks()?;
let disk = &disks[0];

// Get baseline stats
let stats1 = disk.io_stats()?;
std::thread::sleep(Duration::from_secs(1));
let stats2 = disk.io_stats()?;

// Calculate throughput
let read_bytes_per_sec = stats2.read_bytes - stats1.read_bytes;
let write_bytes_per_sec = stats2.write_bytes - stats1.write_bytes;

println!("Read: {} MB/s", read_bytes_per_sec / 1_000_000);
println!("Write: {} MB/s", write_bytes_per_sec / 1_000_000);
```

### Temperature Monitoring

```rust
use simon::disk;

let disks = disk::enumerate_disks()?;

for disk in disks {
    if let Ok(Some(temp)) = disk.temperature() {
        println!("{}: {:.1}°C", disk.name(), temp);
    }
}
```

### Filesystem Information

```rust
use simon::disk;

let disks = disk::enumerate_disks()?;

for disk in disks {
    if let Ok(filesystems) = disk.filesystem_info() {
        for fs in filesystems {
            println!("{}: {} - {:.1}% used",
                fs.mount_point.display(),
                fs.fs_type,
                fs.usage_percent()
            );
        }
    }
}
```

### Per-Process I/O (Linux)

```rust
use simon::disk::linux;

// Get I/O stats for a specific process
let io = linux::get_process_io(1234)?;

println!("Process 1234:");
println!("  Read: {} MB", io.read_bytes / 1_000_000);
println!("  Write: {} MB", io.write_bytes / 1_000_000);
println!("  Syscalls: {} read, {} write", 
    io.read_syscalls, 
    io.write_syscalls
);
```

## Platform-Specific Implementation Details

### Linux

**Data Sources:**
- `/sys/block/*` - Block device information and statistics
- `/sys/block/*/device/model` - Device model
- `/sys/block/*/device/vendor` - Device vendor
- `/sys/block/*/size` - Capacity in 512-byte sectors
- `/sys/block/*/queue/*` - Queue and block size information
- `/sys/block/*/stat` - I/O statistics
- `/sys/block/*/device/hwmon/hwmon*/temp1_input` - NVMe temperature
- `/proc/mounts` - Mounted filesystems
- `/proc/[pid]/io` - Per-process I/O statistics

**Device Types:**
- NVMe: Devices matching `nvme*` pattern
- SATA SSD: `/sys/block/*/queue/rotational` = 0
- SATA HDD: `/sys/block/*/queue/rotational` = 1

**Filtering:**
- Excludes: `loop*`, `ram*`, `dm-*` (virtual devices)

**Future Enhancements:**
- `smartctl` integration for SMART data
- `nvme-cli` integration for NVMe-specific info
- ioctl calls for direct device queries
- udev integration for hotplug support

### Windows (Planned)

**Data Sources:**
- WMI `Win32_DiskDrive` - Device enumeration and info
- WMI `Win32_PerfRawData_PerfDisk_PhysicalDisk` - I/O statistics
- WMI `MSStorageDriver_FailurePredictStatus` - SMART health
- `DeviceIoControl` with `IOCTL_STORAGE_QUERY_PROPERTY` - Device properties
- `DeviceIoControl` with `IOCTL_ATA_PASS_THROUGH` - SMART attributes
- `DeviceIoControl` with `IOCTL_SCSI_PASS_THROUGH` - NVMe passthrough

**Challenges:**
- Administrator privileges required for some operations
- Different APIs for SATA vs NVMe devices
- Complex NVMe passthrough command structures

### macOS (Planned)

**Data Sources:**
- IOKit `IOBlockStorageDevice` - Device enumeration
- IOKit statistics dictionary - I/O statistics
- `diskutil info` - Device information and SMART
- `smartctl` - SMART attributes
- IOKit `AppleNVMeController` - NVMe temperature

**Challenges:**
- Requires entitlements for some IOKit operations
- SMART data parsing from diskutil output
- Different APIs for different macOS versions

## SMART Attribute Reference

Common SMART attributes (standardized IDs):

| ID   | Name                      | Description                       |
| ---- | ------------------------- | --------------------------------- |
| 0x01 | Read Error Rate           | Frequency of read errors          |
| 0x05 | Reallocated Sectors Count | Count of bad sectors remapped     |
| 0x09 | Power-On Hours            | Total hours powered on            |
| 0x0C | Power Cycle Count         | Number of power cycles            |
| 0xC2 | Temperature               | Current temperature (Celsius)     |
| 0xC3 | Hardware ECC Recovered    | Errors corrected by ECC           |
| 0xC4 | Reallocation Events       | Count of reallocation attempts    |
| 0xC5 | Current Pending Sectors   | Sectors waiting to be remapped    |
| 0xC6 | Uncorrectable Sectors     | Sectors that couldn't be remapped |
| 0xC7 | UltraDMA CRC Errors       | Interface errors during transfers |

**Critical Attributes** (often indicate impending failure):
- Reallocated Sectors (0x05)
- Current Pending Sectors (0xC5)
- Uncorrectable Sectors (0xC6)

## NVMe Log Pages

NVMe SMART / Health Information (Log Page 0x02):
- Critical warnings
- Composite temperature
- Available spare
- Available spare threshold
- Percentage used
- Data units read/written
- Host read/write commands
- Controller busy time
- Power cycles
- Power on hours
- Unsafe shutdowns
- Media errors
- Error log entries

## Performance Considerations

- **Caching**: Device info rarely changes, should be cached
- **I/O Stats**: Updated frequently (every few seconds acceptable)
- **Temperature**: Poll every 5-30 seconds
- **SMART**: Query every minute or on-demand
- **Filesystem**: Update every 10-30 seconds

## Security Considerations

- **Permissions**: Some operations require root/admin
  - SMART data (Linux: CAP_SYS_ADMIN or disk group)
  - NVMe admin commands (root/admin only)
  - Per-process I/O (read own process or CAP_SYS_PTRACE)

- **Safety**: All operations are read-only
  - No write commands implemented
  - No firmware updates
  - No secure erase

## Testing

Run the example:
```bash
# Linux (may need sudo for full functionality)
cargo run --example disk_monitor

# With elevated privileges
sudo cargo run --example disk_monitor
```

Expected output shows:
- Device name and type
- Model, vendor, capacity
- I/O statistics (read/write bytes and operations)
- Temperature (for NVMe devices)
- Mounted filesystems with usage

## Future Enhancements

1. **SMART Integration**
   - Parse smartctl output or use direct ioctl
   - Health prediction algorithms
   - Attribute change tracking

2. **NVMe Advanced Features**
   - Full NVMe admin command support
   - Namespace management
   - Telemetry log parsing
   - Firmware slot information

3. **Performance Metrics**
   - IOPS calculation (requires historical data)
   - Latency histograms
   - Queue depth tracking
   - Throughput averaging

4. **Alert System**
   - Temperature thresholds
   - SMART attribute warnings
   - Capacity warnings
   - Health degradation alerts

5. **Historical Data**
   - Time-series storage
   - Wear leveling tracking
   - Performance trend analysis

## Related Projects

- [smartmontools](https://www.smartmontools.org/) - SMART monitoring
- [nvme-cli](https://github.com/linux-nvme/nvme-cli) - NVMe management
- [iotop](https://github.com/Tomas-M/iotop) - Per-process I/O monitoring
- [iostat](https://man7.org/linux/man-pages/man1/iostat.1.html) - I/O statistics

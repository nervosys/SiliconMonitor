//! AI API Types - Data structures for tool results
//!
//! These types are designed to be serializable and provide clear, structured
//! data that AI systems can easily understand and reason about.

use serde::{Deserialize, Serialize};

/// Complete system summary - single snapshot of all key metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemSummary {
    /// Unix timestamp of this snapshot
    pub timestamp: u64,

    // GPU Summary
    /// Number of GPUs detected
    pub gpu_count: usize,
    /// Summary for each GPU
    pub gpus: Vec<GpuSummary>,

    // Process Summary
    /// Total number of processes
    pub process_count: usize,
    /// Total CPU usage across all processes
    pub total_cpu_percent: f32,
    /// Number of processes using GPU
    pub gpu_process_count: usize,
    /// Top 5 CPU-consuming processes
    pub top_cpu_processes: Vec<ProcessSummary>,
    /// Top 5 memory-consuming processes
    pub top_memory_processes: Vec<ProcessSummary>,
    /// Top 5 GPU memory-consuming processes
    pub top_gpu_processes: Vec<ProcessSummary>,

    // Memory Summary
    /// Memory information
    pub memory: Option<MemorySummary>,

    // Network Summary
    /// Number of active network interfaces
    pub active_network_interfaces: usize,
    /// Network interface summaries
    pub network_interfaces: Vec<NetworkSummary>,

    // Disk Summary
    /// Number of disks
    pub disk_count: usize,
    /// Disk summaries
    pub disks: Vec<DiskSummary>,
}

/// GPU summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSummary {
    /// GPU name/model
    pub name: String,
    /// Vendor (NVIDIA, AMD, Intel, Apple)
    pub vendor: String,
    /// Current utilization percentage
    pub utilization_percent: f32,
    /// Used memory in MB
    pub memory_used_mb: u64,
    /// Total memory in MB
    pub memory_total_mb: u64,
    /// Temperature in Celsius
    pub temperature_c: Option<i32>,
    /// Power draw in watts
    pub power_watts: Option<f32>,
}

/// Process summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSummary {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// CPU usage percentage
    pub cpu_percent: f32,
    /// Memory usage in MB
    pub memory_mb: u64,
    /// GPU memory usage in MB
    pub gpu_memory_mb: u64,
}

/// Memory summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySummary {
    /// Total RAM in MB
    pub total_mb: u64,
    /// Used RAM in MB
    pub used_mb: u64,
    /// Free RAM in MB
    pub free_mb: u64,
    /// Cached RAM in MB
    pub cached_mb: u64,
    /// Total swap in MB
    pub swap_total_mb: u64,
    /// Used swap in MB
    pub swap_used_mb: u64,
    /// Memory usage percentage
    pub usage_percent: f32,
}

/// Network interface summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSummary {
    /// Interface name
    pub name: String,
    /// Receive rate in bytes/sec
    pub rx_bytes_per_sec: u64,
    /// Transmit rate in bytes/sec
    pub tx_bytes_per_sec: u64,
    /// Total received in MB
    pub rx_total_mb: u64,
    /// Total transmitted in MB
    pub tx_total_mb: u64,
    /// Whether interface is up
    pub is_up: bool,
}

/// Disk summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSummary {
    /// Disk device name
    pub name: String,
    /// Disk model
    pub model: String,
    /// Size in GB
    pub size_gb: u64,
    /// Disk type (NVMe, SATA, etc.)
    pub disk_type: String,
    /// Temperature in Celsius
    pub temperature_c: Option<f32>,
}

/// Detailed GPU information for individual GPU queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDetails {
    /// GPU index
    pub index: usize,
    /// GPU name/model
    pub name: String,
    /// Vendor
    pub vendor: String,
    /// PCI bus info
    pub pci_info: Option<PciDetails>,
    /// Current utilization
    pub utilization: UtilizationDetails,
    /// Memory information
    pub memory: MemoryDetails,
    /// Thermal information
    pub thermal: ThermalDetails,
    /// Power information
    pub power: PowerDetails,
    /// Clock speeds
    pub clocks: Option<ClockDetails>,
    /// Driver version
    pub driver_version: Option<String>,
    /// CUDA version (NVIDIA only)
    pub cuda_version: Option<String>,
    /// Compute capability
    pub compute_capability: Option<String>,
    /// Processes using this GPU
    pub processes: Vec<GpuProcessInfo>,
}

/// PCI bus details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciDetails {
    pub bus: u32,
    pub device: u32,
    pub domain: u32,
    pub device_id: u32,
    pub vendor_id: u32,
}

/// Utilization details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilizationDetails {
    /// GPU compute utilization %
    pub gpu_percent: u32,
    /// Memory controller utilization %
    pub memory_percent: Option<u32>,
    /// Encoder utilization %
    pub encoder_percent: Option<u32>,
    /// Decoder utilization %
    pub decoder_percent: Option<u32>,
}

/// Memory details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetails {
    /// Total memory in bytes
    pub total_bytes: u64,
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Free memory in bytes
    pub free_bytes: u64,
    /// Usage percentage
    pub usage_percent: f32,
    /// Total in MB (convenience)
    pub total_mb: u64,
    /// Used in MB (convenience)
    pub used_mb: u64,
}

/// Thermal details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalDetails {
    /// Current temperature in Celsius
    pub temperature_c: Option<i32>,
    /// Slowdown threshold
    pub slowdown_threshold_c: Option<i32>,
    /// Shutdown threshold
    pub shutdown_threshold_c: Option<i32>,
    /// Maximum recorded temperature
    pub max_temperature_c: Option<i32>,
    /// Temperature status (normal, throttling, critical)
    pub status: String,
}

/// Power details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerDetails {
    /// Current power draw in watts
    pub current_watts: Option<f32>,
    /// Power limit in watts
    pub limit_watts: Option<f32>,
    /// Default power limit
    pub default_limit_watts: Option<f32>,
    /// Minimum power limit
    pub min_limit_watts: Option<f32>,
    /// Maximum power limit
    pub max_limit_watts: Option<f32>,
    /// Power state (P0, P1, etc.)
    pub power_state: Option<String>,
}

/// Clock speed details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockDetails {
    /// Graphics clock in MHz
    pub graphics_mhz: Option<u32>,
    /// Memory clock in MHz
    pub memory_mhz: Option<u32>,
    /// SM clock in MHz
    pub sm_mhz: Option<u32>,
    /// Max graphics clock
    pub max_graphics_mhz: Option<u32>,
    /// Max memory clock
    pub max_memory_mhz: Option<u32>,
}

/// GPU process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// GPU memory used in bytes
    pub gpu_memory_bytes: u64,
    /// GPU memory in MB
    pub gpu_memory_mb: u64,
    /// Process type (compute, graphics, etc.)
    pub process_type: String,
}

/// Detailed CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuDetails {
    /// CPU model name
    pub model: String,
    /// Number of physical cores
    pub physical_cores: usize,
    /// Number of logical cores
    pub logical_cores: usize,
    /// Per-core information
    pub cores: Vec<CpuCoreDetails>,
    /// Total utilization
    pub total_utilization: CpuUtilization,
    /// Overall frequency
    pub frequency: Option<FrequencyDetails>,
}

/// Per-core CPU details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCoreDetails {
    /// Core ID
    pub id: usize,
    /// Whether core is online
    pub online: bool,
    /// Governor
    pub governor: String,
    /// Frequency info
    pub frequency: Option<FrequencyDetails>,
    /// Utilization breakdown
    pub utilization: CpuUtilization,
}

/// CPU utilization breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuUtilization {
    /// User mode %
    pub user_percent: f32,
    /// System mode %
    pub system_percent: f32,
    /// Nice %
    pub nice_percent: f32,
    /// Idle %
    pub idle_percent: f32,
    /// Total usage (100 - idle)
    pub total_percent: f32,
}

/// Frequency details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyDetails {
    /// Current frequency in MHz
    pub current_mhz: u32,
    /// Minimum frequency in MHz
    pub min_mhz: u32,
    /// Maximum frequency in MHz
    pub max_mhz: u32,
}

/// Detailed process information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetails {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// Full command line
    pub command_line: Option<String>,
    /// Process state
    pub state: String,
    /// Parent PID
    pub parent_pid: Option<u32>,
    /// User owner
    pub user: Option<String>,
    /// CPU usage percent
    pub cpu_percent: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Memory usage in MB
    pub memory_mb: u64,
    /// Virtual memory in bytes
    pub virtual_memory_bytes: u64,
    /// Thread count
    pub thread_count: u32,
    /// Start time (Unix timestamp)
    pub start_time: Option<u64>,
    /// GPU information if using GPU
    pub gpu_info: Option<ProcessGpuInfo>,
}

/// Process GPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessGpuInfo {
    /// GPU indices being used
    pub gpu_indices: Vec<usize>,
    /// Total GPU memory used
    pub total_gpu_memory_bytes: u64,
    /// GPU memory in MB
    pub total_gpu_memory_mb: u64,
    /// Per-GPU breakdown
    pub per_gpu: Vec<PerGpuUsage>,
}

/// Per-GPU usage for a process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerGpuUsage {
    /// GPU index
    pub gpu_index: usize,
    /// Memory used on this GPU
    pub memory_bytes: u64,
    /// Memory in MB
    pub memory_mb: u64,
}

/// Network interface details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterfaceDetails {
    /// Interface name
    pub name: String,
    /// MAC address
    pub mac_address: Option<String>,
    /// IPv4 addresses
    pub ipv4_addresses: Vec<String>,
    /// IPv6 addresses
    pub ipv6_addresses: Vec<String>,
    /// MTU
    pub mtu: Option<u32>,
    /// Interface state
    pub is_up: bool,
    /// Running state
    pub is_running: bool,
    /// Is loopback
    pub is_loopback: bool,
    /// Link speed in Mbps
    pub speed_mbps: Option<u32>,
    /// Statistics
    pub stats: NetworkStats,
    /// Current bandwidth
    pub bandwidth: BandwidthInfo,
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Bytes received
    pub rx_bytes: u64,
    /// Bytes transmitted
    pub tx_bytes: u64,
    /// Packets received
    pub rx_packets: u64,
    /// Packets transmitted
    pub tx_packets: u64,
    /// Receive errors
    pub rx_errors: u64,
    /// Transmit errors
    pub tx_errors: u64,
    /// Dropped packets (receive)
    pub rx_dropped: u64,
    /// Dropped packets (transmit)
    pub tx_dropped: u64,
}

/// Bandwidth information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthInfo {
    /// Receive rate in bytes/sec
    pub rx_bytes_per_sec: f64,
    /// Transmit rate in bytes/sec
    pub tx_bytes_per_sec: f64,
    /// Receive rate in Mbps
    pub rx_mbps: f64,
    /// Transmit rate in Mbps
    pub tx_mbps: f64,
}

/// Disk details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskDetails {
    /// Device name
    pub name: String,
    /// Model
    pub model: String,
    /// Serial number
    pub serial: Option<String>,
    /// Firmware version
    pub firmware: Option<String>,
    /// Size in bytes
    pub size_bytes: u64,
    /// Size in GB
    pub size_gb: u64,
    /// Disk type
    pub disk_type: String,
    /// Temperature in Celsius
    pub temperature_c: Option<f32>,
    /// Health status
    pub health: Option<DiskHealthDetails>,
    /// I/O statistics
    pub io_stats: Option<DiskIoDetails>,
    /// Filesystem info
    pub filesystems: Vec<FilesystemDetails>,
}

/// Disk health details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskHealthDetails {
    /// Overall health status
    pub status: String,
    /// Power-on hours
    pub power_on_hours: Option<u64>,
    /// Power cycle count
    pub power_cycles: Option<u64>,
    /// Percentage used (SSD)
    pub percentage_used: Option<u8>,
    /// Available spare (SSD)
    pub available_spare: Option<u8>,
    /// SMART attributes
    pub smart_attributes: Vec<SmartAttributeInfo>,
}

/// SMART attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAttributeInfo {
    /// Attribute ID
    pub id: u8,
    /// Attribute name
    pub name: String,
    /// Current value
    pub value: u8,
    /// Worst value seen
    pub worst: u8,
    /// Threshold
    pub threshold: u8,
    /// Raw value
    pub raw: u64,
}

/// Disk I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskIoDetails {
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Read operations
    pub read_ops: u64,
    /// Write operations
    pub write_ops: u64,
    /// Read latency (average) in ms
    pub read_latency_ms: Option<f64>,
    /// Write latency (average) in ms
    pub write_latency_ms: Option<f64>,
}

/// Filesystem details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemDetails {
    /// Mount point
    pub mount_point: String,
    /// Filesystem type
    pub fs_type: String,
    /// Total size in bytes
    pub total_bytes: u64,
    /// Used space in bytes
    pub used_bytes: u64,
    /// Available space in bytes
    pub available_bytes: u64,
    /// Usage percentage
    pub usage_percent: f32,
}

/// Motherboard sensor reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Sensor name
    pub name: String,
    /// Sensor type
    pub sensor_type: String,
    /// Current value
    pub value: f64,
    /// Unit
    pub unit: String,
    /// Min value
    pub min: Option<f64>,
    /// Max value
    pub max: Option<f64>,
    /// Critical threshold
    pub critical: Option<f64>,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoDetails {
    /// Hostname
    pub hostname: String,
    /// OS name
    pub os_name: String,
    /// OS version
    pub os_version: String,
    /// Kernel version
    pub kernel_version: String,
    /// Architecture
    pub architecture: String,
    /// BIOS vendor
    pub bios_vendor: Option<String>,
    /// BIOS version
    pub bios_version: Option<String>,
    /// System manufacturer
    pub manufacturer: Option<String>,
    /// System model
    pub model: Option<String>,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Boot time (Unix timestamp)
    pub boot_time: u64,
}

/// Driver information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverDetails {
    /// Driver name
    pub name: String,
    /// Driver type
    pub driver_type: String,
    /// Version
    pub version: String,
    /// Provider
    pub provider: Option<String>,
    /// Date
    pub date: Option<String>,
}

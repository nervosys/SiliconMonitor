//! Unified silicon monitoring module
//!
//! This module provides comprehensive monitoring for all types of silicon:
//! - CPUs (including hybrid architectures like P/E cores)
//! - NPUs/ASICs (Neural engines, AI accelerators)
//! - I/O controllers (PCIe, NVMe, USB, Thunderbolt)
//! - Network silicon (WiFi, Ethernet, offload engines)

#[cfg(feature = "apple")]
pub mod apple;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

use crate::error::Result;

/// CPU cluster type (for hybrid architectures)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuClusterType {
    /// Performance cores (P-cores)
    Performance,
    /// Efficiency cores (E-cores)
    Efficiency,
    /// Standard cores (no hybrid architecture)
    Standard,
}

/// Per-core CPU information
#[derive(Debug, Clone)]
pub struct CpuCore {
    /// Core ID
    pub id: u32,
    /// Cluster type
    pub cluster: CpuClusterType,
    /// Current frequency in MHz
    pub frequency_mhz: u32,
    /// Utilization percentage (0-100)
    pub utilization: u8,
    /// Temperature in Celsius (if available)
    pub temperature: Option<i32>,
}

/// CPU cluster information
#[derive(Debug, Clone)]
pub struct CpuCluster {
    /// Cluster type
    pub cluster_type: CpuClusterType,
    /// Core IDs in this cluster
    pub core_ids: Vec<u32>,
    /// Average frequency in MHz
    pub frequency_mhz: u32,
    /// Average utilization percentage (0-100)
    pub utilization: u8,
    /// Power consumption in watts (if available)
    pub power_watts: Option<f32>,
}

/// NPU/Neural Engine information
#[derive(Debug, Clone)]
pub struct NpuInfo {
    /// NPU name (e.g., "Apple Neural Engine", "Intel AI Boost")
    pub name: String,
    /// Vendor
    pub vendor: String,
    /// Core count (if applicable)
    pub cores: Option<u32>,
    /// Utilization percentage (0-100)
    pub utilization: u8,
    /// Power consumption in watts (if available)
    pub power_watts: Option<f32>,
    /// Frequency in MHz (if available)
    pub frequency_mhz: Option<u32>,
}

/// I/O controller information
#[derive(Debug, Clone)]
pub struct IoController {
    /// Controller type (e.g., "PCIe", "NVMe", "USB", "Thunderbolt")
    pub controller_type: String,
    /// Controller name
    pub name: String,
    /// Current bandwidth in MB/s
    pub bandwidth_mbps: f64,
    /// Maximum bandwidth in MB/s
    pub max_bandwidth_mbps: f64,
    /// Power consumption in watts (if available)
    pub power_watts: Option<f32>,
}

/// Network silicon information
#[derive(Debug, Clone)]
pub struct NetworkSilicon {
    /// Interface name (e.g., "WiFi", "Ethernet")
    pub interface: String,
    /// Link speed in Mbps
    pub link_speed_mbps: u32,
    /// RX bandwidth in MB/s
    pub rx_bandwidth_mbps: f64,
    /// TX bandwidth in MB/s
    pub tx_bandwidth_mbps: f64,
    /// Packet rate (packets/sec)
    pub packet_rate: u64,
    /// Power state (if available)
    pub power_state: Option<String>,
}

/// Comprehensive silicon snapshot
#[derive(Debug, Clone)]
pub struct SiliconSnapshot {
    /// CPU cores
    pub cpu_cores: Vec<CpuCore>,
    /// CPU clusters
    pub cpu_clusters: Vec<CpuCluster>,
    /// NPU/Neural engines
    pub npus: Vec<NpuInfo>,
    /// I/O controllers
    pub io_controllers: Vec<IoController>,
    /// Network silicon
    pub network: Vec<NetworkSilicon>,
}

/// Silicon monitor trait
pub trait SiliconMonitor {
    /// Get CPU information (cores and clusters)
    fn cpu_info(&self) -> Result<(Vec<CpuCore>, Vec<CpuCluster>)>;

    /// Get NPU information
    fn npu_info(&self) -> Result<Vec<NpuInfo>>;

    /// Get I/O controller information
    fn io_info(&self) -> Result<Vec<IoController>>;

    /// Get network silicon information
    fn network_info(&self) -> Result<Vec<NetworkSilicon>>;
}

//! Windows silicon monitoring
//!
//! Comprehensive hardware monitoring for Windows using WMI and Performance Counters

use super::*;
use crate::error::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(target_os = "windows")]
use serde::Deserialize;
#[cfg(target_os = "windows")]
use wmi::{COMLibrary, WMIConnection};

#[cfg(target_os = "windows")]
use ::windows::Win32::System::SystemInformation::*;

// WMI structures for CPU temperature and model information

/// MSAcpi_ThermalZoneTemperature from root\WMI namespace
#[cfg(target_os = "windows")]
#[derive(Deserialize, Debug)]
#[serde(rename = "MSAcpi_ThermalZoneTemperature")]
#[serde(rename_all = "PascalCase")]
#[allow(dead_code)]
struct MsAcpiThermalZoneTemperature {
    instance_name: Option<String>,
    current_temperature: Option<u32>, // In tenths of Kelvin
}

/// Win32_Processor for CPU model detection
#[cfg(target_os = "windows")]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct Win32Processor {
    name: Option<String>,
}

// Global state for per-core utilization tracking
static PREV_TOTAL_TIME: AtomicU64 = AtomicU64::new(0);
static PREV_IDLE_TIME: AtomicU64 = AtomicU64::new(0);

/// Windows silicon monitor
pub struct WindowsSiliconMonitor {
    cpu_count: usize,
    base_frequency_mhz: u32,
}

impl WindowsSiliconMonitor {
    /// Create a new Windows silicon monitor
    pub fn new() -> Result<Self> {
        let cpu_count = Self::detect_cpu_count();
        let base_frequency_mhz = Self::detect_base_frequency();

        Ok(Self {
            cpu_count,
            base_frequency_mhz,
        })
    }

    #[cfg(target_os = "windows")]
    fn detect_cpu_count() -> usize {
        unsafe {
            let mut system_info: SYSTEM_INFO = std::mem::zeroed();
            GetSystemInfo(&mut system_info);
            system_info.dwNumberOfProcessors as usize
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn detect_cpu_count() -> usize {
        1
    }

    /// Detect base CPU frequency from registry
    #[cfg(target_os = "windows")]
    fn detect_base_frequency() -> u32 {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(cpu_key) = hklm.open_subkey("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0")
        {
            if let Ok(mhz) = cpu_key.get_value::<u32, _>("~MHz") {
                return mhz;
            }
        }
        0
    }

    #[cfg(not(target_os = "windows"))]
    fn detect_base_frequency() -> u32 {
        0
    }

    /// Read current CPU frequency using CallNtPowerInformation
    #[cfg(target_os = "windows")]
    fn read_cpu_frequencies(&self) -> Vec<u32> {
        use std::mem;

        // PROCESSOR_POWER_INFORMATION structure
        #[repr(C)]
        #[derive(Clone, Default)]
        struct ProcessorPowerInformation {
            number: u32,
            max_mhz: u32,
            current_mhz: u32,
            mhz_limit: u32,
            max_idle_state: u32,
            current_idle_state: u32,
        }

        // PowerInformationLevel::ProcessorInformation = 11
        const PROCESSOR_INFORMATION: u32 = 11;

        #[link(name = "powrprof")]
        extern "system" {
            fn CallNtPowerInformation(
                InformationLevel: u32,
                InputBuffer: *const std::ffi::c_void,
                InputBufferLength: u32,
                OutputBuffer: *mut std::ffi::c_void,
                OutputBufferLength: u32,
            ) -> i32;
        }

        let buffer_size = self.cpu_count * mem::size_of::<ProcessorPowerInformation>();
        let mut buffer: Vec<ProcessorPowerInformation> =
            vec![ProcessorPowerInformation::default(); self.cpu_count];

        let result = unsafe {
            CallNtPowerInformation(
                PROCESSOR_INFORMATION,
                std::ptr::null(),
                0,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                buffer_size as u32,
            )
        };

        if result == 0 {
            // STATUS_SUCCESS
            buffer.iter().map(|p| p.current_mhz).collect()
        } else {
            // Fallback to base frequency
            vec![self.base_frequency_mhz; self.cpu_count]
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn read_cpu_frequencies(&self) -> Vec<u32> {
        vec![0; self.cpu_count]
    }

    /// Read CPU utilization using GetSystemTimes
    /// Returns overall utilization percentage
    #[cfg(target_os = "windows")]
    fn read_cpu_utilization_percent(&self) -> u8 {
        use ::windows::Win32::Foundation::FILETIME;

        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemTimes(
                lpIdleTime: *mut FILETIME,
                lpKernelTime: *mut FILETIME,
                lpUserTime: *mut FILETIME,
            ) -> i32;
        }

        let mut idle_time: FILETIME = unsafe { std::mem::zeroed() };
        let mut kernel_time: FILETIME = unsafe { std::mem::zeroed() };
        let mut user_time: FILETIME = unsafe { std::mem::zeroed() };

        let result = unsafe { GetSystemTimes(&mut idle_time, &mut kernel_time, &mut user_time) };

        if result == 0 {
            return 0;
        }

        // Convert FILETIME to u64
        let idle = ((idle_time.dwHighDateTime as u64) << 32) | (idle_time.dwLowDateTime as u64);
        let kernel =
            ((kernel_time.dwHighDateTime as u64) << 32) | (kernel_time.dwLowDateTime as u64);
        let user = ((user_time.dwHighDateTime as u64) << 32) | (user_time.dwLowDateTime as u64);

        let total = kernel + user; // kernel includes idle

        // Get previous values
        let prev_total = PREV_TOTAL_TIME.load(Ordering::Relaxed);
        let prev_idle = PREV_IDLE_TIME.load(Ordering::Relaxed);

        // Store current values
        PREV_TOTAL_TIME.store(total, Ordering::Relaxed);
        PREV_IDLE_TIME.store(idle, Ordering::Relaxed);

        // Calculate delta
        let total_delta = total.saturating_sub(prev_total);
        let idle_delta = idle.saturating_sub(prev_idle);

        if total_delta == 0 || prev_total == 0 {
            return 0; // First call or no change
        }

        let used_delta = total_delta.saturating_sub(idle_delta);
        let utilization = (used_delta as f64 / total_delta as f64 * 100.0) as u8;
        utilization.min(100)
    }

    #[cfg(not(target_os = "windows"))]
    fn read_cpu_utilization_percent(&self) -> u8 {
        0
    }

    /// Read CPU temperature using WMI
    /// Query: SELECT * FROM MSAcpi_ThermalZoneTemperature
    /// Note: Returns zone temperature (often CPU package temp), requires admin privileges
    #[cfg(target_os = "windows")]
    fn read_cpu_temperature(&self, cpu_id: u32) -> Option<i32> {
        // Cache the temperature reading (WMI queries are expensive)
        use std::sync::OnceLock;
        static CACHED_TEMPS: OnceLock<Vec<i32>> = OnceLock::new();

        let temps = CACHED_TEMPS.get_or_init(|| Self::query_thermal_zones().unwrap_or_default());

        // Return cached temperature for the CPU ID (use first zone if only one)
        temps.get(cpu_id as usize).or(temps.first()).copied()
    }

    #[cfg(target_os = "windows")]
    fn query_thermal_zones() -> Option<Vec<i32>> {
        // Initialize COM library
        let com = COMLibrary::new().ok()?;

        // Connect to root\WMI namespace (not root\cimv2)
        let wmi = WMIConnection::with_namespace_path("root\\WMI", com.into()).ok()?;

        // Query thermal zones
        let zones: Vec<MsAcpiThermalZoneTemperature> = wmi.query().ok()?;

        let temps: Vec<i32> = zones
            .iter()
            .filter_map(|z| z.current_temperature)
            .map(|temp_decikelvin| {
                // Convert from tenths of Kelvin to Celsius
                // Formula: (K/10) - 273.15 = C
                let kelvin = temp_decikelvin as f64 / 10.0;
                let celsius = kelvin - 273.15;
                celsius.round() as i32
            })
            .collect();

        if temps.is_empty() {
            None
        } else {
            Some(temps)
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn read_cpu_temperature(&self, _cpu_id: u32) -> Option<i32> {
        None
    }

    /// Read CPU utilization using Performance Counters
    fn read_cpu_utilization(&self) -> HashMap<u32, u8> {
        // Use overall system utilization for all cores (simplified)
        let overall_util = self.read_cpu_utilization_percent();
        (0..self.cpu_count as u32)
            .map(|id| (id, overall_util))
            .collect()
    }

    /// Determine if CPU has hybrid architecture (P+E cores)
    /// Intel 12th gen and later support this on Windows
    /// AMD Ryzen 9000 series with Zen 5 also has hybrid (CCD-based)
    #[allow(dead_code)]
    #[cfg(target_os = "windows")]
    fn has_hybrid_architecture(&self) -> bool {
        Self::query_cpu_model()
            .map(|name| {
                let name_lower = name.to_lowercase();
                // Intel hybrid: 12th Gen (Alder Lake) and later
                name_lower.contains("12th gen")
                    || name_lower.contains("13th gen")
                    || name_lower.contains("14th gen")
                    || name_lower.contains("core ultra")
                    // AMD Ryzen with heterogeneous CCDs
                    || (name_lower.contains("ryzen") && name_lower.contains("9"))
            })
            .unwrap_or(false)
    }

    #[cfg(target_os = "windows")]
    fn query_cpu_model() -> Option<String> {
        let com = COMLibrary::new().ok()?;
        let wmi = WMIConnection::new(com.into()).ok()?;
        let procs: Vec<Win32Processor> = wmi.query().ok()?;
        procs.first().and_then(|p| p.name.clone())
    }

    #[allow(dead_code)]
    #[cfg(not(target_os = "windows"))]
    fn has_hybrid_architecture(&self) -> bool {
        false
    }

    /// Determine cluster type for a core
    #[cfg(target_os = "windows")]
    fn determine_cluster_type(&self, cpu_id: u32) -> CpuClusterType {
        // Use GetSystemCpuSetInformation to detect P/E cores on Windows 10+
        use std::sync::OnceLock;
        static CORE_TYPES: OnceLock<Vec<CpuClusterType>> = OnceLock::new();

        let types = CORE_TYPES.get_or_init(|| {
            Self::detect_core_types()
                .unwrap_or_else(|| vec![CpuClusterType::Standard; self.cpu_count])
        });

        types
            .get(cpu_id as usize)
            .copied()
            .unwrap_or(CpuClusterType::Standard)
    }

    #[cfg(target_os = "windows")]
    fn detect_core_types() -> Option<Vec<CpuClusterType>> {
        use ::windows::Win32::Foundation::HANDLE;
        use ::windows::Win32::System::SystemInformation::{
            GetSystemCpuSetInformation, SYSTEM_CPU_SET_INFORMATION,
        };

        // First call to get required buffer size
        let mut length: u32 = 0;
        unsafe {
            let _ = GetSystemCpuSetInformation(None, 0, &mut length, HANDLE::default(), 0);
        }

        if length == 0 {
            return None;
        }

        // Allocate buffer and get info
        let count = length as usize / std::mem::size_of::<SYSTEM_CPU_SET_INFORMATION>();
        let mut buffer: Vec<SYSTEM_CPU_SET_INFORMATION> =
            vec![unsafe { std::mem::zeroed() }; count];

        let result = unsafe {
            GetSystemCpuSetInformation(
                Some(buffer.as_mut_ptr()),
                length,
                &mut length,
                HANDLE::default(),
                0,
            )
        };

        if !result.as_bool() {
            return None;
        }

        // Parse the CPU set information to detect efficiency cores
        let mut types = Vec::new();
        for info in &buffer {
            // Check EfficiencyClass: 0 = Performance, 1 = Efficiency
            let cluster_type = unsafe {
                if info.Anonymous.CpuSet.EfficiencyClass > 0 {
                    CpuClusterType::Efficiency // E-core
                } else {
                    CpuClusterType::Performance // P-core
                }
            };
            types.push(cluster_type);
        }

        if types.is_empty() {
            None
        } else {
            Some(types)
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn determine_cluster_type(&self, _cpu_id: u32) -> CpuClusterType {
        CpuClusterType::Standard
    }
}

impl SiliconMonitor for WindowsSiliconMonitor {
    fn cpu_info(&self) -> Result<(Vec<CpuCore>, Vec<CpuCluster>)> {
        let mut cores = Vec::new();
        let utilization_map = self.read_cpu_utilization();
        let frequencies = self.read_cpu_frequencies();

        for cpu_id in 0..self.cpu_count as u32 {
            let cluster = self.determine_cluster_type(cpu_id);
            let frequency = frequencies
                .get(cpu_id as usize)
                .copied()
                .unwrap_or(self.base_frequency_mhz);
            let utilization = utilization_map.get(&cpu_id).copied().unwrap_or(0);
            let temperature = self.read_cpu_temperature(cpu_id);

            cores.push(CpuCore {
                id: cpu_id,
                cluster,
                frequency_mhz: frequency,
                utilization,
                temperature,
            });
        }

        // Calculate cluster averages
        let avg_freq = if !cores.is_empty() {
            cores.iter().map(|c| c.frequency_mhz).sum::<u32>() / cores.len() as u32
        } else {
            0
        };

        let avg_util = if !cores.is_empty() {
            cores.iter().map(|c| c.utilization as u32).sum::<u32>() / cores.len() as u32
        } else {
            0
        };

        let clusters = vec![CpuCluster {
            cluster_type: CpuClusterType::Standard,
            core_ids: (0..self.cpu_count as u32).collect(),
            frequency_mhz: avg_freq,
            utilization: avg_util as u8,
            power_watts: None,
        }];

        Ok((cores, clusters))
    }

    fn npu_info(&self) -> Result<Vec<NpuInfo>> {
        // Detect Intel AI Boost (NPU) or other NPUs via WMI
        #[cfg(target_os = "windows")]
        {
            let mut npus = Vec::new();

            if let Ok(com) = wmi::COMLibrary::new() {
                if let Ok(wmi_conn) = wmi::WMIConnection::new(com) {
                    // Query PnP devices for NPU class (Processing Accelerators)
                    #[derive(serde::Deserialize, Debug)]
                    #[serde(rename_all = "PascalCase")]
                    struct PnpDevice {
                        name: Option<String>,
                        manufacturer: Option<String>,
                        status: Option<String>,
                    }

                    // PNPClass "System" devices with NPU/AI/Neural in name
                    if let Ok(devices) = wmi_conn.raw_query::<PnpDevice>(
                        "SELECT Name, Manufacturer, Status FROM Win32_PnPEntity WHERE Name LIKE '%NPU%' OR Name LIKE '%Neural%' OR Name LIKE '%AI Boost%' OR Name LIKE '%AI Accelerator%'",
                    ) {
                        for device in devices {
                            if let Some(name) = device.name {
                                let vendor = device.manufacturer.unwrap_or_else(|| {
                                    if name.to_lowercase().contains("intel") { "Intel".to_string() }
                                    else if name.to_lowercase().contains("qualcomm") { "Qualcomm".to_string() }
                                    else { "Unknown".to_string() }
                                });
                                let cores = if vendor == "Intel" { Some(16) } // Intel NPU ~16 compute units
                                    else if vendor == "Qualcomm" { Some(8) }
                                    else { None };
                                npus.push(NpuInfo {
                                    name,
                                    vendor,
                                    cores,
                                    utilization: 0, // NPU utilization requires vendor-specific APIs
                                    power_watts: None,
                                    frequency_mhz: None,
                                });
                            }
                        }
                    }
                }
            }

            return Ok(npus);
        }

        #[cfg(not(target_os = "windows"))]
        Ok(Vec::new())
    }

    fn io_info(&self) -> Result<Vec<IoController>> {
        let mut controllers = Vec::new();

        // Use WMI to get disk I/O performance data
        if let Ok(com) = wmi::COMLibrary::new() {
            if let Ok(wmi_conn) = wmi::WMIConnection::new(com) {
                // Query disk I/O performance
                #[derive(serde::Deserialize, Debug)]
                #[serde(rename_all = "PascalCase")]
                struct DiskPerf {
                    name: Option<String>,
                    disk_read_bytes_per_sec: Option<u64>,
                    disk_write_bytes_per_sec: Option<u64>,
                }

                if let Ok(disks) = wmi_conn.raw_query::<DiskPerf>(
                    "SELECT Name, DiskReadBytesPerSec, DiskWriteBytesPerSec FROM Win32_PerfFormattedData_PerfDisk_PhysicalDisk",
                ) {
                    for disk in disks {
                        if let Some(name) = disk.name {
                            // Skip the _Total aggregate
                            if name == "_Total" {
                                continue;
                            }

                            let read_mbps = disk.disk_read_bytes_per_sec.unwrap_or(0) as f64 / (1024.0 * 1024.0);
                            let write_mbps = disk.disk_write_bytes_per_sec.unwrap_or(0) as f64 / (1024.0 * 1024.0);
                            let bandwidth = read_mbps + write_mbps;

                            // Determine controller type from disk name
                            let controller_type = if name.contains("NVMe") {
                                "NVMe"
                            } else if name.contains("SSD") {
                                "SATA SSD"
                            } else {
                                "Storage"
                            }.to_string();

                            controllers.push(IoController {
                                controller_type,
                                name: name.clone(),
                                bandwidth_mbps: bandwidth,
                                max_bandwidth_mbps: 3500.0, // Assume PCIe 3.0 NVMe max
                                power_watts: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(controllers)
    }

    fn network_info(&self) -> Result<Vec<NetworkSilicon>> {
        let mut networks = Vec::new();

        // Use WMI to get network interface performance
        if let Ok(com) = wmi::COMLibrary::new() {
            if let Ok(wmi_conn) = wmi::WMIConnection::new(com) {
                // Query network adapter status
                #[derive(serde::Deserialize, Debug)]
                #[serde(rename_all = "PascalCase")]
                #[allow(dead_code)]
                struct NetAdapter {
                    name: Option<String>,
                    speed: Option<u64>,
                    net_connection_status: Option<u16>,
                }

                // Query network performance data
                #[derive(serde::Deserialize, Debug)]
                #[serde(rename_all = "PascalCase")]
                struct NetPerf {
                    name: Option<String>,
                    bytes_received_per_sec: Option<u64>,
                    bytes_sent_per_sec: Option<u64>,
                    packets_per_sec: Option<u64>,
                    current_bandwidth: Option<u64>,
                }

                // Get adapters first
                let mut adapter_speeds: HashMap<String, u64> = HashMap::new();
                if let Ok(adapters) = wmi_conn.raw_query::<NetAdapter>(
                    "SELECT Name, Speed, NetConnectionStatus FROM Win32_NetworkAdapter WHERE NetConnectionStatus = 2",
                ) {
                    for adapter in adapters {
                        if let (Some(name), Some(speed)) = (adapter.name, adapter.speed) {
                            adapter_speeds.insert(name, speed);
                        }
                    }
                }

                // Get performance data
                if let Ok(perfs) = wmi_conn.raw_query::<NetPerf>(
                    "SELECT Name, BytesReceivedPerSec, BytesSentPerSec, PacketsPerSec, CurrentBandwidth FROM Win32_PerfFormattedData_Tcpip_NetworkInterface",
                ) {
                    for perf in perfs {
                        if let Some(name) = perf.name {
                            // Skip loopback and virtual adapters
                            if name.contains("Loopback") || name.contains("Virtual") || name.contains("Hyper-V") {
                                continue;
                            }

                            let rx_mbps = perf.bytes_received_per_sec.unwrap_or(0) as f64 / (1024.0 * 1024.0);
                            let tx_mbps = perf.bytes_sent_per_sec.unwrap_or(0) as f64 / (1024.0 * 1024.0);
                            let packets = perf.packets_per_sec.unwrap_or(0);
                            let bandwidth_bps = perf.current_bandwidth.unwrap_or(0);
                            let link_speed_mbps = (bandwidth_bps / 1_000_000) as u32;

                            // Determine interface type from name
                            let interface = if name.contains("Wi-Fi") || name.contains("Wireless") || name.contains("WLAN") {
                                "WiFi"
                            } else if name.contains("Bluetooth") {
                                "Bluetooth"
                            } else if name.contains("10Gbit") || name.contains("10GbE") {
                                "10GbE"
                            } else {
                                "Ethernet"
                            }.to_string();

                            networks.push(NetworkSilicon {
                                interface,
                                link_speed_mbps,
                                rx_bandwidth_mbps: rx_mbps,
                                tx_bandwidth_mbps: tx_mbps,
                                packet_rate: packets,
                                power_state: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(networks)
    }
}

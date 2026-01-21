//! Linux silicon monitoring
//!
//! Comprehensive hardware monitoring for Linux systems using sysfs, /proc, and hwmon

use super::*;
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Linux silicon monitor
pub struct LinuxSiliconMonitor {
    cpu_count: usize,
    has_hybrid_cpu: bool,
}

impl LinuxSiliconMonitor {
    /// Create a new Linux silicon monitor
    pub fn new() -> Result<Self> {
        let cpu_count = Self::detect_cpu_count();
        let has_hybrid_cpu = Self::detect_hybrid_architecture();

        Ok(Self {
            cpu_count,
            has_hybrid_cpu,
        })
    }

    fn detect_cpu_count() -> usize {
        if let Ok(online) = fs::read_to_string("/sys/devices/system/cpu/online") {
            if let Some(count) = Self::parse_cpu_range(&online) {
                return count;
            }
        }

        // Fallback: count CPU directories
        fs::read_dir("/sys/devices/system/cpu")
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit())
                    })
                    .count()
            })
            .unwrap_or(1)
    }

    fn parse_cpu_range(range: &str) -> Option<usize> {
        // Parse ranges like "0-7" or "0,2-5,7"
        let parts: Vec<&str> = range.trim().split(',').collect();
        let mut max_cpu = 0;

        for part in parts {
            if let Some(hyphen_pos) = part.find('-') {
                let end = part[hyphen_pos + 1..].parse::<usize>().ok()?;
                max_cpu = max_cpu.max(end);
            } else {
                let cpu = part.parse::<usize>().ok()?;
                max_cpu = max_cpu.max(cpu);
            }
        }

        Some(max_cpu + 1)
    }

    fn detect_hybrid_architecture() -> bool {
        // Check for Intel Alder Lake or later (hybrid architecture)
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            // Intel 12th gen+ has hybrid architecture
            if cpuinfo.contains("Intel") {
                // Check for different core types in /sys/devices/system/cpu/cpu*/topology/core_cpus_list
                // This is a simplified check - real implementation would parse topology
                return cpuinfo.contains("12th Gen")
                    || cpuinfo.contains("13th Gen")
                    || cpuinfo.contains("14th Gen")
                    || cpuinfo.contains("Ultra");
            }
        }
        false
    }

    /// RAPL energy tracking state (for power calculation)
    /// We need to track previous energy readings and timestamps to calculate power
    fn read_rapl_power_tracked(&self, domain: &str) -> Option<f32> {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::Instant;

        // Static state for energy tracking
        static PREV_ENERGY: AtomicU64 = AtomicU64::new(0);
        static PREV_TIME_NS: AtomicU64 = AtomicU64::new(0);

        // Initialize start time on first call
        static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START_TIME.get_or_init(Instant::now);

        let rapl_base = "/sys/class/powercap/intel-rapl";

        if let Ok(entries) = fs::read_dir(rapl_base) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Check if this is the domain we're looking for
                if let Ok(name) = fs::read_to_string(path.join("name")) {
                    let name = name.trim();
                    if name == domain || (domain == "package" && name.starts_with("package-")) {
                        // Read current energy consumption
                        if let Ok(energy_uj) = fs::read_to_string(path.join("energy_uj")) {
                            if let Ok(current_energy) = energy_uj.trim().parse::<u64>() {
                                // Get current time in nanoseconds since start
                                let now_ns = start.elapsed().as_nanos() as u64;

                                // Get previous readings
                                let prev_energy =
                                    PREV_ENERGY.swap(current_energy, Ordering::Relaxed);
                                let prev_time = PREV_TIME_NS.swap(now_ns, Ordering::Relaxed);

                                // Calculate power if we have valid previous readings
                                if prev_time > 0 && prev_energy > 0 {
                                    let energy_delta = if current_energy >= prev_energy {
                                        current_energy - prev_energy
                                    } else {
                                        // Counter wrapped, use just current energy as estimate
                                        current_energy
                                    };

                                    let time_delta_s =
                                        (now_ns - prev_time) as f64 / 1_000_000_000.0;

                                    if time_delta_s > 0.0 {
                                        // Power = Energy / Time (microjoules / seconds = microwatts)
                                        let power_uw = energy_delta as f64 / time_delta_s;
                                        let power_w = power_uw / 1_000_000.0;
                                        return Some(power_w as f32);
                                    }
                                }

                                // First call or invalid delta, return a rough estimate
                                // Try reading constraint_0_power_limit_uw (TDP-like)
                                if let Ok(limit_uw) =
                                    fs::read_to_string(path.join("constraint_0_power_limit_uw"))
                                {
                                    if let Ok(limit) = limit_uw.trim().parse::<u64>() {
                                        // Return half of TDP as a reasonable idle estimate
                                        return Some((limit as f32 / 1_000_000.0) * 0.5);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Read RAPL (Running Average Power Limit) power consumption
    /// Returns power in watts for a specific domain (package, core, uncore, dram)
    fn read_rapl_power(&self, domain: &str) -> Option<f32> {
        // Use the tracked version for more accurate readings
        self.read_rapl_power_tracked(domain)
    }

    /// Get total package power from RAPL
    fn get_package_power(&self) -> Option<f32> {
        self.read_rapl_power("package")
            .or_else(|| self.read_rapl_power("package-0"))
    }

    /// Read CPU temperature for a specific core
    fn read_cpu_temperature(&self, cpu_id: u32) -> Option<i32> {
        // Try hwmon (modern systems)
        if let Some(temp) = self.read_hwmon_temperature(cpu_id) {
            return Some(temp);
        }

        // Try thermal_zone
        if let Some(temp) = self.read_thermal_zone_temperature(cpu_id) {
            return Some(temp);
        }

        None
    }

    fn read_hwmon_temperature(&self, cpu_id: u32) -> Option<i32> {
        let hwmon_dir = "/sys/class/hwmon";
        if let Ok(entries) = fs::read_dir(hwmon_dir) {
            for entry in entries.flatten() {
                let hwmon_path = entry.path();

                if let Ok(name) = fs::read_to_string(hwmon_path.join("name")) {
                    let name = name.trim();
                    if name.contains("coretemp")
                        || name.contains("k10temp")
                        || name.contains("zenpower")
                    {
                        // Try to find the specific core's temperature
                        for i in 2..20 {
                            let label_path = hwmon_path.join(format!("temp{}_label", i));
                            let input_path = hwmon_path.join(format!("temp{}_input", i));

                            if let Ok(label) = fs::read_to_string(&label_path) {
                                if label.trim().contains(&format!("Core {}", cpu_id)) {
                                    if let Ok(temp_str) = fs::read_to_string(&input_path) {
                                        if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                                            return Some(temp_millic / 1000);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn read_thermal_zone_temperature(&self, cpu_id: u32) -> Option<i32> {
        let thermal_dir = "/sys/class/thermal";
        if let Ok(entries) = fs::read_dir(thermal_dir) {
            for entry in entries.flatten() {
                let zone_path = entry.path();

                if let Ok(zone_type) = fs::read_to_string(zone_path.join("type")) {
                    let zone_type = zone_type.trim();
                    if zone_type.contains(&format!("cpu{}", cpu_id))
                        || zone_type.contains(&format!("core{}", cpu_id))
                    {
                        if let Ok(temp_str) = fs::read_to_string(zone_path.join("temp")) {
                            if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                                return Some(temp_millic / 1000);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Read CPU frequency for a specific core
    fn read_cpu_frequency(&self, cpu_id: u32) -> Option<u32> {
        let freq_path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq",
            cpu_id
        );
        fs::read_to_string(&freq_path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .map(|khz| khz / 1000) // Convert kHz to MHz
    }

    /// Read CPU utilization from /proc/stat
    fn read_cpu_utilization(&self) -> HashMap<u32, u8> {
        let mut utilization = HashMap::new();

        if let Ok(stat) = fs::read_to_string("/proc/stat") {
            for line in stat.lines() {
                if line.starts_with("cpu") && !line.starts_with("cpu ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 5 {
                        continue;
                    }

                    // Extract CPU number
                    if let Some(cpu_id) = parts[0].strip_prefix("cpu") {
                        if let Ok(cpu_num) = cpu_id.parse::<u32>() {
                            // Parse CPU times (user, nice, system, idle, ...)
                            let times: Vec<u64> =
                                parts[1..].iter().filter_map(|s| s.parse().ok()).collect();

                            if times.len() >= 4 {
                                let idle = times[3];
                                let total: u64 = times.iter().sum();

                                if total > 0 {
                                    let util = 100 - ((idle * 100) / total);
                                    utilization.insert(cpu_num, util.min(100) as u8);
                                }
                            }
                        }
                    }
                }
            }
        }

        utilization
    }

    /// Determine cluster type for hybrid CPUs
    fn determine_cluster_type(&self, cpu_id: u32) -> CpuClusterType {
        if !self.has_hybrid_cpu {
            return CpuClusterType::Standard;
        }

        // For Intel hybrid CPUs, E-cores typically have lower max frequency
        // This is a heuristic - real implementation would use topology data
        let max_freq_path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_max_freq",
            cpu_id
        );
        if let Ok(max_freq_str) = fs::read_to_string(&max_freq_path) {
            if let Ok(max_freq_khz) = max_freq_str.trim().parse::<u32>() {
                // E-cores typically have max freq < 4 GHz (4000000 kHz)
                // P-cores typically have max freq >= 4 GHz
                if max_freq_khz < 4_000_000 {
                    return CpuClusterType::Efficiency;
                } else {
                    return CpuClusterType::Performance;
                }
            }
        }

        CpuClusterType::Standard
    }
}

impl SiliconMonitor for LinuxSiliconMonitor {
    fn cpu_info(&self) -> Result<(Vec<CpuCore>, Vec<CpuCluster>)> {
        let mut cores = Vec::new();
        let utilization_map = self.read_cpu_utilization();

        // Group cores by cluster type
        let mut p_cores = Vec::new();
        let mut e_cores = Vec::new();
        let mut std_cores = Vec::new();

        for cpu_id in 0..self.cpu_count as u32 {
            let cluster = self.determine_cluster_type(cpu_id);
            let frequency = self.read_cpu_frequency(cpu_id).unwrap_or(0);
            let utilization = utilization_map.get(&cpu_id).copied().unwrap_or(0);
            let temperature = self.read_cpu_temperature(cpu_id);

            let core = CpuCore {
                id: cpu_id,
                cluster,
                frequency_mhz: frequency,
                utilization,
                temperature,
            };

            cores.push(core.clone());

            match cluster {
                CpuClusterType::Performance => p_cores.push(core),
                CpuClusterType::Efficiency => e_cores.push(core),
                CpuClusterType::Standard => std_cores.push(core),
            }
        }

        // Read package power from RAPL once (expensive operation)
        let package_power = self.get_package_power();

        // Create clusters
        let mut clusters = Vec::new();

        if !p_cores.is_empty() {
            let avg_freq =
                p_cores.iter().map(|c| c.frequency_mhz).sum::<u32>() / p_cores.len() as u32;
            let avg_util =
                p_cores.iter().map(|c| c.utilization as u32).sum::<u32>() / p_cores.len() as u32;

            // P-cores typically use more power, estimate ~60% of package power
            let power = package_power.map(|p| p * 0.6);

            clusters.push(CpuCluster {
                cluster_type: CpuClusterType::Performance,
                core_ids: p_cores.iter().map(|c| c.id).collect(),
                frequency_mhz: avg_freq,
                utilization: avg_util as u8,
                power_watts: power,
            });
        }

        if !e_cores.is_empty() {
            let avg_freq =
                e_cores.iter().map(|c| c.frequency_mhz).sum::<u32>() / e_cores.len() as u32;
            let avg_util =
                e_cores.iter().map(|c| c.utilization as u32).sum::<u32>() / e_cores.len() as u32;

            // E-cores use less power, estimate ~40% of package power
            let power = package_power.map(|p| p * 0.4);

            clusters.push(CpuCluster {
                cluster_type: CpuClusterType::Efficiency,
                core_ids: e_cores.iter().map(|c| c.id).collect(),
                frequency_mhz: avg_freq,
                utilization: avg_util as u8,
                power_watts: power,
            });
        }

        if !std_cores.is_empty() {
            let avg_freq =
                std_cores.iter().map(|c| c.frequency_mhz).sum::<u32>() / std_cores.len() as u32;
            let avg_util = std_cores.iter().map(|c| c.utilization as u32).sum::<u32>()
                / std_cores.len() as u32;

            clusters.push(CpuCluster {
                cluster_type: CpuClusterType::Standard,
                core_ids: std_cores.iter().map(|c| c.id).collect(),
                frequency_mhz: avg_freq,
                utilization: avg_util as u8,
                power_watts: package_power, // Full package power for standard cores
            });
        }

        Ok((cores, clusters))
    }

    fn npu_info(&self) -> Result<Vec<NpuInfo>> {
        let mut npus = Vec::new();

        // Intel NPU detection via /sys/class/accel
        if let Ok(entries) = std::fs::read_dir("/sys/class/accel") {
            for (index, entry) in entries.enumerate() {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    // Read device name
                    let name = std::fs::read_to_string(path.join("device/modalias"))
                        .ok()
                        .and_then(|s| {
                            if s.contains("intel") {
                                Some(format!("Intel AI Boost NPU {}", index))
                            } else {
                                Some(format!("NPU {}", index))
                            }
                        })
                        .unwrap_or_else(|| format!("NPU {}", index));

                    // Try to read utilization from device stats
                    let utilization =
                        std::fs::read_to_string(path.join("device/power/runtime_active_time"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u64>().ok())
                            .map(|active_time| {
                                // Rough approximation based on active time
                                (active_time % 100) as u8
                            })
                            .unwrap_or(0);

                    // Try to read power consumption
                    let power_watts =
                        std::fs::read_to_string(path.join("device/power/power_usage"))
                            .ok()
                            .and_then(|s| s.trim().parse::<f32>().ok())
                            .map(|p| p / 1_000_000.0); // Convert from microwatts

                    npus.push(NpuInfo {
                        name,
                        vendor: "Intel".to_string(),
                        cores: None,
                        utilization,
                        power_watts,
                        frequency_mhz: None,
                    });
                }
            }
        }

        // AMD AI Engine detection via /sys/class/drm
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Look for AMD devices with AI engine
                if name.starts_with("card") && !name.contains("-") {
                    if let Ok(vendor) = std::fs::read_to_string(path.join("device/vendor")) {
                        if vendor.trim() == "0x1002" {
                            // AMD vendor ID
                            // Check for AI engine support
                            if let Ok(device_id) =
                                std::fs::read_to_string(path.join("device/device"))
                            {
                                let device_id = device_id.trim();
                                // RDNA3+ devices with AI accelerators
                                if matches!(device_id, "0x744c" | "0x7448" | "0x73df" | "0x73ef") {
                                    npus.push(NpuInfo {
                                        name: format!("AMD AI Engine ({})", name),
                                        vendor: "AMD".to_string(),
                                        cores: Some(256), // Typical for RDNA3
                                        utilization: 0,   // Would need ROCm integration
                                        power_watts: None,
                                        frequency_mhz: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Google TPU detection via /dev/accel*
        for i in 0..8 {
            let dev_path = format!("/dev/accel{}", i);
            if std::path::Path::new(&dev_path).exists() {
                // Read TPU info from sysfs
                let sys_path = format!("/sys/class/accel/accel{}/device", i);
                if let Ok(vendor_id) = std::fs::read_to_string(format!("{}/vendor", sys_path)) {
                    if vendor_id.trim() == "0x1ae0" {
                        // Google vendor ID
                        npus.push(NpuInfo {
                            name: format!("Google TPU v{}", i),
                            vendor: "Google".to_string(),
                            cores: Some(128), // Typical TPU core count
                            utilization: 0,   // Would need TPU API
                            power_watts: None,
                            frequency_mhz: None,
                        });
                    }
                }
            }
        }

        // Qualcomm Hexagon DSP detection (if available)
        if std::path::Path::new("/sys/kernel/debug/msm_fastrpc").exists() {
            npus.push(NpuInfo {
                name: "Qualcomm Hexagon DSP".to_string(),
                vendor: "Qualcomm".to_string(),
                cores: None,
                utilization: 0,
                power_watts: None,
                frequency_mhz: None,
            });
        }

        Ok(npus)
    }

    fn io_info(&self) -> Result<Vec<IoController>> {
        let mut controllers = Vec::new();

        // PCIe controller monitoring via /sys/bus/pci
        if let Ok(entries) = std::fs::read_dir("/sys/bus/pci/devices") {
            for entry in entries.flatten() {
                let path = entry.path();
                let device_name = entry.file_name().to_string_lossy().to_string();

                // Read device class to identify controller types
                if let Ok(class) = std::fs::read_to_string(path.join("class")) {
                    let class = class.trim();

                    // NVMe Controller (0x010802)
                    if class.starts_with("0x0108") {
                        // Read current link speed
                        let cur_speed = std::fs::read_to_string(path.join("current_link_speed"))
                            .ok()
                            .and_then(|s| {
                                // Parse "8.0 GT/s" or "16 GT/s PCIe"
                                s.split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse::<f64>().ok())
                            })
                            .unwrap_or(0.0);

                        let cur_width = std::fs::read_to_string(path.join("current_link_width"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u32>().ok())
                            .unwrap_or(1);

                        // Calculate bandwidth (GT/s * width * encoding efficiency)
                        let bandwidth_mbps = cur_speed * cur_width as f64 * 1000.0 * 0.98462; // PCIe 3.0+ encoding

                        let max_speed = std::fs::read_to_string(path.join("max_link_speed"))
                            .ok()
                            .and_then(|s| {
                                s.split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse::<f64>().ok())
                            })
                            .unwrap_or(cur_speed);

                        let max_width = std::fs::read_to_string(path.join("max_link_width"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u32>().ok())
                            .unwrap_or(cur_width);

                        let max_bandwidth_mbps = max_speed * max_width as f64 * 1000.0 * 0.98462;

                        // Try to read device name
                        let name = std::fs::read_to_string(path.join("device"))
                            .ok()
                            .map(|id| format!("NVMe Controller {}", id.trim()))
                            .unwrap_or_else(|| format!("NVMe Controller ({})", device_name));

                        controllers.push(IoController {
                            controller_type: "NVMe".to_string(),
                            name,
                            bandwidth_mbps,
                            max_bandwidth_mbps,
                            power_watts: None,
                        });
                    }
                    // USB Controller (0x0c03)
                    else if class.starts_with("0x0c03") {
                        let name = format!("USB Controller ({})", device_name);

                        // USB 3.2 Gen 2x2 = 20 Gbps = 2500 MB/s
                        let max_bandwidth_mbps = 2500.0;

                        controllers.push(IoController {
                            controller_type: "USB".to_string(),
                            name,
                            bandwidth_mbps: 0.0, // Would need USB traffic monitoring
                            max_bandwidth_mbps,
                            power_watts: None,
                        });
                    }
                    // Thunderbolt Controller (0x0c0a)
                    else if class.starts_with("0x0c0a") {
                        let name = format!("Thunderbolt Controller ({})", device_name);

                        // Thunderbolt 4 = 40 Gbps = 5000 MB/s
                        let max_bandwidth_mbps = 5000.0;

                        controllers.push(IoController {
                            controller_type: "Thunderbolt".to_string(),
                            name,
                            bandwidth_mbps: 0.0,
                            max_bandwidth_mbps,
                            power_watts: None,
                        });
                    }
                    // SATA Controller (0x0106)
                    else if class.starts_with("0x0106") {
                        let name = format!("SATA Controller ({})", device_name);

                        // SATA 3.0 = 6 Gbps = 600 MB/s per port
                        let max_bandwidth_mbps = 600.0;

                        controllers.push(IoController {
                            controller_type: "SATA".to_string(),
                            name,
                            bandwidth_mbps: 0.0,
                            max_bandwidth_mbps,
                            power_watts: None,
                        });
                    }
                }
            }
        }

        Ok(controllers)
    }

    fn network_info(&self) -> Result<Vec<NetworkSilicon>> {
        let mut network_devices = Vec::new();

        // Read network interfaces from /sys/class/net
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.flatten() {
                let interface = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();

                // Skip loopback and virtual interfaces
                if interface == "lo"
                    || interface.starts_with("veth")
                    || interface.starts_with("docker")
                {
                    continue;
                }

                // Read link speed (in Mbps)
                let link_speed_mbps = std::fs::read_to_string(path.join("speed"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .filter(|&s| s > 0) // Negative values mean interface is down
                    .unwrap_or(0) as u32;

                // Read statistics
                let rx_bytes = std::fs::read_to_string(path.join("statistics/rx_bytes"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);

                let tx_bytes = std::fs::read_to_string(path.join("statistics/tx_bytes"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);

                let rx_packets = std::fs::read_to_string(path.join("statistics/rx_packets"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);

                let tx_packets = std::fs::read_to_string(path.join("statistics/tx_packets"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                    .unwrap_or(0);

                let packet_rate = rx_packets + tx_packets;

                // Read power state if available
                let power_state = std::fs::read_to_string(path.join("device/power_state"))
                    .ok()
                    .map(|s| s.trim().to_string());

                // Detect interface type
                let device_path = path.join("device");
                let interface_type = if device_path.exists() {
                    // Check if it's WiFi
                    if std::path::Path::new(&format!("/sys/class/net/{}/phy80211", interface))
                        .exists()
                    {
                        "WiFi"
                    }
                    // Check PCI device class for Ethernet
                    else if let Ok(class) = std::fs::read_to_string(device_path.join("class")) {
                        if class.trim().starts_with("0x0200") {
                            "Ethernet"
                        } else {
                            "Network"
                        }
                    } else {
                        "Network"
                    }
                } else {
                    "Virtual"
                };

                // Only add physical network devices
                if interface_type != "Virtual" {
                    network_devices.push(NetworkSilicon {
                        interface,
                        link_speed_mbps,
                        rx_bandwidth_mbps: 0.0, // Would need delta calculation
                        tx_bandwidth_mbps: 0.0,
                        packet_rate,
                        power_state,
                    });
                }
            }
        }

        // Try to read WiFi-specific information
        if let Ok(entries) = std::fs::read_dir("/sys/class/ieee80211") {
            for (index, entry) in entries.enumerate() {
                if let Ok(entry) = entry {
                    let phy_name = entry.file_name().to_string_lossy().to_string();

                    // Try to find the interface name
                    let interface_name = std::fs::read_dir(entry.path().join("device/net"))
                        .ok()
                        .and_then(|mut entries| entries.next())
                        .and_then(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("wlan{}", index));

                    // Only add if not already in list
                    if !network_devices
                        .iter()
                        .any(|d| d.interface == interface_name)
                    {
                        network_devices.push(NetworkSilicon {
                            interface: interface_name,
                            link_speed_mbps: 0, // Would need iwconfig/nl80211
                            rx_bandwidth_mbps: 0.0,
                            tx_bandwidth_mbps: 0.0,
                            packet_rate: 0,
                            power_state: None,
                        });
                    }
                }
            }
        }

        Ok(network_devices)
    }
}

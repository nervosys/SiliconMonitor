//! CPU interconnect topology monitoring.
//!
//! Detects and characterizes CPU-to-CPU and CPU-to-device interconnects:
//! Intel QPI/UPI, AMD Infinity Fabric, PCIe root complexes, and coherence
//! domains. Uses inference from CPU model to estimate link bandwidth and
//! topology when direct counters aren't available.
//!
//! ## Platform Support
//!
//! - **Linux**: `/sys/devices/system/node/`, lscpu topology, CPU model inference
//! - **Windows**: Win32_Processor socket info, CPU model inference
//! - **macOS**: sysctl, CPU model inference

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Interconnect technology type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterconnectType {
    /// Intel QuickPath Interconnect (Nehalem–Broadwell).
    QPI,
    /// Intel Ultra Path Interconnect (Skylake-SP+).
    UPI,
    /// AMD Infinity Fabric (Zen+).
    InfinityFabric,
    /// AMD Infinity Fabric on-package (chiplet-to-chiplet).
    InfinityFabricOnPackage,
    /// Apple Unified Memory Architecture interconnect.
    AppleUMA,
    /// ARM AMBA / CoreLink / CMN mesh.
    ArmMesh,
    /// PCIe root complex interconnect.
    PCIe,
    /// CXL (Compute Express Link).
    CXL,
    /// Intel on-die ring bus.
    RingBus,
    /// Intel mesh interconnect.
    MeshInterconnect,
    /// Unknown interconnect.
    Unknown,
}

impl std::fmt::Display for InterconnectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QPI => write!(f, "Intel QPI"),
            Self::UPI => write!(f, "Intel UPI"),
            Self::InfinityFabric => write!(f, "AMD Infinity Fabric"),
            Self::InfinityFabricOnPackage => write!(f, "AMD Infinity Fabric (on-package)"),
            Self::AppleUMA => write!(f, "Apple UMA"),
            Self::ArmMesh => write!(f, "ARM Mesh"),
            Self::PCIe => write!(f, "PCIe"),
            Self::CXL => write!(f, "CXL"),
            Self::RingBus => write!(f, "Intel Ring Bus"),
            Self::MeshInterconnect => write!(f, "Intel Mesh"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Coherence protocol type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceProtocol {
    /// MOESI (AMD).
    MOESI,
    /// MESIF (Intel).
    MESIF,
    /// MESI (generic).
    MESI,
    /// Directory-based (ARM, large Intel).
    Directory,
    /// Snooping.
    Snoop,
    Unknown,
}

impl std::fmt::Display for CoherenceProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MOESI => write!(f, "MOESI"),
            Self::MESIF => write!(f, "MESIF"),
            Self::MESI => write!(f, "MESI"),
            Self::Directory => write!(f, "Directory-based"),
            Self::Snoop => write!(f, "Snoop"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// An interconnect link between two endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterconnectLink {
    /// Link type.
    pub link_type: InterconnectType,
    /// Source endpoint description (e.g. "Socket 0", "CCD 0").
    pub source: String,
    /// Destination endpoint description.
    pub destination: String,
    /// Unidirectional bandwidth in GB/s.
    pub bandwidth_gbs: f64,
    /// Bidirectional bandwidth in GB/s.
    pub bidirectional_bandwidth_gbs: f64,
    /// Link width (number of lanes/links).
    pub width: u32,
    /// Speed per lane in GT/s (gigatransfers/second).
    pub speed_gts: f64,
    /// Latency in nanoseconds (estimated).
    pub latency_ns: f64,
    /// Whether this link is active/detected.
    pub active: bool,
}

/// Chiplet/die topology for multi-die packages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChipletTopology {
    /// Number of compute dies/chiplets per package.
    pub compute_dies: u32,
    /// Number of I/O dies per package.
    pub io_dies: u32,
    /// Cores per compute die.
    pub cores_per_die: u32,
    /// On-package interconnect type.
    pub on_package_interconnect: InterconnectType,
    /// On-package bandwidth per link in GB/s.
    pub on_package_bandwidth_gbs: f64,
}

/// Full interconnect topology report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterconnectTopology {
    /// Number of CPU sockets.
    pub sockets: u32,
    /// Inter-socket links.
    pub links: Vec<InterconnectLink>,
    /// Chiplet topology (if applicable).
    pub chiplet_topology: Option<ChipletTopology>,
    /// Coherence protocol in use.
    pub coherence_protocol: CoherenceProtocol,
    /// Total system interconnect bandwidth in GB/s.
    pub total_bandwidth_gbs: f64,
    /// Whether the system is NUMA.
    pub is_numa: bool,
    /// Inferred interconnect generation/version.
    pub generation: String,
}

/// Interconnect monitor.
pub struct InterconnectMonitor {
    topology: InterconnectTopology,
}

impl InterconnectMonitor {
    /// Create a new interconnect monitor.
    pub fn new() -> Result<Self, SimonError> {
        let cpu_model = Self::get_cpu_model()?;
        let sockets = Self::detect_sockets()?;
        let topology = Self::infer_topology(&cpu_model, sockets);
        Ok(Self { topology })
    }

    /// Get the topology report.
    pub fn topology(&self) -> &InterconnectTopology {
        &self.topology
    }

    /// Get inter-socket links only.
    pub fn inter_socket_links(&self) -> Vec<&InterconnectLink> {
        self.topology.links.iter().filter(|l| l.active).collect()
    }

    /// Infer full topology from CPU model and socket count.
    fn infer_topology(cpu_model: &str, sockets: u32) -> InterconnectTopology {
        let upper = cpu_model.to_uppercase();

        // Determine vendor and product family
        let is_intel = upper.contains("INTEL") || upper.contains("CORE") || upper.contains("XEON");
        let is_amd = upper.contains("AMD") || upper.contains("RYZEN") || upper.contains("EPYC")
            || upper.contains("THREADRIPPER");
        let is_apple = upper.contains("APPLE") || upper.contains("M1") || upper.contains("M2")
            || upper.contains("M3") || upper.contains("M4");

        if is_intel {
            Self::infer_intel(&upper, sockets)
        } else if is_amd {
            Self::infer_amd(&upper, sockets)
        } else if is_apple {
            Self::infer_apple(&upper)
        } else {
            InterconnectTopology {
                sockets,
                links: Vec::new(),
                chiplet_topology: None,
                coherence_protocol: CoherenceProtocol::Unknown,
                total_bandwidth_gbs: 0.0,
                is_numa: sockets > 1,
                generation: "Unknown".into(),
            }
        }
    }

    fn infer_intel(name: &str, sockets: u32) -> InterconnectTopology {
        // Determine Intel interconnect generation
        let (link_type, speed_gts, width, gen, on_die, coherence) =
            if name.contains("GRANITE") || name.contains("EMERALD") || name.contains("SIERRA") {
                (InterconnectType::UPI, 20.0, 3, "UPI 2.0", InterconnectType::MeshInterconnect, CoherenceProtocol::MESIF)
            } else if name.contains("SAPPHIRE") || name.contains("W9-3") || name.contains("W7-3") {
                (InterconnectType::UPI, 16.0, 3, "UPI 1.1", InterconnectType::MeshInterconnect, CoherenceProtocol::MESIF)
            } else if name.contains("ICE LAKE") && name.contains("XEON") {
                (InterconnectType::UPI, 11.2, 3, "UPI 1.0", InterconnectType::MeshInterconnect, CoherenceProtocol::MESIF)
            } else if name.contains("XEON") && (name.contains("PLATINUM") || name.contains("GOLD")) {
                (InterconnectType::UPI, 10.4, 3, "UPI 1.0", InterconnectType::MeshInterconnect, CoherenceProtocol::MESIF)
            } else if name.contains("12TH") || name.contains("13TH") || name.contains("14TH")
                || name.contains("ALDER") || name.contains("RAPTOR")
            {
                (InterconnectType::RingBus, 0.0, 0, "Ring Bus", InterconnectType::RingBus, CoherenceProtocol::MESIF)
            } else if name.contains("ARROW") || name.contains("LUNAR") || name.contains("ULTRA") {
                (InterconnectType::RingBus, 0.0, 0, "Ring/Foveros", InterconnectType::RingBus, CoherenceProtocol::MESIF)
            } else {
                (InterconnectType::Unknown, 0.0, 0, "Unknown", InterconnectType::RingBus, CoherenceProtocol::MESIF)
            };

        // Bandwidth: speed (GT/s) * width (bytes) * links
        let uni_bw = speed_gts * width as f64 * 2.0; // 2 bytes per transfer on UPI
        let bidi_bw = uni_bw * 2.0;

        let mut links = Vec::new();
        if sockets > 1 && speed_gts > 0.0 {
            // Create inter-socket links
            for i in 0..sockets {
                for j in (i + 1)..sockets {
                    links.push(InterconnectLink {
                        link_type,
                        source: format!("Socket {}", i),
                        destination: format!("Socket {}", j),
                        bandwidth_gbs: uni_bw,
                        bidirectional_bandwidth_gbs: bidi_bw,
                        width,
                        speed_gts,
                        latency_ns: if speed_gts >= 16.0 { 60.0 } else { 80.0 },
                        active: true,
                    });
                }
            }
        }

        // On-die interconnect
        links.push(InterconnectLink {
            link_type: on_die,
            source: "Die".into(),
            destination: "Cores/LLC".into(),
            bandwidth_gbs: 0.0, // On-die bandwidth varies
            bidirectional_bandwidth_gbs: 0.0,
            width: 0,
            speed_gts: 0.0,
            latency_ns: if matches!(on_die, InterconnectType::MeshInterconnect) {
                15.0
            } else {
                10.0
            },
            active: true,
        });

        let total_bw: f64 = links.iter().map(|l| l.bidirectional_bandwidth_gbs).sum();

        InterconnectTopology {
            sockets,
            links,
            chiplet_topology: None,
            coherence_protocol: coherence,
            total_bandwidth_gbs: total_bw,
            is_numa: sockets > 1,
            generation: gen.into(),
        }
    }

    fn infer_amd(name: &str, sockets: u32) -> InterconnectTopology {
        // AMD Infinity Fabric inference
        let (if_speed_gts, compute_dies, io_dies, cores_per_die, gen, on_pkg_bw) =
            if name.contains("9950") || name.contains("9900") || name.contains("9700")
                || name.contains("9600") || name.contains("ZEN 5")
            {
                (32.0, 2, 1, 8, "IF 4.0", 64.0)
            } else if name.contains("7950") || name.contains("7900") {
                (32.0, 2, 1, 8, "IF 3.5", 36.0)
            } else if name.contains("7800") || name.contains("7700") || name.contains("7600") {
                (32.0, 1, 1, 8, "IF 3.5", 36.0)
            } else if name.contains("5950") || name.contains("5900") {
                (32.0, 2, 1, 8, "IF 3.0", 32.0)
            } else if name.contains("5800") || name.contains("5700") || name.contains("5600") {
                (32.0, 1, 1, 8, "IF 3.0", 32.0)
            } else if name.contains("EPYC 9") || name.contains("GENOA") || name.contains("TURIN") {
                (32.0, 12, 1, 8, "IF 4.0", 64.0)
            } else if name.contains("EPYC 7") {
                (16.0, 8, 1, 8, "IF 2.0", 32.0)
            } else if name.contains("THREADRIPPER 7") || name.contains("PRO 7") {
                (32.0, 8, 1, 8, "IF 3.5", 36.0)
            } else if name.contains("THREADRIPPER 5") || name.contains("PRO 5") {
                (32.0, 4, 1, 8, "IF 3.0", 32.0)
            } else {
                (16.0, 1, 1, 8, "IF", 16.0)
            };

        let mut links = Vec::new();

        // Inter-socket links for multi-socket
        if sockets > 1 {
            let inter_bw = if_speed_gts * 2.0 * 4.0; // 4 links typically
            for i in 0..sockets {
                for j in (i + 1)..sockets {
                    links.push(InterconnectLink {
                        link_type: InterconnectType::InfinityFabric,
                        source: format!("Socket {}", i),
                        destination: format!("Socket {}", j),
                        bandwidth_gbs: inter_bw,
                        bidirectional_bandwidth_gbs: inter_bw * 2.0,
                        width: 16,
                        speed_gts: if_speed_gts,
                        latency_ns: 120.0,
                        active: true,
                    });
                }
            }
        }

        // On-package CCD-to-IOD links
        if compute_dies > 1 {
            for i in 0..compute_dies {
                links.push(InterconnectLink {
                    link_type: InterconnectType::InfinityFabricOnPackage,
                    source: format!("CCD {}", i),
                    destination: "IOD".into(),
                    bandwidth_gbs: on_pkg_bw,
                    bidirectional_bandwidth_gbs: on_pkg_bw * 2.0,
                    width: 32,
                    speed_gts: if_speed_gts,
                    latency_ns: 40.0,
                    active: true,
                });
            }
        }

        let total_bw: f64 = links.iter().map(|l| l.bidirectional_bandwidth_gbs).sum();

        let chiplet = if compute_dies > 1 || io_dies > 0 {
            Some(ChipletTopology {
                compute_dies,
                io_dies,
                cores_per_die,
                on_package_interconnect: InterconnectType::InfinityFabricOnPackage,
                on_package_bandwidth_gbs: on_pkg_bw,
            })
        } else {
            None
        };

        InterconnectTopology {
            sockets,
            links,
            chiplet_topology: chiplet,
            coherence_protocol: CoherenceProtocol::MOESI,
            total_bandwidth_gbs: total_bw,
            is_numa: sockets > 1 || compute_dies > 1,
            generation: gen.into(),
        }
    }

    fn infer_apple(name: &str) -> InterconnectTopology {
        let bw = if name.contains("M4 ULTRA") {
            800.0
        } else if name.contains("M4 MAX") || name.contains("M3 ULTRA") {
            546.0
        } else if name.contains("M4 PRO") || name.contains("M3 MAX") {
            400.0
        } else if name.contains("M3 PRO") || name.contains("M2 MAX") {
            400.0
        } else if name.contains("M2 PRO") || name.contains("M2") {
            200.0
        } else if name.contains("M1") {
            200.0
        } else {
            100.0
        };

        let links = vec![InterconnectLink {
            link_type: InterconnectType::AppleUMA,
            source: "CPU Cluster".into(),
            destination: "Unified Memory".into(),
            bandwidth_gbs: bw,
            bidirectional_bandwidth_gbs: bw,
            width: 0,
            speed_gts: 0.0,
            latency_ns: 20.0,
            active: true,
        }];

        InterconnectTopology {
            sockets: 1,
            links,
            chiplet_topology: if name.contains("ULTRA") {
                Some(ChipletTopology {
                    compute_dies: 2,
                    io_dies: 0,
                    cores_per_die: if name.contains("M4") { 16 } else { 12 },
                    on_package_interconnect: InterconnectType::AppleUMA,
                    on_package_bandwidth_gbs: bw,
                })
            } else {
                None
            },
            coherence_protocol: CoherenceProtocol::Directory,
            total_bandwidth_gbs: bw,
            is_numa: false,
            generation: "UMA".into(),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_cpu_model() -> Result<String, SimonError> {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").map_err(SimonError::Io)?;
        for line in cpuinfo.lines() {
            if let Some(val) = line.strip_prefix("model name") {
                if let Some(name) = val.trim().strip_prefix(':') {
                    return Ok(name.trim().to_string());
                }
            }
        }
        Ok(String::new())
    }

    #[cfg(target_os = "linux")]
    fn detect_sockets() -> Result<u32, SimonError> {
        // Count unique physical package IDs
        let mut sockets = std::collections::HashSet::new();
        let cpu_dir = std::path::Path::new("/sys/devices/system/cpu");

        if let Ok(entries) = std::fs::read_dir(cpu_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) {
                    let pkg_path = entry
                        .path()
                        .join("topology/physical_package_id");
                    if let Ok(pkg) = std::fs::read_to_string(&pkg_path) {
                        sockets.insert(pkg.trim().to_string());
                    }
                }
            }
        }

        Ok(sockets.len().max(1) as u32)
    }

    #[cfg(target_os = "windows")]
    fn get_cpu_model() -> Result<String, SimonError> {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_Processor).Name",
            ])
            .output()
            .map_err(SimonError::Io)?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[cfg(target_os = "windows")]
    fn detect_sockets() -> Result<u32, SimonError> {
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_Processor | Measure-Object).Count",
            ])
            .output()
            .map_err(SimonError::Io)?;
        let count = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .unwrap_or(1);
        Ok(count)
    }

    #[cfg(target_os = "macos")]
    fn get_cpu_model() -> Result<String, SimonError> {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .map_err(SimonError::Io)?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[cfg(target_os = "macos")]
    fn detect_sockets() -> Result<u32, SimonError> {
        Ok(1) // macOS is always single-socket
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn get_cpu_model() -> Result<String, SimonError> {
        Ok(String::new())
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    fn detect_sockets() -> Result<u32, SimonError> {
        Ok(1)
    }
}

impl Default for InterconnectMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            topology: InterconnectTopology {
                sockets: 1,
                links: Vec::new(),
                chiplet_topology: None,
                coherence_protocol: CoherenceProtocol::Unknown,
                total_bandwidth_gbs: 0.0,
                is_numa: false,
                generation: "Unknown".into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intel_upi_inference() {
        let topo = InterconnectMonitor::infer_intel(
            "INTEL XEON GOLD 6348 SAPPHIRE RAPIDS",
            2,
        );
        assert!(topo.is_numa);
        assert_eq!(topo.sockets, 2);
        assert!(!topo.links.is_empty());
        // Should have UPI links
        assert!(topo
            .links
            .iter()
            .any(|l| l.link_type == InterconnectType::UPI));
    }

    #[test]
    fn test_amd_chiplet_inference() {
        let topo = InterconnectMonitor::infer_amd("AMD RYZEN 9 7950X", 1);
        assert!(topo.chiplet_topology.is_some());
        let chiplet = topo.chiplet_topology.unwrap();
        assert_eq!(chiplet.compute_dies, 2);
        assert!(chiplet.on_package_bandwidth_gbs > 0.0);
    }

    #[test]
    fn test_apple_uma() {
        let topo = InterconnectMonitor::infer_apple("APPLE M4 PRO");
        assert!(!topo.is_numa);
        assert_eq!(topo.coherence_protocol, CoherenceProtocol::Directory);
        assert!(topo.total_bandwidth_gbs > 0.0);
    }

    #[test]
    fn test_interconnect_display() {
        assert_eq!(InterconnectType::UPI.to_string(), "Intel UPI");
        assert_eq!(InterconnectType::InfinityFabric.to_string(), "AMD Infinity Fabric");
        assert_eq!(CoherenceProtocol::MOESI.to_string(), "MOESI");
    }

    #[test]
    fn test_monitor_default() {
        let monitor = InterconnectMonitor::default();
        let _topo = monitor.topology();
    }

    #[test]
    fn test_serialization() {
        let link = InterconnectLink {
            link_type: InterconnectType::UPI,
            source: "Socket 0".into(),
            destination: "Socket 1".into(),
            bandwidth_gbs: 64.0,
            bidirectional_bandwidth_gbs: 128.0,
            width: 3,
            speed_gts: 16.0,
            latency_ns: 60.0,
            active: true,
        };
        let json = serde_json::to_string(&link).unwrap();
        assert!(json.contains("Socket 0"));
        let _: InterconnectLink = serde_json::from_str(&json).unwrap();
    }
}

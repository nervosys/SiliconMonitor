//! CPU cache topology detection — L1/L2/L3 sizes, line sizes, associativity.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/devices/system/cpu/cpu0/cache/`
//! - **Windows**: Uses WMI (`Win32_CacheMemory`) or `GetLogicalProcessorInformationEx`
//! - **macOS**: Uses `sysctl hw.cacheconfig`, `hw.l1dcachesize`, etc.
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::cpu_cache::CpuCacheMonitor;
//!
//! let monitor = CpuCacheMonitor::new().unwrap();
//! for cache in monitor.caches() {
//!     println!("{} ({}): {} KB, {}-way, {}-byte lines",
//!         cache.level, cache.cache_type, cache.size_kb, cache.associativity, cache.line_size);
//! }
//! println!("Total L3: {} KB", monitor.total_l3_kb());
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Cache level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CacheLevel {
    L1,
    L2,
    L3,
    L4,
}

/// Cache type (data, instruction, or unified)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheType {
    /// Data cache only
    Data,
    /// Instruction cache only
    Instruction,
    /// Unified data + instruction
    Unified,
    /// Unknown type
    Unknown,
}

/// Information about a single CPU cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCacheInfo {
    /// Cache level (L1, L2, L3, L4)
    pub level: CacheLevel,
    /// Cache type (Data, Instruction, Unified)
    pub cache_type: CacheType,
    /// Cache size in KiB
    pub size_kb: u64,
    /// Cache line size in bytes
    pub line_size: u32,
    /// Set associativity (0 = fully associative)
    pub associativity: u32,
    /// Number of sets
    pub sets: u64,
    /// Number of physical partitions
    pub partitions: u32,
    /// Which CPU cores share this cache (e.g., "0-3")
    pub shared_cpu_list: String,
    /// Cache index within the topology
    pub index: u32,
}

/// CPU cache topology information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCacheTopology {
    /// All detected cache instances
    pub caches: Vec<CpuCacheInfo>,
    /// Total L1 data cache across all cores (KiB)
    pub total_l1d_kb: u64,
    /// Total L1 instruction cache across all cores (KiB)
    pub total_l1i_kb: u64,
    /// Total L2 cache (KiB)
    pub total_l2_kb: u64,
    /// Total L3 cache (KiB)
    pub total_l3_kb: u64,
}

/// Monitor for CPU cache topology
pub struct CpuCacheMonitor {
    topology: CpuCacheTopology,
}

impl CpuCacheMonitor {
    /// Create a new CpuCacheMonitor and detect cache topology.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            topology: CpuCacheTopology {
                caches: Vec::new(),
                total_l1d_kb: 0,
                total_l1i_kb: 0,
                total_l2_kb: 0,
                total_l3_kb: 0,
            },
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh cache detection.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.topology.caches.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        self.compute_totals();
        Ok(())
    }

    /// Get all detected caches.
    pub fn caches(&self) -> &[CpuCacheInfo] {
        &self.topology.caches
    }

    /// Get full topology info.
    pub fn topology(&self) -> &CpuCacheTopology {
        &self.topology
    }

    /// Total L1 data cache in KiB.
    pub fn total_l1d_kb(&self) -> u64 {
        self.topology.total_l1d_kb
    }

    /// Total L2 cache in KiB.
    pub fn total_l2_kb(&self) -> u64 {
        self.topology.total_l2_kb
    }

    /// Total L3 cache in KiB.
    pub fn total_l3_kb(&self) -> u64 {
        self.topology.total_l3_kb
    }

    /// Get caches at a specific level.
    pub fn caches_at_level(&self, level: CacheLevel) -> Vec<&CpuCacheInfo> {
        self.topology
            .caches
            .iter()
            .filter(|c| c.level == level)
            .collect()
    }

    fn compute_totals(&mut self) {
        // Deduplicate by shared_cpu_list to avoid counting the same cache twice
        let mut seen_l3: std::collections::HashSet<String> = std::collections::HashSet::new();

        self.topology.total_l1d_kb = 0;
        self.topology.total_l1i_kb = 0;
        self.topology.total_l2_kb = 0;
        self.topology.total_l3_kb = 0;

        for cache in &self.topology.caches {
            match (cache.level, cache.cache_type) {
                (CacheLevel::L1, CacheType::Data) => {
                    self.topology.total_l1d_kb += cache.size_kb;
                }
                (CacheLevel::L1, CacheType::Instruction) => {
                    self.topology.total_l1i_kb += cache.size_kb;
                }
                (CacheLevel::L2, _) => {
                    self.topology.total_l2_kb += cache.size_kb;
                }
                (CacheLevel::L3, _) => {
                    if seen_l3.insert(cache.shared_cpu_list.clone()) {
                        self.topology.total_l3_kb += cache.size_kb;
                    }
                }
                _ => {}
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        // Read from cpu0's cache hierarchy (representative)
        let cpu_base = std::path::Path::new("/sys/devices/system/cpu/cpu0/cache");
        if !cpu_base.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(cpu_base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("index") {
                    continue;
                }

                let idx: u32 = name
                    .strip_prefix("index")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let base = entry.path();

                let level_str = Self::read_trimmed(&base.join("level"));
                let level = match level_str.as_str() {
                    "1" => CacheLevel::L1,
                    "2" => CacheLevel::L2,
                    "3" => CacheLevel::L3,
                    "4" => CacheLevel::L4,
                    _ => continue,
                };

                let type_str = Self::read_trimmed(&base.join("type"));
                let cache_type = match type_str.to_lowercase().as_str() {
                    "data" => CacheType::Data,
                    "instruction" => CacheType::Instruction,
                    "unified" => CacheType::Unified,
                    _ => CacheType::Unknown,
                };

                let size_str = Self::read_trimmed(&base.join("size"));
                let size_kb = Self::parse_size_kb(&size_str);

                let line_size: u32 = Self::read_trimmed(&base.join("coherency_line_size"))
                    .parse()
                    .unwrap_or(64);

                let associativity: u32 = Self::read_trimmed(&base.join("ways_of_associativity"))
                    .parse()
                    .unwrap_or(0);

                let sets: u64 = Self::read_trimmed(&base.join("number_of_sets"))
                    .parse()
                    .unwrap_or(0);

                let partitions: u32 = Self::read_trimmed(&base.join("physical_line_partition"))
                    .parse()
                    .unwrap_or(1);

                let shared_cpu_list = Self::read_trimmed(&base.join("shared_cpu_list"));

                self.topology.caches.push(CpuCacheInfo {
                    level,
                    cache_type,
                    size_kb,
                    line_size,
                    associativity,
                    sets,
                    partitions,
                    shared_cpu_list,
                    index: idx,
                });
            }
        }

        self.topology.caches.sort_by_key(|c| (c.level, c.index));
    }

    #[cfg(target_os = "linux")]
    fn read_trimmed(path: &std::path::Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[cfg(target_os = "linux")]
    fn parse_size_kb(s: &str) -> u64 {
        let s = s.trim();
        if let Some(kb) = s.strip_suffix('K') {
            kb.parse().unwrap_or(0)
        } else if let Some(mb) = s.strip_suffix('M') {
            mb.parse::<u64>().unwrap_or(0) * 1024
        } else {
            s.parse().unwrap_or(0)
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_CacheMemory | Select-Object Purpose, InstalledSize, CacheSpeed, Level, Associativity, LineSize, NumberOfBlocks, Status | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for (i, item) in items.iter().enumerate() {
                        let purpose = item["Purpose"].as_str().unwrap_or("");
                        let wmi_level = item["Level"].as_u64().unwrap_or(0);
                        // WMI Level: 3=L1, 4=L2, 5=L3 (CIM enumeration)
                        let level = match wmi_level {
                            3 => CacheLevel::L1,
                            4 => CacheLevel::L2,
                            5 => CacheLevel::L3,
                            _ => {
                                // Infer from purpose string
                                let p = purpose.to_lowercase();
                                if p.contains("l1") { CacheLevel::L1 }
                                else if p.contains("l2") { CacheLevel::L2 }
                                else if p.contains("l3") { CacheLevel::L3 }
                                else { CacheLevel::L2 }
                            }
                        };

                        let cache_type = {
                            let p = purpose.to_lowercase();
                            if p.contains("data") {
                                CacheType::Data
                            } else if p.contains("instruction") || p.contains("code") {
                                CacheType::Instruction
                            } else {
                                CacheType::Unified
                            }
                        };

                        let size_kb = item["InstalledSize"].as_u64().unwrap_or(0);
                        let line_size = item["LineSize"].as_u64().unwrap_or(64) as u32;
                        let assoc = item["Associativity"].as_u64().unwrap_or(0) as u32;

                        self.topology.caches.push(CpuCacheInfo {
                            level,
                            cache_type,
                            size_kb,
                            line_size,
                            associativity: assoc,
                            sets: 0,
                            partitions: 1,
                            shared_cpu_list: String::new(),
                            index: i as u32,
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        let read_sysctl = |name: &str| -> u64 {
            std::process::Command::new("sysctl")
                .args(["-n", name])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
        };

        // L1 data cache
        let l1d = read_sysctl("hw.l1dcachesize");
        if l1d > 0 {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L1,
                cache_type: CacheType::Data,
                size_kb: l1d / 1024,
                line_size: read_sysctl("hw.cachelinesize") as u32,
                associativity: 0,
                sets: 0,
                partitions: 1,
                shared_cpu_list: String::new(),
                index: 0,
            });
        }

        // L1 instruction cache
        let l1i = read_sysctl("hw.l1icachesize");
        if l1i > 0 {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L1,
                cache_type: CacheType::Instruction,
                size_kb: l1i / 1024,
                line_size: read_sysctl("hw.cachelinesize") as u32,
                associativity: 0,
                sets: 0,
                partitions: 1,
                shared_cpu_list: String::new(),
                index: 1,
            });
        }

        // L2 cache
        let l2 = read_sysctl("hw.l2cachesize");
        if l2 > 0 {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L2,
                cache_type: CacheType::Unified,
                size_kb: l2 / 1024,
                line_size: read_sysctl("hw.cachelinesize") as u32,
                associativity: 0,
                sets: 0,
                partitions: 1,
                shared_cpu_list: String::new(),
                index: 2,
            });
        }

        // L3 cache
        let l3 = read_sysctl("hw.l3cachesize");
        if l3 > 0 {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L3,
                cache_type: CacheType::Unified,
                size_kb: l3 / 1024,
                line_size: read_sysctl("hw.cachelinesize") as u32,
                associativity: 0,
                sets: 0,
                partitions: 1,
                shared_cpu_list: String::new(),
                index: 3,
            });
        }
    }
}

impl Default for CpuCacheMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            topology: CpuCacheTopology {
                caches: Vec::new(),
                total_l1d_kb: 0,
                total_l1i_kb: 0,
                total_l2_kb: 0,
                total_l3_kb: 0,
            },
        })
    }
}

impl std::fmt::Display for CacheLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L1 => write!(f, "L1"),
            Self::L2 => write!(f, "L2"),
            Self::L3 => write!(f, "L3"),
            Self::L4 => write!(f, "L4"),
        }
    }
}

impl std::fmt::Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data => write!(f, "Data"),
            Self::Instruction => write!(f, "Instruction"),
            Self::Unified => write!(f, "Unified"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_monitor_creation() {
        let monitor = CpuCacheMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_cache_monitor_default() {
        let monitor = CpuCacheMonitor::default();
        let _ = monitor.caches();
        let _ = monitor.topology();
    }

    #[test]
    fn test_cache_level_display() {
        assert_eq!(CacheLevel::L1.to_string(), "L1");
        assert_eq!(CacheLevel::L3.to_string(), "L3");
    }

    #[test]
    fn test_cache_serialization() {
        let cache = CpuCacheInfo {
            level: CacheLevel::L2,
            cache_type: CacheType::Unified,
            size_kb: 256,
            line_size: 64,
            associativity: 8,
            sets: 512,
            partitions: 1,
            shared_cpu_list: "0-1".into(),
            index: 0,
        };
        let json = serde_json::to_string(&cache).unwrap();
        assert!(json.contains("256"));
        let _: CpuCacheInfo = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_topology_serialization() {
        let topo = CpuCacheTopology {
            caches: Vec::new(),
            total_l1d_kb: 128,
            total_l1i_kb: 128,
            total_l2_kb: 1024,
            total_l3_kb: 8192,
        };
        let json = serde_json::to_string(&topo).unwrap();
        assert!(json.contains("8192"));
    }
}

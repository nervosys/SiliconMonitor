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
//!     // Every geometry field is optional: a platform that does not publish
//!     // one leaves it `None` rather than standing in x86's 64-byte line.
//!     let line = cache.line_size.map_or("?".to_string(), |b| format!("{b}-byte"));
//!     let ways = cache.associativity.map_or("?".to_string(), |w| format!("{w}-way"));
//!     println!("{} ({}): {} KB, {ways}, {line} lines",
//!         cache.level, cache.cache_type, cache.size_kb);
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
    /// Cache line size in bytes. `None` when the platform did not report it —
    /// never 64, which is x86's value and not Apple silicon's 128.
    pub line_size: Option<u32>,
    /// Set associativity. `None` when the platform did not report it; `Some(0)`
    /// is the real, distinct value meaning fully associative.
    pub associativity: Option<u32>,
    /// Number of sets. `None` when the platform did not report it.
    pub sets: Option<u64>,
    /// Number of physical partitions. `None` when the platform did not report it.
    pub partitions: Option<u32>,
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
        self.refresh_linux()?;

        #[cfg(target_os = "windows")]
        self.refresh_windows()?;

        #[cfg(target_os = "macos")]
        self.refresh_macos()?;

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
                (CacheLevel::L3, _) if seen_l3.insert(cache.shared_cpu_list.clone()) => {
                    self.topology.total_l3_kb += cache.size_kb;
                }
                _ => {}
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) -> Result<(), SimonError> {
        // Read from cpu0's cache hierarchy (representative)
        let cpu_base = std::path::Path::new("/sys/devices/system/cpu/cpu0/cache");
        if !cpu_base.exists() {
            // A real answer: this kernel publishes no cache topology.
            return Ok(());
        }

        {
            let entries = std::fs::read_dir(cpu_base)
                .map_err(|e| SimonError::System(format!("cannot read {cpu_base:?}: {e}")))?;
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

                let line_size: Option<u32> = Self::read_trimmed(&base.join("coherency_line_size"))
                    .parse()
                    .ok();

                let associativity: Option<u32> =
                    Self::read_trimmed(&base.join("ways_of_associativity"))
                        .parse()
                        .ok();

                let sets: Option<u64> = Self::read_trimmed(&base.join("number_of_sets"))
                    .parse()
                    .ok();

                let partitions: Option<u32> =
                    Self::read_trimmed(&base.join("physical_line_partition"))
                        .parse()
                        .ok();

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
        Ok(())
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
    fn refresh_windows(&mut self) -> Result<(), SimonError> {
        const QUERY: &str = concat!(
            "Get-CimInstance Win32_CacheMemory | Select-Object Purpose, ",
            "InstalledSize, CacheSpeed, Level, Associativity, LineSize, ",
            "NumberOfBlocks, Status | ConvertTo-Json -Compress"
        );

        let Some(val) =
            crate::core::command::capture_json("powershell", &["-NoProfile", "-Command", QUERY])?
        else {
            return Ok(());
        };

        for (i, item) in crate::core::command::json_items(&val).iter().enumerate() {
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
                    if p.contains("l1") {
                        CacheLevel::L1
                    } else if p.contains("l2") {
                        CacheLevel::L2
                    } else if p.contains("l3") {
                        CacheLevel::L3
                    } else {
                        CacheLevel::L2
                    }
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
            let line_size = item["LineSize"].as_u64().map(|v| v as u32);
            let assoc = item["Associativity"].as_u64().map(|v| v as u32);

            self.topology.caches.push(CpuCacheInfo {
                level,
                cache_type,
                size_kb,
                line_size,
                associativity: assoc,
                // Win32_CacheMemory carries neither field.
                sets: None,
                partitions: None,
                shared_cpu_list: String::new(),
                index: i as u32,
            });
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) -> Result<(), SimonError> {
        // `sysctl` ships with macOS, so failing to run it is a failure. A key
        // it does not know is a different matter -- `hw.l3cachesize` is absent
        // on parts with no L3 -- so one key exiting non-zero stays `None`
        // rather than failing the whole read.
        let read_sysctl = |name: &str| -> Option<u64> {
            crate::core::command::capture("sysctl", &["-n", name])
                .ok()
                .and_then(|s| s.trim().parse().ok())
        };
        let line_size = read_sysctl("hw.cachelinesize").map(|v| v as u32);

        // L1 data cache
        if let Some(l1d) = read_sysctl("hw.l1dcachesize").filter(|v| *v > 0) {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L1,
                cache_type: CacheType::Data,
                size_kb: l1d / 1024,
                line_size,
                // sysctl publishes none of these three.
                associativity: None,
                sets: None,
                partitions: None,
                shared_cpu_list: String::new(),
                index: 0,
            });
        }

        // L1 instruction cache
        if let Some(l1i) = read_sysctl("hw.l1icachesize").filter(|v| *v > 0) {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L1,
                cache_type: CacheType::Instruction,
                size_kb: l1i / 1024,
                line_size,
                // sysctl publishes none of these three.
                associativity: None,
                sets: None,
                partitions: None,
                shared_cpu_list: String::new(),
                index: 1,
            });
        }

        // L2 cache
        if let Some(l2) = read_sysctl("hw.l2cachesize").filter(|v| *v > 0) {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L2,
                cache_type: CacheType::Unified,
                size_kb: l2 / 1024,
                line_size,
                // sysctl publishes none of these three.
                associativity: None,
                sets: None,
                partitions: None,
                shared_cpu_list: String::new(),
                index: 2,
            });
        }

        // L3 cache
        if let Some(l3) = read_sysctl("hw.l3cachesize").filter(|v| *v > 0) {
            self.topology.caches.push(CpuCacheInfo {
                level: CacheLevel::L3,
                cache_type: CacheType::Unified,
                size_kb: l3 / 1024,
                line_size,
                // sysctl publishes none of these three.
                associativity: None,
                sets: None,
                partitions: None,
                shared_cpu_list: String::new(),
                index: 3,
            });
        }
        Ok(())
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

    /// Constructing the monitor either enumerates or says why it could not.
    ///
    /// See the identically-shaped tests in `camera`, `usb` and the rest: this
    /// asserted `is_ok()`, which was true by construction while `refresh` could
    /// not fail. A failure must carry a reason, because a reason is the whole
    /// difference between "this machine has none" and "nobody looked".
    #[test]
    fn test_cache_monitor_creation() {
        match CpuCacheMonitor::new() {
            Ok(_monitor) => {}
            Err(e) => {
                let why = e.to_string();
                assert!(
                    why.len() > 10,
                    "enumeration failed without saying why: {why:?}"
                );
            }
        }
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
            line_size: Some(64),
            associativity: Some(8),
            sets: Some(512),
            partitions: Some(1),
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
    /// The ontology entity for `cpu.cache.{n}.line_size` says the field exists
    /// "rather than assumed to be 64 bytes". Until 6.0.0 all three readers
    /// assumed exactly that: Linux `unwrap_or(64)`, Windows `unwrap_or(64)`,
    /// macOS a sysctl that returned 0 on failure. 64 is x86's line; Apple
    /// silicon's is 128.
    #[test]
    fn cache_geometry_is_absent_rather_than_assumed() {
        let unread = CpuCacheInfo {
            level: CacheLevel::L1,
            cache_type: CacheType::Data,
            size_kb: 64,
            line_size: None,
            associativity: None,
            sets: None,
            partitions: None,
            shared_cpu_list: String::new(),
            index: 0,
        };
        assert!(unread.line_size.is_none());
        // Zero associativity is a real value -- fully associative -- so it can
        // never double as the marker for "not read".
        let fully = CpuCacheInfo {
            associativity: Some(0),
            ..unread.clone()
        };
        assert_ne!(fully.associativity, unread.associativity);
    }
}

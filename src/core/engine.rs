//! Engine monitoring (DLA, APE, NVENC, NVDEC, etc.)

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Engine information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    /// Engine online status
    pub online: bool,
    /// Current frequency in MHz
    pub current: u32,
    /// Maximum frequency in MHz (optional)
    pub max: Option<u32>,
    /// Minimum frequency in MHz (optional)
    pub min: Option<u32>,
}

/// Engine statistics
/// Contains groups of engines (e.g., APE, DLA, NVENC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStats {
    /// Engine groups (name -> engines in that group)
    pub groups: HashMap<String, HashMap<String, EngineInfo>>,
}

impl EngineStats {
    /// Create a new engine stats instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            groups: HashMap::new(),
        })
    }

    /// Get all engines in a group
    pub fn get_group(&self, name: &str) -> Option<&HashMap<String, EngineInfo>> {
        self.groups.get(name)
    }

    /// Get a specific engine
    pub fn get_engine(&self, group: &str, name: &str) -> Option<&EngineInfo> {
        self.groups.get(group)?.get(name)
    }

    /// Get count of engine groups
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Get total count of all engines
    pub fn engine_count(&self) -> usize {
        self.groups.values().map(|g| g.len()).sum()
    }
}

impl Default for EngineStats {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod linux {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// List of known engine names to search for
    const KNOWN_ENGINES: &[&str] = &[
        "ape", "dla", "pva", "vic", "nvjpg", "nvenc", "nvdec", "se", "cvnas", "msenc", "ofa",
    ];

    /// Read engine statistics (Linux)
    pub fn read_engine_stats() -> Result<EngineStats> {
        let mut stats = EngineStats::new()?;

        let engine_root = "/sys/kernel/debug/clk";
        if !Path::new(engine_root).exists() {
            // Debug clk not available
            return Ok(stats);
        }

        // Discover all available engines
        let discovered = discover_engines(engine_root)?;

        // Read status for each engine
        for (group_name, engine_paths) in discovered {
            let mut group_engines = HashMap::new();

            for (engine_name, path) in engine_paths {
                if let Ok(engine_info) = read_engine_info(&path) {
                    group_engines.insert(engine_name, engine_info);
                }
            }

            if !group_engines.is_empty() {
                stats.groups.insert(group_name, group_engines);
            }
        }

        Ok(stats)
    }

    fn discover_engines(root: &str) -> Result<HashMap<String, Vec<(String, String)>>> {
        let mut engines: HashMap<String, Vec<(String, String)>> = HashMap::new();

        // Walk through all directories in clk
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let dir_name = entry.file_name().to_string_lossy().to_string();

            // Check if this matches any known engine
            for &engine_name in KNOWN_ENGINES {
                let engine_lower = engine_name.to_lowercase();
                let dir_lower = dir_name.to_lowercase();

                if dir_lower.starts_with(&engine_lower) {
                    // Check for numbered engines (dla0, dla1, etc.)
                    if dir_lower.len() > engine_lower.len() {
                        let suffix = &dir_lower[engine_lower.len()..];
                        if suffix.chars().all(|c| c.is_ascii_digit()) {
                            // Numbered engine
                            let group = engine_name.to_uppercase();
                            let engine_full_name = dir_name.to_uppercase();
                            engines
                                .entry(group)
                                .or_insert_with(Vec::new)
                                .push((engine_full_name, path.to_string_lossy().to_string()));
                            break;
                        }
                    } else if dir_lower == engine_lower {
                        // Exact match
                        let group = engine_name.to_uppercase();
                        let engine_full_name = dir_name.to_uppercase();
                        engines
                            .entry(group)
                            .or_insert_with(Vec::new)
                            .push((engine_full_name, path.to_string_lossy().to_string()));
                        break;
                    }
                }
            }
        }

        Ok(engines)
    }

    fn read_engine_info(path: &str) -> Result<EngineInfo> {
        let mut info = EngineInfo {
            online: false,
            current: 0,
            max: None,
            min: None,
        };

        // Read online status
        let enable_count_path = format!("{}/clk_enable_count", path);
        if path_exists(&enable_count_path) {
            if let Ok(count) = read_file_u32(&enable_count_path) {
                info.online = count == 1;
            }
        }

        // Read current frequency
        let rate_path = format!("{}/clk_rate", path);
        if path_exists(&rate_path) {
            if let Ok(rate) = read_file_u32(&rate_path) {
                info.current = rate / 1000; // Hz to MHz
            }
        }

        // Read max frequency
        let max_rate_path = format!("{}/clk_max_rate", path);
        if path_exists(&max_rate_path) {
            if let Ok(rate) = read_file_u32(&max_rate_path) {
                // Check for invalid value (FFFF_FFFF_FFFF_FFFF)
                if rate != u32::MAX && rate != 0 {
                    info.max = Some(rate / 1000);
                }
            }
        }

        // Read min frequency (only if max is valid)
        if info.max.is_some() {
            let min_rate_path = format!("{}/clk_min_rate", path);
            if path_exists(&min_rate_path) {
                if let Ok(rate) = read_file_u32(&min_rate_path) {
                    if rate != 0 {
                        info.min = Some(rate / 1000);
                    }
                }
            }
        }

        Ok(info)
    }
}

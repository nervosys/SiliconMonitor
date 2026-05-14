//! Active-profile detection — for each running process, determine whether
//! the NVIDIA DRS database (or, on Linux, any GPU profile source) has a
//! per-application profile that would apply to it.
//!
//! Answers the question "does NVIDIA Profile Inspector have a profile for
//! this process?" — useful for AI agents to suggest enabling DLSS, FPS
//! caps, etc. on apps that lack a tuned profile.

use serde::{Deserialize, Serialize};

/// One entry in an active-profiles report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAppProfile {
    pub pid: u32,
    /// Process image name (e.g. "cyberpunk2077.exe"), case-preserved when
    /// available.
    pub name: String,
    /// Full executable path when reported by the OS.
    pub exe_path: Option<String>,
    /// Whether this process's image name appears in the NVIDIA DRS database
    /// — i.e. NVIDIA ships or the user has stored a per-application profile.
    pub has_nvidia_drs_profile: bool,
}

impl ActiveAppProfile {
    pub fn has_any_profile(&self) -> bool {
        self.has_nvidia_drs_profile
    }
}

/// Look up active per-application profiles for all running processes.
///
/// Reads the DRS string set once and joins against the process list. The
/// result is sorted: processes with a known profile first (most useful for
/// users), then alphabetically by name.
pub fn active_profiles_for_processes() -> Vec<ActiveAppProfile> {
    let nvidia_set: std::collections::BTreeSet<String> = {
        #[cfg(windows)]
        {
            super::nvidia_drs::drs_exe_set()
        }
        #[cfg(not(windows))]
        {
            std::collections::BTreeSet::new()
        }
    };

    let processes = match crate::process_monitor::ProcessMonitor::new() {
        Ok(mut m) => m.processes().unwrap_or_default(),
        Err(_) => return Vec::new(),
    };

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in processes {
        // Process name may be "foo.exe" already (Windows) or "foo" (Linux).
        // Normalize: lowercase + ensure .exe for comparison key.
        let lower = p.name.to_ascii_lowercase();
        let key = if lower.ends_with(".exe") {
            lower.clone()
        } else {
            format!("{}.exe", lower)
        };
        if !seen.insert((p.pid, key.clone())) {
            continue;
        }
        let has_nvidia = nvidia_set.contains(&key);
        out.push(ActiveAppProfile {
            pid: p.pid,
            name: p.name.clone(),
            exe_path: None, // Filled in by future work using a richer ProcessInfo field
            has_nvidia_drs_profile: has_nvidia,
        });
    }
    out.sort_by(|a, b| {
        b.has_any_profile()
            .cmp(&a.has_any_profile())
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });
    out
}

/// Filter the active profile list to only entries with a known profile.
pub fn matched_active_profiles() -> Vec<ActiveAppProfile> {
    active_profiles_for_processes()
        .into_iter()
        .filter(ActiveAppProfile::has_any_profile)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        // Must not panic on any platform.
        let _ = active_profiles_for_processes();
        let _ = matched_active_profiles();
    }

    #[test]
    fn matched_subset_of_full() {
        let all = active_profiles_for_processes();
        let matched = matched_active_profiles();
        assert!(matched.len() <= all.len());
        for m in &matched {
            assert!(m.has_any_profile());
        }
    }
}

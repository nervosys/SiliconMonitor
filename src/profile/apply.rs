//! Apply (write) layer for hardware profile settings.
//!
//! Most of the [`super::ProfileInspector`] surface is read-only. This module
//! adds a narrow, audited write path for the settings that providers
//! explicitly opt into via [`Setting::writable`].
//!
//! ## Design principles
//!
//! - **Opt-in writes.** Each handler advertises an exact `(subsystem,
//!   setting_id)` it can write. There is no generic write path.
//! - **Audit everything.** Every attempt — whether allowed, refused, or
//!   failed — appends a JSON line to `<state_dir>/simon_profile_audit.log`.
//! - **No silent elevation.** If the OS rejects the write for permissions,
//!   the error bubbles up with a "needs admin/root" hint. We never re-launch
//!   ourselves elevated.
//! - **Confirmation required.** Callers (CLI/MCP) gate apply behind explicit
//!   confirmation. The library function itself does not prompt.
//!
//! Writing arbitrary NVAPI DWORDs, MSRs, or BIOS values is intentionally out
//! of scope. Add handlers only when the API is well-defined and the failure
//! mode is reversible.

use super::{Setting, SettingValue, Subsystem};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Result of an apply attempt — always returned (never panics), always logged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplyOutcome {
    pub setting_id: String,
    pub subsystem: Subsystem,
    pub requested: SettingValue,
    pub status: ApplyStatus,
    pub message: String,
    /// Unix epoch seconds.
    pub timestamp: u64,
    /// The value that was in effect before this write, when the handler could
    /// read it.
    ///
    /// This is what makes a write reversible, and until it existed nothing in
    /// this crate could undo one: an autonomous tuner could set a governor and
    /// then had no way to put back what it found. `None` means the prior value
    /// was not readable — not that there wasn't one — so
    /// [`revert_setting`] refuses rather than guessing at a default.
    #[serde(default)]
    pub previous: Option<SettingValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    /// Write succeeded; new value is in effect.
    Applied,
    /// Setting is read-only (no registered handler).
    NotWritable,
    /// The OS rejected the call (permission, busy, etc.).
    Refused,
    /// The handler ran but reports the underlying API failed.
    Failed,
    /// The library would write but the caller did not confirm.
    NeedsConfirm,
}

/// Trait implemented by per-setting write handlers.
pub trait ApplyHandler: Send + Sync {
    /// The setting this handler covers (must match `Setting.id`).
    fn setting_id(&self) -> &str;
    /// The subsystem this setting belongs to.
    fn subsystem(&self) -> Subsystem;
    /// Try to write the requested value. Must not panic; surface any error
    /// via the returned [`ApplyStatus`].
    fn apply(&self, value: &SettingValue) -> ApplyOutcome;

    /// The value currently in effect, when it can be read.
    ///
    /// Defaults to `None` so existing handlers keep compiling, but a handler
    /// that does not implement this makes its setting one-way: `apply_setting`
    /// records no prior value, and [`revert_setting`] will refuse to guess one.
    /// Implement it wherever the source can be read back.
    fn read_current(&self) -> Option<SettingValue> {
        None
    }
}

/// Built-in handler registry. New handlers register themselves by appearing
/// in [`builtin_handlers`].
// Not a vec! literal: which handlers exist depends on `cfg`, and those attributes
// apply to statements, not to elements of an expression.
#[allow(clippy::vec_init_then_push)]
pub fn builtin_handlers() -> Vec<Box<dyn ApplyHandler>> {
    let mut out: Vec<Box<dyn ApplyHandler>> = Vec::new();
    #[cfg(all(feature = "nvidia", target_os = "linux"))]
    out.push(Box::new(NvidiaPersistenceModeHandler));
    #[cfg(target_os = "linux")]
    {
        out.push(Box::new(LinuxCpufreqGovernorHandler));
        out.push(Box::new(LinuxAmdPerfLevelHandler));
        out.push(Box::new(LinuxIntelGtMaxFreqHandler));
    }
    #[cfg(windows)]
    out.push(Box::new(WindowsActivePowerSchemeHandler));
    out
}

/// Put back the value an earlier [`apply_setting`] overwrote.
///
/// Takes the outcome of the write being undone rather than a setting id and a
/// value, because the whole point is that the caller does not have to have kept
/// the old value anywhere: it travels with the outcome.
///
/// Refuses when `previous` is `None`. That happens when the handler could not
/// read the setting before writing it, and the honest answer there is "this
/// cannot be undone" rather than a guess at a default — putting a machine into a
/// state it was never in is a worse failure than leaving it in the state the
/// caller chose.
///
/// Goes through [`apply_setting`], so a revert is confirmed and audit-logged on
/// exactly the same terms as the write it undoes. An autonomous loop that could
/// revert without confirmation would be a write path with no confirmation.
pub fn revert_setting(applied: &ApplyOutcome, confirm: bool) -> ApplyOutcome {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Only a write that took effect needs undoing. Reverting a refused or failed
    // apply would write a value the caller never asked for.
    if applied.status != ApplyStatus::Applied {
        return ApplyOutcome {
            setting_id: applied.setting_id.clone(),
            subsystem: applied.subsystem,
            requested: applied.requested.clone(),
            status: ApplyStatus::NotWritable,
            message: format!(
                "Nothing to revert: the apply being undone ended as {:?}, not Applied.",
                applied.status
            ),
            timestamp: now,
            previous: None,
        };
    }

    let Some(previous) = applied.previous.clone() else {
        return ApplyOutcome {
            setting_id: applied.setting_id.clone(),
            subsystem: applied.subsystem,
            requested: applied.requested.clone(),
            status: ApplyStatus::NotWritable,
            message: format!(
                "Cannot revert {:?}: no prior value was recorded, because the handler                  could not read the setting before writing it. The setting is still at                  the applied value.",
                applied.setting_id
            ),
            timestamp: now,
            previous: None,
        };
    };

    apply_setting(&applied.setting_id, previous, confirm)
}

/// Apply a setting by id. Returns an [`ApplyOutcome`] in every case (no
/// panicking, no unhandled errors). Audit-logs the attempt.
pub fn apply_setting(setting_id: &str, value: SettingValue, confirm: bool) -> ApplyOutcome {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let handler = builtin_handlers()
        .into_iter()
        .find(|h| h.setting_id() == setting_id);

    let outcome = match handler {
        None => ApplyOutcome {
            setting_id: setting_id.into(),
            subsystem: Subsystem::Gpu, // placeholder — overridden if handler exists
            requested: value.clone(),
            status: ApplyStatus::NotWritable,
            message: format!(
                "No registered ApplyHandler for setting id {:?}. \
                The setting may be read-only on this platform or not yet implemented.",
                setting_id
            ),
            timestamp: now,
            previous: None,
        },
        Some(h) if !confirm => ApplyOutcome {
            setting_id: setting_id.into(),
            subsystem: h.subsystem(),
            requested: value.clone(),
            status: ApplyStatus::NeedsConfirm,
            message:
                "Apply rejected: explicit confirmation required (pass confirm=true or --confirm)."
                    .into(),
            timestamp: now,
            previous: None,
        },
        Some(h) => {
            // Read before writing. Afterwards the old value is gone, and a
            // tuner that cannot say what it overwrote cannot put it back.
            let previous = h.read_current();
            let mut outcome = h.apply(&value);
            outcome.previous = previous;
            outcome
        }
    };

    audit_log(&outcome);
    outcome
}

/// Resolve the audit log path. Honors `SIMON_AUDIT_LOG` if set; otherwise
/// falls back to `%LOCALAPPDATA%\simon\profile_audit.log` on Windows,
/// `$XDG_STATE_HOME/simon/profile_audit.log` on Linux, and the OS temp dir
/// elsewhere.
pub fn audit_log_path() -> PathBuf {
    static CACHED: OnceLock<PathBuf> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            if let Ok(p) = std::env::var("SIMON_AUDIT_LOG") {
                return PathBuf::from(p);
            }
            #[cfg(windows)]
            {
                if let Ok(local) = std::env::var("LOCALAPPDATA") {
                    return PathBuf::from(local).join("simon").join("profile_audit.log");
                }
            }
            #[cfg(unix)]
            {
                if let Ok(state) = std::env::var("XDG_STATE_HOME") {
                    return PathBuf::from(state).join("simon").join("profile_audit.log");
                }
                if let Ok(home) = std::env::var("HOME") {
                    return PathBuf::from(home)
                        .join(".local")
                        .join("state")
                        .join("simon")
                        .join("profile_audit.log");
                }
            }
            std::env::temp_dir().join("simon_profile_audit.log")
        })
        .clone()
}

fn audit_log(outcome: &ApplyOutcome) {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        if let Ok(line) = serde_json::to_string(outcome) {
            let _ = writeln!(f, "{}", line);
        }
    }
}

// ─── Built-in handlers ──────────────────────────────────────────────────────

/// Toggle NVIDIA's persistence mode on GPU 0 via NVML.
#[cfg(all(feature = "nvidia", target_os = "linux"))]
struct NvidiaPersistenceModeHandler;

#[cfg(all(feature = "nvidia", target_os = "linux"))]
impl ApplyHandler for NvidiaPersistenceModeHandler {
    fn setting_id(&self) -> &str {
        "persistence_mode"
    }
    fn subsystem(&self) -> Subsystem {
        Subsystem::Gpu
    }
    fn apply(&self, value: &SettingValue) -> ApplyOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = match value {
            SettingValue::Bool(b) => *b,
            other => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: other.clone(),
                    status: ApplyStatus::Failed,
                    message: "persistence_mode requires a Bool value.".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        use nvml_wrapper::Nvml;
        match Nvml::init().and_then(|n| {
            let mut dev = n.device_by_index(0)?;
            dev.set_persistent(target)?;
            Ok(())
        }) {
            Ok(_) => ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: ApplyStatus::Applied,
                message: format!("NVIDIA persistence mode on GPU 0 set to {}.", target),
                timestamp: now,
                previous: None,
            },
            Err(e) => {
                let s = e.to_string();
                let refused = s.contains("Permission")
                    || s.contains("permission")
                    || s.contains("denied")
                    || s.contains("PrivilegedDriver");
                ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: if refused {
                        ApplyStatus::Refused
                    } else {
                        ApplyStatus::Failed
                    },
                    message: if refused {
                        format!("NVML write rejected — run as administrator/root: {}", s)
                    } else {
                        format!("NVML write failed: {}", s)
                    },
                    timestamp: now,
                    previous: None,
                }
            }
        }
    }
}

/// Set the Linux CPU governor on CPU0 (which propagates to the policy).
#[cfg(target_os = "linux")]
struct LinuxCpufreqGovernorHandler;

#[cfg(target_os = "linux")]
impl ApplyHandler for LinuxCpufreqGovernorHandler {
    fn setting_id(&self) -> &str {
        "scaling_governor"
    }
    // Reading back the same path that `apply` writes. By inspection only: this
    // session had no Linux machine to run it on, and CI compiles but does not
    // exercise a sysfs write.
    fn subsystem(&self) -> Subsystem {
        Subsystem::Cpu
    }
    fn read_current(&self) -> Option<SettingValue> {
        std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .ok()
            .map(|s| SettingValue::Text(s.trim().to_string()))
    }
    fn apply(&self, value: &SettingValue) -> ApplyOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = match value {
            SettingValue::Text(s) => s.clone(),
            other => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: other.clone(),
                    status: ApplyStatus::Failed,
                    message: "scaling_governor requires a Text value (e.g. \"performance\")."
                        .into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        let path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
        match std::fs::write(path, &target) {
            Ok(_) => ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: ApplyStatus::Applied,
                message: format!("CPU0 scaling_governor set to {:?}.", target),
                timestamp: now,
                previous: None,
            },
            Err(e) => {
                let refused = e.kind() == std::io::ErrorKind::PermissionDenied;
                ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: if refused {
                        ApplyStatus::Refused
                    } else {
                        ApplyStatus::Failed
                    },
                    message: if refused {
                        "Permission denied — write to /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor requires root.".into()
                    } else {
                        format!("sysfs write failed: {}", e)
                    },
                    timestamp: now,
                    previous: None,
                }
            }
        }
    }
}

/// Write `/sys/class/drm/cardN/device/power_dpm_force_performance_level` for
/// the first AMD GPU. Accepted values: "auto", "low", "high", "manual",
/// "profile_standard", "profile_min_sclk", "profile_min_mclk", "profile_peak".
#[cfg(target_os = "linux")]
struct LinuxAmdPerfLevelHandler;

#[cfg(target_os = "linux")]
impl ApplyHandler for LinuxAmdPerfLevelHandler {
    fn setting_id(&self) -> &str {
        "perf_level"
    }
    fn subsystem(&self) -> Subsystem {
        Subsystem::Gpu
    }
    fn apply(&self, value: &SettingValue) -> ApplyOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let target = match value {
            SettingValue::Text(s) => s.clone(),
            other => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: other.clone(),
                    status: ApplyStatus::Failed,
                    message: "perf_level requires a Text value (e.g. \"auto\").".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        // Locate the first AMD card.
        let card = match find_amd_card() {
            Some(c) => c,
            None => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: ApplyStatus::Failed,
                    message: "No AMD GPU card found in /sys/class/drm.".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        let path = card
            .join("device")
            .join("power_dpm_force_performance_level");
        match std::fs::write(&path, &target) {
            Ok(_) => ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: ApplyStatus::Applied,
                message: format!("AMD perf level set to {:?} via {}", target, path.display()),
                timestamp: now,
                previous: None,
            },
            Err(e) => {
                let refused = e.kind() == std::io::ErrorKind::PermissionDenied;
                ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: if refused {
                        ApplyStatus::Refused
                    } else {
                        ApplyStatus::Failed
                    },
                    message: if refused {
                        format!(
                            "Permission denied — sysfs write to {} requires root.",
                            path.display()
                        )
                    } else {
                        format!("sysfs write failed: {}", e)
                    },
                    timestamp: now,
                    previous: None,
                }
            }
        }
    }
}

/// Write `/sys/class/drm/cardN/gt_max_freq_mhz` for the first Intel GPU.
#[cfg(target_os = "linux")]
struct LinuxIntelGtMaxFreqHandler;

#[cfg(target_os = "linux")]
impl ApplyHandler for LinuxIntelGtMaxFreqHandler {
    fn setting_id(&self) -> &str {
        "gt_max_freq_mhz"
    }
    fn subsystem(&self) -> Subsystem {
        Subsystem::Gpu
    }
    fn apply(&self, value: &SettingValue) -> ApplyOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mhz = match value {
            SettingValue::Uint(u) => *u,
            SettingValue::Int(i) if *i >= 0 => *i as u64,
            other => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: other.clone(),
                    status: ApplyStatus::Failed,
                    message: "gt_max_freq_mhz requires a non-negative integer (MHz).".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        let card = match find_intel_card() {
            Some(c) => c,
            None => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: ApplyStatus::Failed,
                    message: "No Intel GPU card found in /sys/class/drm.".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        let path = card.join("gt_max_freq_mhz");
        match std::fs::write(&path, mhz.to_string()) {
            Ok(_) => ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: ApplyStatus::Applied,
                message: format!("Intel GT max frequency set to {} MHz.", mhz),
                timestamp: now,
                previous: None,
            },
            Err(e) => {
                let refused = e.kind() == std::io::ErrorKind::PermissionDenied;
                ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: if refused {
                        ApplyStatus::Refused
                    } else {
                        ApplyStatus::Failed
                    },
                    message: if refused {
                        format!(
                            "Permission denied — sysfs write to {} requires root.",
                            path.display()
                        )
                    } else {
                        format!("sysfs write failed: {}", e)
                    },
                    timestamp: now,
                    previous: None,
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn find_amd_card() -> Option<std::path::PathBuf> {
    find_drm_card_with_vendor("0x1002")
}

#[cfg(target_os = "linux")]
fn find_intel_card() -> Option<std::path::PathBuf> {
    find_drm_card_with_vendor("0x8086")
}

#[cfg(target_os = "linux")]
fn find_drm_card_with_vendor(target_vendor: &str) -> Option<std::path::PathBuf> {
    let rd = std::fs::read_dir("/sys/class/drm").ok()?;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("device").join("vendor")).ok()?;
        if vendor.trim() == target_vendor {
            return Some(path);
        }
    }
    None
}

/// Switch the active Windows power scheme via `PowerSetActiveScheme`.
/// Accepts a string GUID in the canonical 8-4-4-4-12 format.
#[cfg(windows)]
struct WindowsActivePowerSchemeHandler;

#[cfg(windows)]
impl ApplyHandler for WindowsActivePowerSchemeHandler {
    fn setting_id(&self) -> &str {
        "active_scheme_guid"
    }
    fn subsystem(&self) -> Subsystem {
        Subsystem::Cpu
    }
    fn read_current(&self) -> Option<SettingValue> {
        use windows::Win32::System::Power::PowerGetActiveScheme;
        let mut guid_ptr: *mut windows::core::GUID = std::ptr::null_mut();
        // SAFETY: PowerGetActiveScheme writes a pointer to a GUID allocated by
        // the system; None selects the same user root that `apply` writes to.
        // The allocation is released with LocalFree below, as the API documents.
        let err = unsafe { PowerGetActiveScheme(None, &mut guid_ptr) };
        if err.0 != 0 || guid_ptr.is_null() {
            return None;
        }
        // SAFETY: non-null and written by the call above.
        let guid = unsafe { *guid_ptr };
        // SAFETY: the API allocates with LocalAlloc, so LocalFree is the
        // documented counterpart. Leaking here would leak on every read, and a
        // tuning loop reads on every cycle.
        unsafe {
            let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
                guid_ptr as *mut _,
            ));
        }
        Some(SettingValue::Text(format!("{:?}", guid).to_lowercase()))
    }
    fn apply(&self, value: &SettingValue) -> ApplyOutcome {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let guid_str = match value {
            SettingValue::Text(s) => s.trim_matches(|c| c == '{' || c == '}').to_string(),
            other => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: other.clone(),
                    status: ApplyStatus::Failed,
                    message: "active_scheme_guid requires a Text value (GUID).".into(),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        let guid = match parse_guid(&guid_str) {
            Some(g) => g,
            None => {
                return ApplyOutcome {
                    setting_id: self.setting_id().into(),
                    subsystem: self.subsystem(),
                    requested: value.clone(),
                    status: ApplyStatus::Failed,
                    message: format!("Invalid GUID format: {:?}", guid_str),
                    timestamp: now,
                    previous: None,
                };
            }
        };
        use windows::Win32::System::Power::PowerSetActiveScheme;
        // SAFETY: PowerSetActiveScheme is a Win32 FFI call. The HKEY argument
        // None selects the system user root, which is the documented value
        // for "current user / default" usage. The GUID pointer is to a stack
        // value that lives for the duration of the call.
        let win_error = unsafe { PowerSetActiveScheme(None, Some(&guid)) };
        if win_error.0 == 0 {
            ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: ApplyStatus::Applied,
                message: format!("Windows active power scheme set to {{{}}}.", guid_str),
                timestamp: now,
                previous: None,
            }
        } else {
            // ERROR_ACCESS_DENIED = 5
            let refused = win_error.0 == 5;
            ApplyOutcome {
                setting_id: self.setting_id().into(),
                subsystem: self.subsystem(),
                requested: value.clone(),
                status: if refused {
                    ApplyStatus::Refused
                } else {
                    ApplyStatus::Failed
                },
                message: if refused {
                    "Permission denied — try running as administrator.".into()
                } else {
                    format!("PowerSetActiveScheme failed: WIN32_ERROR {}", win_error.0)
                },
                timestamp: now,
                previous: None,
            }
        }
    }
}

#[cfg(windows)]
fn parse_guid(s: &str) -> Option<windows::core::GUID> {
    // Expected format: 8-4-4-4-12 hex chars, e.g. "381b4222-f694-41f0-9685-ff5bb260df2e"
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return None;
    }
    let d1 = u32::from_str_radix(parts[0], 16).ok()?;
    let d2 = u16::from_str_radix(parts[1], 16).ok()?;
    let d3 = u16::from_str_radix(parts[2], 16).ok()?;
    let d4_hi = u16::from_str_radix(parts[3], 16).ok()?;
    let d4_lo = u64::from_str_radix(parts[4], 16).ok()?;
    let mut d4 = [0u8; 8];
    d4[0] = (d4_hi >> 8) as u8;
    d4[1] = (d4_hi & 0xFF) as u8;
    for i in 0..6 {
        d4[2 + i] = ((d4_lo >> ((5 - i) * 8)) & 0xFF) as u8;
    }
    Some(windows::core::GUID::from_values(d1, d2, d3, d4))
}

/// Public list of writable setting ids on this build.
pub fn writable_setting_ids() -> Vec<String> {
    builtin_handlers()
        .into_iter()
        .map(|h| h.setting_id().to_string())
        .collect()
}

#[allow(dead_code)]
fn _doctest_anchor(_s: &Setting) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_setting_is_not_writable() {
        let outcome = apply_setting(
            "definitely-not-a-real-setting",
            SettingValue::Bool(true),
            true,
        );
        assert_eq!(outcome.status, ApplyStatus::NotWritable);
    }

    #[test]
    fn confirm_required() {
        // Pick any registered id (or skip if none on this build).
        let ids = writable_setting_ids();
        if let Some(id) = ids.first() {
            // Even without OS permission, the no-confirm path should return
            // NeedsConfirm before attempting the write.
            let outcome = apply_setting(id, SettingValue::Bool(true), false);
            assert_eq!(outcome.status, ApplyStatus::NeedsConfirm);
        }
    }

    #[test]
    fn audit_log_path_resolves() {
        let p = audit_log_path();
        assert!(p.to_string_lossy().to_lowercase().contains("profile_audit"));
    }

    #[test]
    fn outcome_serializable() {
        let outcome = ApplyOutcome {
            setting_id: "x".into(),
            subsystem: Subsystem::Cpu,
            requested: SettingValue::Bool(true),
            status: ApplyStatus::NotWritable,
            message: "test".into(),
            timestamp: 0,
            previous: None,
        };
        let s = serde_json::to_string(&outcome).unwrap();
        assert!(s.contains("not_writable"));
    }

    /// A write that cannot say what it overwrote cannot be undone, and the
    /// refusal has to be explicit rather than a silent no-op.
    #[test]
    fn revert_refuses_when_no_prior_value_was_recorded() {
        let applied = ApplyOutcome {
            setting_id: "scaling_governor".into(),
            subsystem: Subsystem::Cpu,
            requested: SettingValue::Text("performance".into()),
            status: ApplyStatus::Applied,
            message: "applied".into(),
            timestamp: 0,
            previous: None,
        };
        let out = revert_setting(&applied, true);
        assert_eq!(out.status, ApplyStatus::NotWritable);
        assert!(
            out.message.contains("no prior value"),
            "the refusal should say why, got: {}",
            out.message
        );
    }

    /// Reverting something that never took effect would write a value the
    /// caller never asked for.
    #[test]
    fn revert_refuses_an_apply_that_did_not_take_effect() {
        for status in [
            ApplyStatus::Refused,
            ApplyStatus::Failed,
            ApplyStatus::NeedsConfirm,
            ApplyStatus::NotWritable,
        ] {
            let applied = ApplyOutcome {
                setting_id: "scaling_governor".into(),
                subsystem: Subsystem::Cpu,
                requested: SettingValue::Text("performance".into()),
                status,
                message: String::new(),
                timestamp: 0,
                previous: Some(SettingValue::Text("powersave".into())),
            };
            let out = revert_setting(&applied, true);
            assert_eq!(
                out.status,
                ApplyStatus::NotWritable,
                "reverting a {status:?} apply must refuse"
            );
        }
    }

    /// A revert is a write, so it needs confirmation on the same terms.
    /// Otherwise an autonomous loop would have an unconfirmed write path.
    #[test]
    fn revert_without_confirmation_is_refused() {
        let applied = ApplyOutcome {
            setting_id: "active_scheme_guid".into(),
            subsystem: Subsystem::Cpu,
            requested: SettingValue::Text("381b4222-f694-41f0-9685-ff5bb260df2e".into()),
            status: ApplyStatus::Applied,
            message: "applied".into(),
            timestamp: 0,
            previous: Some(SettingValue::Text(
                "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c".into(),
            )),
        };
        let out = revert_setting(&applied, false);
        assert_eq!(out.status, ApplyStatus::NeedsConfirm);
    }

    /// The prior value round-trips through serialisation, so an outcome
    /// recorded in an audit log stays revertible after being read back.
    #[test]
    fn a_recorded_prior_value_survives_serialisation() {
        let outcome = ApplyOutcome {
            setting_id: "scaling_governor".into(),
            subsystem: Subsystem::Cpu,
            requested: SettingValue::Text("performance".into()),
            status: ApplyStatus::Applied,
            message: String::new(),
            timestamp: 0,
            previous: Some(SettingValue::Text("schedutil".into())),
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: ApplyOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(back.previous, Some(SettingValue::Text("schedutil".into())));
    }

    /// Outcomes written before `previous` existed still deserialise, so an
    /// existing audit log does not become unreadable.
    #[test]
    fn an_outcome_without_previous_still_deserialises() {
        // Built by removing the key from a real outcome rather than hand-written,
        // so the test cannot pass or fail on a guess at the wire format.
        let outcome = ApplyOutcome {
            setting_id: "scaling_governor".into(),
            subsystem: Subsystem::Cpu,
            requested: SettingValue::Text("performance".into()),
            status: ApplyStatus::Applied,
            message: String::new(),
            timestamp: 0,
            previous: Some(SettingValue::Text("schedutil".into())),
        };
        let mut value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&outcome).unwrap()).unwrap();
        value.as_object_mut().unwrap().remove("previous");
        let back: ApplyOutcome = serde_json::from_value(value).unwrap();
        assert_eq!(back.previous, None);
    }

    /// The Windows handler can read the scheme it writes.
    ///
    /// Without this the setting is one-way: `apply_setting` records no prior
    /// value and `revert_setting` refuses. Cross-checked against
    /// `simon profile explain active_scheme_guid`, which reaches the same
    /// registry through entirely different code in `profile::cpu`.
    #[cfg(windows)]
    #[test]
    fn the_windows_power_scheme_reads_back_as_a_guid() {
        let current = WindowsActivePowerSchemeHandler.read_current();
        let Some(SettingValue::Text(guid)) = current else {
            panic!("active power scheme did not read back as text: {current:?}");
        };
        println!("read_current -> {guid}");
        assert_eq!(guid.len(), 36, "expected a bare 36-char GUID, got {guid:?}");
        assert_eq!(
            guid.matches('-').count(),
            4,
            "expected GUID dash grouping, got {guid:?}"
        );
        assert!(
            guid.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "GUID should be hex and dashes, got {guid:?}"
        );
    }
}

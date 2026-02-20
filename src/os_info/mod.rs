//! OS and kernel information — kernel version, hostname, boot time, loaded modules.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/proc/version`, `/proc/uptime`, `/proc/modules`, `/proc/cmdline`, `uname`
//! - **Windows**: Uses WMI (`Win32_OperatingSystem`), `ver`, registry
//! - **macOS**: Uses `sw_vers`, `uname`, `sysctl`
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::os_info::OsInfoMonitor;
//!
//! let monitor = OsInfoMonitor::new().unwrap();
//! let info = monitor.info();
//! println!("OS: {} {} ({})", info.os_name, info.os_version, info.architecture);
//! println!("Kernel: {}", info.kernel_version);
//! println!("Hostname: {}", info.hostname);
//! println!("Uptime: {} seconds", info.uptime_seconds);
//! println!("Loaded modules: {}", info.loaded_modules.len());
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Operating system family
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OsFamily {
    Linux,
    Windows,
    MacOS,
    FreeBSD,
    Unknown,
}

/// Boot mode
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootMode {
    UEFI,
    BIOS,
    Unknown,
}

/// Loaded kernel module / driver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelModule {
    /// Module name
    pub name: String,
    /// Module size in bytes
    pub size_bytes: u64,
    /// Number of instances (use count)
    pub instances: u32,
    /// Modules that depend on this one
    pub used_by: Vec<String>,
    /// Module state (e.g., "Live")
    pub state: String,
}

/// Comprehensive OS and kernel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    /// OS family (Linux, Windows, macOS)
    pub os_family: OsFamily,
    /// OS name (e.g., "Ubuntu", "Windows 11", "macOS Sequoia")
    pub os_name: String,
    /// OS version string
    pub os_version: String,
    /// OS build number
    pub os_build: String,
    /// Kernel version string (e.g., "6.8.0-51-generic")
    pub kernel_version: String,
    /// Full kernel version line
    pub kernel_full: String,
    /// CPU architecture (e.g., "x86_64", "aarch64")
    pub architecture: String,
    /// Hostname
    pub hostname: String,
    /// Domain name (if configured)
    pub domain: String,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Boot time as Unix timestamp
    pub boot_timestamp: u64,
    /// Boot mode (UEFI or BIOS)
    pub boot_mode: BootMode,
    /// Kernel command line (Linux)
    pub kernel_cmdline: String,
    /// Number of logged-in users
    pub user_count: u32,
    /// Loaded kernel modules
    pub loaded_modules: Vec<KernelModule>,
    /// Timezone string (e.g., "America/New_York")
    pub timezone: String,
    /// System locale
    pub locale: String,
    /// Whether this is a 64-bit OS
    pub is_64bit: bool,
    /// Whether Secure Boot is enabled
    pub secure_boot: bool,
}

/// Monitor for OS and kernel information
pub struct OsInfoMonitor {
    info: OsInfo,
}

impl OsInfoMonitor {
    /// Create a new OsInfoMonitor and gather system information.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            info: OsInfo {
                os_family: OsFamily::Unknown,
                os_name: String::new(),
                os_version: String::new(),
                os_build: String::new(),
                kernel_version: String::new(),
                kernel_full: String::new(),
                architecture: String::new(),
                hostname: String::new(),
                domain: String::new(),
                uptime_seconds: 0,
                boot_timestamp: 0,
                boot_mode: BootMode::Unknown,
                kernel_cmdline: String::new(),
                user_count: 0,
                loaded_modules: Vec::new(),
                timezone: String::new(),
                locale: String::new(),
                is_64bit: cfg!(target_pointer_width = "64"),
                secure_boot: false,
            },
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh information.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.info.loaded_modules.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        // Common: hostname from std
        if self.info.hostname.is_empty() {
            if let Ok(output) = std::process::Command::new("hostname").output() {
                self.info.hostname = String::from_utf8(output.stdout)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
            }
        }

        Ok(())
    }

    /// Get the gathered OS information.
    pub fn info(&self) -> &OsInfo {
        &self.info
    }

    /// Get loaded kernel modules.
    pub fn modules(&self) -> &[KernelModule] {
        &self.info.loaded_modules
    }

    /// Get the number of loaded modules.
    pub fn module_count(&self) -> usize {
        self.info.loaded_modules.len()
    }

    /// Find a specific module by name.
    pub fn find_module(&self, name: &str) -> Option<&KernelModule> {
        self.info
            .loaded_modules
            .iter()
            .find(|m| m.name == name)
    }

    // ── Linux ──

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        self.info.os_family = OsFamily::Linux;

        // /etc/os-release
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                    self.info.os_name = val.trim_matches('"').to_string();
                } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
                    self.info.os_version = val.trim_matches('"').to_string();
                } else if let Some(val) = line.strip_prefix("BUILD_ID=") {
                    self.info.os_build = val.trim_matches('"').to_string();
                }
            }
        }

        // /proc/version
        if let Ok(content) = std::fs::read_to_string("/proc/version") {
            self.info.kernel_full = content.trim().to_string();
            // "Linux version 6.8.0-51-generic ..."
            if let Some(ver) = content.split_whitespace().nth(2) {
                self.info.kernel_version = ver.to_string();
            }
        }

        // Architecture via uname
        if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
            self.info.architecture = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }

        // Hostname
        if let Ok(content) = std::fs::read_to_string("/etc/hostname") {
            self.info.hostname = content.trim().to_string();
        }

        // Domain
        if let Ok(content) = std::fs::read_to_string("/proc/sys/kernel/domainname") {
            let d = content.trim();
            if d != "(none)" {
                self.info.domain = d.to_string();
            }
        }

        // Uptime
        if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
            if let Some(secs_str) = content.split_whitespace().next() {
                if let Ok(secs) = secs_str.parse::<f64>() {
                    self.info.uptime_seconds = secs as u64;
                }
            }
        }

        // Boot timestamp from /proc/stat
        if let Ok(content) = std::fs::read_to_string("/proc/stat") {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("btime ") {
                    if let Ok(ts) = rest.trim().parse() {
                        self.info.boot_timestamp = ts;
                    }
                }
            }
        }

        // Boot mode
        self.info.boot_mode = if std::path::Path::new("/sys/firmware/efi").exists() {
            BootMode::UEFI
        } else {
            BootMode::BIOS
        };

        // Secure Boot
        if let Ok(output) = std::process::Command::new("mokutil")
            .arg("--sb-state")
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            self.info.secure_boot = text.contains("SecureBoot enabled");
        }

        // Kernel command line
        if let Ok(content) = std::fs::read_to_string("/proc/cmdline") {
            self.info.kernel_cmdline = content.trim().to_string();
        }

        // Logged-in users (who)
        if let Ok(output) = std::process::Command::new("who").output() {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            self.info.user_count = text.lines().count() as u32;
        }

        // Loaded modules from /proc/modules
        if let Ok(content) = std::fs::read_to_string("/proc/modules") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let name = parts[0].to_string();
                    let size_bytes = parts[1].parse().unwrap_or(0);
                    let instances = parts[2].parse().unwrap_or(0);
                    let used_by: Vec<String> = if parts[3] == "-" {
                        Vec::new()
                    } else {
                        parts[3]
                            .trim_end_matches(',')
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect()
                    };
                    let state = parts.get(4).unwrap_or(&"").to_string();

                    self.info.loaded_modules.push(KernelModule {
                        name,
                        size_bytes,
                        instances,
                        used_by,
                        state,
                    });
                }
            }
        }

        // Timezone
        if let Ok(tz) = std::fs::read_link("/etc/localtime") {
            let path = tz.to_string_lossy().to_string();
            if let Some(pos) = path.find("zoneinfo/") {
                self.info.timezone = path[pos + 9..].to_string();
            }
        }
        if self.info.timezone.is_empty() {
            if let Ok(content) = std::fs::read_to_string("/etc/timezone") {
                self.info.timezone = content.trim().to_string();
            }
        }

        // Locale
        if let Ok(val) = std::env::var("LANG") {
            self.info.locale = val;
        }
    }

    // ── Windows ──

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        self.info.os_family = OsFamily::Windows;

        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber, CSName, OSArchitecture, LastBootUpTime, NumberOfUsers, Locale, CurrentTimeZone | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    self.info.os_name = val["Caption"].as_str().unwrap_or("Windows").trim().to_string();
                    self.info.os_version = val["Version"].as_str().unwrap_or("").to_string();
                    self.info.os_build = val["BuildNumber"].as_str().unwrap_or("").to_string();
                    self.info.hostname = val["CSName"].as_str().unwrap_or("").to_string();
                    let arch = val["OSArchitecture"].as_str().unwrap_or("");
                    self.info.architecture = if arch.contains("64") {
                        "x86_64".to_string()
                    } else if arch.contains("ARM") {
                        "aarch64".to_string()
                    } else {
                        "x86".to_string()
                    };
                    self.info.user_count = val["NumberOfUsers"].as_u64().unwrap_or(0) as u32;
                    self.info.locale = val["Locale"].as_str().unwrap_or("").to_string();

                    // Parse boot time
                    if let Some(boot_str) = val["LastBootUpTime"].as_str() {
                        // "/Date(1234567890000)/" format
                        if let Some(ts) = boot_str
                            .strip_prefix("/Date(")
                            .and_then(|s| s.strip_suffix(")/"))
                            .and_then(|s| s.parse::<i64>().ok())
                        {
                            self.info.boot_timestamp = (ts / 1000) as u64;
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            self.info.uptime_seconds = now.saturating_sub(self.info.boot_timestamp);
                        }
                    }
                }
            }
        }

        // Kernel version (Windows kernel)
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/c", "ver"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            self.info.kernel_full = text.trim().to_string();
            // Extract version from "Microsoft Windows [Version 10.0.22631.4037]"
            if let Some(start) = text.find("Version ") {
                let rest = &text[start + 8..];
                if let Some(end) = rest.find(']') {
                    self.info.kernel_version = rest[..end].to_string();
                }
            }
        }

        // Boot mode (UEFI or BIOS)
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "$env:firmware_type"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default().trim().to_string();
            self.info.boot_mode = if text.to_lowercase().contains("uefi") {
                BootMode::UEFI
            } else if text.to_lowercase().contains("bios") {
                BootMode::BIOS
            } else {
                BootMode::Unknown
            };
        }

        // Secure Boot
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Confirm-SecureBootUEFI 2>$null"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            self.info.secure_boot = text.trim().to_lowercase() == "true";
        }

        // Timezone
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "(Get-TimeZone).Id"])
            .output()
        {
            self.info.timezone = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }

        // Loaded drivers (kernel modules equivalent)
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_SystemDriver | Where-Object State -eq 'Running' | Select-Object Name, DisplayName, State -First 200 | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for item in &items {
                        self.info.loaded_modules.push(KernelModule {
                            name: item["Name"].as_str().unwrap_or("").to_string(),
                            size_bytes: 0,
                            instances: 1,
                            used_by: Vec::new(),
                            state: item["State"].as_str().unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
    }

    // ── macOS ──

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        self.info.os_family = OsFamily::MacOS;

        // sw_vers for OS info
        if let Ok(output) = std::process::Command::new("sw_vers").output() {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            for line in text.lines() {
                if let Some(val) = line.strip_prefix("ProductName:") {
                    self.info.os_name = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("ProductVersion:") {
                    self.info.os_version = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("BuildVersion:") {
                    self.info.os_build = val.trim().to_string();
                }
            }
        }

        // Kernel
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            self.info.kernel_version = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }
        if let Ok(output) = std::process::Command::new("uname").arg("-v").output() {
            self.info.kernel_full = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }

        // Architecture
        if let Ok(output) = std::process::Command::new("uname").arg("-m").output() {
            self.info.architecture = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }

        // Hostname
        if let Ok(output) = std::process::Command::new("hostname").output() {
            self.info.hostname = String::from_utf8(output.stdout)
                .unwrap_or_default()
                .trim()
                .to_string();
        }

        // Uptime via sysctl
        if let Ok(output) = std::process::Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            // "{ sec = 1234567890, usec = 0 } ..."
            if let Some(sec_str) = text
                .split("sec = ")
                .nth(1)
                .and_then(|s| s.split(',').next())
            {
                if let Ok(boot_sec) = sec_str.trim().parse::<u64>() {
                    self.info.boot_timestamp = boot_sec;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    self.info.uptime_seconds = now.saturating_sub(boot_sec);
                }
            }
        }

        // Boot mode - always UEFI on modern Mac
        self.info.boot_mode = BootMode::UEFI;

        // Secure Boot (Apple silicon always has it)
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPHardwareDataType"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            self.info.secure_boot = text.contains("Apple M") || text.contains("Apple T2");
        }

        // Timezone
        if let Ok(tz) = std::fs::read_link("/etc/localtime") {
            let path = tz.to_string_lossy().to_string();
            if let Some(pos) = path.find("zoneinfo/") {
                self.info.timezone = path[pos + 9..].to_string();
            }
        }

        // Locale
        if let Ok(val) = std::env::var("LANG") {
            self.info.locale = val;
        }

        // Loaded kernel extensions
        if let Ok(output) = std::process::Command::new("kextstat")
            .args(["-l"])
            .output()
        {
            let text = String::from_utf8(output.stdout).unwrap_or_default();
            for line in text.lines().skip(1) {
                // "  Index Refs Address ... Name (Version) <Linked Against>"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 6 {
                    let name = parts[5].to_string();
                    let refs: u32 = parts[1].parse().unwrap_or(0);
                    let size: u64 = u64::from_str_radix(
                        parts[3].trim_start_matches("0x"),
                        16,
                    )
                    .unwrap_or(0);

                    self.info.loaded_modules.push(KernelModule {
                        name,
                        size_bytes: size,
                        instances: refs,
                        used_by: Vec::new(),
                        state: "Live".to_string(),
                    });
                }
            }
        }
    }
}

impl Default for OsInfoMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            info: OsInfo {
                os_family: OsFamily::Unknown,
                os_name: String::new(),
                os_version: String::new(),
                os_build: String::new(),
                kernel_version: String::new(),
                kernel_full: String::new(),
                architecture: String::new(),
                hostname: String::new(),
                domain: String::new(),
                uptime_seconds: 0,
                boot_timestamp: 0,
                boot_mode: BootMode::Unknown,
                kernel_cmdline: String::new(),
                user_count: 0,
                loaded_modules: Vec::new(),
                timezone: String::new(),
                locale: String::new(),
                is_64bit: cfg!(target_pointer_width = "64"),
                secure_boot: false,
            },
        })
    }
}

impl std::fmt::Display for OsFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linux => write!(f, "Linux"),
            Self::Windows => write!(f, "Windows"),
            Self::MacOS => write!(f, "macOS"),
            Self::FreeBSD => write!(f, "FreeBSD"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for BootMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UEFI => write!(f, "UEFI"),
            Self::BIOS => write!(f, "BIOS"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_info_creation() {
        let monitor = OsInfoMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_os_info_default() {
        let monitor = OsInfoMonitor::default();
        let info = monitor.info();
        assert!(!info.hostname.is_empty() || info.os_family == OsFamily::Unknown);
    }

    #[test]
    fn test_os_info_has_data() {
        if let Ok(monitor) = OsInfoMonitor::new() {
            let info = monitor.info();
            // Should have at least an OS family detected
            assert_ne!(info.os_family, OsFamily::Unknown);
            // Should have kernel version
            assert!(!info.kernel_version.is_empty());
        }
    }

    #[test]
    fn test_os_family_display() {
        assert_eq!(OsFamily::Linux.to_string(), "Linux");
        assert_eq!(OsFamily::Windows.to_string(), "Windows");
        assert_eq!(BootMode::UEFI.to_string(), "UEFI");
    }

    #[test]
    fn test_os_info_serialization() {
        let info = OsInfo {
            os_family: OsFamily::Linux,
            os_name: "Ubuntu 22.04".into(),
            os_version: "22.04".into(),
            os_build: "".into(),
            kernel_version: "6.8.0".into(),
            kernel_full: "Linux version 6.8.0".into(),
            architecture: "x86_64".into(),
            hostname: "myhost".into(),
            domain: "".into(),
            uptime_seconds: 3600,
            boot_timestamp: 1700000000,
            boot_mode: BootMode::UEFI,
            kernel_cmdline: "root=/dev/sda1".into(),
            user_count: 1,
            loaded_modules: Vec::new(),
            timezone: "America/New_York".into(),
            locale: "en_US.UTF-8".into(),
            is_64bit: true,
            secure_boot: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("Ubuntu"));
        let _: OsInfo = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_kernel_module_serialization() {
        let module = KernelModule {
            name: "nvidia".into(),
            size_bytes: 1048576,
            instances: 1,
            used_by: vec!["nvidia_uvm".into()],
            state: "Live".into(),
        };
        let json = serde_json::to_string(&module).unwrap();
        assert!(json.contains("nvidia"));
        let _: KernelModule = serde_json::from_str(&json).unwrap();
    }
}

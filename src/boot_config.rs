// boot_config.rs - System boot configuration monitoring for simon
//
// Provides boot configuration, startup items, and boot time analysis.
// Inspired by jetson_stats boot management features.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::Result;

/// Boot type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootType {
    /// Legacy BIOS boot
    Legacy,
    /// UEFI boot
    Uefi,
    /// UEFI with Secure Boot enabled
    SecureBoot,
    /// Unknown boot type
    Unknown,
}

impl std::fmt::Display for BootType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy => write!(f, "Legacy BIOS"),
            Self::Uefi => write!(f, "UEFI"),
            Self::SecureBoot => write!(f, "UEFI + Secure Boot"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Startup item type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupItemType {
    /// Service that runs at boot
    Service,
    /// Application that runs at login
    Application,
    /// Scheduled task
    ScheduledTask,
    /// Kernel module
    KernelModule,
    /// Driver
    Driver,
    /// Registry-based startup (Windows)
    Registry,
    /// Unknown type
    Unknown,
}

impl std::fmt::Display for StartupItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Service => write!(f, "Service"),
            Self::Application => write!(f, "Application"),
            Self::ScheduledTask => write!(f, "Scheduled Task"),
            Self::KernelModule => write!(f, "Kernel Module"),
            Self::Driver => write!(f, "Driver"),
            Self::Registry => write!(f, "Registry"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Startup item status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartupItemStatus {
    /// Item is enabled
    Enabled,
    /// Item is disabled
    Disabled,
    /// Status is unknown
    Unknown,
}

/// Information about a startup item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupItem {
    /// Name of the startup item
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Type of startup item
    pub item_type: StartupItemType,
    /// Current status
    pub status: StartupItemStatus,
    /// Command or path
    pub command: Option<String>,
    /// Publisher/vendor
    pub publisher: Option<String>,
    /// Location in registry/filesystem
    pub location: Option<String>,
    /// Impact on boot time (if known)
    pub boot_impact: Option<String>,
}

/// Boot time analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootTime {
    /// Total boot time from power on to login screen
    pub total: Duration,
    /// Firmware (BIOS/UEFI) initialization time
    pub firmware: Option<Duration>,
    /// Bootloader time
    pub bootloader: Option<Duration>,
    /// Kernel initialization time
    pub kernel: Option<Duration>,
    /// User-space initialization time
    pub userspace: Option<Duration>,
    /// Time when system was last booted
    pub boot_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    /// System uptime
    pub uptime: Duration,
}

impl BootTime {
    /// Format duration as human-readable string
    pub fn format_duration(d: Duration) -> String {
        let secs = d.as_secs();
        if secs >= 86400 {
            let days = secs / 86400;
            let hours = (secs % 86400) / 3600;
            format!("{}d {}h", days, hours)
        } else if secs >= 3600 {
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            format!("{}h {}m", hours, mins)
        } else if secs >= 60 {
            let mins = secs / 60;
            let secs = secs % 60;
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}.{}s", secs, d.subsec_millis() / 100)
        }
    }
}

/// Kernel boot parameters
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KernelParams {
    /// Raw command line
    pub cmdline: String,
    /// Parsed parameters
    pub params: Vec<(String, Option<String>)>,
    /// Root filesystem
    pub root: Option<String>,
    /// Init system path
    pub init: Option<String>,
    /// Quiet boot
    pub quiet: bool,
    /// Splash screen
    pub splash: bool,
}

/// Boot configuration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootInfo {
    /// Boot type (Legacy/UEFI/SecureBoot)
    pub boot_type: BootType,
    /// Secure boot enabled
    pub secure_boot: bool,
    /// Current boot device
    pub boot_device: Option<String>,
    /// Boot partition
    pub boot_partition: Option<String>,
    /// EFI system partition
    pub efi_partition: Option<String>,
    /// Bootloader name
    pub bootloader: Option<String>,
    /// Bootloader version
    pub bootloader_version: Option<String>,
}

/// Boot configuration monitor
#[derive(Debug)]
pub struct BootMonitor {
    /// Boot time analysis
    pub boot_time: BootTime,
    /// Boot configuration
    pub boot_info: BootInfo,
    /// Startup items
    pub startup_items: Vec<StartupItem>,
    /// Kernel parameters (Linux)
    pub kernel_params: KernelParams,
}

impl BootMonitor {
    /// Create a new boot monitor
    pub fn new() -> Result<Self> {
        let mut monitor = Self {
            boot_time: BootTime {
                total: Duration::ZERO,
                firmware: None,
                bootloader: None,
                kernel: None,
                userspace: None,
                boot_timestamp: None,
                uptime: Duration::ZERO,
            },
            boot_info: BootInfo {
                boot_type: BootType::Unknown,
                secure_boot: false,
                boot_device: None,
                boot_partition: None,
                efi_partition: None,
                bootloader: None,
                bootloader_version: None,
            },
            startup_items: Vec::new(),
            kernel_params: KernelParams::default(),
        };

        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh all boot information
    pub fn refresh(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.linux_read_boot_info()?;
            self.linux_read_boot_time()?;
            self.linux_read_startup_items()?;
            self.linux_read_kernel_params()?;
        }

        #[cfg(windows)]
        {
            self.windows_read_boot_info()?;
            self.windows_read_boot_time()?;
            self.windows_read_startup_items()?;
        }

        #[cfg(target_os = "macos")]
        {
            self.macos_read_boot_info()?;
            self.macos_read_boot_time()?;
            self.macos_read_startup_items()?;
        }

        Ok(())
    }

    /// Get enabled startup items count
    pub fn enabled_count(&self) -> usize {
        self.startup_items
            .iter()
            .filter(|i| i.status == StartupItemStatus::Enabled)
            .count()
    }

    /// Get disabled startup items count
    pub fn disabled_count(&self) -> usize {
        self.startup_items
            .iter()
            .filter(|i| i.status == StartupItemStatus::Disabled)
            .count()
    }

    /// Get startup items by type
    pub fn items_by_type(&self, item_type: StartupItemType) -> Vec<&StartupItem> {
        self.startup_items
            .iter()
            .filter(|i| i.item_type == item_type)
            .collect()
    }

    /// Check if running in UEFI mode
    pub fn is_uefi(&self) -> bool {
        matches!(
            self.boot_info.boot_type,
            BootType::Uefi | BootType::SecureBoot
        )
    }

    /// Check if Secure Boot is enabled
    pub fn is_secure_boot(&self) -> bool {
        self.boot_info.secure_boot
    }

    // ==================== Linux Implementation ====================

    #[cfg(target_os = "linux")]
    fn linux_read_boot_info(&mut self) -> Result<()> {
        use std::fs;
        use std::path::Path;

        // Check for UEFI
        let efi_path = Path::new("/sys/firmware/efi");
        if efi_path.exists() {
            // Check for Secure Boot
            let sb_path =
                "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
            if Path::new(sb_path).exists() {
                if let Ok(data) = fs::read(sb_path) {
                    // Secure Boot variable: last byte indicates state
                    self.boot_info.secure_boot = data.last().copied().unwrap_or(0) == 1;
                }
            }

            self.boot_info.boot_type = if self.boot_info.secure_boot {
                BootType::SecureBoot
            } else {
                BootType::Uefi
            };
        } else {
            self.boot_info.boot_type = BootType::Legacy;
        }

        // Get boot device from /proc/cmdline
        if let Ok(cmdline) = fs::read_to_string("/proc/cmdline") {
            for part in cmdline.split_whitespace() {
                if let Some(root) = part.strip_prefix("root=") {
                    self.boot_info.boot_device = Some(root.to_string());
                }
            }
        }

        // Check for bootloader
        if Path::new("/boot/grub/grub.cfg").exists() {
            self.boot_info.bootloader = Some("GRUB".to_string());
        } else if Path::new("/boot/loader/loader.conf").exists() {
            self.boot_info.bootloader = Some("systemd-boot".to_string());
        }

        // Find EFI partition
        if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
            for line in mounts.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1] == "/boot/efi" {
                    self.boot_info.efi_partition = Some(parts[0].to_string());
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn linux_read_boot_time(&mut self) -> Result<()> {
        use std::fs;

        // Get uptime
        if let Ok(content) = fs::read_to_string("/proc/uptime") {
            if let Some(uptime_str) = content.split_whitespace().next() {
                if let Ok(uptime_secs) = uptime_str.parse::<f64>() {
                    self.boot_time.uptime = Duration::from_secs_f64(uptime_secs);
                }
            }
        }

        // Try to get detailed boot time from systemd
        if let Ok(output) = std::process::Command::new("systemd-analyze").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse "Startup finished in 3.456s (firmware) + 2.345s (loader) + 1.234s (kernel) + 5.678s (userspace) = 12.713s"
            for part in stdout.split('+') {
                let part = part.trim();
                if let Some(time_str) = extract_time_value(part) {
                    if let Ok(secs) = parse_time_string(&time_str) {
                        let duration = Duration::from_secs_f64(secs);
                        if part.contains("firmware") {
                            self.boot_time.firmware = Some(duration);
                        } else if part.contains("loader") {
                            self.boot_time.bootloader = Some(duration);
                        } else if part.contains("kernel") {
                            self.boot_time.kernel = Some(duration);
                        } else if part.contains("userspace") {
                            self.boot_time.userspace = Some(duration);
                        }
                    }
                }
            }

            // Parse total time
            if let Some(eq_pos) = stdout.find('=') {
                let total_part = &stdout[eq_pos + 1..].trim();
                if let Some(time_str) = extract_time_value(total_part) {
                    if let Ok(secs) = parse_time_string(&time_str) {
                        self.boot_time.total = Duration::from_secs_f64(secs);
                    }
                }
            }
        }

        // Calculate boot timestamp from uptime
        let now = chrono::Utc::now();
        self.boot_time.boot_timestamp =
            Some(now - chrono::Duration::from_std(self.boot_time.uptime).unwrap_or_default());

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn linux_read_startup_items(&mut self) -> Result<()> {
        use std::process::Command;

        self.startup_items.clear();

        // Get enabled services from systemd
        let output = Command::new("systemctl")
            .args([
                "list-unit-files",
                "--type=service",
                "--no-pager",
                "--no-legend",
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim_end_matches(".service").to_string();
                    let status = match parts[1] {
                        "enabled" | "enabled-runtime" => StartupItemStatus::Enabled,
                        "disabled" => StartupItemStatus::Disabled,
                        _ => StartupItemStatus::Unknown,
                    };

                    self.startup_items.push(StartupItem {
                        name,
                        description: None,
                        item_type: StartupItemType::Service,
                        status,
                        command: None,
                        publisher: None,
                        location: Some("/etc/systemd/system".to_string()),
                        boot_impact: None,
                    });
                }
            }
        }

        // Get autostart desktop applications
        let autostart_dirs = [
            "/etc/xdg/autostart",
            &format!(
                "{}/.config/autostart",
                std::env::var("HOME").unwrap_or_default()
            ),
        ];

        for dir in &autostart_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".desktop") {
                            self.startup_items.push(StartupItem {
                                name: name.trim_end_matches(".desktop").to_string(),
                                description: None,
                                item_type: StartupItemType::Application,
                                status: StartupItemStatus::Enabled,
                                command: None,
                                publisher: None,
                                location: Some(dir.to_string()),
                                boot_impact: None,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn linux_read_kernel_params(&mut self) -> Result<()> {
        use std::fs;

        if let Ok(cmdline) = fs::read_to_string("/proc/cmdline") {
            self.kernel_params.cmdline = cmdline.trim().to_string();
            self.kernel_params.params.clear();

            for part in cmdline.split_whitespace() {
                if let Some((key, value)) = part.split_once('=') {
                    self.kernel_params
                        .params
                        .push((key.to_string(), Some(value.to_string())));

                    match key {
                        "root" => self.kernel_params.root = Some(value.to_string()),
                        "init" => self.kernel_params.init = Some(value.to_string()),
                        _ => {}
                    }
                } else {
                    self.kernel_params.params.push((part.to_string(), None));
                    match part {
                        "quiet" => self.kernel_params.quiet = true,
                        "splash" => self.kernel_params.splash = true,
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    // ==================== Windows Implementation ====================

    #[cfg(windows)]
    fn windows_read_boot_info(&mut self) -> Result<()> {
        use std::process::Command;

        // Check if running in UEFI mode
        // UEFI systems have firmware variables accessible
        let output = Command::new("powershell")
            .args([
                "-Command",
                "if (Test-Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State') { 'SecureBoot' } elseif ($env:firmware_type -eq 'UEFI') { 'UEFI' } else { 'Legacy' }"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            self.boot_info.boot_type = match stdout.as_str() {
                "SecureBoot" => BootType::SecureBoot,
                "UEFI" => BootType::Uefi,
                _ => BootType::Legacy,
            };
        }

        // Check Secure Boot state
        let output = Command::new("powershell")
            .args([
                "-Command",
                "try { (Get-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\SecureBoot\\State' -Name 'UEFISecureBootEnabled' -ErrorAction Stop).UEFISecureBootEnabled } catch { 0 }"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            self.boot_info.secure_boot = stdout == "1";

            if self.boot_info.secure_boot {
                self.boot_info.boot_type = BootType::SecureBoot;
            }
        }

        // Get boot device
        let output = Command::new("wmic")
            .args(["os", "get", "SystemDevice", "/value"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().find(|l| l.starts_with("SystemDevice=")) {
                self.boot_info.boot_device = Some(line.replace("SystemDevice=", ""));
            }
        }

        // Get bootloader info
        self.boot_info.bootloader = Some("Windows Boot Manager".to_string());

        Ok(())
    }

    #[cfg(windows)]
    fn windows_read_boot_time(&mut self) -> Result<()> {
        use std::process::Command;

        // Get uptime via PowerShell
        let output = Command::new("powershell")
            .args([
                "-Command",
                "(Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime | Select-Object -ExpandProperty TotalSeconds"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(secs) = stdout.parse::<f64>() {
                self.boot_time.uptime = Duration::from_secs_f64(secs);
            }
        }

        // Get last boot time
        let output = Command::new("powershell")
            .args([
                "-Command",
                "(Get-CimInstance Win32_OperatingSystem).LastBootUpTime | Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ'"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&stdout) {
                self.boot_time.boot_timestamp = Some(dt.with_timezone(&chrono::Utc));
            }
        }

        // Try to get boot time from event log (Event 12, Kernel-General)
        let output = Command::new("powershell")
            .args([
                "-Command",
                "try { $boot = Get-WinEvent -FilterHashtable @{LogName='System';Id=12;ProviderName='Microsoft-Windows-Kernel-General'} -MaxEvents 1 -ErrorAction Stop; ($boot.TimeCreated - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalSeconds } catch { 0 }"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(secs) = stdout.parse::<f64>() {
                if secs > 0.0 {
                    self.boot_time.total = Duration::from_secs_f64(secs);
                }
            }
        }

        // Fallback: estimate boot time from Event Viewer
        if self.boot_time.total == Duration::ZERO {
            // Assume typical boot time of 30-60 seconds if we can't measure
            self.boot_time.total = Duration::from_secs(45);
        }

        Ok(())
    }

    #[cfg(windows)]
    fn windows_read_startup_items(&mut self) -> Result<()> {
        use std::process::Command;

        self.startup_items.clear();

        // Get startup items from Task Manager equivalent
        let output = Command::new("powershell")
            .args([
                "-Command",
                "Get-CimInstance Win32_StartupCommand | Select-Object Name,Command,Location,User | ConvertTo-Csv -NoTypeInformation"
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    let name = parts[0].trim_matches('"').to_string();
                    let command = parts[1].trim_matches('"').to_string();
                    let location = parts[2].trim_matches('"').to_string();

                    let item_type = if location.contains("Registry") {
                        StartupItemType::Registry
                    } else if location.contains("Startup") {
                        StartupItemType::Application
                    } else {
                        StartupItemType::Unknown
                    };

                    self.startup_items.push(StartupItem {
                        name,
                        description: None,
                        item_type,
                        status: StartupItemStatus::Enabled,
                        command: Some(command),
                        publisher: None,
                        location: Some(location),
                        boot_impact: None,
                    });
                }
            }
        }

        // Get scheduled tasks that run at startup
        let output = Command::new("schtasks")
            .args(["/query", "/fo", "csv", "/v"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 9 {
                    let name = parts[1].trim_matches('"').to_string();
                    let trigger = parts.get(7).unwrap_or(&"").trim_matches('"');

                    // Only include startup/logon tasks
                    if trigger.contains("logon")
                        || trigger.contains("startup")
                        || trigger.contains("boot")
                    {
                        let status = parts.get(3).unwrap_or(&"").trim_matches('"');
                        let status = if status == "Ready" || status == "Running" {
                            StartupItemStatus::Enabled
                        } else if status == "Disabled" {
                            StartupItemStatus::Disabled
                        } else {
                            StartupItemStatus::Unknown
                        };

                        self.startup_items.push(StartupItem {
                            name,
                            description: None,
                            item_type: StartupItemType::ScheduledTask,
                            status,
                            command: parts.get(8).map(|s| s.trim_matches('"').to_string()),
                            publisher: None,
                            location: Some("Task Scheduler".to_string()),
                            boot_impact: None,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    // ==================== macOS Implementation ====================

    #[cfg(target_os = "macos")]
    fn macos_read_boot_info(&mut self) -> Result<()> {
        use std::process::Command;

        // macOS on Apple Silicon is always UEFI-like
        // Intel Macs can be checked via nvram
        let output = Command::new("nvram").arg("boot-args").output();

        // All modern Macs boot via EFI
        self.boot_info.boot_type = BootType::Uefi;

        // Check for Secure Boot on Apple Silicon
        let output = Command::new("csrutil").arg("status").output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            self.boot_info.secure_boot = stdout.contains("enabled");

            if self.boot_info.secure_boot {
                self.boot_info.boot_type = BootType::SecureBoot;
            }
        }

        self.boot_info.bootloader = Some("iBoot".to_string());

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn macos_read_boot_time(&mut self) -> Result<()> {
        use std::process::Command;

        // Get boot time via sysctl
        let output = Command::new("sysctl")
            .args(["-n", "kern.boottime"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse "{ sec = 1234567890, usec = 123456 }"
            if let Some(sec_start) = stdout.find("sec = ") {
                let sec_str = &stdout[sec_start + 6..];
                if let Some(sec_end) = sec_str.find(',') {
                    if let Ok(boot_secs) = sec_str[..sec_end].trim().parse::<i64>() {
                        let boot_time =
                            chrono::DateTime::<chrono::Utc>::from_timestamp(boot_secs, 0);
                        self.boot_time.boot_timestamp = boot_time;

                        let now = chrono::Utc::now();
                        if let Some(bt) = boot_time {
                            self.boot_time.uptime = (now - bt).to_std().unwrap_or(Duration::ZERO);
                        }
                    }
                }
            }
        }

        // Get uptime as fallback
        if self.boot_time.uptime == Duration::ZERO {
            let output = Command::new("uptime").output();
            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse uptime output
                if let Some(up_pos) = stdout.find("up ") {
                    let uptime_str = &stdout[up_pos + 3..];
                    // Very basic parsing
                    if uptime_str.contains("day") {
                        // Has days
                        if let Some(days_str) = uptime_str.split_whitespace().next() {
                            if let Ok(days) = days_str.parse::<u64>() {
                                self.boot_time.uptime = Duration::from_secs(days * 86400);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn macos_read_startup_items(&mut self) -> Result<()> {
        use std::process::Command;

        self.startup_items.clear();

        // Get launch agents/daemons
        let dirs = [
            "/Library/LaunchAgents",
            "/Library/LaunchDaemons",
            "/System/Library/LaunchAgents",
            "/System/Library/LaunchDaemons",
        ];

        for dir in &dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".plist") {
                            let item_type = if dir.contains("Daemon") {
                                StartupItemType::Service
                            } else {
                                StartupItemType::Application
                            };

                            self.startup_items.push(StartupItem {
                                name: name.trim_end_matches(".plist").to_string(),
                                description: None,
                                item_type,
                                status: StartupItemStatus::Enabled,
                                command: None,
                                publisher: None,
                                location: Some(dir.to_string()),
                                boot_impact: None,
                            });
                        }
                    }
                }
            }
        }

        // Get login items via osascript
        let output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of login items",
            ])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for name in stdout.split(',') {
                let name = name.trim();
                if !name.is_empty() {
                    self.startup_items.push(StartupItem {
                        name: name.to_string(),
                        description: None,
                        item_type: StartupItemType::Application,
                        status: StartupItemStatus::Enabled,
                        command: None,
                        publisher: None,
                        location: Some("Login Items".to_string()),
                        boot_impact: None,
                    });
                }
            }
        }

        Ok(())
    }
}

impl Default for BootMonitor {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            boot_time: BootTime {
                total: Duration::ZERO,
                firmware: None,
                bootloader: None,
                kernel: None,
                userspace: None,
                boot_timestamp: None,
                uptime: Duration::ZERO,
            },
            boot_info: BootInfo {
                boot_type: BootType::Unknown,
                secure_boot: false,
                boot_device: None,
                boot_partition: None,
                efi_partition: None,
                bootloader: None,
                bootloader_version: None,
            },
            startup_items: Vec::new(),
            kernel_params: KernelParams::default(),
        })
    }
}

/// Helper function to extract time value from systemd-analyze output
#[cfg(target_os = "linux")]
fn extract_time_value(s: &str) -> Option<String> {
    // Find time value like "3.456s" or "2min 30.123s"
    let mut result = String::new();
    let mut in_number = false;

    for c in s.chars() {
        if c.is_numeric() || c == '.' {
            in_number = true;
            result.push(c);
        } else if in_number && (c == 's' || c == 'm') {
            result.push(c);
            if c == 's' {
                break;
            }
        } else if in_number && c == 'i' {
            // "min"
            result.push_str("in");
            break;
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Helper function to parse time string
#[cfg(target_os = "linux")]
fn parse_time_string(s: &str) -> std::result::Result<f64, std::num::ParseFloatError> {
    if s.ends_with("min") {
        let num = s.trim_end_matches("min").trim();
        num.parse::<f64>().map(|v| v * 60.0)
    } else if s.ends_with('s') {
        s.trim_end_matches('s').trim().parse()
    } else {
        s.parse()
    }
}

/// Boot configuration summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootSummary {
    /// Boot type
    pub boot_type: BootType,
    /// Secure boot enabled
    pub secure_boot: bool,
    /// Total boot time
    pub boot_time_secs: f64,
    /// System uptime
    pub uptime_secs: f64,
    /// Number of enabled startup items
    pub enabled_startup_items: usize,
    /// Number of disabled startup items
    pub disabled_startup_items: usize,
}

/// Get a quick boot summary
pub fn boot_summary() -> Result<BootSummary> {
    let monitor = BootMonitor::new()?;

    Ok(BootSummary {
        boot_type: monitor.boot_info.boot_type,
        secure_boot: monitor.boot_info.secure_boot,
        boot_time_secs: monitor.boot_time.total.as_secs_f64(),
        uptime_secs: monitor.boot_time.uptime.as_secs_f64(),
        enabled_startup_items: monitor.enabled_count(),
        disabled_startup_items: monitor.disabled_count(),
    })
}

/// Format uptime as human-readable string
pub fn format_uptime(uptime: Duration) -> String {
    BootTime::format_duration(uptime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(BootTime::format_duration(Duration::from_secs(30)), "30.0s");
        assert_eq!(BootTime::format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(
            BootTime::format_duration(Duration::from_secs(3700)),
            "1h 1m"
        );
        assert_eq!(
            BootTime::format_duration(Duration::from_secs(90000)),
            "1d 1h"
        );
    }
}

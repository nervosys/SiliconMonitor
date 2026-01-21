//! Unified Process Monitoring with GPU Attribution
//!
//! This module provides cross-platform process monitoring with GPU usage attribution.
//! It combines system-wide process enumeration with GPU-specific process tracking from
//! NVIDIA NVML, AMD sysfs, and Intel GPU drivers.
//!
//! The [`ProcessMonitor`] correlates system processes with GPU usage by matching process IDs
//! (PIDs) from GPU driver data with information from `/proc` (Linux), task manager (Windows),
//! or similar platform-specific sources.
//!
//! # Examples
//!
//! ## Basic Process Monitoring
//!
//! ```no_run
//! use simon::{ProcessMonitor, GpuCollection};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create monitor with GPU attribution
//! let gpus = GpuCollection::auto_detect()?;
//! let mut monitor = ProcessMonitor::with_gpus(gpus)?;
//!
//! // Get all processes
//! let processes = monitor.processes()?;
//! println!("Total processes: {}", processes.len());
//!
//! // Get GPU processes only
//! let gpu_processes = monitor.gpu_processes()?;
//! println!("GPU processes: {}", gpu_processes.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Top GPU Consumers
//!
//! ```no_run
//! use simon::{ProcessMonitor, GpuCollection};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let gpus = GpuCollection::auto_detect()?;
//! let mut monitor = ProcessMonitor::with_gpus(gpus)?;
//!
//! // Get top 10 processes by GPU memory usage
//! let top_gpu = monitor.processes_by_gpu_memory()?;
//! println!("Top GPU consumers:");
//! for proc in top_gpu.iter().take(10) {
//!     println!("  {} (PID {}): {} MB on {} GPUs",
//!         proc.name,
//!         proc.pid,
//!         proc.total_gpu_memory_bytes / 1024 / 1024,
//!         proc.gpu_indices.len()
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Top CPU Consumers
//!
//! ```no_run
//! use simon::{ProcessMonitor, GpuCollection};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let gpus = GpuCollection::auto_detect()?;
//! let mut monitor = ProcessMonitor::with_gpus(gpus)?;
//!
//! // Get top 10 processes by CPU usage
//! let top_cpu = monitor.processes_by_cpu()?;
//! println!("Top CPU consumers:");
//! for proc in top_cpu.iter().take(10) {
//!     println!("  {} (PID {}): {:.1}%",
//!         proc.name,
//!         proc.pid,
//!         proc.cpu_percent
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Monitor Specific Process
//!
//! ```no_run
//! use simon::{ProcessMonitor, GpuCollection};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let gpus = GpuCollection::auto_detect()?;
//! let mut monitor = ProcessMonitor::with_gpus(gpus)?;
//!
//! // Get specific process by PID
//! if let Some(proc) = monitor.process_by_pid(1234)? {
//!     println!("Process: {}", proc.name);
//!     println!("CPU: {:.1}%", proc.cpu_percent);
//!     println!("Memory: {} MB", proc.memory_bytes / 1024 / 1024);
//!     println!("GPU Memory: {} MB", proc.total_gpu_memory_bytes / 1024 / 1024);
//!     println!("Using GPUs: {:?}", proc.gpu_indices);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Platform Support
//!
//! | Platform | Process Enum | GPU Attribution | CPU % | Memory | User |
//! |----------|--------------|-----------------|-------|--------|------|
//! | Linux    | ✅ /proc      | ✅ All vendors  | ✅    | ✅     | ✅   |
//! | Windows  | 🚧 Stubs     | 🚧              | 🚧    | 🚧     | 🚧   |
//! | macOS    | 🚧 Stubs     | 🚧              | 🚧    | 🚧     | 🚧   |

use crate::error::{Result, SimonError};
use crate::gpu::GpuCollection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Process category for smart classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcessCategory {
    /// System kernel and core OS processes
    System,
    /// System services and daemons
    Service,
    /// Desktop environment and window managers
    Desktop,
    /// Web browsers
    Browser,
    /// Development tools (IDEs, compilers, debuggers)
    Development,
    /// AI/ML frameworks and tools
    AiMl,
    /// Games and game engines
    Gaming,
    /// Media players and editors
    Media,
    /// Communication apps (chat, video calls)
    Communication,
    /// Productivity apps (office, notes)
    Productivity,
    /// Containers and virtualization
    Container,
    /// Network and security tools
    Network,
    /// Database servers
    Database,
    /// GPU compute and graphics
    GpuCompute,
    /// Shell and terminal processes
    Shell,
    /// User applications (uncategorized)
    Application,
    /// Unknown/unclassified processes
    Unknown,
}

impl ProcessCategory {
    /// Classify a process by its name and characteristics
    pub fn classify(name: &str, user: Option<&str>, is_gpu_process: bool) -> Self {
        let name_lower = name.to_lowercase();

        // GPU processes get special handling
        if is_gpu_process {
            // AI/ML specific
            if Self::matches_any(
                &name_lower,
                &[
                    "python",
                    "python3",
                    "jupyter",
                    "conda",
                    "pytorch",
                    "tensorflow",
                    "torch",
                    "ollama",
                    "llama",
                    "whisper",
                    "stable-diffusion",
                    "comfyui",
                    "automatic1111",
                    "onnx",
                    "triton",
                    "vllm",
                    "tgi",
                ],
            ) {
                return Self::AiMl;
            }
            // Games
            if Self::matches_any(
                &name_lower,
                &[
                    "steam", "game", "unity", "unreal", "godot", "wine", "proton", "lutris",
                    "heroic", "bottles",
                ],
            ) {
                return Self::Gaming;
            }
            return Self::GpuCompute;
        }

        // System processes (PID 1, kernel threads, etc.)
        if Self::matches_any(
            &name_lower,
            &[
                "init",
                "systemd",
                "kernel",
                "kthread",
                "ksoftirq",
                "kworker",
                "rcu_",
                "migration",
                "watchdog",
                "cpuhp",
                "idle",
                "swapper",
                "launchd",
                "system",
                "csrss",
                "smss",
                "wininit",
                "services",
                "lsass",
                "svchost",
                "dwm",
                "ntoskrnl",
            ],
        ) {
            return Self::System;
        }

        // Services and daemons
        if Self::matches_any(
            &name_lower,
            &[
                "cron",
                "crond",
                "atd",
                "cupsd",
                "avahi",
                "dbus",
                "udev",
                "polkit",
                "udisks",
                "accounts-daemon",
                "colord",
                "fwupd",
                "gdm",
                "lightdm",
                "sddm",
                "login",
                "getty",
                "agetty",
                "su",
                "sudo",
                "ssh",
                "sshd",
                "rsyslog",
                "journald",
                "logind",
                "networkmanager",
                "wpa_supplicant",
                "dhclient",
                "thermald",
                "irqbalance",
                "snapd",
                "flatpak",
                "packagekit",
                "apt",
                "dnf",
                "yum",
                "pacman",
                "zypper",
            ],
        ) {
            return Self::Service;
        }

        // Desktop environments
        if Self::matches_any(
            &name_lower,
            &[
                "gnome",
                "kde",
                "plasma",
                "xfce",
                "mate",
                "cinnamon",
                "lxde",
                "lxqt",
                "i3",
                "sway",
                "hyprland",
                "awesome",
                "bspwm",
                "dwm",
                "openbox",
                "fluxbox",
                "xorg",
                "x11",
                "wayland",
                "mutter",
                "kwin",
                "picom",
                "compton",
                "compositor",
                "nautilus",
                "dolphin",
                "thunar",
                "nemo",
                "caja",
                "pcmanfm",
                "explorer",
                "finder",
                "gvfs",
                "tracker",
                "baloo",
                "mimeapps",
                "xdg-",
            ],
        ) {
            return Self::Desktop;
        }

        // Browsers
        if Self::matches_any(
            &name_lower,
            &[
                "firefox",
                "chrome",
                "chromium",
                "brave",
                "edge",
                "safari",
                "opera",
                "vivaldi",
                "librewolf",
                "waterfox",
                "tor-browser",
                "qutebrowser",
                "web-content",
                "webextension",
                "gpu-process",
            ],
        ) {
            return Self::Browser;
        }

        // Development tools
        if Self::matches_any(
            &name_lower,
            &[
                "code",
                "vscode",
                "codium",
                "vim",
                "nvim",
                "neovim",
                "emacs",
                "sublime",
                "atom",
                "jetbrains",
                "idea",
                "pycharm",
                "webstorm",
                "clion",
                "rider",
                "goland",
                "rust-analyzer",
                "gopls",
                "clangd",
                "pylsp",
                "tsserver",
                "node",
                "npm",
                "yarn",
                "pnpm",
                "cargo",
                "rustc",
                "gcc",
                "g++",
                "clang",
                "make",
                "cmake",
                "ninja",
                "git",
                "gh",
                "gdb",
                "lldb",
                "valgrind",
                "strace",
                "ltrace",
                "perf",
                "htop",
                "btop",
                "top",
                "docker-compose",
                "kubectl",
            ],
        ) {
            return Self::Development;
        }

        // AI/ML tools (non-GPU or pre-GPU detection)
        if Self::matches_any(
            &name_lower,
            &[
                "python", "python3", "jupyter", "ipython", "conda", "pip", "poetry", "pdm", "uv",
                "ruff", "mypy",
            ],
        ) {
            return Self::AiMl;
        }

        // Media
        if Self::matches_any(
            &name_lower,
            &[
                "vlc",
                "mpv",
                "mplayer",
                "totem",
                "celluloid",
                "parole",
                "rhythmbox",
                "spotify",
                "audacious",
                "clementine",
                "lollypop",
                "gimp",
                "inkscape",
                "krita",
                "blender",
                "kdenlive",
                "shotcut",
                "obs",
                "ffmpeg",
                "handbrake",
                "audacity",
                "ardour",
                "lmms",
                "darktable",
                "rawtherapee",
                "digikam",
                "shotwell",
                "eog",
                "gwenview",
                "feh",
                "sxiv",
                "mpd",
                "pulseaudio",
                "pipewire",
                "wireplumber",
                "alsa",
                "jack",
            ],
        ) {
            return Self::Media;
        }

        // Communication
        if Self::matches_any(
            &name_lower,
            &[
                "discord",
                "slack",
                "teams",
                "zoom",
                "skype",
                "telegram",
                "signal",
                "element",
                "matrix",
                "thunderbird",
                "evolution",
                "geary",
                "mutt",
                "neomutt",
                "weechat",
                "irssi",
                "hexchat",
            ],
        ) {
            return Self::Communication;
        }

        // Productivity
        if Self::matches_any(
            &name_lower,
            &[
                "libreoffice",
                "soffice",
                "writer",
                "calc",
                "impress",
                "obsidian",
                "notion",
                "joplin",
                "simplenote",
                "standard-notes",
                "zettlr",
                "logseq",
                "roam",
                "okular",
                "evince",
                "zathura",
                "calibre",
                "foliate",
                "gnome-calendar",
                "gnome-contacts",
            ],
        ) {
            return Self::Productivity;
        }

        // Containers & Virtualization
        if Self::matches_any(
            &name_lower,
            &[
                "docker",
                "containerd",
                "runc",
                "cri-o",
                "podman",
                "buildah",
                "skopeo",
                "kubernetes",
                "kubelet",
                "k3s",
                "k8s",
                "minikube",
                "qemu",
                "kvm",
                "libvirt",
                "virt-manager",
                "virtualbox",
                "vmware",
                "vagrant",
                "lxc",
                "lxd",
                "incus",
                "systemd-nspawn",
            ],
        ) {
            return Self::Container;
        }

        // Network & Security
        if Self::matches_any(
            &name_lower,
            &[
                "nginx",
                "apache",
                "httpd",
                "caddy",
                "traefik",
                "haproxy",
                "squid",
                "dnsmasq",
                "bind",
                "named",
                "unbound",
                "pihole",
                "openvpn",
                "wireguard",
                "iptables",
                "nftables",
                "firewalld",
                "ufw",
                "fail2ban",
                "snort",
                "suricata",
                "wireshark",
                "tcpdump",
                "nmap",
                "curl",
                "wget",
                "rsync",
                "syncthing",
                "rclone",
            ],
        ) {
            return Self::Network;
        }

        // Databases
        if Self::matches_any(
            &name_lower,
            &[
                "postgres",
                "postgresql",
                "mysql",
                "mariadb",
                "sqlite",
                "mongodb",
                "redis",
                "memcached",
                "elasticsearch",
                "opensearch",
                "cassandra",
                "couchdb",
                "influxdb",
                "clickhouse",
                "duckdb",
            ],
        ) {
            return Self::Database;
        }

        // Gaming (non-GPU or waiting for GPU)
        if Self::matches_any(
            &name_lower,
            &[
                "steam",
                "steamwebhelper",
                "game",
                "unity",
                "unreal",
                "godot",
                "wine",
                "proton",
                "lutris",
                "heroic",
                "bottles",
                "gamescope",
                "mangohud",
                "gamemode",
            ],
        ) {
            return Self::Gaming;
        }

        // Shells
        if Self::matches_any(
            &name_lower,
            &[
                "bash",
                "zsh",
                "fish",
                "sh",
                "dash",
                "ksh",
                "tcsh",
                "csh",
                "powershell",
                "pwsh",
                "cmd",
                "terminal",
                "konsole",
                "gnome-terminal",
                "alacritty",
                "kitty",
                "wezterm",
                "foot",
                "tilix",
                "terminator",
                "tmux",
                "screen",
                "byobu",
            ],
        ) {
            return Self::Shell;
        }

        // Check user context - root/system users often run services
        if let Some(u) = user {
            let u_lower = u.to_lowercase();
            if u_lower == "root" || u_lower == "system" || u_lower.starts_with("_") {
                return Self::Service;
            }
        }

        Self::Unknown
    }

    /// Check if name matches any pattern (substring match)
    fn matches_any(name: &str, patterns: &[&str]) -> bool {
        patterns.iter().any(|p| name.contains(p))
    }

    /// Get display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Service => "Services",
            Self::Desktop => "Desktop",
            Self::Browser => "Browsers",
            Self::Development => "Development",
            Self::AiMl => "AI/ML",
            Self::Gaming => "Gaming",
            Self::Media => "Media",
            Self::Communication => "Communication",
            Self::Productivity => "Productivity",
            Self::Container => "Containers",
            Self::Network => "Network",
            Self::Database => "Database",
            Self::GpuCompute => "GPU Compute",
            Self::Shell => "Shell",
            Self::Application => "Applications",
            Self::Unknown => "Other",
        }
    }

    /// Get an emoji/icon for the category
    pub fn icon(&self) -> &'static str {
        match self {
            Self::System => "⚙️",
            Self::Service => "🔧",
            Self::Desktop => "🖥️",
            Self::Browser => "🌐",
            Self::Development => "💻",
            Self::AiMl => "🤖",
            Self::Gaming => "🎮",
            Self::Media => "🎵",
            Self::Communication => "💬",
            Self::Productivity => "📝",
            Self::Container => "📦",
            Self::Network => "🌍",
            Self::Database => "🗄️",
            Self::GpuCompute => "🔥",
            Self::Shell => "🐚",
            Self::Application => "📱",
            Self::Unknown => "❓",
        }
    }

    /// Get all categories in display order
    pub fn all() -> &'static [ProcessCategory] {
        &[
            Self::AiMl,
            Self::GpuCompute,
            Self::Gaming,
            Self::Browser,
            Self::Development,
            Self::Media,
            Self::Communication,
            Self::Productivity,
            Self::Container,
            Self::Database,
            Self::Network,
            Self::Desktop,
            Self::Service,
            Self::System,
            Self::Shell,
            Self::Application,
            Self::Unknown,
        ]
    }
}

impl std::fmt::Display for ProcessCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl Default for ProcessCategory {
    fn default() -> Self {
        Self::Unknown
    }
}

/// GPU process type classification for process monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessGpuType {
    /// Process uses graphics rendering (3D, OpenGL, Vulkan, DirectX)
    Graphical,
    /// Process uses compute workloads (CUDA, OpenCL, SYCL)
    Compute,
    /// Process uses both graphics and compute
    GraphicalCompute,
    /// Unknown or unable to determine
    Unknown,
}

impl ProcessGpuType {
    /// Create from engine usage pattern
    pub fn from_engine_usage(gfx: u64, compute: u64) -> Self {
        match (gfx > 0, compute > 0) {
            (true, true) => Self::GraphicalCompute,
            (true, false) => Self::Graphical,
            (false, true) => Self::Compute,
            (false, false) => Self::Unknown,
        }
    }
}

impl std::fmt::Display for ProcessGpuType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphical => write!(f, "Graphics"),
            Self::Compute => write!(f, "Compute"),
            Self::GraphicalCompute => write!(f, "Gfx+Compute"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Unified process information with GPU attribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMonitorInfo {
    /// Process ID
    pub pid: u32,
    /// Parent Process ID
    pub parent_pid: Option<u32>,
    /// Process name/command
    pub name: String,
    /// User running the process
    pub user: Option<String>,
    /// Process category (smart classification)
    pub category: ProcessCategory,
    /// CPU usage percentage (0-100 per core, can exceed 100 on multi-core)
    pub cpu_percent: f32,
    /// Memory usage in bytes (Working Set / RSS)
    pub memory_bytes: u64,
    /// Virtual memory size in bytes
    pub virtual_memory_bytes: u64,
    /// Private memory in bytes (unique to this process)
    pub private_bytes: u64,
    /// Number of threads
    pub thread_count: u32,
    /// Number of open handles (Windows) or file descriptors (Unix)
    pub handle_count: u32,
    /// I/O read bytes (total since process start)
    pub io_read_bytes: u64,
    /// I/O write bytes (total since process start)
    pub io_write_bytes: u64,
    /// Process start time (Unix timestamp in seconds)
    pub start_time: Option<u64>,
    /// GPU indices this process is using
    pub gpu_indices: Vec<usize>,
    /// GPU memory usage per GPU (GPU index -> memory in bytes)
    pub gpu_memory_per_device: HashMap<usize, u64>,
    /// Total GPU memory used across all GPUs
    pub total_gpu_memory_bytes: u64,
    /// Process state (R=Running, S=Sleeping, D=Disk sleep, Z=Zombie, T=Stopped)
    pub state: char,
    /// Process priority/nice value
    pub priority: Option<i32>,

    // nvtop feature parity: Per-process engine utilization
    /// GPU graphics engine time used (nanoseconds)
    pub gfx_engine_used: Option<u64>,
    /// GPU compute engine time used (nanoseconds)
    pub compute_engine_used: Option<u64>,
    /// GPU encoder time used (nanoseconds)
    pub enc_engine_used: Option<u64>,
    /// GPU decoder time used (nanoseconds)
    pub dec_engine_used: Option<u64>,
    /// GPU usage percentage (0-100)
    pub gpu_usage_percent: Option<f32>,
    /// Encoder usage percentage (0-100)
    pub encoder_usage_percent: Option<f32>,
    /// Decoder usage percentage (0-100)
    pub decoder_usage_percent: Option<f32>,
    /// GPU process type (Graphics, Compute, Mixed)
    pub gpu_process_type: ProcessGpuType,
    /// GPU memory percentage of total device memory
    pub gpu_memory_percentage: Option<f32>,
}

impl ProcessMonitorInfo {
    /// Get total CPU usage percentage
    pub fn cpu_usage(&self) -> f32 {
        self.cpu_percent
    }

    /// Get memory usage in megabytes
    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get total GPU memory usage in megabytes
    pub fn gpu_memory_mb(&self) -> f64 {
        self.total_gpu_memory_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Check if process is using any GPU
    pub fn is_gpu_process(&self) -> bool {
        !self.gpu_indices.is_empty()
    }

    /// Get number of GPUs used by this process
    pub fn gpu_count(&self) -> usize {
        self.gpu_indices.len()
    }

    /// Get the process category
    pub fn category(&self) -> ProcessCategory {
        self.category
    }

    /// Reclassify the process (e.g., after GPU attribution is added)
    pub fn reclassify(&mut self) {
        self.category =
            ProcessCategory::classify(&self.name, self.user.as_deref(), self.is_gpu_process());
    }
}

/// Statistics for a process category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    /// The category
    pub category: ProcessCategory,
    /// Number of processes in this category
    pub process_count: usize,
    /// Number of GPU-using processes in this category
    pub gpu_process_count: usize,
    /// Total CPU usage percentage across all processes
    pub total_cpu_percent: f32,
    /// Total memory usage in bytes
    pub total_memory_bytes: u64,
    /// Total GPU memory usage in bytes
    pub total_gpu_memory_bytes: u64,
}

impl CategoryStats {
    /// Get memory usage in MB
    pub fn memory_mb(&self) -> f64 {
        self.total_memory_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get GPU memory usage in MB
    pub fn gpu_memory_mb(&self) -> f64 {
        self.total_gpu_memory_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Process monitor that combines system and GPU process information
pub struct ProcessMonitor {
    /// GPU collection for GPU process tracking
    gpu_collection: Option<GpuCollection>,
    /// Cache of last update time (for CPU percentage calculation)
    last_update: std::time::Instant,
}

impl ProcessMonitor {
    /// Create a new process monitor
    ///
    /// Automatically detects available GPUs for GPU process attribution.
    pub fn new() -> Result<Self> {
        let gpu_collection = GpuCollection::auto_detect().ok();

        Ok(Self {
            gpu_collection,
            last_update: std::time::Instant::now(),
        })
    }

    /// Create a process monitor with a pre-initialized GPU collection
    ///
    /// This is useful when you already have a [`GpuCollection`] instance
    /// and want to reuse it for process monitoring.
    pub fn with_gpus(gpu_collection: GpuCollection) -> Result<Self> {
        Ok(Self {
            gpu_collection: Some(gpu_collection),
            last_update: std::time::Instant::now(),
        })
    }

    /// Create a process monitor without GPU tracking
    pub fn without_gpu() -> Result<Self> {
        Ok(Self {
            gpu_collection: None,
            last_update: std::time::Instant::now(),
        })
    }

    /// Get all running processes with GPU attribution
    pub fn processes(&mut self) -> Result<Vec<ProcessMonitorInfo>> {
        // Get system processes
        let mut system_processes = self.get_system_processes()?;

        // Add GPU information if available
        if let Some(ref gpu_collection) = self.gpu_collection {
            self.add_gpu_attribution(&mut system_processes, gpu_collection)?;

            // Reclassify processes after GPU attribution (GPU processes may change category)
            for proc in &mut system_processes {
                if proc.is_gpu_process() {
                    proc.reclassify();
                }
            }
        }

        self.last_update = std::time::Instant::now();

        Ok(system_processes)
    }

    /// Get processes sorted by CPU usage (descending)
    pub fn processes_by_cpu(&mut self) -> Result<Vec<ProcessMonitorInfo>> {
        let mut procs = self.processes()?;
        procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(procs)
    }

    /// Get processes sorted by memory usage (descending)
    pub fn processes_by_memory(&mut self) -> Result<Vec<ProcessMonitorInfo>> {
        let mut procs = self.processes()?;
        procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        Ok(procs)
    }

    /// Get processes sorted by GPU memory usage (descending)
    pub fn processes_by_gpu_memory(&mut self) -> Result<Vec<ProcessMonitorInfo>> {
        let mut procs = self.processes()?;
        procs.sort_by(|a, b| b.total_gpu_memory_bytes.cmp(&a.total_gpu_memory_bytes));
        Ok(procs)
    }

    /// Get only GPU-using processes
    pub fn gpu_processes(&mut self) -> Result<Vec<ProcessMonitorInfo>> {
        let procs = self.processes()?;
        Ok(procs.into_iter().filter(|p| p.is_gpu_process()).collect())
    }

    /// Get process by PID
    pub fn process_by_pid(&mut self, pid: u32) -> Result<Option<ProcessMonitorInfo>> {
        let procs = self.processes()?;
        Ok(procs.into_iter().find(|p| p.pid == pid))
    }

    /// Get processes filtered by category
    pub fn processes_by_category(
        &mut self,
        category: ProcessCategory,
    ) -> Result<Vec<ProcessMonitorInfo>> {
        let procs = self.processes()?;
        Ok(procs
            .into_iter()
            .filter(|p| p.category == category)
            .collect())
    }

    /// Get processes grouped by category
    pub fn processes_grouped_by_category(
        &mut self,
    ) -> Result<HashMap<ProcessCategory, Vec<ProcessMonitorInfo>>> {
        let procs = self.processes()?;
        let mut grouped: HashMap<ProcessCategory, Vec<ProcessMonitorInfo>> = HashMap::new();

        for proc in procs {
            grouped.entry(proc.category).or_default().push(proc);
        }

        Ok(grouped)
    }

    /// Get category statistics (count and total CPU/memory per category)
    pub fn category_stats(&mut self) -> Result<Vec<CategoryStats>> {
        let grouped = self.processes_grouped_by_category()?;
        let mut stats: Vec<CategoryStats> = grouped
            .into_iter()
            .map(|(category, procs)| {
                let count = procs.len();
                let total_cpu: f32 = procs.iter().map(|p| p.cpu_percent).sum();
                let total_memory: u64 = procs.iter().map(|p| p.memory_bytes).sum();
                let total_gpu_memory: u64 = procs.iter().map(|p| p.total_gpu_memory_bytes).sum();
                let gpu_process_count = procs.iter().filter(|p| p.is_gpu_process()).count();

                CategoryStats {
                    category,
                    process_count: count,
                    gpu_process_count,
                    total_cpu_percent: total_cpu,
                    total_memory_bytes: total_memory,
                    total_gpu_memory_bytes: total_gpu_memory,
                }
            })
            .collect();

        // Sort by total CPU usage descending
        stats.sort_by(|a, b| {
            b.total_cpu_percent
                .partial_cmp(&a.total_cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(stats)
    }

    /// Get total number of running processes
    pub fn process_count(&mut self) -> Result<usize> {
        Ok(self.processes()?.len())
    }

    /// Get number of GPU-using processes
    pub fn gpu_process_count(&mut self) -> Result<usize> {
        Ok(self.gpu_processes()?.len())
    }

    /// Get number of detected GPUs
    pub fn gpu_count(&self) -> usize {
        self.gpu_collection.as_ref().map(|gc| gc.len()).unwrap_or(0)
    }

    /// Update process list (refresh data)
    ///
    /// This method triggers a refresh of the internal process cache.
    /// The actual update happens automatically in `processes()`, so this
    /// is primarily for explicit refresh semantics in user code.
    pub fn update(&mut self) -> Result<()> {
        // Refresh by calling processes() and discarding result
        let _ = self.processes()?;
        Ok(())
    }

    // Platform-specific system process enumeration
    #[cfg(target_os = "linux")]
    fn get_system_processes(&self) -> Result<Vec<ProcessMonitorInfo>> {
        linux::enumerate_processes()
    }

    #[cfg(target_os = "windows")]
    fn get_system_processes(&self) -> Result<Vec<ProcessMonitorInfo>> {
        windows_impl::enumerate_processes()
    }

    #[cfg(target_os = "macos")]
    fn get_system_processes(&self) -> Result<Vec<ProcessMonitorInfo>> {
        macos::enumerate_processes()
    }

    /// Add GPU attribution to processes
    fn add_gpu_attribution(
        &self,
        processes: &mut [ProcessMonitorInfo],
        gpu_collection: &GpuCollection,
    ) -> Result<()> {
        // Create a map of PID -> Process for quick lookup
        let mut process_map: HashMap<u32, &mut ProcessMonitorInfo> =
            processes.iter_mut().map(|p| (p.pid, p)).collect();

        // Iterate through each GPU and its processes
        for (gpu_idx, gpu) in gpu_collection.gpus().iter().enumerate() {
            if let Ok(gpu_processes) = gpu.processes() {
                for gpu_proc in gpu_processes {
                    let pid = gpu_proc.pid;
                    let gpu_mem = gpu_proc.memory_usage.unwrap_or(0);

                    if let Some(proc_info) = process_map.get_mut(&pid) {
                        // Add this GPU to the process's GPU list
                        if !proc_info.gpu_indices.contains(&gpu_idx) {
                            proc_info.gpu_indices.push(gpu_idx);
                        }

                        // Add GPU memory for this device
                        proc_info.gpu_memory_per_device.insert(gpu_idx, gpu_mem);
                        proc_info.total_gpu_memory_bytes += gpu_mem;

                        // Copy user from GPU process if not already set
                        if proc_info.user.is_none() && !gpu_proc.user.is_empty() {
                            proc_info.user = Some(gpu_proc.user.clone());
                        }

                        // Copy GPU utilization data
                        if gpu_proc.gpu_usage.is_some() {
                            proc_info.gpu_usage_percent = gpu_proc.gpu_usage.map(|u| u as f32);
                        }
                        if gpu_proc.encoder_usage.is_some() {
                            proc_info.encoder_usage_percent =
                                gpu_proc.encoder_usage.map(|u| u as f32);
                        }
                        if gpu_proc.decoder_usage.is_some() {
                            proc_info.decoder_usage_percent =
                                gpu_proc.decoder_usage.map(|u| u as f32);
                        }
                        if gpu_proc.memory_usage_percent.is_some() {
                            proc_info.gpu_memory_percentage =
                                gpu_proc.memory_usage_percent.map(|u| u as f32);
                        }

                        // Update process type based on GPU process type
                        proc_info.gpu_process_type = match gpu_proc.process_type {
                            crate::gpu::GpuProcessType::Graphics => ProcessGpuType::Graphical,
                            crate::gpu::GpuProcessType::Compute => ProcessGpuType::Compute,
                            crate::gpu::GpuProcessType::GraphicsAndCompute => {
                                ProcessGpuType::GraphicalCompute
                            }
                            crate::gpu::GpuProcessType::Unknown => ProcessGpuType::Unknown,
                        };
                    }
                }
            }
        }

        Ok(())
    }

    /// Kill a process by PID
    ///
    /// This method attempts to terminate a process. On Unix systems, it sends SIGTERM
    /// by default, which allows the process to clean up. On Windows, it terminates
    /// the process forcefully.
    ///
    /// # Safety
    ///
    /// Killing processes requires appropriate permissions:
    /// - On Linux/Unix: Must have permission to send signals to the target process
    /// - On Windows: Must have PROCESS_TERMINATE access rights
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to terminate
    /// * `force` - If true, use SIGKILL (Unix) or forceful termination (Windows)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use simon::{ProcessMonitor, GpuCollection};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let gpus = GpuCollection::auto_detect()?;
    /// let mut monitor = ProcessMonitor::with_gpus(gpus)?;
    ///
    /// // Gracefully terminate process 1234
    /// monitor.kill_process(1234, false)?;
    ///
    /// // Force kill process 5678
    /// monitor.kill_process(5678, true)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn kill_process(&self, pid: u32, force: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            use std::process::Command;

            let signal = if force { "KILL" } else { "TERM" };
            let status = Command::new("kill")
                .arg(format!("-{}", signal))
                .arg(pid.to_string())
                .status()
                .map_err(|e| SimonError::Io(e))?;

            if !status.success() {
                return Err(SimonError::Other(format!(
                    "Failed to kill process {}: {}",
                    pid,
                    status.code().unwrap_or(-1)
                )));
            }

            Ok(())
        }

        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::CloseHandle;
            use windows::Win32::System::Threading::{
                OpenProcess, TerminateProcess, PROCESS_TERMINATE,
            };

            let _ = force; // Windows always uses forceful termination

            unsafe {
                let handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| {
                    SimonError::Other(format!("Failed to open process {}: {}", pid, e))
                })?;

                if handle.is_invalid() {
                    return Err(SimonError::Other(format!(
                        "Invalid handle for process {}",
                        pid
                    )));
                }

                let result = TerminateProcess(handle, 1);
                let _ = CloseHandle(handle);

                if result.is_err() {
                    return Err(SimonError::Other(format!(
                        "Failed to terminate process {}",
                        pid
                    )));
                }

                Ok(())
            }
        }

        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            let signal = if force { "KILL" } else { "TERM" };
            let status = Command::new("kill")
                .arg(format!("-{}", signal))
                .arg(pid.to_string())
                .status()
                .map_err(|e| SimonError::Io(e))?;

            if !status.success() {
                return Err(SimonError::Other(format!(
                    "Failed to kill process {}: {}",
                    pid,
                    status.code().unwrap_or(-1)
                )));
            }

            Ok(())
        }

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = (pid, force);
            Err(SimonError::UnsupportedPlatform(
                "Process termination not supported on this platform".to_string(),
            ))
        }
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            gpu_collection: None,
            last_update: std::time::Instant::now(),
        })
    }
}

// Linux-specific process enumeration
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::fs;
    use std::path::Path;

    pub fn enumerate_processes() -> Result<Vec<ProcessMonitorInfo>> {
        let mut processes = Vec::new();

        let proc_dir = Path::new("/proc");
        if !proc_dir.exists() {
            return Err(SimonError::UnsupportedPlatform(
                "/proc filesystem not available".to_string(),
            ));
        }

        // Read uptime for CPU calculation
        let uptime = read_uptime()?;

        // Iterate through /proc entries
        for entry in fs::read_dir(proc_dir)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            // Check if directory name is a number (PID)
            if let Ok(pid) = filename_str.parse::<u32>() {
                if let Ok(proc_info) = read_process_info(pid, uptime) {
                    processes.push(proc_info);
                }
            }
        }

        Ok(processes)
    }

    fn read_uptime() -> Result<f64> {
        let uptime_str = fs::read_to_string("/proc/uptime")?;
        let uptime: f64 = uptime_str
            .split_whitespace()
            .next()
            .ok_or_else(|| SimonError::Parse("Invalid uptime format".to_string()))?
            .parse()
            .map_err(|e| SimonError::Parse(format!("Failed to parse uptime: {}", e)))?;
        Ok(uptime)
    }

    fn read_process_info(pid: u32, uptime: f64) -> Result<ProcessMonitorInfo> {
        let proc_path = format!("/proc/{}", pid);
        let proc_dir = Path::new(&proc_path);

        if !proc_dir.exists() {
            return Err(SimonError::DeviceNotFound(format!(
                "Process {} not found",
                pid
            )));
        }

        // Read /proc/[pid]/stat
        let stat_path = format!("{}/stat", proc_path);
        let stat_content = fs::read_to_string(&stat_path)?;

        // Parse stat file (fields documented in proc(5) man page)
        let (name, stat_fields) = parse_stat_line(&stat_content)?;

        if stat_fields.len() < 22 {
            return Err(SimonError::Parse(
                "Insufficient fields in stat file".to_string(),
            ));
        }

        // Extract fields (0-indexed after splitting on ')')
        let state = stat_fields[0].chars().next().unwrap_or('?');
        let utime: u64 = stat_fields[11].parse().unwrap_or(0);
        let stime: u64 = stat_fields[12].parse().unwrap_or(0);
        let priority: i32 = stat_fields[15].parse().unwrap_or(0);
        let starttime: u64 = stat_fields[19].parse().unwrap_or(0);

        // Calculate CPU percentage
        let clk_tck = 100.0; // SC_CLK_TCK, typically 100
        let total_time = (utime + stime) as f64 / clk_tck;
        let seconds_since_boot = uptime;
        let proc_uptime = (seconds_since_boot - (starttime as f64 / clk_tck)).max(1.0);
        let cpu_percent = ((total_time / proc_uptime) * 100.0) as f32;

        // Read /proc/[pid]/statm for memory
        let statm_path = format!("{}/statm", proc_path);
        let memory_bytes = if let Ok(statm_content) = fs::read_to_string(&statm_path) {
            let parts: Vec<&str> = statm_content.split_whitespace().collect();
            if parts.len() > 1 {
                // RSS (Resident Set Size) in pages, multiply by page size (typically 4KB)
                parts[1].parse::<u64>().unwrap_or(0) * 4096
            } else {
                0
            }
        } else {
            0
        };

        // Try to read user
        let user = read_process_user(pid);

        // Classify the process
        let category = ProcessCategory::classify(&name, user.as_deref(), false);

        Ok(ProcessMonitorInfo {
            pid,
            parent_pid: None, // TODO: Parse from /proc/[pid]/stat
            name,
            user,
            category,
            cpu_percent,
            memory_bytes,
            virtual_memory_bytes: 0, // TODO: Parse from /proc/[pid]/statm
            private_bytes: 0,        // TODO: Parse from /proc/[pid]/smaps
            thread_count: 0,         // TODO: Parse from /proc/[pid]/stat
            handle_count: 0,         // TODO: Count /proc/[pid]/fd
            io_read_bytes: 0,        // TODO: Parse from /proc/[pid]/io
            io_write_bytes: 0,       // TODO: Parse from /proc/[pid]/io
            start_time: None,        // TODO: Parse from /proc/[pid]/stat
            gpu_indices: Vec::new(),
            gpu_memory_per_device: HashMap::new(),
            total_gpu_memory_bytes: 0,
            state,
            priority: Some(priority),
            gfx_engine_used: None,
            compute_engine_used: None,
            enc_engine_used: None,
            dec_engine_used: None,
            gpu_usage_percent: None,
            encoder_usage_percent: None,
            decoder_usage_percent: None,
            gpu_process_type: ProcessGpuType::Unknown,
            gpu_memory_percentage: None,
        })
    }

    fn parse_stat_line(stat: &str) -> Result<(String, Vec<String>)> {
        // Format: pid (name) state ...
        // Name can contain spaces and parentheses, so we need to find the last ')'
        let start = stat
            .find('(')
            .ok_or_else(|| SimonError::Parse("No opening parenthesis in stat".to_string()))?;
        let end = stat
            .rfind(')')
            .ok_or_else(|| SimonError::Parse("No closing parenthesis in stat".to_string()))?;

        let name = stat[start + 1..end].to_string();
        let rest = &stat[end + 2..]; // Skip ') '

        let fields: Vec<String> = rest.split_whitespace().map(|s| s.to_string()).collect();

        Ok((name, fields))
    }

    fn read_process_user(pid: u32) -> Option<String> {
        // Read UID from /proc/[pid]/status
        let status_path = format!("/proc/{}/status", pid);
        if let Ok(content) = fs::read_to_string(&status_path) {
            for line in content.lines() {
                if line.starts_with("Uid:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() > 1 {
                        // Get real UID (first number after "Uid:")
                        if let Ok(uid) = parts[1].parse::<u32>() {
                            // Try to resolve UID to username
                            return get_username_from_uid(uid);
                        }
                    }
                }
            }
        }
        None
    }

    fn get_username_from_uid(uid: u32) -> Option<String> {
        // Simple approach: read /etc/passwd
        if let Ok(content) = fs::read_to_string("/etc/passwd") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() > 2 {
                    if let Ok(line_uid) = parts[2].parse::<u32>() {
                        if line_uid == uid {
                            return Some(parts[0].to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

// Windows-specific process enumeration
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::Security::{
        GetTokenInformation, LookupAccountSidW, TokenUser, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER,
    };
    use ::windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use ::windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
    };
    use ::windows::Win32::System::Threading::{
        GetPriorityClass, GetProcessHandleCount, GetProcessIoCounters, GetProcessTimes,
        OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, IO_COUNTERS,
        PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    /// Get the username of a process by PID
    fn get_process_user(pid: u32) -> Option<String> {
        unsafe {
            // Try with limited access first (works on more processes)
            let process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);

            if let Ok(handle) = process_handle {
                if !handle.is_invalid() {
                    let mut token_handle = ::windows::Win32::Foundation::HANDLE::default();

                    if OpenProcessToken(handle, TOKEN_QUERY, &mut token_handle).is_ok() {
                        // Get token user info size
                        let mut return_length = 0u32;
                        let _ = GetTokenInformation(
                            token_handle,
                            TokenUser,
                            None,
                            0,
                            &mut return_length,
                        );

                        if return_length > 0 {
                            let mut token_info = vec![0u8; return_length as usize];
                            if GetTokenInformation(
                                token_handle,
                                TokenUser,
                                Some(token_info.as_mut_ptr() as *mut _),
                                return_length,
                                &mut return_length,
                            )
                            .is_ok()
                            {
                                let token_user = &*(token_info.as_ptr() as *const TOKEN_USER);

                                // Lookup username from SID
                                let mut name_buf = [0u16; 256];
                                let mut name_len = name_buf.len() as u32;
                                let mut domain_buf = [0u16; 256];
                                let mut domain_len = domain_buf.len() as u32;
                                let mut sid_type = SID_NAME_USE::default();

                                if LookupAccountSidW(
                                    None,
                                    token_user.User.Sid,
                                    ::windows::core::PWSTR(name_buf.as_mut_ptr()),
                                    &mut name_len,
                                    ::windows::core::PWSTR(domain_buf.as_mut_ptr()),
                                    &mut domain_len,
                                    &mut sid_type,
                                )
                                .is_ok()
                                {
                                    let _ = CloseHandle(token_handle);
                                    let _ = CloseHandle(handle);

                                    let name =
                                        String::from_utf16_lossy(&name_buf[..name_len as usize]);
                                    return Some(name);
                                }
                            }
                        }
                        let _ = CloseHandle(token_handle);
                    }
                    let _ = CloseHandle(handle);
                }
            }
        }
        None
    }

    pub fn enumerate_processes() -> Result<Vec<ProcessMonitorInfo>> {
        let mut processes = Vec::new();

        unsafe {
            // Take a snapshot of all processes
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| {
                SimonError::Other(format!("Failed to create process snapshot: {}", e))
            })?;

            if snapshot.is_invalid() {
                return Err(SimonError::Other("Invalid snapshot handle".to_string()));
            }

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            // Get first process
            if Process32FirstW(snapshot, &mut entry).is_err() {
                let _ = CloseHandle(snapshot);
                return Ok(processes);
            }

            // Iterate through all processes
            loop {
                let pid = entry.th32ProcessID;

                // Try to open process for querying
                let process_handle =
                    OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);

                if let Ok(handle) = process_handle {
                    if !handle.is_invalid() {
                        // Get process name
                        let mut name_buffer = [0u16; 260];
                        let mut name_len = name_buffer.len() as u32;

                        use ::windows::core::PWSTR;
                        let name = if QueryFullProcessImageNameW(
                            handle,
                            ::windows::Win32::System::Threading::PROCESS_NAME_WIN32,
                            PWSTR(name_buffer.as_mut_ptr()),
                            &mut name_len,
                        )
                        .is_ok()
                        {
                            let os_string = OsString::from_wide(&name_buffer[..name_len as usize]);
                            std::path::Path::new(&os_string)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string()
                        } else {
                            // Fallback to executable name from snapshot
                            let null_pos = entry
                                .szExeFile
                                .iter()
                                .position(|&c| c == 0)
                                .unwrap_or(entry.szExeFile.len());
                            String::from_utf16_lossy(&entry.szExeFile[..null_pos])
                        };

                        // Get memory info
                        let mut mem_counters = PROCESS_MEMORY_COUNTERS_EX::default();
                        let (memory_bytes, virtual_memory_bytes, private_bytes) =
                            if GetProcessMemoryInfo(
                                handle,
                                std::ptr::addr_of_mut!(mem_counters) as *mut _,
                                std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                            )
                            .is_ok()
                            {
                                (
                                    mem_counters.WorkingSetSize as u64,
                                    mem_counters.PagefileUsage as u64,
                                    mem_counters.PrivateUsage as u64,
                                )
                            } else {
                                (0, 0, 0)
                            };

                        // Get handle count
                        let mut handle_count: u32 = 0;
                        let _ = GetProcessHandleCount(handle, &mut handle_count);

                        // Get I/O counters
                        let mut io_counters = IO_COUNTERS::default();
                        let (io_read_bytes, io_write_bytes) =
                            if GetProcessIoCounters(handle, &mut io_counters).is_ok() {
                                (
                                    io_counters.ReadTransferCount,
                                    io_counters.WriteTransferCount,
                                )
                            } else {
                                (0, 0)
                            };

                        // Get priority class
                        let priority_class = GetPriorityClass(handle);

                        // Get CPU times
                        let mut creation_time = Default::default();
                        let mut exit_time = Default::default();
                        let mut kernel_time = Default::default();
                        let mut user_time = Default::default();

                        let (_cpu_time_ms, start_time) = if GetProcessTimes(
                            handle,
                            &mut creation_time,
                            &mut exit_time,
                            &mut kernel_time,
                            &mut user_time,
                        )
                        .is_ok()
                        {
                            // Convert FILETIME (100ns units) to milliseconds
                            let kernel_100ns = (kernel_time.dwHighDateTime as u64) << 32
                                | (kernel_time.dwLowDateTime as u64);
                            let user_100ns = (user_time.dwHighDateTime as u64) << 32
                                | (user_time.dwLowDateTime as u64);

                            // Convert creation_time FILETIME to Unix timestamp
                            // FILETIME is 100ns intervals since Jan 1, 1601
                            // Unix epoch is Jan 1, 1970 - difference is 116444736000000000 (100ns intervals)
                            let creation_100ns = (creation_time.dwHighDateTime as u64) << 32
                                | (creation_time.dwLowDateTime as u64);
                            let unix_epoch_diff: u64 = 116444736000000000;
                            let start_unix = if creation_100ns > unix_epoch_diff {
                                Some((creation_100ns - unix_epoch_diff) / 10_000_000)
                            // Convert to seconds
                            } else {
                                None
                            };

                            (((kernel_100ns + user_100ns) / 10_000) as u64, start_unix)
                        } else {
                            (0, None)
                        };

                        // Get user
                        let user = get_process_user(pid);

                        // Classify the process
                        let category = ProcessCategory::classify(&name, user.as_deref(), false);

                        processes.push(ProcessMonitorInfo {
                            pid,
                            name,
                            user,
                            category,
                            cpu_percent: 0.0, // Would need multiple samples to calculate
                            memory_bytes,
                            gpu_indices: Vec::new(),
                            gpu_memory_per_device: HashMap::new(),
                            total_gpu_memory_bytes: 0,
                            state: 'R', // Windows doesn't expose state easily - assume running
                            priority: Some(priority_class as i32),
                            gfx_engine_used: None,
                            compute_engine_used: None,
                            enc_engine_used: None,
                            dec_engine_used: None,
                            gpu_usage_percent: None,
                            encoder_usage_percent: None,
                            decoder_usage_percent: None,
                            gpu_process_type: ProcessGpuType::Unknown,
                            gpu_memory_percentage: None,
                            // New Windows-specific fields
                            parent_pid: Some(entry.th32ParentProcessID),
                            virtual_memory_bytes,
                            private_bytes,
                            thread_count: entry.cntThreads,
                            handle_count,
                            io_read_bytes,
                            io_write_bytes,
                            start_time,
                        });

                        let _ = CloseHandle(handle);
                    }
                }

                // Move to next process
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }

            let _ = CloseHandle(snapshot);
        }

        Ok(processes)
    }
}

// macOS-specific process enumeration
#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::ffi::CStr;
    use std::mem;

    // macOS proc_info constants
    const PROC_PIDTASKALLINFO: i32 = 2;
    const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

    // FFI declarations for libproc
    #[link(name = "proc", kind = "dylib")]
    extern "C" {
        fn proc_listpids(proc_type: u32, type_info: u32, buffer: *mut u8, buffer_size: i32) -> i32;

        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut u8, buffer_size: i32) -> i32;

        fn proc_pidpath(pid: i32, buffer: *mut u8, buffer_size: u32) -> i32;
    }

    #[repr(C)]
    struct ProcTaskAllInfo {
        pbsd: ProcBsdInfo,
        ptinfo: ProcTaskInfo,
    }

    #[repr(C)]
    struct ProcBsdInfo {
        pbi_flags: u32,
        pbi_status: u32,
        pbi_xstatus: u32,
        pbi_pid: u32,
        pbi_ppid: u32,
        pbi_uid: u32,
        pbi_gid: u32,
        pbi_ruid: u32,
        pbi_rgid: u32,
        pbi_svuid: u32,
        pbi_svgid: u32,
        _pad1: u32,
        pbi_comm: [u8; 16],
        pbi_name: [u8; 32],
        pbi_nfiles: u32,
        pbi_pgid: u32,
        pbi_pjobc: u32,
        e_tdev: u32,
        e_tpgid: u32,
        pbi_nice: i32,
        pbi_start_tvsec: u64,
        pbi_start_tvusec: u64,
    }

    #[repr(C)]
    struct ProcTaskInfo {
        pti_virtual_size: u64,
        pti_resident_size: u64,
        pti_total_user: u64,
        pti_total_system: u64,
        pti_threads_user: u64,
        pti_threads_system: u64,
        pti_policy: i32,
        pti_faults: i32,
        pti_pageins: i32,
        pti_cow_faults: i32,
        pti_messages_sent: i32,
        pti_messages_received: i32,
        pti_syscalls_mach: i32,
        pti_syscalls_unix: i32,
        pti_csw: i32,
        pti_threadnum: i32,
        pti_numrunning: i32,
        pti_priority: i32,
    }

    pub fn enumerate_processes() -> Result<Vec<ProcessMonitorInfo>> {
        let mut processes = Vec::new();

        unsafe {
            // First, get the number of processes
            let num_pids = proc_listpids(1, 0, std::ptr::null_mut(), 0); // PROC_ALL_PIDS = 1
            if num_pids <= 0 {
                return Err(SimonError::Other("Failed to get process count".to_string()));
            }

            // Allocate buffer for PIDs
            let buffer_size = num_pids * mem::size_of::<i32>() as i32;
            let mut pid_buffer = vec![0i32; (num_pids / mem::size_of::<i32>() as i32) as usize];

            // Get all PIDs
            let actual_size = proc_listpids(
                1, // PROC_ALL_PIDS
                0,
                pid_buffer.as_mut_ptr() as *mut u8,
                buffer_size,
            );

            if actual_size <= 0 {
                return Err(SimonError::Other("Failed to list processes".to_string()));
            }

            let num_processes = (actual_size / mem::size_of::<i32>() as i32) as usize;

            // Iterate through each PID
            for &pid in &pid_buffer[..num_processes] {
                if pid <= 0 {
                    continue;
                }

                // Get process info
                let mut task_info: ProcTaskAllInfo = mem::zeroed();
                let info_size = proc_pidinfo(
                    pid,
                    PROC_PIDTASKALLINFO,
                    0,
                    &mut task_info as *mut _ as *mut u8,
                    mem::size_of::<ProcTaskAllInfo>() as i32,
                );

                if info_size <= 0 {
                    continue; // Process may have exited
                }

                // Get process path
                let mut path_buffer = [0u8; PROC_PIDPATHINFO_MAXSIZE];
                let path_len = proc_pidpath(
                    pid,
                    path_buffer.as_mut_ptr(),
                    PROC_PIDPATHINFO_MAXSIZE as u32,
                );

                let name = if path_len > 0 {
                    // Extract filename from path
                    let path_slice = &path_buffer[..path_len as usize];
                    if let Ok(path_str) = CStr::from_bytes_until_nul(path_slice)
                        .map(|c| c.to_string_lossy().to_string())
                    {
                        std::path::Path::new(&path_str)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    } else {
                        // Fallback to comm name
                        let null_pos = task_info
                            .pbsd
                            .pbi_comm
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(task_info.pbsd.pbi_comm.len());
                        String::from_utf8_lossy(&task_info.pbsd.pbi_comm[..null_pos]).to_string()
                    }
                } else {
                    // Use process name from pbsd info
                    let null_pos = task_info
                        .pbsd
                        .pbi_comm
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(task_info.pbsd.pbi_comm.len());
                    String::from_utf8_lossy(&task_info.pbsd.pbi_comm[..null_pos]).to_string()
                };

                // Convert times from microseconds to milliseconds
                let cpu_time_ms =
                    (task_info.ptinfo.pti_total_user + task_info.ptinfo.pti_total_system) / 1000;

                // Status mapping
                let status = match task_info.pbsd.pbi_status {
                    1 => "Idle",
                    2 => "Running",
                    3 => "Sleeping",
                    4 => "Stopped",
                    5 => "Zombie",
                    _ => "Unknown",
                }
                .to_string();

                let user_str = format!("{}", task_info.pbsd.pbi_uid);
                let category = ProcessCategory::classify(&name, Some(&user_str), false);

                processes.push(ProcessMonitorInfo {
                    pid: pid as u32,
                    parent_pid: Some(task_info.pbsd.pbi_ppid),
                    name,
                    user: Some(user_str),
                    category,
                    cpu_percent: 0.0, // Would need multiple samples to calculate
                    memory_bytes: task_info.ptinfo.pti_resident_size,
                    virtual_memory_bytes: task_info.ptinfo.pti_virtual_size,
                    private_bytes: 0, // Not easily available on macOS
                    thread_count: 0,  // TODO: Get from task_info
                    handle_count: 0,  // Not available on macOS
                    io_read_bytes: 0, // Would need ioreg
                    io_write_bytes: 0,
                    start_time: Some(task_info.pbsd.pbi_start_tvsec),
                    gpu_indices: Vec::new(),
                    gpu_memory_per_device: HashMap::new(),
                    total_gpu_memory_bytes: 0,
                    state: status.chars().next().unwrap_or('U'), // First char of status
                    priority: Some(task_info.ptinfo.pti_priority),
                    gfx_engine_used: None,
                    compute_engine_used: None,
                    enc_engine_used: None,
                    dec_engine_used: None,
                    gpu_usage_percent: None,
                    encoder_usage_percent: None,
                    decoder_usage_percent: None,
                    gpu_process_type: ProcessGpuType::Unknown,
                    gpu_memory_percentage: None,
                });
            }
        }

        Ok(processes)
    }
}

//! Unified monitoring backend for CLI, TUI, and GUI modes
//!
//! This module provides a single, consistent interface for accessing all hardware
//! monitoring data and the AI agent system. All frontends (CLI, TUI, GUI) should
//! use this backend to ensure consistent behavior.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::agent::{Agent, AgentConfig, AgentResponse, ModelSize};
use crate::connections::{ConnectionInfo, ConnectionMonitor, ConnectionState, Protocol};
use crate::core::cpu::CpuStats;
use crate::core::memory::MemoryStats;
use crate::disk::{self, DiskDevice};
use crate::error::{Result, SimonError};
use crate::gpu::{GpuCollection, GpuDynamicInfo, GpuStaticInfo};
use crate::motherboard::{self, DriverInfo, MotherboardDevice, SystemInfo as MBSystemInfo};
use crate::network_monitor::NetworkMonitor;
use crate::process_monitor::{ProcessMonitor, ProcessMonitorInfo};
use crate::system_stats::SystemStats;
use crate::SiliconMonitor;

/// Default history buffer size for time-series data
pub const DEFAULT_HISTORY_SIZE: usize = 60;

/// Default update interval
pub const DEFAULT_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

// ============================================================================
// UNIFIED SYSTEM STATE FOR AI AGENT
// ============================================================================

/// Complete system state snapshot for AI agent context
/// This extends the GPU-only SystemState to include all hardware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSystemState {
    /// CPU state
    pub cpu: Option<CpuState>,

    /// Memory state
    pub memory: Option<MemoryState>,

    /// GPU/Accelerator states
    pub accelerators: Vec<AcceleratorState>,

    /// Disk states
    pub disks: Vec<DiskState>,

    /// Network interface states
    pub network: Vec<NetworkState>,

    /// Top processes by resource usage
    pub top_processes: Vec<ProcessState>,

    /// System information
    pub system: Option<SystemInfoState>,

    /// Timestamp of state capture
    pub timestamp: u64,
}

/// CPU state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuState {
    /// CPU model name
    pub name: String,

    /// Number of cores
    pub cores: usize,

    /// Number of threads
    pub threads: usize,

    /// Overall utilization (0-100%)
    pub utilization: f32,

    /// Temperature (Celsius)
    pub temperature: Option<f32>,

    /// Current frequency (MHz)
    pub frequency_mhz: Option<u64>,

    /// Per-core utilization
    pub per_core_usage: Vec<f32>,
}

/// Memory state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryState {
    /// Total RAM (bytes)
    pub total_bytes: u64,

    /// Used RAM (bytes)
    pub used_bytes: u64,

    /// Available RAM (bytes)
    pub available_bytes: u64,

    /// Usage percentage
    pub usage_percent: f32,

    /// Total swap (bytes)
    pub swap_total_bytes: u64,

    /// Used swap (bytes)
    pub swap_used_bytes: u64,

    /// Swap usage percentage
    pub swap_usage_percent: f32,
}

/// Accelerator state (GPU/NPU/FPGA/etc.) for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceleratorState {
    /// Accelerator index
    pub index: usize,

    /// Accelerator type
    pub accel_type: String,

    /// Device name
    pub name: String,

    /// Vendor name
    pub vendor: String,

    /// Utilization (0-100%)
    pub utilization: f32,

    /// Memory used (bytes)
    pub memory_used_bytes: u64,

    /// Memory total (bytes)
    pub memory_total_bytes: u64,

    /// Memory usage percentage
    pub memory_usage_percent: f32,

    /// Temperature (Celsius)
    pub temperature: Option<f32>,

    /// Power usage (Watts)
    pub power_watts: Option<f32>,

    /// Power limit (Watts)
    pub power_limit_watts: Option<f32>,

    /// Core clock (MHz)
    pub clock_mhz: Option<u32>,

    /// Memory clock (MHz)
    pub memory_clock_mhz: Option<u32>,

    /// Number of processes using this accelerator
    pub process_count: usize,
}

/// Disk state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskState {
    /// Disk name/model
    pub name: String,

    /// Mount point
    pub mount_point: String,

    /// Filesystem type
    pub filesystem: String,

    /// Total space (bytes)
    pub total_bytes: u64,

    /// Used space (bytes)
    pub used_bytes: u64,

    /// Usage percentage
    pub usage_percent: f32,
}

/// Network interface state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkState {
    /// Interface name
    pub name: String,

    /// Is interface up
    pub is_up: bool,

    /// Bytes received
    pub rx_bytes: u64,

    /// Bytes transmitted
    pub tx_bytes: u64,

    /// Receive rate (bytes/sec)
    pub rx_rate: f64,

    /// Transmit rate (bytes/sec)
    pub tx_rate: f64,
}

/// Process state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessState {
    /// Process ID
    pub pid: u32,

    /// Process name
    pub name: String,

    /// CPU usage percentage
    pub cpu_percent: f32,

    /// Memory usage (bytes)
    pub memory_bytes: u64,

    /// GPU memory usage (bytes)
    pub gpu_memory_bytes: u64,

    /// GPU indices used
    pub gpu_indices: Vec<usize>,
}

/// System information state for AI context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoState {
    /// Hostname
    pub hostname: String,

    /// Operating system
    pub os: String,

    /// Kernel version
    pub kernel: String,

    /// System uptime (seconds)
    pub uptime_secs: u64,
}

impl FullSystemState {
    /// Create an empty system state
    pub fn empty() -> Self {
        Self {
            cpu: None,
            memory: None,
            accelerators: Vec::new(),
            disks: Vec::new(),
            network: Vec::new(),
            top_processes: Vec::new(),
            system: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Format state as natural language text for AI agent context
    pub fn to_context_string(&self) -> String {
        let mut ctx = String::new();
        ctx.push_str("=== Current System State ===\n\n");

        // System info
        if let Some(ref sys) = self.system {
            ctx.push_str(&format!(
                "Host: {} | OS: {} | Kernel: {} | Uptime: {}h {}m\n\n",
                sys.hostname,
                sys.os,
                sys.kernel,
                sys.uptime_secs / 3600,
                (sys.uptime_secs % 3600) / 60
            ));
        }

        // CPU
        if let Some(ref cpu) = self.cpu {
            ctx.push_str(&format!(
                "CPU: {} ({} cores)\n  Utilization: {:.1}%",
                cpu.name, cpu.cores, cpu.utilization
            ));
            if let Some(temp) = cpu.temperature {
                ctx.push_str(&format!(" | Temp: {:.0}°C", temp));
            }
            if let Some(freq) = cpu.frequency_mhz {
                ctx.push_str(&format!(" | Freq: {} MHz", freq));
            }
            ctx.push_str("\n\n");
        }

        // Memory
        if let Some(ref mem) = self.memory {
            let used_gb = mem.used_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
            let total_gb = mem.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
            ctx.push_str(&format!(
                "Memory: {:.1}GB / {:.1}GB ({:.1}%)\n",
                used_gb, total_gb, mem.usage_percent
            ));
            if mem.swap_total_bytes > 0 {
                let swap_used_gb = mem.swap_used_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                let swap_total_gb = mem.swap_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                ctx.push_str(&format!(
                    "Swap: {:.1}GB / {:.1}GB ({:.1}%)\n",
                    swap_used_gb, swap_total_gb, mem.swap_usage_percent
                ));
            }
            ctx.push_str("\n");
        }

        // Accelerators
        if !self.accelerators.is_empty() {
            ctx.push_str("Accelerators:\n");
            for accel in &self.accelerators {
                ctx.push_str(&format!(
                    "  {} {}: {} ({})\n",
                    accel.accel_type, accel.index, accel.name, accel.vendor
                ));
                ctx.push_str(&format!("    Utilization: {:.0}%", accel.utilization));
                if let Some(temp) = accel.temperature {
                    ctx.push_str(&format!(" | Temp: {:.0}°C", temp));
                }
                if let Some(power) = accel.power_watts {
                    ctx.push_str(&format!(" | Power: {:.0}W", power));
                    if let Some(limit) = accel.power_limit_watts {
                        ctx.push_str(&format!("/{:.0}W", limit));
                    }
                }
                ctx.push('\n');
                let mem_used_gb = accel.memory_used_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                let mem_total_gb = accel.memory_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                ctx.push_str(&format!(
                    "    Memory: {:.1}GB / {:.1}GB ({:.1}%)\n",
                    mem_used_gb, mem_total_gb, accel.memory_usage_percent
                ));
            }
            ctx.push('\n');
        }

        // Disks
        if !self.disks.is_empty() {
            ctx.push_str("Disks:\n");
            for disk in &self.disks {
                let used_gb = disk.used_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                let total_gb = disk.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                ctx.push_str(&format!(
                    "  {}: {:.1}GB / {:.1}GB ({:.1}%) at {}\n",
                    disk.name, used_gb, total_gb, disk.usage_percent, disk.mount_point
                ));
            }
            ctx.push('\n');
        }

        // Top processes
        if !self.top_processes.is_empty() {
            ctx.push_str("Top Processes (by CPU):\n");
            for proc in self.top_processes.iter().take(5) {
                ctx.push_str(&format!(
                    "  {} (PID {}): CPU {:.1}%, Memory {:.0}MB",
                    proc.name,
                    proc.pid,
                    proc.cpu_percent,
                    proc.memory_bytes as f64 / 1024.0 / 1024.0
                ));
                if proc.gpu_memory_bytes > 0 {
                    ctx.push_str(&format!(
                        ", GPU Mem {:.0}MB",
                        proc.gpu_memory_bytes as f64 / 1024.0 / 1024.0
                    ));
                }
                ctx.push('\n');
            }
        }

        ctx
    }
}

// ============================================================================
// HISTORY BUFFER FOR TIME-SERIES DATA
// ============================================================================

/// Generic history buffer for time-series data
#[derive(Debug, Clone)]
pub struct HistoryBuffer<T: Clone> {
    data: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> HistoryBuffer<T> {
    /// Create a new history buffer with given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new value, removing oldest if at capacity
    pub fn push(&mut self, value: T) {
        if self.data.len() >= self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(value);
    }

    /// Get all values as a slice
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.data.iter()
    }

    /// Get the most recent value
    pub fn latest(&self) -> Option<&T> {
        self.data.back()
    }

    /// Get the previous value (second to last)
    pub fn previous(&self) -> Option<&T> {
        self.data.iter().rev().nth(1)
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get number of values in buffer
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Convert to Vec
    pub fn to_vec(&self) -> Vec<T> {
        self.data.iter().cloned().collect()
    }
}

impl<T: Clone + Into<u64>> HistoryBuffer<T> {
    /// Convert values to u64 for sparkline rendering
    pub fn as_u64_vec(&self) -> Vec<u64> {
        self.data.iter().cloned().map(Into::into).collect()
    }
}

impl<T: Clone + Into<f32>> HistoryBuffer<T> {
    /// Convert values to f32 for chart rendering
    pub fn as_f32_vec(&self) -> Vec<f32> {
        self.data.iter().cloned().map(Into::into).collect()
    }
}

// ============================================================================
// UNIFIED MONITORING BACKEND
// ============================================================================

/// Unified monitoring backend providing consistent data access for CLI, TUI, and GUI
pub struct MonitoringBackend {
    // === Hardware monitors ===
    /// GPU/Accelerator collection
    gpu_collection: Option<GpuCollection>,

    /// Process monitor (with GPU attribution)
    process_monitor: Option<ProcessMonitor>,

    /// Network monitor
    network_monitor: Option<NetworkMonitor>,

    /// Connection monitor (netstat-like)
    connection_monitor: Option<ConnectionMonitor>,

    /// Disk devices
    disks: Vec<Box<dyn DiskDevice>>,

    /// Motherboard sensors
    motherboard_sensors: Vec<Box<dyn MotherboardDevice>>,

    // === Cached data ===
    /// Cached CPU stats
    cpu_stats: Option<CpuStats>,

    /// Cached memory stats
    memory_stats: Option<MemoryStats>,

    /// Cached GPU static info
    gpu_static_info: Vec<GpuStaticInfo>,

    /// Cached GPU dynamic info
    gpu_dynamic_info: Vec<GpuDynamicInfo>,

    /// Cached process list
    processes: Vec<ProcessMonitorInfo>,

    /// Cached connection list
    connections: Vec<ConnectionInfo>,

    /// System info
    system_info: Option<MBSystemInfo>,

    /// System stats (load avg, vmstat, etc.)
    system_stats: Option<SystemStats>,

    /// Driver info
    driver_info: Vec<DriverInfo>,

    // === History buffers ===
    /// CPU utilization history
    cpu_history: HistoryBuffer<f32>,

    /// Memory utilization history
    memory_history: HistoryBuffer<f32>,

    /// Per-accelerator utilization history
    accelerator_histories: Vec<HistoryBuffer<f32>>,

    /// Per-accelerator memory history
    accelerator_memory_histories: Vec<HistoryBuffer<f32>>,

    /// Per-accelerator temperature history
    accelerator_temp_histories: Vec<HistoryBuffer<f32>>,

    /// Network RX rate history
    network_rx_history: HistoryBuffer<f32>,

    /// Network TX rate history
    network_tx_history: HistoryBuffer<f32>,

    // === AI Agent ===
    /// AI agent for natural language queries
    agent: Option<Agent>,

    /// SiliconMonitor for agent context
    silicon_monitor: Option<SiliconMonitor>,

    /// Agent response history
    agent_history: VecDeque<AgentResponse>,

    // === Timing ===
    /// Last update time
    last_update: Instant,

    /// Update interval
    update_interval: Duration,

    /// Start time for uptime calculation
    start_time: Instant,

    // === System identification ===
    /// Hostname
    hostname: String,

    /// OS info string
    os_info: String,
}

impl MonitoringBackend {
    /// Create a new monitoring backend with all available monitors
    pub fn new() -> Result<Self> {
        Self::with_config(BackendConfig::default())
    }

    /// Create a new monitoring backend with custom configuration
    pub fn with_config(config: BackendConfig) -> Result<Self> {
        // Initialize GPU collection
        let gpu_collection = GpuCollection::auto_detect().ok();

        // Initialize GPU static info
        let (gpu_static_info, gpu_dynamic_info) = if let Some(ref gpus) = gpu_collection {
            let infos = gpus.snapshot_all().unwrap_or_default();
            let static_info: Vec<GpuStaticInfo> =
                infos.iter().map(|i| i.static_info.clone()).collect();
            let dynamic_info: Vec<GpuDynamicInfo> =
                infos.iter().map(|i| i.dynamic_info.clone()).collect();
            (static_info, dynamic_info)
        } else {
            (Vec::new(), Vec::new())
        };

        let gpu_count = gpu_static_info.len();

        // Initialize process monitor (standalone, will update GPU info later)
        let process_monitor = ProcessMonitor::new().ok();

        // Initialize network monitor
        let network_monitor = NetworkMonitor::new().ok();

        // Initialize connection monitor
        let connection_monitor = ConnectionMonitor::new().ok();

        // Initialize disk devices
        let disks = disk::enumerate_disks().unwrap_or_default();

        // Initialize motherboard sensors
        let motherboard_sensors = motherboard::enumerate_sensors().unwrap_or_default();

        // Get system info
        let system_info = motherboard::get_system_info().ok();

        // Get driver info
        let driver_info = motherboard::get_driver_versions().unwrap_or_default();

        // Get system stats
        let system_stats = SystemStats::new().ok();

        // Get hostname and OS info
        #[cfg(any(feature = "cli", feature = "gui"))]
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        #[cfg(not(any(feature = "cli", feature = "gui")))]
        let hostname = "unknown".to_string();

        let os_info = std::env::consts::OS.to_string();

        // Initialize AI agent if enabled (uses auto-detect to find available backends)
        let agent = if config.enable_agent {
            AgentConfig::auto_detect()
                .ok()
                .map(|agent_cfg| {
                    agent_cfg
                        .with_caching(true)
                        .with_cache_size(50)
                        .with_timeout(Duration::from_secs(config.agent_timeout_secs))
                })
                .and_then(|agent_cfg| Agent::new(agent_cfg).ok())
        } else {
            None
        };

        // Create SiliconMonitor for agent (uses shared GPU collection)
        let silicon_monitor = SiliconMonitor::new().ok();

        let mut backend = Self {
            gpu_collection,
            process_monitor,
            network_monitor,
            connection_monitor,
            disks,
            motherboard_sensors,
            cpu_stats: None,
            memory_stats: None,
            gpu_static_info,
            gpu_dynamic_info,
            processes: Vec::new(),
            connections: Vec::new(),
            system_info,
            system_stats,
            driver_info,
            cpu_history: HistoryBuffer::new(config.history_size),
            memory_history: HistoryBuffer::new(config.history_size),
            accelerator_histories: (0..gpu_count)
                .map(|_| HistoryBuffer::new(config.history_size))
                .collect(),
            accelerator_memory_histories: (0..gpu_count)
                .map(|_| HistoryBuffer::new(config.history_size))
                .collect(),
            accelerator_temp_histories: (0..gpu_count)
                .map(|_| HistoryBuffer::new(config.history_size))
                .collect(),
            network_rx_history: HistoryBuffer::new(config.history_size),
            network_tx_history: HistoryBuffer::new(config.history_size),
            agent,
            silicon_monitor,
            agent_history: VecDeque::with_capacity(config.agent_history_size),
            last_update: Instant::now(),
            update_interval: config.update_interval,
            start_time: Instant::now(),
            hostname,
            os_info,
        };

        // Perform initial update
        backend.update()?;

        Ok(backend)
    }

    /// Update all monitored data
    pub fn update(&mut self) -> Result<()> {
        self.update_cpu()?;
        self.update_memory()?;
        self.update_gpus()?;
        self.update_processes()?;
        self.update_network()?;
        self.update_connections()?;
        self.update_disks()?;
        self.update_system_stats()?;

        self.last_update = Instant::now();
        Ok(())
    }

    /// Check if update is needed based on interval
    pub fn should_update(&self) -> bool {
        self.last_update.elapsed() >= self.update_interval
    }

    /// Update if interval has elapsed
    pub fn update_if_needed(&mut self) -> Result<bool> {
        if self.should_update() {
            self.update()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // === CPU ===

    fn update_cpu(&mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_cpu_stats() {
                self.cpu_stats = Some(stats.clone());
                let utilization = 100.0 - stats.total.idle;
                self.cpu_history.push(utilization);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_cpu_stats() {
                self.cpu_stats = Some(stats.clone());
                let utilization = 100.0 - stats.total.idle;
                self.cpu_history.push(utilization);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS CPU stats via sysctl and host_processor_info
            // Use Command to get CPU info from sysctl
            use std::process::Command;
            let mut stats = CpuStats::new()?;

            // Get CPU model name
            let model = Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Apple CPU".to_string());

            // Get CPU frequency
            let freq_hz = Command::new("sysctl")
                .args(["-n", "hw.cpufrequency"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(0);
            let freq_mhz = (freq_hz / 1_000_000) as u32;

            // Get core count
            let ncpu = num_cpus::get();

            // Get load averages for overall utilization
            let load_avg = Command::new("sysctl")
                .args(["-n", "vm.loadavg"])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| {
                    // Format: "{ 1.23 4.56 7.89 }"
                    let parts: Vec<&str> = s
                        .trim()
                        .trim_matches(|c| c == '{' || c == '}')
                        .split_whitespace()
                        .collect();
                    parts.first().and_then(|v| v.parse::<f32>().ok())
                })
                .unwrap_or(0.0);

            let utilization = (load_avg / ncpu as f32 * 100.0).clamp(0.0, 100.0);

            // Build per-core entries (uniform distribution as approximation)
            for i in 0..ncpu {
                stats.cores.push(crate::core::cpu::CpuCore {
                    id: i,
                    online: true,
                    governor: "performance".to_string(),
                    frequency: if freq_mhz > 0 {
                        Some(crate::core::cpu::CpuFrequency {
                            current: freq_mhz,
                            min: freq_mhz / 2,
                            max: freq_mhz,
                        })
                    } else {
                        None
                    },
                    user: Some(utilization * 0.6),
                    nice: Some(0.0),
                    system: Some(utilization * 0.4),
                    idle: Some(100.0 - utilization),
                    model: model.clone(),
                });
            }

            stats.total = crate::core::cpu::CpuTotal {
                user: utilization * 0.6,
                nice: 0.0,
                system: utilization * 0.4,
                idle: 100.0 - utilization,
            };

            self.cpu_stats = Some(stats);
            self.cpu_history.push(utilization);
        }

        Ok(())
    }

    pub fn cpu_stats(&self) -> Option<&CpuStats> {
        self.cpu_stats.as_ref()
    }

    pub fn cpu_utilization(&self) -> f32 {
        self.cpu_stats
            .as_ref()
            .map(|s| 100.0 - s.total.idle)
            .unwrap_or(0.0)
    }

    pub fn cpu_history(&self) -> &HistoryBuffer<f32> {
        &self.cpu_history
    }

    // === Memory ===

    fn update_memory(&mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_memory_stats() {
                self.memory_stats = Some(stats.clone());
                let usage = stats.ram_usage_percent();
                self.memory_history.push(usage);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_memory_stats() {
                self.memory_stats = Some(stats.clone());
                let usage = stats.ram_usage_percent();
                self.memory_history.push(usage);
            }
        }

        Ok(())
    }

    pub fn memory_stats(&self) -> Option<&MemoryStats> {
        self.memory_stats.as_ref()
    }

    pub fn memory_utilization(&self) -> f32 {
        self.memory_stats
            .as_ref()
            .map(|s| s.ram_usage_percent())
            .unwrap_or(0.0)
    }

    pub fn memory_history(&self) -> &HistoryBuffer<f32> {
        &self.memory_history
    }

    // === GPUs/Accelerators ===

    fn update_gpus(&mut self) -> Result<()> {
        if let Some(ref gpus) = self.gpu_collection {
            if let Ok(infos) = gpus.snapshot_all() {
                self.gpu_dynamic_info = infos.iter().map(|i| i.dynamic_info.clone()).collect();

                // Update histories
                for (i, info) in self.gpu_dynamic_info.iter().enumerate() {
                    if i < self.accelerator_histories.len() {
                        self.accelerator_histories[i].push(info.utilization as f32);
                        self.accelerator_memory_histories[i].push(info.memory.utilization as f32);
                        if let Some(temp) = info.thermal.temperature {
                            self.accelerator_temp_histories[i].push(temp as f32);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn gpu_collection(&self) -> Option<&GpuCollection> {
        self.gpu_collection.as_ref()
    }

    pub fn gpu_static_info(&self) -> &[GpuStaticInfo] {
        &self.gpu_static_info
    }

    pub fn gpu_dynamic_info(&self) -> &[GpuDynamicInfo] {
        &self.gpu_dynamic_info
    }

    pub fn gpu_count(&self) -> usize {
        self.gpu_static_info.len()
    }

    pub fn accelerator_history(&self, index: usize) -> Option<&HistoryBuffer<f32>> {
        self.accelerator_histories.get(index)
    }

    pub fn accelerator_memory_history(&self, index: usize) -> Option<&HistoryBuffer<f32>> {
        self.accelerator_memory_histories.get(index)
    }

    pub fn accelerator_temp_history(&self, index: usize) -> Option<&HistoryBuffer<f32>> {
        self.accelerator_temp_histories.get(index)
    }

    // === Processes ===

    fn update_processes(&mut self) -> Result<()> {
        if let Some(ref mut monitor) = self.process_monitor {
            self.processes = monitor.processes().unwrap_or_default();
        }
        Ok(())
    }

    pub fn processes(&self) -> &[ProcessMonitorInfo] {
        &self.processes
    }

    pub fn processes_by_cpu(&self) -> Vec<&ProcessMonitorInfo> {
        let mut procs: Vec<_> = self.processes.iter().collect();
        procs.sort_by(|a, b| {
            b.cpu_percent
                .partial_cmp(&a.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        procs
    }

    pub fn processes_by_memory(&self) -> Vec<&ProcessMonitorInfo> {
        let mut procs: Vec<_> = self.processes.iter().collect();
        procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
        procs
    }

    pub fn processes_by_gpu(&self, gpu_index: usize) -> Vec<&ProcessMonitorInfo> {
        let mut procs: Vec<_> = self
            .processes
            .iter()
            .filter(|p| p.gpu_indices.contains(&gpu_index))
            .collect();
        procs.sort_by(|a, b| {
            let a_mem = a.gpu_memory_per_device.get(&gpu_index).unwrap_or(&0);
            let b_mem = b.gpu_memory_per_device.get(&gpu_index).unwrap_or(&0);
            b_mem.cmp(a_mem)
        });
        procs
    }

    // === Network ===

    fn update_network(&mut self) -> Result<()> {
        // Network monitoring - rates are handled internally by NetworkMonitor
        Ok(())
    }

    pub fn network_monitor(&self) -> Option<&NetworkMonitor> {
        self.network_monitor.as_ref()
    }

    pub fn network_monitor_mut(&mut self) -> Option<&mut NetworkMonitor> {
        self.network_monitor.as_mut()
    }

    // === Connections ===

    fn update_connections(&mut self) -> Result<()> {
        if let Some(ref mut monitor) = self.connection_monitor {
            self.connections = monitor.all_connections().unwrap_or_default();
        }
        Ok(())
    }

    pub fn connections(&self) -> &[ConnectionInfo] {
        &self.connections
    }

    pub fn connections_filtered(
        &self,
        protocol: Option<Protocol>,
        state: Option<ConnectionState>,
    ) -> Vec<&ConnectionInfo> {
        self.connections
            .iter()
            .filter(|c| protocol.map_or(true, |p| c.protocol == p))
            .filter(|c| state.map_or(true, |s| c.state == s))
            .collect()
    }

    // === Disks ===

    fn update_disks(&mut self) -> Result<()> {
        // Disk info is mostly static, updated less frequently
        Ok(())
    }

    pub fn disks(&self) -> &[Box<dyn DiskDevice>] {
        &self.disks
    }

    pub fn refresh_disks(&mut self) {
        self.disks = disk::enumerate_disks().unwrap_or_default();
    }

    // === System Stats ===

    fn update_system_stats(&mut self) -> Result<()> {
        // System stats are refreshed during read operations
        Ok(())
    }

    pub fn system_stats(&self) -> Option<&SystemStats> {
        self.system_stats.as_ref()
    }

    pub fn system_info(&self) -> Option<&MBSystemInfo> {
        self.system_info.as_ref()
    }

    pub fn driver_info(&self) -> &[DriverInfo] {
        &self.driver_info
    }

    pub fn motherboard_sensors(&self) -> &[Box<dyn MotherboardDevice>] {
        &self.motherboard_sensors
    }

    // === System Identification ===

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn os_info(&self) -> &str {
        &self.os_info
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    // === AI Agent ===

    /// Check if AI agent is available
    pub fn has_agent(&self) -> bool {
        self.agent.is_some()
    }

    /// Get reference to the AI agent
    pub fn agent(&self) -> Option<&Agent> {
        self.agent.as_ref()
    }

    /// Get mutable reference to the AI agent
    pub fn agent_mut(&mut self) -> Option<&mut Agent> {
        self.agent.as_mut()
    }

    /// Query the AI agent with a natural language question
    pub fn ask_agent(&mut self, question: &str) -> Result<AgentResponse> {
        let agent = self
            .agent
            .as_mut()
            .ok_or_else(|| SimonError::Other("AI agent not available".to_string()))?;

        let monitor = self
            .silicon_monitor
            .as_ref()
            .ok_or_else(|| SimonError::Other("Silicon monitor not available".to_string()))?;

        // Get response using the agent's ask method
        let response = agent.ask(question, monitor)?;

        // Store in history
        self.agent_history.push_front(response.clone());
        if self.agent_history.len() > 50 {
            self.agent_history.pop_back();
        }

        Ok(response)
    }

    /// Get agent response history
    pub fn agent_history(&self) -> &VecDeque<AgentResponse> {
        &self.agent_history
    }

    /// Clear agent history
    pub fn clear_agent_history(&mut self) {
        self.agent_history.clear();
    }

    // === Full System State ===

    /// Get complete system state snapshot for AI context or export
    pub fn get_full_system_state(&self) -> FullSystemState {
        let mut state = FullSystemState::empty();

        // CPU state
        if let Some(ref cpu) = self.cpu_stats {
            state.cpu = Some(CpuState {
                name: cpu
                    .cores
                    .first()
                    .map(|c| c.model.clone())
                    .unwrap_or_else(|| "CPU".to_string()),
                cores: cpu.cores.len(),
                threads: cpu.cores.len(), // Simplified
                utilization: 100.0 - cpu.total.idle,
                temperature: None, // Would need thermal zone access
                frequency_mhz: cpu
                    .cores
                    .first()
                    .and_then(|c| c.frequency.as_ref().map(|f| f.current as u64)),
                per_core_usage: cpu
                    .cores
                    .iter()
                    .map(|c| 100.0 - c.idle.unwrap_or(100.0))
                    .collect(),
            });
        }

        // Memory state
        if let Some(ref mem) = self.memory_stats {
            let ram_usage = mem.ram_usage_percent();
            let swap_usage = if mem.swap.total > 0 {
                (mem.swap.used as f32 / mem.swap.total as f32) * 100.0
            } else {
                0.0
            };

            state.memory = Some(MemoryState {
                total_bytes: mem.ram.total * 1024,
                used_bytes: mem.ram.used * 1024,
                available_bytes: mem.ram.free * 1024,
                usage_percent: ram_usage,
                swap_total_bytes: mem.swap.total * 1024,
                swap_used_bytes: mem.swap.used * 1024,
                swap_usage_percent: swap_usage,
            });
        }

        // Accelerator states
        for (i, (static_info, dynamic_info)) in self
            .gpu_static_info
            .iter()
            .zip(self.gpu_dynamic_info.iter())
            .enumerate()
        {
            let mem_usage = if dynamic_info.memory.total > 0 {
                (dynamic_info.memory.used as f32 / dynamic_info.memory.total as f32) * 100.0
            } else {
                0.0
            };

            state.accelerators.push(AcceleratorState {
                index: i,
                accel_type: "GPU".to_string(), // Could be extended for NPU/FPGA
                name: static_info.name.clone(),
                vendor: format!("{:?}", static_info.vendor),
                utilization: dynamic_info.utilization as f32,
                memory_used_bytes: dynamic_info.memory.used,
                memory_total_bytes: dynamic_info.memory.total,
                memory_usage_percent: mem_usage,
                temperature: dynamic_info.thermal.temperature.map(|t| t as f32),
                power_watts: dynamic_info.power.draw.map(|p| p as f32 / 1000.0),
                power_limit_watts: dynamic_info.power.limit.map(|p| p as f32 / 1000.0),
                clock_mhz: dynamic_info.clocks.graphics,
                memory_clock_mhz: dynamic_info.clocks.memory,
                process_count: dynamic_info.processes.len(),
            });
        }

        // Disk states
        for disk in &self.disks {
            if let Ok(disk_info) = disk.info() {
                // Try to get filesystem info for mount point
                let (mount_point, fs_type, used) = if let Ok(fs_info) = disk.filesystem_info() {
                    if let Some(first_fs) = fs_info.first() {
                        (
                            first_fs.mount_point.to_string_lossy().to_string(),
                            first_fs.fs_type.clone(),
                            first_fs.used_size,
                        )
                    } else {
                        (String::new(), String::new(), 0)
                    }
                } else {
                    (String::new(), String::new(), 0)
                };

                let usage = if disk_info.capacity > 0 {
                    (used as f32 / disk_info.capacity as f32) * 100.0
                } else {
                    0.0
                };

                state.disks.push(DiskState {
                    name: disk.name().to_string(),
                    mount_point,
                    filesystem: fs_type,
                    total_bytes: disk_info.capacity,
                    used_bytes: used,
                    usage_percent: usage,
                });
            }
        }

        // Top processes
        let top_procs = self.processes_by_cpu();
        for proc in top_procs.iter().take(10) {
            state.top_processes.push(ProcessState {
                pid: proc.pid,
                name: proc.name.clone(),
                cpu_percent: proc.cpu_percent,
                memory_bytes: proc.memory_bytes,
                gpu_memory_bytes: proc.total_gpu_memory_bytes,
                gpu_indices: proc.gpu_indices.clone(),
            });
        }

        // System info
        if let Some(ref sys) = self.system_info {
            state.system = Some(SystemInfoState {
                hostname: self.hostname.clone(),
                os: if sys.os_version.is_empty() {
                    self.os_info.clone()
                } else {
                    sys.os_version.clone()
                },
                kernel: sys.kernel_version.clone().unwrap_or_else(|| String::new()),
                uptime_secs: self.uptime().as_secs(),
            });
        }

        state.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        state
    }

    /// Reset all history buffers
    pub fn reset_history(&mut self) {
        self.cpu_history.clear();
        self.memory_history.clear();
        for hist in &mut self.accelerator_histories {
            hist.clear();
        }
        for hist in &mut self.accelerator_memory_histories {
            hist.clear();
        }
        for hist in &mut self.accelerator_temp_histories {
            hist.clear();
        }
        self.network_rx_history.clear();
        self.network_tx_history.clear();
    }
}

// ============================================================================
// BACKEND CONFIGURATION
// ============================================================================

/// Configuration for the monitoring backend
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// Enable AI agent
    pub enable_agent: bool,

    /// AI agent model size
    pub agent_model_size: ModelSize,

    /// AI agent timeout (seconds)
    pub agent_timeout_secs: u64,

    /// History buffer size
    pub history_size: usize,

    /// Agent history size
    pub agent_history_size: usize,

    /// Update interval
    pub update_interval: Duration,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            enable_agent: true,
            agent_model_size: ModelSize::Medium,
            agent_timeout_secs: 10,
            history_size: DEFAULT_HISTORY_SIZE,
            agent_history_size: 50,
            update_interval: DEFAULT_UPDATE_INTERVAL,
        }
    }
}

impl BackendConfig {
    /// Create config with agent disabled (for faster startup)
    pub fn without_agent() -> Self {
        Self {
            enable_agent: false,
            ..Default::default()
        }
    }

    /// Create config with custom history size
    pub fn with_history_size(mut self, size: usize) -> Self {
        self.history_size = size;
        self
    }

    /// Create config with custom update interval
    pub fn with_update_interval(mut self, interval: Duration) -> Self {
        self.update_interval = interval;
        self
    }
}

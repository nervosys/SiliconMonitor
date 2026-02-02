//! Application state management

use crate::agent::{Agent, AgentConfig, AgentResponse};
use crate::gpu::traits::Device;
use crate::network_monitor::NetworkMonitor;
use crate::{ProcessMonitor, ProcessMonitorInfo, SiliconMonitor};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Maximum number of data points to keep in history
const MAX_HISTORY: usize = 60;

/// Maximum number of agent responses to keep
const MAX_AGENT_HISTORY: usize = 10;

/// Type of accelerator device
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceleratorType {
    /// GPU - Graphics Processing Unit
    Gpu,
    /// NPU - Neural Processing Unit
    Npu,
    /// TPU - Tensor Processing Unit
    Tpu,
    /// FPGA - Field Programmable Gate Array
    Fpga,
    /// DLA - Deep Learning Accelerator (e.g., Jetson DLA)
    Dla,
    /// VPU - Vision Processing Unit (e.g., Intel Movidius)
    Vpu,
    /// IPU - Intelligence Processing Unit (e.g., Graphcore)
    Ipu,
    /// Custom/Other accelerator
    Other,
}

impl std::fmt::Display for AcceleratorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceleratorType::Gpu => write!(f, "GPU"),
            AcceleratorType::Npu => write!(f, "NPU"),
            AcceleratorType::Tpu => write!(f, "TPU"),
            AcceleratorType::Fpga => write!(f, "FPGA"),
            AcceleratorType::Dla => write!(f, "DLA"),
            AcceleratorType::Vpu => write!(f, "VPU"),
            AcceleratorType::Ipu => write!(f, "IPU"),
            AcceleratorType::Other => write!(f, "ACC"),
        }
    }
}

/// Unified accelerator information structure
#[derive(Clone, Default)]
pub struct AcceleratorInfo {
    /// Device name (e.g., "NVIDIA GeForce RTX 4090", "Intel Neural Compute Stick 2")
    pub name: String,
    /// Vendor name (e.g., "NVIDIA", "AMD", "Intel", "Xilinx")
    pub vendor: String,
    /// Type of accelerator
    pub accel_type: AcceleratorType,
    /// Compute/core utilization (0-100%)
    pub utilization: f32,
    /// Temperature in Celsius
    pub temperature: Option<f32>,
    /// Power consumption in Watts
    pub power: Option<f32>,
    /// Power limit in Watts
    pub power_limit: Option<f32>,
    /// Total memory in bytes
    pub memory_total: u64,
    /// Used memory in bytes
    pub memory_used: u64,
    /// Core/compute clock in MHz
    pub clock_core: Option<u32>,
    /// Memory clock in MHz
    pub clock_memory: Option<u32>,
    /// Fan speed in RPM
    pub fan_speed_rpm: Option<u32>,
    /// Fan speed percentage (0-100)
    pub fan_speed_percent: Option<f32>,
    /// PCIe link generation (e.g., 4 for Gen4)
    pub pcie_gen: Option<u32>,
    /// PCIe link width (e.g., 16 for x16)
    pub pcie_width: Option<u32>,
    /// PCIe throughput in MB/s
    pub pcie_throughput: Option<f64>,
    /// Encoder utilization (video encoding, 0-100%)
    pub encoder_util: Option<f32>,
    /// Decoder utilization (video decoding, 0-100%)
    pub decoder_util: Option<f32>,
    /// Last time encoder was active
    pub encoder_last_active: Option<Instant>,
    /// Last time decoder was active
    pub decoder_last_active: Option<Instant>,
    /// Device-specific status string (e.g., "P0", "Active", "Idle")
    pub status: Option<String>,
    /// Firmware/driver version
    pub firmware_version: Option<String>,
    /// Serial number or UUID
    pub serial: Option<String>,
    /// PCIe slot info (for PCIe devices)
    pub pcie_slot: Option<String>,
}

impl Default for AcceleratorType {
    fn default() -> Self {
        AcceleratorType::Gpu
    }
}

/// Process display mode - which device's processes to show
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessDisplayMode {
    /// Show CPU processes
    Cpu,
    /// Show GPU processes (with index)
    Gpu(usize),
    /// Show NPU processes (with index)
    Npu(usize),
    /// Show accelerator processes (unified, with index)
    Accelerator(usize),
    /// Show all processes
    All,
}

impl Default for ProcessDisplayMode {
    fn default() -> Self {
        ProcessDisplayMode::All
    }
}

/// Application state
pub struct App {
    /// Currently selected tab
    pub selected_tab: usize,
    /// Tab names
    pub tabs: Vec<&'static str>,
    /// CPU history (utilization percentages)
    pub cpu_history: VecDeque<u64>,
    /// Memory history (used percentage)
    pub memory_history: VecDeque<u64>,
    /// GPU histories (one per GPU) - kept for backward compatibility
    pub gpu_histories: Vec<VecDeque<u64>>,
    /// Accelerator histories (one per accelerator)
    pub accelerator_histories: Vec<VecDeque<u64>>,
    /// Network RX rate history
    pub network_rx_history: VecDeque<u64>,
    /// Network TX rate history
    pub network_tx_history: VecDeque<u64>,
    /// Current CPU info
    pub cpu_info: CpuInfo,
    /// Current memory info
    pub memory_info: MemoryInfo,
    /// GPU information (kept for backward compatibility)
    pub gpu_info: Vec<GpuInfo>,
    /// All accelerators (GPUs, NPUs, FPGAs, etc.)
    pub accelerators: Vec<AcceleratorInfo>,
    /// System information
    pub system_info: SystemInfo,
    /// Disk information
    pub disk_info: Vec<DiskInfo>,
    /// Network information
    pub network_info: NetworkInfo,
    /// Update interval
    pub update_interval: Duration,
    /// Last update time
    pub last_update: Instant,
    /// Scroll position for lists
    pub scroll_position: usize,
    /// GPU devices for monitoring
    gpu_devices: Vec<Box<dyn Device>>,
    /// Network monitor
    network_monitor: Option<NetworkMonitor>,
    /// Application configuration
    pub config: crate::config::Config,
    /// Status message to display (cleared after timeout)
    pub status_message: Option<(String, Instant)>,
    /// AI Agent for queries
    pub agent: Option<Agent>,
    /// Agent query input mode
    pub agent_input_mode: bool,
    /// Current agent query being typed
    pub agent_input: String,
    /// Agent response history
    pub agent_history: VecDeque<AgentResponse>,
    /// Agent loading state
    pub agent_loading: bool,
    /// Process display mode - which device's processes to show
    pub process_display_mode: ProcessDisplayMode,
    /// Process monitor for tracking system and GPU processes
    process_monitor: Option<ProcessMonitor>,
    /// Cached processes from last update
    pub processes: Vec<ProcessMonitorInfo>,
    /// Cached sorted/filtered process indices for fast rendering
    cached_process_order: Vec<usize>,
    /// Cached filtered process count for scroll bounds
    filtered_process_count: usize,
    /// Background initialization state
    init_state: InitState,
    /// Receiver for background-initialized GPU devices
    gpu_init_rx: Option<std::sync::mpsc::Receiver<Vec<Box<dyn Device + Send>>>>,
    /// Receiver for background-initialized agent
    agent_init_rx: Option<std::sync::mpsc::Receiver<Option<Agent>>>,
    /// Receiver for background-initialized process monitor
    process_init_rx: Option<std::sync::mpsc::Receiver<Option<ProcessMonitor>>>,
}

/// Background initialization state
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InitState {
    /// Still loading components
    Loading,
    /// All components ready
    Ready,
}

impl Default for InitState {
    fn default() -> Self {
        InitState::Loading
    }
}

#[derive(Clone, Default)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
    pub threads: usize,
    pub utilization: f32,
    pub temperature: Option<f32>,
    pub frequency: Option<u64>,
    pub per_core_usage: Vec<f32>,
}

#[derive(Clone, Default)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

/// Legacy GPU info struct - kept for backward compatibility
#[derive(Clone, Default)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub utilization: f32,
    pub temperature: Option<f32>,
    pub power: Option<f32>,
    pub power_limit: Option<f32>,
    pub memory_total: u64,
    pub memory_used: u64,
    pub clock_graphics: Option<u32>,
    pub clock_memory: Option<u32>,
    /// Fan speed in RPM
    pub fan_speed_rpm: Option<u32>,
    /// Fan speed percentage (0-100)
    pub fan_speed_percent: Option<f32>,
    /// PCIe generation
    pub pcie_gen: Option<u32>,
    /// PCIe link width (e.g., 16 for x16)
    pub pcie_width: Option<u32>,
    /// Encoder utilization (0-100%)
    pub encoder_util: Option<f32>,
    /// Decoder utilization (0-100%)
    pub decoder_util: Option<f32>,
    /// Last time encoder was active (for auto-hide)
    pub encoder_last_active: Option<Instant>,
    /// Last time decoder was active (for auto-hide)
    pub decoder_last_active: Option<Instant>,
}

impl From<&GpuInfo> for AcceleratorInfo {
    fn from(gpu: &GpuInfo) -> Self {
        AcceleratorInfo {
            name: gpu.name.clone(),
            vendor: gpu.vendor.clone(),
            accel_type: AcceleratorType::Gpu,
            utilization: gpu.utilization,
            temperature: gpu.temperature,
            power: gpu.power,
            power_limit: gpu.power_limit,
            memory_total: gpu.memory_total,
            memory_used: gpu.memory_used,
            clock_core: gpu.clock_graphics,
            clock_memory: gpu.clock_memory,
            fan_speed_rpm: gpu.fan_speed_rpm,
            fan_speed_percent: gpu.fan_speed_percent,
            pcie_gen: gpu.pcie_gen,
            pcie_width: gpu.pcie_width,
            pcie_throughput: None,
            encoder_util: gpu.encoder_util,
            decoder_util: gpu.decoder_util,
            encoder_last_active: gpu.encoder_last_active,
            decoder_last_active: gpu.decoder_last_active,
            status: None,
            firmware_version: None,
            serial: None,
            pcie_slot: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime: Duration,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Default)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub used: u64,
    pub filesystem: String,
    /// Read bytes per second
    pub read_rate: f64,
    /// Write bytes per second
    pub write_rate: f64,
}

/// Network interface information for display
#[derive(Clone, Default)]
pub struct NetworkInterfaceInfo {
    /// Interface name (e.g., "eth0", "Ethernet")
    pub name: String,
    /// Is interface up
    pub is_up: bool,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total bytes transmitted
    pub tx_bytes: u64,
    /// Receive rate (bytes/sec)
    pub rx_rate: f64,
    /// Transmit rate (bytes/sec)
    pub tx_rate: f64,
    /// Link speed in Mbps (if available)
    pub speed_mbps: Option<u32>,
}

/// Aggregated network statistics
#[derive(Clone, Default)]
pub struct NetworkInfo {
    /// All network interfaces
    pub interfaces: Vec<NetworkInterfaceInfo>,
    /// Total RX rate across all interfaces (bytes/sec)
    pub total_rx_rate: f64,
    /// Total TX rate across all interfaces (bytes/sec)
    pub total_tx_rate: f64,
    /// Total bytes received
    pub total_rx_bytes: u64,
    /// Total bytes transmitted
    pub total_tx_bytes: u64,
}

impl App {
    /// Create a new application instance (blocking - full initialization)
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut app = Self::new_fast()?;
        // Wait for background init to complete
        while app.init_state != InitState::Ready {
            app.check_background_init();
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(app)
    }

    /// Create app with fast startup - slow components initialize in background
    pub fn new_fast() -> Result<Self, Box<dyn std::error::Error>> {
        use std::sync::mpsc;

        // Load config synchronously (fast - just file read)
        let config = crate::config::Config::load().unwrap_or_default();
        let update_interval = Duration::from_millis(config.general.update_interval_ms as u64);

        // Initialize network monitor synchronously (usually fast)
        let network_monitor = NetworkMonitor::new().ok();

        // Spawn background thread for GPU enumeration (slow - NVML init, device enumeration)
        let (gpu_tx, gpu_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut gpu_devices: Vec<Box<dyn Device + Send>> = Vec::new();

            #[cfg(feature = "nvidia")]
            {
                if let Ok(nvidia_devices) = crate::gpu::nvidia_new::enumerate() {
                    for device in nvidia_devices {
                        gpu_devices.push(Box::new(device));
                    }
                }
            }

            #[cfg(feature = "amd")]
            {
                if let Ok(mut amd_devices) = crate::gpu::amd_rocm::enumerate() {
                    for device in amd_devices.drain(..) {
                        gpu_devices.push(device);
                    }
                }
            }

            #[cfg(feature = "intel")]
            {
                if let Ok(mut intel_devices) = crate::gpu::intel_levelzero::enumerate() {
                    for device in intel_devices.drain(..) {
                        gpu_devices.push(device);
                    }
                }
            }

            let _ = gpu_tx.send(gpu_devices);
        });

        // Spawn background thread for agent detection (potentially slow - network checks)
        let (agent_tx, agent_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let agent = AgentConfig::auto_detect()
                .ok()
                .map(|config| {
                    config
                        .with_caching(true)
                        .with_cache_size(50)
                        .with_timeout(Duration::from_secs(5))
                })
                .and_then(|config| Agent::new(config).ok());
            let _ = agent_tx.send(agent);
        });

        // Spawn background thread for process monitor (can be slow on Windows)
        let (proc_tx, proc_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = proc_tx.send(ProcessMonitor::new().ok());
        });

        let mut app = Self {
            selected_tab: 0,
            tabs: vec![
                "Overview",
                "CPU",
                "Accelerators",
                "Memory",
                "Peripherals",
                "System",
                "Agent",
            ],
            cpu_history: VecDeque::with_capacity(MAX_HISTORY),
            memory_history: VecDeque::with_capacity(MAX_HISTORY),
            gpu_histories: Vec::new(),
            accelerator_histories: Vec::new(),
            network_rx_history: VecDeque::with_capacity(MAX_HISTORY),
            network_tx_history: VecDeque::with_capacity(MAX_HISTORY),
            cpu_info: CpuInfo::default(),
            memory_info: MemoryInfo::default(),
            gpu_info: Vec::new(),
            accelerators: Vec::new(),
            system_info: SystemInfo::default(),
            disk_info: Vec::new(),
            network_info: NetworkInfo::default(),
            update_interval,
            last_update: Instant::now(),
            scroll_position: 0,
            gpu_devices: Vec::new(), // Will be populated from background thread
            network_monitor,
            config,
            status_message: Some(("Loading...".to_string(), Instant::now())),
            agent: None, // Will be populated from background thread
            agent_input_mode: false,
            agent_input: String::new(),
            agent_history: VecDeque::with_capacity(MAX_AGENT_HISTORY),
            agent_loading: false,
            process_display_mode: ProcessDisplayMode::default(),
            process_monitor: None, // Will be populated from background thread
            processes: Vec::new(),
            cached_process_order: Vec::new(),
            filtered_process_count: 0,
            init_state: InitState::Loading,
            gpu_init_rx: Some(gpu_rx),
            agent_init_rx: Some(agent_rx),
            process_init_rx: Some(proc_rx),
        };

        // Do initial fast update for immediate data (CPU, Memory are fast)
        let _ = app.update_cpu();
        let _ = app.update_memory();
        let _ = app.update_network();
        let _ = app.update_system();

        Ok(app)
    }

    /// Check and apply background initialization results
    pub fn check_background_init(&mut self) {
        let mut all_done = true;

        // Check GPU init
        if let Some(ref rx) = self.gpu_init_rx {
            match rx.try_recv() {
                Ok(devices) => {
                    // Convert Vec<Box<dyn Device + Send>> to Vec<Box<dyn Device>>
                    self.gpu_devices = devices.into_iter().map(|d| d as Box<dyn Device>).collect();
                    self.gpu_init_rx = None;
                    // Trigger GPU update now that devices are available
                    let _ = self.update_gpu();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    all_done = false;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.gpu_init_rx = None;
                }
            }
        }

        // Check agent init
        if let Some(ref rx) = self.agent_init_rx {
            match rx.try_recv() {
                Ok(agent) => {
                    self.agent = agent;
                    self.agent_init_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    all_done = false;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.agent_init_rx = None;
                }
            }
        }

        // Check process monitor init
        if let Some(ref rx) = self.process_init_rx {
            match rx.try_recv() {
                Ok(monitor) => {
                    self.process_monitor = monitor;
                    self.process_init_rx = None;
                    // Trigger process update
                    let _ = self.update_processes();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    all_done = false;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.process_init_rx = None;
                }
            }
        }

        // Update state when all done
        if all_done
            && self.gpu_init_rx.is_none()
            && self.agent_init_rx.is_none()
            && self.process_init_rx.is_none()
        {
            if self.init_state != InitState::Ready {
                self.init_state = InitState::Ready;
                self.status_message = None; // Clear "Loading..." message
            }
        }
    }

    /// Check if app is still initializing
    pub fn is_loading(&self) -> bool {
        self.init_state == InitState::Loading
    }

    /// Update all monitoring data (legacy - calls both fast and slow)
    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_fast()?;
        self.update_slow()?;
        Ok(())
    }

    /// Fast updates - CPU, GPU, Memory, Network (called every 500ms)
    pub fn update_fast(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_cpu()?;
        self.update_memory()?;
        self.update_gpu()?;
        self.update_network()?;
        self.last_update = Instant::now();
        Ok(())
    }

    /// Slow updates - System, Disks, Processes (called every 2s)
    pub fn update_slow(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_system()?;
        self.update_disks()?;
        self.update_processes()?;
        Ok(())
    }

    /// Update processes only (called every 1s)
    pub fn update_processes_only(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_processes()
    }

    /// Update disks only (called every 5s - expensive)
    pub fn update_disks_only(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_disks()
    }

    fn update_cpu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Try to get real CPU stats from platform module
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_cpu_stats() {
                let utilization = 100.0 - stats.total.idle;
                let num_cpus = stats.cores.len();

                self.cpu_info = CpuInfo {
                    name: stats
                        .cores
                        .first()
                        .map(|c| c.model.clone())
                        .unwrap_or_else(|| "CPU".to_string()),
                    cores: num_cpus,
                    threads: num_cpus,
                    utilization,
                    temperature: None, // Requires admin for WMI thermal zone access
                    frequency: stats
                        .cores
                        .first()
                        .and_then(|c| c.frequency.as_ref().map(|f| f.current as u64)),
                    per_core_usage: stats
                        .cores
                        .iter()
                        .map(|c| 100.0 - c.idle.unwrap_or(100.0))
                        .collect(),
                };

                // Add to history
                self.cpu_history.push_back(self.cpu_info.utilization as u64);
                if self.cpu_history.len() > MAX_HISTORY {
                    self.cpu_history.pop_front();
                }

                return Ok(());
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_cpu_stats() {
                let utilization = 100.0 - stats.total.idle;
                let num_cpus = stats.cores.len();

                self.cpu_info = CpuInfo {
                    name: stats
                        .cores
                        .first()
                        .map(|c| c.model.clone())
                        .unwrap_or_else(|| "CPU".to_string()),
                    cores: num_cpus,
                    threads: num_cpus,
                    utilization,
                    temperature: None,
                    frequency: stats
                        .cores
                        .first()
                        .and_then(|c| c.frequency.as_ref().map(|f| f.current as u64)),
                    per_core_usage: stats
                        .cores
                        .iter()
                        .map(|c| 100.0 - c.idle.unwrap_or(100.0))
                        .collect(),
                };

                self.cpu_history.push_back(self.cpu_info.utilization as u64);
                if self.cpu_history.len() > MAX_HISTORY {
                    self.cpu_history.pop_front();
                }

                return Ok(());
            }
        }

        // Fallback: use num_cpus for basic info
        let num_cpus = num_cpus::get();
        let utilization = Self::get_cpu_utilization();

        self.cpu_info = CpuInfo {
            name: "CPU".to_string(),
            cores: num_cpus,
            threads: num_cpus,
            utilization,
            temperature: None,
            frequency: None,
            per_core_usage: vec![utilization; num_cpus.min(8)],
        };

        self.cpu_history.push_back(self.cpu_info.utilization as u64);
        if self.cpu_history.len() > MAX_HISTORY {
            self.cpu_history.pop_front();
        }

        Ok(())
    }

    fn get_cpu_utilization() -> f32 {
        // Simple placeholder - returns a random-ish value for demonstration
        // In real implementation, would read from /proc/stat (Linux) or WMI (Windows)
        use std::time::SystemTime;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        ((now % 100) as f32) / 2.0 // 0-50% range for demo
    }

    fn update_memory(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Try to get real memory stats from platform module
        #[cfg(target_os = "windows")]
        {
            if let Ok(stats) = crate::platform::windows::read_memory_stats() {
                // Convert from KB to bytes for consistency
                self.memory_info = MemoryInfo {
                    total: stats.ram.total * 1024,
                    used: stats.ram.used * 1024,
                    available: stats.ram.free * 1024,
                    swap_total: stats.swap.total * 1024,
                    swap_used: stats.swap.used * 1024,
                };

                let used_percent = if self.memory_info.total > 0 {
                    (self.memory_info.used * 100) / self.memory_info.total
                } else {
                    0
                };
                self.memory_history.push_back(used_percent);
                if self.memory_history.len() > MAX_HISTORY {
                    self.memory_history.pop_front();
                }

                return Ok(());
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(stats) = crate::platform::linux::read_memory_stats() {
                self.memory_info = MemoryInfo {
                    total: stats.ram.total * 1024,
                    used: stats.ram.used * 1024,
                    available: stats.ram.free * 1024,
                    swap_total: stats.swap.total * 1024,
                    swap_used: stats.swap.used * 1024,
                };

                let used_percent = if self.memory_info.total > 0 {
                    (self.memory_info.used * 100) / self.memory_info.total
                } else {
                    0
                };
                self.memory_history.push_back(used_percent);
                if self.memory_history.len() > MAX_HISTORY {
                    self.memory_history.pop_front();
                }

                return Ok(());
            }
        }

        // Fallback: placeholder data
        self.memory_info = MemoryInfo {
            total: 32 * 1024 * 1024 * 1024,     // 32 GB
            used: 16 * 1024 * 1024 * 1024,      // 16 GB
            available: 16 * 1024 * 1024 * 1024, // 16 GB
            swap_total: 8 * 1024 * 1024 * 1024, // 8 GB
            swap_used: 1 * 1024 * 1024 * 1024,  // 1 GB
        };

        let used_percent = if self.memory_info.total > 0 {
            (self.memory_info.used * 100) / self.memory_info.total
        } else {
            0
        };
        self.memory_history.push_back(used_percent);
        if self.memory_history.len() > MAX_HISTORY {
            self.memory_history.pop_front();
        }

        Ok(())
    }

    fn update_gpu(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::gpu::traits::FanSpeed;

        // Get real GPU data from devices
        self.gpu_info.clear();

        for device in &self.gpu_devices {
            let name = device.name().unwrap_or_else(|_| "Unknown GPU".to_string());
            let vendor_str = format!("{}", device.vendor());

            // Get memory info
            let (memory_total, memory_used) = if let Ok(mem) = device.memory() {
                (mem.total, mem.used)
            } else {
                (0, 0)
            };

            // Get clocks
            let (clock_graphics, clock_memory) = if let Ok(clocks) = device.clocks() {
                (Some(clocks.graphics), Some(clocks.memory))
            } else {
                (None, None)
            };

            // Get utilization
            let utilization = if let Ok(util) = device.utilization() {
                util.gpu
            } else {
                0.0
            };

            // Get temperature
            let temperature = if let Ok(temp) = device.temperature() {
                temp.primary()
            } else {
                None
            };

            // Get power
            let (power, power_limit) = if let Ok(pwr) = device.power() {
                (
                    if pwr.current > 0.0 {
                        Some(pwr.current)
                    } else {
                        None
                    },
                    if pwr.limit > 0.0 {
                        Some(pwr.limit)
                    } else {
                        None
                    },
                )
            } else {
                (None, None)
            };

            // Get fan speed
            let (fan_speed_rpm, fan_speed_percent) = if let Ok(Some(fan)) = device.fan_speed() {
                match fan {
                    FanSpeed::Rpm(rpm) => (Some(rpm), None),
                    FanSpeed::Percent(pct) => (None, Some(pct as f32)),
                }
            } else {
                (None, None)
            };

            // Get PCIe info
            let (pcie_gen, pcie_width) = if let Ok(pci_info) = device.pci_info() {
                (pci_info.pcie_generation, pci_info.pcie_link_width)
            } else {
                (None, None)
            };

            // Get encoder/decoder utilization
            let (encoder_util, decoder_util) = if let Ok(util) = device.utilization() {
                (util.encoder, util.decoder)
            } else {
                (None, None)
            };

            // Determine if encoder/decoder were active (update timestamp)
            let now = Instant::now();
            let encoder_last_active = if encoder_util.is_some() && encoder_util.unwrap() > 0.0 {
                Some(now)
            } else {
                // Preserve previous timestamp if available
                self.gpu_info
                    .get(self.gpu_info.len())
                    .and_then(|prev| prev.encoder_last_active)
            };

            let decoder_last_active = if decoder_util.is_some() && decoder_util.unwrap() > 0.0 {
                Some(now)
            } else {
                // Preserve previous timestamp if available
                self.gpu_info
                    .get(self.gpu_info.len())
                    .and_then(|prev| prev.decoder_last_active)
            };

            self.gpu_info.push(GpuInfo {
                name,
                vendor: vendor_str,
                utilization,
                temperature,
                power,
                power_limit,
                memory_total,
                memory_used,
                clock_graphics,
                clock_memory,
                fan_speed_rpm,
                fan_speed_percent,
                pcie_gen,
                pcie_width,
                encoder_util,
                decoder_util,
                encoder_last_active,
                decoder_last_active,
            });
        }

        // Update GPU histories
        while self.gpu_histories.len() < self.gpu_info.len() {
            self.gpu_histories
                .push(VecDeque::with_capacity(MAX_HISTORY));
        }

        for (i, gpu) in self.gpu_info.iter().enumerate() {
            self.gpu_histories[i].push_back(gpu.utilization as u64);
            if self.gpu_histories[i].len() > MAX_HISTORY {
                self.gpu_histories[i].pop_front();
            }
        }

        // Update unified accelerators list from GPU info
        self.accelerators = self.gpu_info.iter().map(AcceleratorInfo::from).collect();

        // Update accelerator histories
        while self.accelerator_histories.len() < self.accelerators.len() {
            self.accelerator_histories
                .push(VecDeque::with_capacity(MAX_HISTORY));
        }

        for (i, accel) in self.accelerators.iter().enumerate() {
            self.accelerator_histories[i].push_back(accel.utilization as u64);
            if self.accelerator_histories[i].len() > MAX_HISTORY {
                self.accelerator_histories[i].pop_front();
            }
        }

        Ok(())
    }

    fn update_system(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get basic system info

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());

        let os = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);

        let kernel = if cfg!(target_os = "windows") {
            "Windows NT".to_string()
        } else if cfg!(target_os = "linux") {
            std::process::Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        // Get uptime - platform-specific
        #[cfg(target_os = "windows")]
        let uptime = crate::platform::windows::get_system_uptime();

        #[cfg(target_os = "linux")]
        let uptime = std::fs::read_to_string("/proc/uptime")
            .ok()
            .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
            .and_then(|s| s.parse::<f64>().ok())
            .map(|secs| Duration::from_secs(secs as u64))
            .unwrap_or(Duration::from_secs(0));

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        let uptime = Duration::from_secs(0);

        // Get motherboard info if available
        #[cfg(target_os = "windows")]
        let (manufacturer, model) = crate::platform::windows::detect_platform()
            .map(|p| (p.hardware.module.clone(), Some(p.hardware.model.clone())))
            .unwrap_or((None, None));

        #[cfg(not(target_os = "windows"))]
        let (manufacturer, model): (Option<String>, Option<String>) = (None, None);

        self.system_info = SystemInfo {
            hostname,
            os,
            kernel,
            uptime,
            manufacturer,
            model,
        };

        Ok(())
    }

    fn update_disks(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get real disk info from disk monitoring module
        self.disk_info.clear();

        // Try to get real disk information
        match crate::disk::enumerate_disks() {
            Ok(disks) if !disks.is_empty() => {
                for disk in disks {
                    if let Ok(info) = disk.info() {
                        // Get filesystem info if available
                        let (mount_point, filesystem, used) =
                            if let Ok(fs_infos) = disk.filesystem_info() {
                                // Use first filesystem if multiple partitions
                                if let Some(fs) = fs_infos.first() {
                                    (
                                        fs.mount_point.to_string_lossy().to_string(),
                                        fs.fs_type.clone(),
                                        fs.used_size,
                                    )
                                } else {
                                    ("N/A".to_string(), "N/A".to_string(), 0)
                                }
                            } else {
                                ("N/A".to_string(), "N/A".to_string(), 0)
                            };

                        // Get I/O stats for throughput
                        let (read_rate, write_rate) = if let Ok(io_stats) = disk.io_stats() {
                            (
                                io_stats.read_throughput.unwrap_or(0) as f64,
                                io_stats.write_throughput.unwrap_or(0) as f64,
                            )
                        } else {
                            (0.0, 0.0)
                        };

                        self.disk_info.push(DiskInfo {
                            name: info.model,
                            mount_point,
                            total: info.capacity,
                            used,
                            filesystem,
                            read_rate,
                            write_rate,
                        });
                    }
                }
            }
            Ok(_) | Err(_) => {
                // Fallback: Try to use Windows APIs for basic disk space info
                #[cfg(target_os = "windows")]
                {
                    // Windows fallback: Get logical drives using GetDiskFreeSpaceEx
                    if let Ok(drives) = Self::get_windows_drives() {
                        for (drive, total, used, fs_type) in drives {
                            self.disk_info.push(DiskInfo {
                                name: drive.clone(),
                                mount_point: drive,
                                total,
                                used,
                                filesystem: fs_type,
                                read_rate: 0.0,
                                write_rate: 0.0,
                            });
                        }
                    }
                }

                // If still no disks, show message
                if self.disk_info.is_empty() {
                    #[cfg(target_os = "windows")]
                    {
                        self.disk_info.push(DiskInfo {
                            name: "Unable to detect disks".to_string(),
                            mount_point: "Check disk permissions".to_string(),
                            total: 0,
                            used: 0,
                            filesystem: "N/A".to_string(),
                            read_rate: 0.0,
                            write_rate: 0.0,
                        });
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        self.disk_info.push(DiskInfo {
                            name: "No disks detected".to_string(),
                            mount_point: "N/A".to_string(),
                            total: 0,
                            used: 0,
                            filesystem: "N/A".to_string(),
                            read_rate: 0.0,
                            write_rate: 0.0,
                        });
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn get_windows_drives() -> Result<Vec<(String, u64, u64, String)>, Box<dyn std::error::Error>> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let mut drives = Vec::new();

        // Check common drive letters
        for letter in 'A'..='Z' {
            let drive_path = format!("{}:\\", letter);

            // Check if drive exists and get space info
            if let Ok(metadata) = std::fs::metadata(&drive_path) {
                if metadata.is_dir() {
                    // Use GetDiskFreeSpaceEx to get actual disk space
                    let path_wide: Vec<u16> = OsStr::new(&drive_path)
                        .encode_wide()
                        .chain(std::iter::once(0))
                        .collect();

                    let mut free_bytes: u64 = 0;
                    let mut total_bytes: u64 = 0;
                    let mut total_free_bytes: u64 = 0;

                    unsafe {
                        use windows::core::PCWSTR;
                        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

                        if GetDiskFreeSpaceExW(
                            PCWSTR(path_wide.as_ptr()),
                            Some(&mut free_bytes),
                            Some(&mut total_bytes),
                            Some(&mut total_free_bytes),
                        )
                        .is_ok()
                        {
                            let used_bytes = total_bytes.saturating_sub(total_free_bytes);
                            drives.push((drive_path, total_bytes, used_bytes, "NTFS".to_string()));
                        }
                    }
                }
            }
        }

        Ok(drives)
    }

    fn update_network(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut monitor) = self.network_monitor {
            if let Ok(interfaces) = monitor.interfaces() {
                let mut net_interfaces = Vec::new();
                let mut total_rx_rate = 0.0;
                let mut total_tx_rate = 0.0;
                let mut total_rx_bytes = 0;
                let mut total_tx_bytes = 0;

                for iface in &interfaces {
                    // Skip loopback and virtual interfaces
                    if iface.name.starts_with("lo")
                        || iface.name.contains("Loopback")
                        || iface.name.starts_with("vEthernet")
                        || iface.name.starts_with("Local Area Connection*")
                        || iface.name.starts_with("VMware")
                        || iface.name.starts_with("VirtualBox")
                        || !iface.is_active()
                    {
                        continue;
                    }

                    let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, iface);

                    total_rx_rate += rx_rate;
                    total_tx_rate += tx_rate;
                    total_rx_bytes += iface.rx_bytes;
                    total_tx_bytes += iface.tx_bytes;

                    net_interfaces.push(NetworkInterfaceInfo {
                        name: iface.name.clone(),
                        is_up: iface.is_up,
                        rx_bytes: iface.rx_bytes,
                        tx_bytes: iface.tx_bytes,
                        rx_rate,
                        tx_rate,
                        speed_mbps: iface.speed_mbps,
                    });
                }

                // If no real interfaces found, include all non-loopback active interfaces
                if net_interfaces.is_empty() {
                    for iface in &interfaces {
                        if !iface.name.starts_with("lo")
                            && !iface.name.contains("Loopback")
                            && iface.is_active()
                        {
                            let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, iface);
                            total_rx_rate += rx_rate;
                            total_tx_rate += tx_rate;
                            total_rx_bytes += iface.rx_bytes;
                            total_tx_bytes += iface.tx_bytes;

                            net_interfaces.push(NetworkInterfaceInfo {
                                name: iface.name.clone(),
                                is_up: iface.is_up,
                                rx_bytes: iface.rx_bytes,
                                tx_bytes: iface.tx_bytes,
                                rx_rate,
                                tx_rate,
                                speed_mbps: iface.speed_mbps,
                            });
                        }
                    }
                }

                self.network_info = NetworkInfo {
                    interfaces: net_interfaces,
                    total_rx_rate,
                    total_tx_rate,
                    total_rx_bytes,
                    total_tx_bytes,
                };

                // Add to history (convert to KB/s for reasonable scale)
                self.network_rx_history
                    .push_back((total_rx_rate / 1024.0) as u64);
                self.network_tx_history
                    .push_back((total_tx_rate / 1024.0) as u64);
                if self.network_rx_history.len() > MAX_HISTORY {
                    self.network_rx_history.pop_front();
                }
                if self.network_tx_history.len() > MAX_HISTORY {
                    self.network_tx_history.pop_front();
                }
            }
        }
        Ok(())
    }

    fn update_processes(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get processes from process monitor
        if let Some(ref mut monitor) = self.process_monitor {
            self.processes = monitor.processes().unwrap_or_default();
            // Rebuild cached process order for efficient scrolling
            self.rebuild_cached_process_order();
        }
        Ok(())
    }

    /// Rebuild the cached process order based on current display mode
    /// This is called when processes are updated or display mode changes
    fn rebuild_cached_process_order(&mut self) {
        use ProcessDisplayMode::*;

        // Build a list of (index, sort_key) tuples, then sort and extract indices
        let mut indexed: Vec<(usize, (u64, u64))> = Vec::with_capacity(self.processes.len());

        match self.process_display_mode {
            All | Cpu => {
                // Check if we have CPU data
                let has_cpu_data = self.processes.iter().any(|p| p.cpu_percent > 0.1);

                for (i, p) in self.processes.iter().enumerate() {
                    if self.process_display_mode == Cpu && !has_cpu_data {
                        // No CPU data, just use memory
                        indexed.push((i, (p.memory_bytes, 0)));
                    } else if self.process_display_mode == Cpu && p.cpu_percent <= 0.1 {
                        // Skip processes with no CPU usage in CPU mode
                        continue;
                    } else {
                        // For All mode: Boost GPU processes to show them first
                        // GPU processes get a large bonus in the sort key
                        let gpu_boost: u64 = if !p.gpu_indices.is_empty() {
                            1_000_000_000 // 1B bonus for GPU processes
                        } else {
                            0
                        };
                        // CPU percent as secondary key (scaled to u64 for comparison)
                        let cpu_key = (p.cpu_percent * 1000.0) as u64;
                        indexed.push((i, (gpu_boost + cpu_key, p.memory_bytes)));
                    }
                }
                // Sort descending - GPU processes first (due to boost), then by CPU
                indexed.sort_by(|a, b| b.1.cmp(&a.1));
            }
            Gpu(gpu_idx) | Accelerator(gpu_idx) => {
                for (i, p) in self.processes.iter().enumerate() {
                    if p.gpu_indices.contains(&gpu_idx) {
                        let mem = *p.gpu_memory_per_device.get(&gpu_idx).unwrap_or(&0);
                        indexed.push((i, (mem, 0)));
                    }
                }
                // Sort descending by GPU memory
                indexed.sort_by(|a, b| b.1.cmp(&a.1));
            }
            Npu(_) => {
                // No NPU support yet
            }
        }

        self.cached_process_order = indexed.into_iter().map(|(i, _)| i).collect();
        self.filtered_process_count = self.cached_process_order.len();
    }

    /// Get processes using cached order (fast - no sorting during render)
    /// Use this during UI rendering for smooth scrolling
    pub fn get_processes_cached(&self) -> Vec<&ProcessMonitorInfo> {
        self.cached_process_order
            .iter()
            .filter_map(|&i| self.processes.get(i))
            .collect()
    }

    /// Get a slice of cached processes for rendering (with pagination)
    /// This is the fastest method for rendering visible processes
    pub fn get_visible_processes(&self, skip: usize, take: usize) -> Vec<&ProcessMonitorInfo> {
        self.cached_process_order
            .iter()
            .skip(skip)
            .take(take)
            .filter_map(|&i| self.processes.get(i))
            .collect()
    }

    /// Get filtered processes based on current display mode
    pub fn get_filtered_processes(&self) -> Vec<&ProcessMonitorInfo> {
        use ProcessDisplayMode::*;

        match self.process_display_mode {
            All => {
                // Show all processes, sorted by CPU usage then memory
                let mut procs: Vec<&ProcessMonitorInfo> = self.processes.iter().collect();
                procs.sort_by(|a, b| {
                    // First compare by CPU, then by memory if CPU is equal
                    match b
                        .cpu_percent
                        .partial_cmp(&a.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                    {
                        std::cmp::Ordering::Equal => b.memory_bytes.cmp(&a.memory_bytes),
                        other => other,
                    }
                });
                procs
            }
            Cpu => {
                // Show top CPU consumers (or all processes sorted by memory if CPU data unavailable)
                let mut procs: Vec<&ProcessMonitorInfo> = self.processes.iter().collect();

                // Check if we have valid CPU data (at least one process with cpu_percent > 0)
                let has_cpu_data = procs.iter().any(|p| p.cpu_percent > 0.1);

                if has_cpu_data {
                    // Filter to processes actually using CPU
                    procs.retain(|p| p.cpu_percent > 0.1);
                    procs.sort_by(|a, b| {
                        b.cpu_percent
                            .partial_cmp(&a.cpu_percent)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                } else {
                    // No CPU data available (e.g., on Windows), sort by memory instead
                    procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));
                }
                procs
            }
            Gpu(gpu_idx) => {
                // Show processes using this specific GPU
                let mut procs: Vec<&ProcessMonitorInfo> = self
                    .processes
                    .iter()
                    .filter(|p| p.gpu_indices.contains(&gpu_idx))
                    .collect();
                procs.sort_by(|a, b| {
                    let a_mem = a.gpu_memory_per_device.get(&gpu_idx).unwrap_or(&0);
                    let b_mem = b.gpu_memory_per_device.get(&gpu_idx).unwrap_or(&0);
                    b_mem.cmp(a_mem)
                });
                procs
            }
            Npu(_npu_idx) => {
                // TODO: Implement NPU process filtering when NPU support is added
                Vec::new()
            }
            Accelerator(accel_idx) => {
                // Show processes using this specific accelerator (GPU-based for now)
                let mut procs: Vec<&ProcessMonitorInfo> = self
                    .processes
                    .iter()
                    .filter(|p| p.gpu_indices.contains(&accel_idx))
                    .collect();
                procs.sort_by(|a, b| {
                    let a_mem = a.gpu_memory_per_device.get(&accel_idx).unwrap_or(&0);
                    let b_mem = b.gpu_memory_per_device.get(&accel_idx).unwrap_or(&0);
                    b_mem.cmp(a_mem)
                });
                procs
            }
        }
    }

    pub fn set_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.selected_tab = index;
            self.scroll_position = 0;
        }
    }

    pub fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % self.tabs.len();
        self.scroll_position = 0;
    }

    pub fn previous_tab(&mut self) {
        if self.selected_tab > 0 {
            self.selected_tab -= 1;
        } else {
            self.selected_tab = self.tabs.len() - 1;
        }
        self.scroll_position = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        // Use cached process count to avoid expensive recalculation
        let visible_rows = 25; // Number of processes shown in UI
        let max_scroll = self.filtered_process_count.saturating_sub(visible_rows);
        if self.scroll_position < max_scroll {
            self.scroll_position = self.scroll_position.saturating_add(1);
        }
    }

    /// Get the current scroll position
    pub fn get_scroll_position(&self) -> usize {
        self.scroll_position
    }

    /// Scroll up by one page (25 rows)
    pub fn scroll_page_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(25);
    }

    /// Scroll down by one page (25 rows)
    pub fn scroll_page_down(&mut self) {
        let visible_rows = 25;
        let max_scroll = self.filtered_process_count.saturating_sub(visible_rows);
        self.scroll_position = (self.scroll_position + 25).min(max_scroll);
    }

    /// Scroll to the top of the process list
    pub fn scroll_to_top(&mut self) {
        self.scroll_position = 0;
    }

    /// Scroll to the bottom of the process list
    pub fn scroll_to_bottom(&mut self) {
        let visible_rows = 25;
        self.scroll_position = self.filtered_process_count.saturating_sub(visible_rows);
    }

    /// Cycle to next process display mode
    pub fn next_process_mode(&mut self) {
        use ProcessDisplayMode::*;
        self.process_display_mode = match self.process_display_mode {
            All => Cpu,
            Cpu => {
                if !self.accelerators.is_empty() {
                    Accelerator(0)
                } else if !self.gpu_info.is_empty() {
                    Gpu(0)
                } else {
                    All
                }
            }
            Accelerator(idx) => {
                if idx + 1 < self.accelerators.len() {
                    Accelerator(idx + 1)
                } else {
                    All
                }
            }
            Gpu(idx) => {
                if idx + 1 < self.gpu_info.len() {
                    Gpu(idx + 1)
                } else {
                    All
                }
            }
            Npu(_idx) => {
                // TODO: Implement NPU cycling when NPU support is added
                All
            }
        };
        self.scroll_position = 0;
        self.rebuild_cached_process_order();
    }

    /// Cycle to previous process display mode
    pub fn previous_process_mode(&mut self) {
        use ProcessDisplayMode::*;
        self.process_display_mode = match self.process_display_mode {
            All => {
                if !self.accelerators.is_empty() {
                    Accelerator(self.accelerators.len() - 1)
                } else if !self.gpu_info.is_empty() {
                    Gpu(self.gpu_info.len() - 1)
                } else {
                    Cpu
                }
            }
            Cpu => All,
            Accelerator(idx) => {
                if idx > 0 {
                    Accelerator(idx - 1)
                } else {
                    Cpu
                }
            }
            Gpu(idx) => {
                if idx > 0 {
                    Gpu(idx - 1)
                } else {
                    Cpu
                }
            }
            Npu(_idx) => {
                // TODO: Implement NPU cycling when NPU support is added
                if !self.accelerators.is_empty() {
                    Accelerator(self.accelerators.len() - 1)
                } else if !self.gpu_info.is_empty() {
                    Gpu(self.gpu_info.len() - 1)
                } else {
                    Cpu
                }
            }
        };
        self.scroll_position = 0;
        self.rebuild_cached_process_order();
    }

    /// Get display name for current process mode
    pub fn process_mode_name(&self) -> String {
        use ProcessDisplayMode::*;
        match self.process_display_mode {
            All => "All Processes".to_string(),
            Cpu => "CPU Processes".to_string(),
            Accelerator(idx) => {
                if let Some(accel) = self.accelerators.get(idx) {
                    format!("{} {} Processes", accel.accel_type, idx)
                } else {
                    format!("Accelerator {} Processes", idx)
                }
            }
            Gpu(idx) => format!("GPU {} Processes", idx),
            Npu(idx) => format!("NPU {} Processes", idx),
        }
    }

    pub fn reset_stats(&mut self) {
        self.cpu_history.clear();
        self.memory_history.clear();
        for history in &mut self.gpu_histories {
            history.clear();
        }
    }

    /// Check if encoder should be shown for a GPU based on timeout
    pub fn should_show_encoder(&self, gpu_index: usize) -> bool {
        if let Some(gpu) = self.gpu_info.get(gpu_index) {
            // If encoder is currently active, always show
            if gpu.encoder_util.is_some() && gpu.encoder_util.unwrap() > 0.0 {
                return true;
            }
            // If encoder was recently active, show based on timeout
            if let Some(last_active) = gpu.encoder_last_active {
                let timeout =
                    Duration::from_secs(self.config.general.encode_decode_hiding_timer as u64);
                return last_active.elapsed() < timeout;
            }
        }
        false
    }

    /// Check if decoder should be shown for a GPU based on timeout
    pub fn should_show_decoder(&self, gpu_index: usize) -> bool {
        if let Some(gpu) = self.gpu_info.get(gpu_index) {
            // If decoder is currently active, always show
            if gpu.decoder_util.is_some() && gpu.decoder_util.unwrap() > 0.0 {
                return true;
            }
            // If decoder was recently active, show based on timeout
            if let Some(last_active) = gpu.decoder_last_active {
                let timeout =
                    Duration::from_secs(self.config.general.encode_decode_hiding_timer as u64);
                return last_active.elapsed() < timeout;
            }
        }
        false
    }

    /// Save current configuration to disk
    pub fn save_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.config.save()?;
        self.set_status_message("Configuration saved successfully");
        Ok(())
    }

    /// Set a temporary status message
    pub fn set_status_message(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
    }

    /// Get current status message if not expired (5 second timeout)
    pub fn get_status_message(&self) -> Option<&str> {
        if let Some((msg, timestamp)) = &self.status_message {
            if timestamp.elapsed() < Duration::from_secs(5) {
                return Some(msg.as_str());
            }
        }
        None
    }

    /// Toggle agent input mode
    pub fn toggle_agent_input(&mut self) {
        self.agent_input_mode = !self.agent_input_mode;
        if self.agent_input_mode {
            self.agent_input.clear();
        }
    }

    /// Add character to agent input
    pub fn agent_input_char(&mut self, c: char) {
        if self.agent_input.len() < 200 {
            // Max 200 chars
            self.agent_input.push(c);
        }
    }

    /// Remove last character from agent input
    pub fn agent_input_backspace(&mut self) {
        self.agent_input.pop();
    }

    /// Submit agent query
    pub fn submit_agent_query(&mut self, monitor: &SiliconMonitor) {
        if self.agent_input.is_empty() {
            return;
        }

        let query = self.agent_input.clone();
        self.agent_input.clear();
        self.agent_input_mode = false;

        // Check if agent is available
        if let Some(ref mut agent) = self.agent {
            self.agent_loading = true;

            // Execute query
            match agent.ask(&query, monitor) {
                Ok(response) => {
                    self.agent_history.push_back(response);
                    if self.agent_history.len() > MAX_AGENT_HISTORY {
                        self.agent_history.pop_front();
                    }
                }
                Err(e) => {
                    self.set_status_message(format!("Agent error: {}", e));
                }
            }

            self.agent_loading = false;
        } else {
            self.set_status_message("Agent not available");
        }
    }

    /// Clear agent history
    pub fn clear_agent_history(&mut self) {
        self.agent_history.clear();
        self.set_status_message("Agent history cleared");
    }

    /// Get agent cache statistics
    pub fn agent_cache_stats(&self) -> Option<String> {
        self.agent
            .as_ref()
            .map(|agent| format!("Cache: {} entries", agent.cache_size()))
    }
}

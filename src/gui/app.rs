//! Main application state and logic for Silicon Monitor GUI

use eframe::egui;
use egui::{RichText, ScrollArea, Vec2};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

use super::theme::{self, threshold_color, trend_indicator, CyberColors};
use super::widgets::{
    CyberProgressBar, MetricCard, QuickLookPanel, SectionHeader, SparklineChart, ThresholdLegend,
};

use crate::ai_api::AiDataApi;
use crate::connections::{ConnectionInfo, ConnectionMonitor, ConnectionState, Protocol};
use crate::core::cpu::CpuStats;
use crate::core::memory::MemoryStats;
#[cfg(target_os = "windows")]
use crate::platform::windows as platform_impl;
use crate::disk::{self, DiskDevice};
use crate::gpu::{GpuCollection, GpuDynamicInfo, GpuStaticInfo};
use crate::motherboard::{self, DriverInfo, MotherboardDevice, SystemInfo as MBSystemInfo};
use crate::network_monitor::NetworkMonitor;
use crate::network_tools::{self, PortStatus};
use crate::process_monitor::{ProcessMonitor, ProcessMonitorInfo};
use crate::system_stats::SystemStats;

const HISTORY_SIZE: usize = 60;
const UPDATE_INTERVAL: Duration = Duration::from_millis(100); // Fast UI updates (10 FPS minimum)
const DATA_POLL_INTERVAL: Duration = Duration::from_millis(250); // Data polling rate
const SLOW_UPDATE_INTERVAL: Duration = Duration::from_secs(2); // Slow updates for heavy ops

/// Background data update result
struct DataUpdateResult {
    cpu_stats: Option<CpuStats>,
    memory_stats: Option<MemoryStats>,
    gpu_dynamic_info: Vec<GpuDynamicInfo>,
    network_rx: u64,
    network_tx: u64,
}

/// Cached disk data to avoid blocking I/O calls on every frame
#[derive(Default, Clone)]
struct CachedDiskData {
    info: Option<crate::disk::DiskInfo>,
    io_stats: Option<crate::disk::DiskIoStats>,
    health: Option<crate::disk::DiskHealth>,
    filesystems: Vec<crate::disk::FilesystemInfo>,
}

/// AI agent response from background thread
struct AgentResponse {
    response: String,
    inference_time_ms: u64,
    from_cache: bool,
}

/// Main application state
pub struct SiliconMonitorApp {
    // Current tab
    current_tab: Tab,

    // Hardware data
    cpu_stats: Option<CpuStats>,
    memory_stats: Option<MemoryStats>,
    #[allow(dead_code)]
    gpu_collection: Option<GpuCollection>,
    gpu_static_info: Vec<GpuStaticInfo>,
    gpu_dynamic_info: Vec<GpuDynamicInfo>,
    network_monitor: Option<NetworkMonitor>,
    process_monitor: Option<ProcessMonitor>,
    process_list: Vec<ProcessMonitorInfo>,

    // Disk data
    disks: Vec<Box<dyn DiskDevice>>,

    // Connection data
    connection_monitor: Option<ConnectionMonitor>,
    connections: Vec<ConnectionInfo>,
    connection_filter: String,
    connection_protocol_filter: Option<Protocol>,
    connection_state_filter: Option<ConnectionState>,

    // System/Motherboard data
    system_info: Option<MBSystemInfo>,
    system_info_tried: bool,
    motherboard_sensors: Vec<Box<dyn MotherboardDevice>>,
    driver_info: Vec<DriverInfo>,
    pcie_devices: Vec<motherboard::PcieDeviceInfo>,
    sata_devices: Vec<motherboard::SataDeviceInfo>,
    system_temps: Option<motherboard::SystemTemperatures>,
    peripherals: Option<motherboard::PeripheralsInfo>,

    // System-wide stats (Linux/BSD style - load avg, vmstat)
    system_stats: Option<SystemStats>,
    context_switches_history: VecDeque<f32>,
    interrupts_history: VecDeque<f32>,
    prev_context_switches: u64,
    prev_interrupts: u64,

    // Historical data for graphs
    cpu_history: VecDeque<f32>,
    per_core_history: Vec<VecDeque<f32>>,
    memory_history: VecDeque<f32>,
    gpu_history: Vec<VecDeque<f32>>,
    gpu_memory_history: Vec<VecDeque<f32>>,
    gpu_temp_history: Vec<VecDeque<f32>>,
    network_rx_history: VecDeque<f32>,
    network_tx_history: VecDeque<f32>,

    // Network rate tracking (bytes/sec)
    network_rates: std::collections::HashMap<String, (f64, f64)>,

    // Timing
    #[allow(dead_code)]
    last_update: Instant,
    last_slow_update: Instant,
    start_time: Instant,

    // System info
    hostname: String,
    os_info: String,

    // Process list state
    process_sort_column: ProcessSortColumn,
    process_sort_ascending: bool,
    process_filter: String,

    // Network Tools state
    nettools_target_host: String,
    nettools_ping_result: Option<crate::network_tools::PingResult>,
    nettools_traceroute_result: Option<crate::network_tools::TracerouteResult>,
    nettools_port_scan_results: Vec<crate::network_tools::PortScanResult>,
    nettools_nmap_result: Option<crate::network_tools::NmapScanResult>,
    nettools_capture_result: Option<crate::network_tools::CaptureResult>,
    nettools_capture_protocol: crate::network_tools::CaptureProtocol,
    nettools_capture_count: u32,
    nettools_port_range_start: u16,
    nettools_port_range_end: u16,
    nettools_is_running: bool,
    nettools_operation: String,
    nettools_dns_results: Vec<std::net::IpAddr>,

    // AI Agent state
    agent: Option<crate::agent::Agent>,
    silicon_monitor: Option<crate::SiliconMonitor>,
    ai_data_api: Option<AiDataApi>,
    agent_query: String,
    agent_history: VecDeque<AgentChatEntry>,
    agent_is_processing: bool,
    agent_response_receiver: Option<Receiver<Result<AgentResponse, String>>>,
    
    // AI configuration UI state
    ai_api_key_input: String,
    ai_selected_backend: AiBackendSelection,
    ai_selected_model: String,
    ai_ollama_models: Vec<String>,
    ai_ollama_starting: bool,
    ai_status_message: Option<(String, bool)>, // (message, is_error)

    // Background loading state
    system_info_receiver: Option<Receiver<SystemInfoResult>>,
    system_info_loading: bool,
    disk_receiver: Option<Receiver<Vec<Box<dyn DiskDevice + Send>>>>,
    disk_loading: bool,
    disk_loaded: bool,
    agent_receiver: Option<Receiver<Option<crate::agent::Agent>>>,
    agent_loading: bool,
    agent_loading_start: Instant,

    // Cached disk data (refreshed periodically, not on every frame)
    cached_disk_data: Vec<CachedDiskData>,
    last_disk_data_refresh: Instant,

    // Background data polling
    data_receiver: Option<Receiver<DataUpdateResult>>,
    data_poll_in_flight: bool,
    last_data_poll: Instant,

    // Settings
    show_settings: bool,
    settings: AppSettings,
}

/// Result from background system info loading
struct SystemInfoResult {
    system_info: Option<MBSystemInfo>,
    sensors: Vec<Box<dyn MotherboardDevice>>,
    drivers: Vec<DriverInfo>,
    pcie_devices: Vec<motherboard::PcieDeviceInfo>,
    sata_devices: Vec<motherboard::SataDeviceInfo>,
    system_temps: Option<motherboard::SystemTemperatures>,
    peripherals: Option<motherboard::PeripheralsInfo>,
}

/// A chat entry in the AI Agent conversation
#[derive(Debug, Clone)]
struct AgentChatEntry {
    role: ChatRole,
    content: String,
    #[allow(dead_code)]
    timestamp: std::time::Instant,
    inference_time_ms: Option<u64>,
    from_cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    CPU,
    Accelerators,
    Memory,
    Disk,
    Processes,
    Network,
    NetworkTools,
    Connections,
    SystemInfo,
    Peripherals,
    AIAssistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessSortColumn {
    Name,
    Pid,
    Cpu,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiBackendSelection {
    Ollama,
    OpenAi,
    Anthropic,
    GitHub,
    LmStudio,
}

impl Default for AiBackendSelection {
    fn default() -> Self {
        Self::Ollama
    }
}

/// Color theme options for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorTheme {
    #[default]
    Cyber,       // Default neon cyber theme
    Ocean,       // Blue/teal oceanic theme
    Forest,      // Green nature theme
    Sunset,      // Orange/red warm theme
    Monochrome,  // Grayscale minimalist
}

impl ColorTheme {
    fn name(&self) -> &'static str {
        match self {
            ColorTheme::Cyber => "Cyber (Default)",
            ColorTheme::Ocean => "Ocean",
            ColorTheme::Forest => "Forest",
            ColorTheme::Sunset => "Sunset",
            ColorTheme::Monochrome => "Monochrome",
        }
    }

    fn all() -> &'static [ColorTheme] {
        &[
            ColorTheme::Cyber,
            ColorTheme::Ocean,
            ColorTheme::Forest,
            ColorTheme::Sunset,
            ColorTheme::Monochrome,
        ]
    }

    /// Get the primary accent color for this theme
    pub fn accent_color(&self) -> egui::Color32 {
        match self {
            ColorTheme::Cyber => CyberColors::CYAN,
            ColorTheme::Ocean => egui::Color32::from_rgb(64, 224, 208),    // Turquoise
            ColorTheme::Forest => egui::Color32::from_rgb(34, 197, 94),    // Green
            ColorTheme::Sunset => egui::Color32::from_rgb(251, 146, 60),   // Orange
            ColorTheme::Monochrome => egui::Color32::from_rgb(200, 200, 200), // Light gray
        }
    }

    /// Get the secondary accent color for this theme
    pub fn secondary_color(&self) -> egui::Color32 {
        match self {
            ColorTheme::Cyber => CyberColors::MAGENTA,
            ColorTheme::Ocean => egui::Color32::from_rgb(56, 189, 248),    // Sky blue
            ColorTheme::Forest => egui::Color32::from_rgb(74, 222, 128),   // Light green
            ColorTheme::Sunset => egui::Color32::from_rgb(248, 113, 113),  // Red
            ColorTheme::Monochrome => egui::Color32::from_rgb(150, 150, 150), // Medium gray
        }
    }
}

/// Application settings
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub color_theme: ColorTheme,
    pub graph_line_thickness: f32,
    pub show_grid_lines: bool,
    pub animation_speed: f32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            color_theme: ColorTheme::Cyber,
            graph_line_thickness: 2.5,
            show_grid_lines: true,
            animation_speed: 1.0,
        }
    }
}

impl SiliconMonitorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply cyber theme
        theme::apply_cyber_theme(&cc.egui_ctx);

        // Initialize monitors
        let gpu_collection = GpuCollection::auto_detect().ok();
        let (gpu_static_info, gpu_dynamic_info) = if let Some(ref gpus) = gpu_collection {
            let static_info: Vec<GpuStaticInfo> = gpus
                .gpus()
                .iter()
                .filter_map(|g| g.static_info().ok())
                .collect();
            let dynamic_info: Vec<GpuDynamicInfo> = gpus
                .gpus()
                .iter()
                .filter_map(|g| g.dynamic_info().ok())
                .collect();
            (static_info, dynamic_info)
        } else {
            (vec![], vec![])
        };

        let gpu_count = gpu_static_info.len();

        // Get initial CPU core count using platform-specific implementation
        #[cfg(target_os = "windows")]
        let cpu_core_count = platform_impl::read_cpu_stats()
            .ok()
            .map(|s| s.cores.len())
            .unwrap_or(0);
        #[cfg(not(target_os = "windows"))]
        let cpu_core_count = num_cpus::get();

        // Get initial CPU stats
        #[cfg(target_os = "windows")]
        let initial_cpu_stats = platform_impl::read_cpu_stats().ok();
        #[cfg(not(target_os = "windows"))]
        let initial_cpu_stats = CpuStats::new().ok();

        // Get initial memory stats
        #[cfg(target_os = "windows")]
        let initial_memory_stats = platform_impl::read_memory_stats().ok();
        #[cfg(not(target_os = "windows"))]
        let initial_memory_stats = MemoryStats::new().ok();

        // Start background loading for AI agent (avoid blocking UI with HTTP timeouts)
        let (agent_tx, agent_rx) = channel();
        std::thread::spawn(move || {
            let agent = crate::agent::AgentConfig::auto_detect()
                .ok()
                .and_then(|config| crate::agent::Agent::new(config).ok());
            let _ = agent_tx.send(agent);
        });

        let mut app = Self {
            current_tab: Tab::Overview,
            cpu_stats: initial_cpu_stats,
            memory_stats: initial_memory_stats,
            gpu_collection,
            gpu_static_info,
            gpu_dynamic_info,
            network_monitor: NetworkMonitor::new().ok(),
            process_monitor: ProcessMonitor::new().ok(),
            process_list: Vec::new(),
            disks: Vec::new(), // Loaded lazily when Disk tab is visited
            connection_monitor: ConnectionMonitor::new().ok(),
            connections: Vec::new(),
            connection_filter: String::new(),
            connection_protocol_filter: None,
            connection_state_filter: None,
            system_info: None, // Will be fetched lazily on GUI thread
            system_info_tried: false,
            motherboard_sensors: Vec::new(), // Will be fetched lazily
            driver_info: Vec::new(),         // Will be fetched lazily
            pcie_devices: Vec::new(),        // Will be fetched lazily
            sata_devices: Vec::new(),        // Will be fetched lazily
            system_temps: None,              // Will be fetched lazily
            peripherals: None,               // Will be fetched lazily
            system_stats: SystemStats::new().ok(),
            context_switches_history: VecDeque::with_capacity(HISTORY_SIZE),
            interrupts_history: VecDeque::with_capacity(HISTORY_SIZE),
            prev_context_switches: 0,
            prev_interrupts: 0,
            cpu_history: VecDeque::with_capacity(HISTORY_SIZE),
            per_core_history: (0..cpu_core_count)
                .map(|_| VecDeque::with_capacity(HISTORY_SIZE))
                .collect(),
            memory_history: VecDeque::with_capacity(HISTORY_SIZE),
            gpu_history: (0..gpu_count)
                .map(|_| VecDeque::with_capacity(HISTORY_SIZE))
                .collect(),
            gpu_memory_history: (0..gpu_count)
                .map(|_| VecDeque::with_capacity(HISTORY_SIZE))
                .collect(),
            gpu_temp_history: (0..gpu_count)
                .map(|_| VecDeque::with_capacity(HISTORY_SIZE))
                .collect(),
            network_rx_history: VecDeque::with_capacity(HISTORY_SIZE),
            network_tx_history: VecDeque::with_capacity(HISTORY_SIZE),
            network_rates: HashMap::new(),
            last_update: Instant::now(),
            last_slow_update: Instant::now(),
            start_time: Instant::now(),
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            os_info: std::env::consts::OS.to_string(),
            process_sort_column: ProcessSortColumn::Cpu,
            process_sort_ascending: false,
            process_filter: String::new(),
            nettools_target_host: "8.8.8.8".to_string(),
            nettools_ping_result: None,
            nettools_traceroute_result: None,
            nettools_port_scan_results: Vec::new(),
            nettools_nmap_result: None,
            nettools_capture_result: None,
            nettools_capture_protocol: crate::network_tools::CaptureProtocol::All,
            nettools_capture_count: 50,
            nettools_port_range_start: 1,
            nettools_port_range_end: 1024,
            nettools_is_running: false,
            nettools_operation: String::new(),
            nettools_dns_results: Vec::new(),

            // AI Agent - loading in background
            agent: None, // Will be populated when background thread completes
            silicon_monitor: crate::SiliconMonitor::new().ok(),
            ai_data_api: AiDataApi::new().ok(),
            agent_query: String::new(),
            agent_history: VecDeque::with_capacity(50),
            agent_is_processing: false,
            agent_response_receiver: None,
            
            // AI configuration UI
            ai_api_key_input: String::new(),
            ai_selected_backend: AiBackendSelection::default(),
            ai_selected_model: {
                let models = Self::detect_ollama_models();
                models.first().cloned().unwrap_or_default()
            },
            ai_ollama_models: Self::detect_ollama_models(),
            ai_ollama_starting: false,
            ai_status_message: None,

            // Background loading state
            system_info_receiver: None,
            system_info_loading: false,
            disk_receiver: None,
            disk_loading: false,
            disk_loaded: false,
            agent_receiver: Some(agent_rx),
            agent_loading: true,
            agent_loading_start: Instant::now(),

            // Cached disk data (avoid per-frame I/O)
            cached_disk_data: Vec::new(),
            last_disk_data_refresh: Instant::now(),

            // Background data polling
            data_receiver: None,
            data_poll_in_flight: false,
            last_data_poll: Instant::now(),

            // Settings
            show_settings: false,
            settings: AppSettings::default(),
        };

        // Initialize history with zeros
        for _ in 0..HISTORY_SIZE {
            app.cpu_history.push_back(0.0);
            app.memory_history.push_back(0.0);
            app.network_rx_history.push_back(0.0);
            app.network_tx_history.push_back(0.0);
            app.context_switches_history.push_back(0.0);
            app.interrupts_history.push_back(0.0);
            for hist in &mut app.gpu_history {
                hist.push_back(0.0);
            }
            for hist in &mut app.gpu_memory_history {
                hist.push_back(0.0);
            }
            for hist in &mut app.gpu_temp_history {
                hist.push_back(0.0);
            }
            for hist in &mut app.per_core_history {
                hist.push_back(0.0);
            }
        }

        // Initialize previous context switch/interrupt values
        if let Some(ref stats) = app.system_stats {
            if let Some(ref vm) = stats.vm_stats {
                app.prev_context_switches = vm.context_switches;
                app.prev_interrupts = vm.interrupts;
            }
        }

        app
    }

    /// Start a background data poll (non-blocking)
    fn start_data_poll(&mut self) {
        if self.data_poll_in_flight {
            return;
        }

        self.data_poll_in_flight = true;
        self.last_data_poll = Instant::now();

        let (tx, rx) = channel();
        self.data_receiver = Some(rx);

        std::thread::spawn(move || {
            // Collect CPU stats
            #[cfg(target_os = "windows")]
            let cpu_stats = crate::platform::windows::read_cpu_stats().ok();
            #[cfg(not(target_os = "windows"))]
            let cpu_stats = CpuStats::new().ok();

            // Collect Memory stats
            #[cfg(target_os = "windows")]
            let memory_stats = crate::platform::windows::read_memory_stats().ok();
            #[cfg(not(target_os = "windows"))]
            let memory_stats = MemoryStats::new().ok();

            // Collect GPU info - create fresh collection in background
            let gpu_dynamic_info: Vec<GpuDynamicInfo> =
                if let Ok(gpus) = GpuCollection::auto_detect() {
                    gpus.gpus()
                        .iter()
                        .filter_map(|g| g.dynamic_info().ok())
                        .collect()
                } else {
                    vec![]
                };

            // Collect network stats (simplified - just totals)
            let mut network_rx = 0u64;
            let mut network_tx = 0u64;

            if let Ok(mut monitor) = NetworkMonitor::new() {
                if let Ok(interfaces) = monitor.interfaces() {
                    network_rx = interfaces.iter().map(|i| i.rx_bytes).sum();
                    network_tx = interfaces.iter().map(|i| i.tx_bytes).sum();
                }
            }

            let _ = tx.send(DataUpdateResult {
                cpu_stats,
                memory_stats,
                gpu_dynamic_info,
                network_rx,
                network_tx,
            });
        });
    }

    /// Apply data from background poll to UI state (fast, non-blocking)
    fn apply_data_update(&mut self, result: DataUpdateResult) {
        // Update CPU
        if let Some(stats) = result.cpu_stats {
            let cpu_usage = 100.0 - stats.total.idle;
            self.cpu_history.pop_front();
            self.cpu_history.push_back(cpu_usage);

            // Update per-core history
            for (i, core) in stats.cores.iter().enumerate() {
                if i < self.per_core_history.len() {
                    let util = core.user.unwrap_or(0.0) + core.system.unwrap_or(0.0);
                    self.per_core_history[i].pop_front();
                    self.per_core_history[i].push_back(util);
                }
            }

            self.cpu_stats = Some(stats);
        }

        // Update Memory
        if let Some(stats) = result.memory_stats {
            let usage = stats.ram_usage_percent();
            self.memory_history.pop_front();
            self.memory_history.push_back(usage);
            self.memory_stats = Some(stats);
        }

        // Update GPU
        self.gpu_dynamic_info = result.gpu_dynamic_info;
        for (i, info) in self.gpu_dynamic_info.iter().enumerate() {
            if i < self.gpu_history.len() {
                self.gpu_history[i].pop_front();
                self.gpu_history[i].push_back(info.utilization as f32);
            }

            if i < self.gpu_memory_history.len() {
                self.gpu_memory_history[i].pop_front();
                let mem_pct = if info.memory.total > 0 {
                    (info.memory.used as f32 / info.memory.total as f32) * 100.0
                } else {
                    0.0
                };
                self.gpu_memory_history[i].push_back(mem_pct);
            }

            if i < self.gpu_temp_history.len() {
                self.gpu_temp_history[i].pop_front();
                let temp = info.thermal.temperature.unwrap_or(0) as f32;
                self.gpu_temp_history[i].push_back(temp);
            }
        }

        // Update Network - calculate rates using persistent monitor
        self.network_rx_history.pop_front();
        self.network_rx_history
            .push_back((result.network_rx as f32 / 1024.0 / 1024.0) % 10000.0);
        self.network_tx_history.pop_front();
        self.network_tx_history
            .push_back((result.network_tx as f32 / 1024.0 / 1024.0) % 10000.0);

        // Calculate network rates using persistent monitor (has prev_stats for rate calculation)
        if let Some(ref mut monitor) = self.network_monitor {
            if let Ok(interfaces) = monitor.interfaces() {
                for iface in &interfaces {
                    let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, iface);
                    self.network_rates.insert(iface.name.clone(), (rx_rate, tx_rate));
                }
            }
        }
    }

    /// Slow update for heavy operations (processes, connections)
    fn update_data_slow(&mut self) {
        // Update Processes (only if tab is visible or list is empty)
        if self.current_tab == Tab::Processes || self.process_list.is_empty() {
            if let Some(ref mut monitor) = self.process_monitor {
                if let Ok(processes) = monitor.processes() {
                    self.process_list = processes;
                }
            }
        }

        // Update Connections (only if tab is visible)
        if self.current_tab == Tab::Connections {
            if let Some(ref monitor) = self.connection_monitor {
                if let Ok(conns) = monitor.all_connections() {
                    self.connections = conns;
                }
            }
        }

        // Update System Stats (Linux/BSD style - load avg, vmstat, etc.)
        if let Ok(stats) = SystemStats::new() {
            // Track context switches per second
            if let Some(ref vm) = stats.vm_stats {
                let ctx_delta = vm
                    .context_switches
                    .saturating_sub(self.prev_context_switches);
                let int_delta = vm.interrupts.saturating_sub(self.prev_interrupts);

                self.context_switches_history.pop_front();
                self.context_switches_history.push_back(ctx_delta as f32);

                self.interrupts_history.pop_front();
                self.interrupts_history.push_back(int_delta as f32);

                self.prev_context_switches = vm.context_switches;
                self.prev_interrupts = vm.interrupts;
            }

            self.system_stats = Some(stats);
        }
    }

    /// Check for completed background loading operations (non-blocking)
    fn check_background_loaders(&mut self) {
        // Check data poll background loading (most important - high frequency)
        if let Some(ref receiver) = self.data_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.apply_data_update(result);
                self.data_poll_in_flight = false;
                self.data_receiver = None;
            }
        }

        // Check AI agent background loading
        if let Some(ref receiver) = self.agent_receiver {
            if let Ok(agent) = receiver.try_recv() {
                self.agent = agent;
                self.agent_loading = false;
                self.agent_receiver = None;
            }
        }

        // Check disk background loading
        if let Some(ref receiver) = self.disk_receiver {
            if let Ok(disks) = receiver.try_recv() {
                // Convert from Send-able to regular DiskDevice
                self.disks = disks.into_iter().map(|d| d as Box<dyn DiskDevice>).collect();
                self.disk_loading = false;
                self.disk_loaded = true;
                self.disk_receiver = None;
            }
        }

        // Check system info background loading
        if let Some(ref receiver) = self.system_info_receiver {
            if let Ok(result) = receiver.try_recv() {
                self.system_info = result.system_info;
                self.motherboard_sensors = result.sensors;
                self.driver_info = result.drivers;
                self.pcie_devices = result.pcie_devices;
                self.sata_devices = result.sata_devices;
                self.system_temps = result.system_temps;
                self.peripherals = result.peripherals;
                self.system_info_loading = false;
                self.system_info_tried = true;
                self.system_info_receiver = None;
            }
        }

        // Check AI agent response (non-blocking)
        if let Some(ref receiver) = self.agent_response_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(response) => {
                        self.agent_history.push_back(AgentChatEntry {
                            role: ChatRole::Assistant,
                            content: response.response,
                            timestamp: std::time::Instant::now(),
                            inference_time_ms: Some(response.inference_time_ms),
                            from_cache: response.from_cache,
                        });
                    }
                    Err(e) => {
                        self.agent_history.push_back(AgentChatEntry {
                            role: ChatRole::Assistant,
                            content: format!("Error: {}", e),
                            timestamp: std::time::Instant::now(),
                            inference_time_ms: None,
                            from_cache: false,
                        });
                    }
                }
                self.agent_is_processing = false;
                self.agent_response_receiver = None;

                // Limit history size
                while self.agent_history.len() > 100 {
                    self.agent_history.pop_front();
                }
            }
        }

        // Refresh cached disk data every 2 seconds (only when disks are loaded)
        if self.disk_loaded && self.last_disk_data_refresh.elapsed() >= Duration::from_secs(2) {
            self.refresh_cached_disk_data();
            self.last_disk_data_refresh = Instant::now();
        }
    }

    /// Refresh cached disk data (called periodically, not every frame)
    fn refresh_cached_disk_data(&mut self) {
        // Ensure we have enough cached entries for all disks
        while self.cached_disk_data.len() < self.disks.len() {
            self.cached_disk_data.push(CachedDiskData::default());
        }

        // Update cache for each disk
        for (i, disk) in self.disks.iter().enumerate() {
            if i < self.cached_disk_data.len() {
                self.cached_disk_data[i] = CachedDiskData {
                    info: disk.info().ok(),
                    io_stats: disk.io_stats().ok(),
                    health: disk.health().ok(),
                    filesystems: disk.filesystem_info().unwrap_or_default(),
                };
            }
        }
    }

    /// Start lazy loading of disk data
    fn start_disk_loading(&mut self) {
        if self.disk_loaded || self.disk_loading {
            return;
        }

        self.disk_loading = true;
        let (tx, rx) = channel();
        self.disk_receiver = Some(rx);

        std::thread::spawn(move || {
            let disks = disk::enumerate_disks().unwrap_or_default();
            // Convert to Send-able type
            let sendable_disks: Vec<Box<dyn DiskDevice + Send>> = disks
                .into_iter()
                .map(|d| d as Box<dyn DiskDevice + Send>)
                .collect();
            let _ = tx.send(sendable_disks);
        });
    }

    /// Start lazy loading of system info
    fn start_system_info_loading(&mut self) {
        if self.system_info_tried || self.system_info_loading {
            return;
        }

        self.system_info_loading = true;
        let (tx, rx) = channel();
        self.system_info_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = SystemInfoResult {
                system_info: motherboard::get_system_info().ok(),
                sensors: motherboard::enumerate_sensors().unwrap_or_default(),
                drivers: motherboard::get_driver_versions().unwrap_or_default(),
                pcie_devices: motherboard::get_pcie_devices().unwrap_or_default(),
                sata_devices: motherboard::get_sata_devices().unwrap_or_default(),
                system_temps: motherboard::get_system_temperatures().ok(),
                peripherals: motherboard::get_peripherals().ok(),
            };
            let _ = tx.send(result);
        });
    }

    fn cpu_usage(&self) -> f32 {
        self.cpu_stats
            .as_ref()
            .map(|s| 100.0 - s.total.idle)
            .unwrap_or(0.0)
    }

    fn memory_usage(&self) -> f32 {
        self.memory_stats
            .as_ref()
            .map(|s| s.ram_usage_percent())
            .unwrap_or(0.0)
    }
}

impl eframe::App for SiliconMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for background loading completions (non-blocking) - must be first!
        self.check_background_loaders();

        // Start new data poll if needed (non-blocking async data collection)
        if self.last_data_poll.elapsed() >= DATA_POLL_INTERVAL && !self.data_poll_in_flight {
            self.start_data_poll();
        }

        // Slow updates (Processes, Connections, System Stats) - every 2s
        if self.last_slow_update.elapsed() >= SLOW_UPDATE_INTERVAL {
            self.update_data_slow();
            self.last_slow_update = Instant::now();
        }

        // Request fast repaint for smooth UI (100ms = 10 FPS minimum)
        ctx.request_repaint_after(UPDATE_INTERVAL);

        // Top panel with title and tabs
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Logo/Title
                ui.heading(RichText::new("⚡ Silicon Monitor").color(CyberColors::CYAN));
                ui.separator();

                // Tabs - use local variable to avoid borrow issues
                let current = self.current_tab;
                let tab_color = |tab: Tab| {
                    if current == tab {
                        CyberColors::CYAN
                    } else {
                        CyberColors::TEXT_SECONDARY
                    }
                };

                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Overview,
                    RichText::new("📊 Overview").color(tab_color(Tab::Overview)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::CPU,
                    RichText::new("🔲 CPU").color(tab_color(Tab::CPU)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Accelerators,
                    RichText::new("⚡ Accelerators").color(tab_color(Tab::Accelerators)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Memory,
                    RichText::new("💾 Memory").color(tab_color(Tab::Memory)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Disk,
                    RichText::new("💿 Disk").color(tab_color(Tab::Disk)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Processes,
                    RichText::new("📋 Processes").color(tab_color(Tab::Processes)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Network,
                    RichText::new("🌐 Network").color(tab_color(Tab::Network)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Connections,
                    RichText::new("🔌 Sockets").color(tab_color(Tab::Connections)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::NetworkTools,
                    RichText::new("🔧 Tools").color(tab_color(Tab::NetworkTools)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::Peripherals,
                    RichText::new("🔌 Peripherals").color(tab_color(Tab::Peripherals)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::SystemInfo,
                    RichText::new("🖥️ System").color(tab_color(Tab::SystemInfo)),
                );
                ui.selectable_value(
                    &mut self.current_tab,
                    Tab::AIAssistant,
                    RichText::new("🤖 AI").color(tab_color(Tab::AIAssistant)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings gear icon
                    let settings_btn = ui.add(
                        egui::Button::new(RichText::new("⚙").size(16.0))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                    );
                    if settings_btn.clicked() {
                        self.show_settings = !self.show_settings;
                    }
                    if settings_btn.hovered() {
                        settings_btn.on_hover_text("Settings");
                    }
                    
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("{}@{}", self.hostname, self.os_info))
                            .color(CyberColors::TEXT_SECONDARY)
                            .small(),
                    );
                });
            });
            ui.add_space(4.0);
        });

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                // Quick stats
                let cpu_usage = self.cpu_usage();
                ui.label(
                    RichText::new(format!("CPU: {:.1}%", cpu_usage))
                        .color(theme::utilization_color(cpu_usage)),
                );
                ui.separator();

                let mem_usage = self.memory_usage();
                ui.label(
                    RichText::new(format!("RAM: {:.1}%", mem_usage))
                        .color(theme::utilization_color(mem_usage)),
                );
                ui.separator();

                for (i, gpu) in self.gpu_dynamic_info.iter().enumerate() {
                    ui.label(
                        RichText::new(format!("GPU{}: {}%", i, gpu.utilization))
                            .color(theme::utilization_color(gpu.utilization as f32)),
                    );
                    if let Some(temp) = gpu.thermal.temperature {
                        ui.label(
                            RichText::new(format!("{}°C", temp))
                                .color(theme::temperature_color(temp as u32)),
                        );
                    }
                    ui.separator();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Press F1 for help")
                            .color(CyberColors::TEXT_MUTED)
                            .small(),
                    );
                });
            });
            ui.add_space(2.0);
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| match self.current_tab {
            Tab::Overview => self.draw_overview(ui),
            Tab::CPU => self.draw_cpu_tab(ui),
            Tab::Accelerators => self.draw_accelerators_tab(ui),
            Tab::Memory => self.draw_memory_tab(ui),
            Tab::Disk => self.draw_disk_tab(ui),
            Tab::Processes => self.draw_processes_tab(ui),
            Tab::Network => self.draw_network_tab(ui),
            Tab::NetworkTools => self.draw_network_tools_tab(ui),
            Tab::Connections => self.draw_connections_tab(ui),
            Tab::SystemInfo => self.draw_system_info_tab(ui),
            Tab::Peripherals => self.draw_peripherals_tab(ui),
            Tab::AIAssistant => self.draw_ai_assistant_tab(ui),
        });

        // Settings window (floating)
        self.draw_settings_window(ctx);
    }
}

impl SiliconMonitorApp {
    fn draw_overview(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            // Glances-style QuickLook panel at the top
            let cpu_usage = self.cpu_usage();
            let mem_usage = self
                .memory_stats
                .as_ref()
                .map(|m| m.ram_usage_percent())
                .unwrap_or(0.0);
            let swap_usage = self
                .memory_stats
                .as_ref()
                .map(|m| {
                    if m.swap.total > 0 {
                        (m.swap.used as f64 / m.swap.total as f64 * 100.0) as f32
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);
            let load_avg = self
                .system_stats
                .as_ref()
                .and_then(|s| s.load_average.as_ref())
                .map(|l| l.one as f32)
                .unwrap_or(0.0);

            // Calculate trends from history
            let cpu_trend = self
                .cpu_history
                .iter()
                .rev()
                .nth(1)
                .map(|&prev| trend_indicator(cpu_usage, prev).0)
                .unwrap_or("→");
            let mem_trend = self
                .memory_history
                .iter()
                .rev()
                .nth(1)
                .map(|&prev| trend_indicator(mem_usage, prev).0)
                .unwrap_or("→");

            ui.add(
                QuickLookPanel::new(cpu_usage, mem_usage, swap_usage, load_avg)
                    .with_trends(cpu_trend, mem_trend),
            );

            ui.add_space(4.0);

            // Threshold legend
            ui.add(ThresholdLegend);

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // System status bar (htop-style)
            ui.horizontal(|ui| {
                // Uptime
                let uptime = self.start_time.elapsed();
                let hours = uptime.as_secs() / 3600;
                let minutes = (uptime.as_secs() % 3600) / 60;
                let seconds = uptime.as_secs() % 60;
                ui.label(
                    RichText::new(format!("⏱ {:02}:{:02}:{:02}", hours, minutes, seconds))
                        .color(CyberColors::CYAN),
                );
                ui.separator();

                // Process state summary (htop-style: Tasks: X, Y thr; 1 running)
                let running_count = self.process_list.iter().filter(|p| p.state == 'R').count();
                let sleeping_count = self.process_list.iter().filter(|p| p.state == 'S').count();
                let zombie_count = self.process_list.iter().filter(|p| p.state == 'Z').count();
                let disk_wait_count = self.process_list.iter().filter(|p| p.state == 'D').count();
                let stopped_count = self.process_list.iter().filter(|p| p.state == 'T').count();

                ui.label(
                    RichText::new(format!("Tasks: {}", self.process_list.len()))
                        .color(CyberColors::TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(format!("{}R", running_count)).color(CyberColors::NEON_GREEN),
                );
                ui.label(RichText::new(format!("{}S", sleeping_count)).color(CyberColors::CYAN));
                if zombie_count > 0 {
                    ui.label(
                        RichText::new(format!("{}Z", zombie_count)).color(CyberColors::NEON_RED),
                    );
                }
                if disk_wait_count > 0 {
                    ui.label(
                        RichText::new(format!("{}D", disk_wait_count))
                            .color(CyberColors::NEON_YELLOW),
                    );
                }
                if stopped_count > 0 {
                    ui.label(
                        RichText::new(format!("{}T", stopped_count))
                            .color(CyberColors::NEON_ORANGE),
                    );
                }
                ui.separator();

                // Connections count
                ui.label(
                    RichText::new(format!("🔌 {} Conn", self.connections.len()))
                        .color(CyberColors::NEON_PURPLE),
                );
                ui.separator();

                // Accelerator count
                if !self.gpu_static_info.is_empty() {
                    ui.label(
                        RichText::new(format!("⚡ {} Accel", self.gpu_static_info.len()))
                            .color(CyberColors::NEON_ORANGE),
                    );
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Top metric cards
            ui.horizontal_wrapped(|ui| {
                // CPU Card
                let cpu_usage = self.cpu_usage();
                ui.add(
                    MetricCard::new("CPU Usage", format!("{:.1}", cpu_usage))
                        .unit("%")
                        .color(theme::utilization_color(cpu_usage)),
                );

                // Memory Card
                if let Some(ref mem) = self.memory_stats {
                    let usage = mem.ram_usage_percent();
                    let used_gb = mem.ram.used as f64 / 1024.0 / 1024.0;
                    ui.add(
                        MetricCard::new("Memory", format!("{:.1}", used_gb))
                            .unit("MB")
                            .color(theme::utilization_color(usage)),
                    );
                }

                // Accelerator Cards
                for (i, (static_info, dynamic_info)) in self
                    .gpu_static_info
                    .iter()
                    .zip(self.gpu_dynamic_info.iter())
                    .enumerate()
                {
                    use crate::gpu::GpuVendor;
                    let accel_type = match static_info.vendor {
                        GpuVendor::Nvidia
                        | GpuVendor::Amd
                        | GpuVendor::Intel
                        | GpuVendor::Apple => "GPU",
                    };
                    ui.add(
                        MetricCard::new(&format!("{} {}", accel_type, i), dynamic_info.utilization)
                            .unit("%")
                            .color(theme::neon_color_by_index(i)),
                    );

                    if let Some(temp) = dynamic_info.thermal.temperature {
                        ui.add(
                            MetricCard::new(
                                &format!(
                                    "{} Temp",
                                    &static_info.name[..static_info.name.len().min(10)]
                                ),
                                temp,
                            )
                            .unit("°C")
                            .color(theme::temperature_color(temp as u32)),
                        );
                    }

                    // GPU Memory
                    let mem_pct = if dynamic_info.memory.total > 0 {
                        (dynamic_info.memory.used as f32 / dynamic_info.memory.total as f32) * 100.0
                    } else {
                        0.0
                    };
                    ui.add(
                        MetricCard::new(&format!("GPU{} Mem", i), format!("{:.0}", mem_pct))
                            .unit("%")
                            .color(theme::utilization_color(mem_pct)),
                    );
                }
            });

            ui.add_space(16.0);

            // Charts section
            ui.columns(2, |columns| {
                // CPU Chart
                columns[0].add(
                    SparklineChart::new(self.cpu_history.iter().cloned().collect())
                        .color(CyberColors::CYAN)
                        .height(100.0)
                        .title("CPU Usage")
                        .unit("%")
                        .max_value(100.0)
                        .show_scale(true)
                        .show_min_max(true),
                );

                // Memory Chart
                columns[1].add(
                    SparklineChart::new(self.memory_history.iter().cloned().collect())
                        .color(CyberColors::MAGENTA)
                        .height(100.0)
                        .title("Memory Usage")
                        .unit("%")
                        .max_value(100.0)
                        .show_scale(true)
                        .show_min_max(true),
                );
            });

            ui.add_space(16.0);

            // GPU Charts
            if !self.gpu_history.is_empty() {
                ui.add(SectionHeader::new("GPU Utilization").icon("🎮"));
                let num_cols = self.gpu_history.len().min(4);
                ui.columns(num_cols.max(1), |columns| {
                    for (i, hist) in self.gpu_history.iter().enumerate() {
                        if i < columns.len() {
                            columns[i].add(
                                SparklineChart::new(hist.iter().cloned().collect())
                                    .color(theme::neon_color_by_index(i))
                                    .height(80.0)
                                    .title(&format!("GPU {}", i))
                                    .unit("%")
                                    .max_value(100.0)
                                    .show_scale(true),
                            );
                        }
                    }
                });
            }

            ui.add_space(16.0);

            // Linux/BSD style System Stats (like htop/vmstat)
            ui.add(SectionHeader::new("System Stats (Linux/BSD Style)").icon("📈"));

            // System info row
            ui.horizontal(|ui| {
                // Load Average (htop/uptime style)
                if let Some(ref stats) = self.system_stats {
                    if let Some(ref load) = stats.load_average {
                        ui.label(
                            RichText::new(format!(
                                "⚖ Load: {:.2}, {:.2}, {:.2}",
                                load.one, load.five, load.fifteen
                            ))
                            .color(CyberColors::CYAN),
                        );
                        ui.separator();
                    }

                    // System uptime (from OS, not app)
                    if let Some(uptime) = stats.uptime_seconds {
                        let days = uptime / 86400;
                        let hours = (uptime % 86400) / 3600;
                        let mins = (uptime % 3600) / 60;
                        let uptime_str = if days > 0 {
                            format!("🖥 Uptime: {}d {:02}h {:02}m", days, hours, mins)
                        } else {
                            format!("🖥 Uptime: {:02}h {:02}m", hours, mins)
                        };
                        ui.label(RichText::new(uptime_str).color(CyberColors::NEON_GREEN));
                        ui.separator();
                    }

                    // Running/Total processes
                    if stats.running_processes > 0 || stats.total_processes > 0 {
                        ui.label(
                            RichText::new(format!(
                                "🔄 Tasks: {} running, {} total",
                                stats.running_processes, stats.total_processes
                            ))
                            .color(CyberColors::NEON_PURPLE),
                        );
                        ui.separator();
                    }

                    // CPUs
                    if stats.num_cpus > 0 {
                        ui.label(
                            RichText::new(format!("💻 {} CPUs", stats.num_cpus))
                                .color(CyberColors::NEON_ORANGE),
                        );
                    }
                }
            });

            ui.add_space(8.0);

            // CPU Time breakdown (like vmstat/top)
            if let Some(ref stats) = self.system_stats {
                if let Some(ref cpu_time) = stats.cpu_time {
                    let total = cpu_time.total() as f32;
                    if total > 0.0 {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("CPU Time: ").color(CyberColors::TEXT_PRIMARY));
                            ui.label(
                                RichText::new(format!(
                                    "us:{:.1}%",
                                    (cpu_time.user as f32 / total) * 100.0
                                ))
                                .color(CyberColors::NEON_GREEN)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "sy:{:.1}%",
                                    (cpu_time.system as f32 / total) * 100.0
                                ))
                                .color(CyberColors::NEON_ORANGE)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "ni:{:.1}%",
                                    (cpu_time.nice as f32 / total) * 100.0
                                ))
                                .color(CyberColors::CYAN)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "id:{:.1}%",
                                    (cpu_time.idle as f32 / total) * 100.0
                                ))
                                .color(CyberColors::TEXT_MUTED)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "wa:{:.1}%",
                                    (cpu_time.iowait as f32 / total) * 100.0
                                ))
                                .color(CyberColors::NEON_RED)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "hi:{:.1}%",
                                    (cpu_time.irq as f32 / total) * 100.0
                                ))
                                .color(CyberColors::MAGENTA)
                                .small(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "si:{:.1}%",
                                    (cpu_time.softirq as f32 / total) * 100.0
                                ))
                                .color(CyberColors::NEON_PURPLE)
                                .small(),
                            );
                            if cpu_time.steal > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "st:{:.1}%",
                                        (cpu_time.steal as f32 / total) * 100.0
                                    ))
                                    .color(CyberColors::NEON_YELLOW)
                                    .small(),
                                );
                            }
                        });
                    }
                }

                // VMstat info (context switches, interrupts, etc.)
                if let Some(ref vm) = stats.vm_stats {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("VMstat: ").color(CyberColors::TEXT_PRIMARY));
                        // Show rates per second
                        let ctx_rate = self.context_switches_history.back().unwrap_or(&0.0);
                        let int_rate = self.interrupts_history.back().unwrap_or(&0.0);
                        ui.label(
                            RichText::new(format!("ctx/s:{:.0}", ctx_rate))
                                .color(CyberColors::CYAN)
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("int/s:{:.0}", int_rate))
                                .color(CyberColors::NEON_GREEN)
                                .small(),
                        );
                        if vm.processes_blocked > 0 {
                            ui.label(
                                RichText::new(format!("blocked:{}", vm.processes_blocked))
                                    .color(CyberColors::NEON_RED)
                                    .small(),
                            );
                        }
                        ui.label(
                            RichText::new(format!("pgpgin:{}", vm.pages_in))
                                .color(CyberColors::NEON_PURPLE)
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("pgpgout:{}", vm.pages_out))
                                .color(CyberColors::NEON_ORANGE)
                                .small(),
                        );
                        if vm.swap_in > 0 || vm.swap_out > 0 {
                            ui.label(
                                RichText::new(format!("swin:{} swout:{}", vm.swap_in, vm.swap_out))
                                    .color(CyberColors::NEON_YELLOW)
                                    .small(),
                            );
                        }
                    });
                }
            }

            ui.add_space(8.0);

            // Context Switches and Interrupts charts (vmstat-style)
            ui.columns(2, |columns| {
                columns[0].add(
                    SparklineChart::new(self.context_switches_history.iter().cloned().collect())
                        .color(CyberColors::CYAN)
                        .height(70.0)
                        .title("Context Switches")
                        .unit("/s")
                        .show_scale(true),
                );

                columns[1].add(
                    SparklineChart::new(self.interrupts_history.iter().cloned().collect())
                        .color(CyberColors::NEON_GREEN)
                        .height(70.0)
                        .title("Interrupts")
                        .unit("/s")
                        .show_scale(true),
                );
            });

            ui.add_space(16.0);

            // Network Charts
            ui.add(SectionHeader::new("Network I/O").icon("🌐"));
            ui.columns(2, |columns| {
                columns[0].add(
                    SparklineChart::new(self.network_rx_history.iter().cloned().collect())
                        .color(CyberColors::NEON_GREEN)
                        .height(70.0)
                        .title("Download")
                        .unit("KB/s")
                        .show_scale(true),
                );

                columns[1].add(
                    SparklineChart::new(self.network_tx_history.iter().cloned().collect())
                        .color(CyberColors::NEON_ORANGE)
                        .height(70.0)
                        .title("Upload")
                        .unit("KB/s")
                        .show_scale(true),
                );
            });
        });
    }

    fn draw_cpu_tab(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add(SectionHeader::new("CPU Overview").icon("🔲"));

            if let Some(ref cpu) = self.cpu_stats {
                // Overall utilization with Glances-style threshold colors
                let cpu_usage = 100.0 - cpu.total.idle;

                // Get trend from history
                let cpu_trend = self
                    .cpu_history
                    .iter()
                    .rev()
                    .nth(1)
                    .map(|&prev| trend_indicator(cpu_usage, prev).0)
                    .unwrap_or("→");

                ui.add(
                    CyberProgressBar::new(cpu_usage / 100.0)
                        .with_threshold_color()
                        .with_trend(cpu_trend)
                        .label("Total CPU")
                        .height(28.0),
                );

                ui.add_space(8.0);

                // CPU History
                ui.add(
                    SparklineChart::new(self.cpu_history.iter().cloned().collect())
                        .color(CyberColors::CYAN)
                        .height(120.0)
                        .title("CPU History")
                        .unit("%")
                        .max_value(100.0)
                        .show_scale(true)
                        .show_min_max(true),
                );

                // Per-core sparklines (if available)
                if !self.per_core_history.is_empty() {
                    ui.add_space(16.0);
                    ui.add(SectionHeader::new("Per-Core History").icon("⚡"));

                    let num_cols =
                        (self.per_core_history.len().min(8) as f32).sqrt().ceil() as usize;
                    let num_cols = num_cols.max(2).min(4);

                    ui.columns(num_cols, |columns| {
                        for (i, hist) in self.per_core_history.iter().enumerate() {
                            let col = i % num_cols;
                            if col < columns.len() {
                                columns[col].add(
                                    SparklineChart::new(hist.iter().cloned().collect())
                                        .color(theme::neon_color_by_index(i))
                                        .height(60.0)
                                        .title(&format!("Core {}", i))
                                        .unit("%")
                                        .max_value(100.0)
                                        .show_scale(true),
                                );
                            }
                        }
                    });
                }

                // CPU Info
                ui.add_space(16.0);
                ui.add(SectionHeader::new("CPU Information").icon("ℹ️"));

                let cores = &cpu.cores;
                egui::Grid::new("cpu_info_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Cores:").color(CyberColors::TEXT_SECONDARY));
                        ui.label(
                            RichText::new(format!("{}", cores.len())).color(CyberColors::CYAN),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Online:").color(CyberColors::TEXT_SECONDARY));
                        ui.label(
                            RichText::new(format!("{}", cpu.online_count()))
                                .color(CyberColors::CYAN),
                        );
                        ui.end_row();

                        if let Some(core) = cores.first() {
                            if let Some(ref freq) = core.frequency {
                                ui.label(
                                    RichText::new("Frequency:").color(CyberColors::TEXT_SECONDARY),
                                );
                                ui.label(
                                    RichText::new(format!("{} MHz", freq.current))
                                        .color(CyberColors::CYAN),
                                );
                                ui.end_row();
                            }

                            if !core.model.is_empty() {
                                ui.label(
                                    RichText::new("Model:").color(CyberColors::TEXT_SECONDARY),
                                );
                                ui.label(RichText::new(&core.model).color(CyberColors::CYAN));
                                ui.end_row();
                            }
                        }
                    });
            } else {
                ui.label(RichText::new("Unable to read CPU statistics").color(CyberColors::ERROR));
            }
        });
    }

    fn draw_accelerators_tab(&mut self, ui: &mut egui::Ui) {
        if self.gpu_static_info.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label(RichText::new("⚡").size(48.0));
                ui.label(
                    RichText::new("No Accelerators Detected")
                        .color(CyberColors::TEXT_SECONDARY)
                        .size(24.0),
                );
                ui.label(
                    RichText::new("No GPUs, NPUs, FPGAs, or other accelerators found.\nInstall drivers or check hardware connection.")
                        .color(CyberColors::TEXT_MUTED),
                );
            });
            return;
        }

        // Auto-scale layout based on device count
        let device_count = self.gpu_static_info.len();
        let available_width = ui.available_width();
        
        // Scale elements based on device count
        let chart_height = if device_count == 1 { 100.0 } else if device_count == 2 { 80.0 } else { 65.0 };
        let bar_height = if device_count <= 2 { 18.0 } else { 14.0 };
        let font_scale = if device_count <= 2 { 1.0 } else if device_count <= 4 { 0.9 } else { 0.8 };
        
        ScrollArea::vertical().show(ui, |ui| {
            for (i, (static_info, dynamic_info)) in self
                .gpu_static_info
                .iter()
                .zip(self.gpu_dynamic_info.iter())
                .enumerate()
            {
                let accel_color = theme::neon_color_by_index(i);
                
                // Device pane frame - compact
                let frame = egui::Frame::none()
                    .fill(CyberColors::SURFACE)
                    .stroke(egui::Stroke::new(1.0, CyberColors::BORDER))
                    .rounding(4.0)
                    .inner_margin(8.0);
                
                frame.show(ui, |ui| {
                    ui.set_width(available_width - 16.0);
                    
                    // Header row: Icon + Name + Live Metrics
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚡").color(accel_color).size(20.0));
                        ui.label(
                            RichText::new(&static_info.name)
                                .color(CyberColors::TEXT_PRIMARY)
                                .strong()
                                .size(18.0),
                        );
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Live metrics on the right
                            if let Some(clock) = dynamic_info.clocks.graphics {
                                ui.label(
                                    RichText::new(format!("{} MHz", clock))
                                        .color(CyberColors::NEON_BLUE)
                                        .size(15.0),
                                );
                            }
                            if let Some(power) = dynamic_info.power.draw {
                                ui.label(
                                    RichText::new(format!("{:.0}W", power as f64 / 1000.0))
                                        .color(CyberColors::NEON_ORANGE)
                                        .size(15.0),
                                );
                            }
                            if let Some(temp) = dynamic_info.thermal.temperature {
                                ui.label(
                                    RichText::new(format!("{}°C", temp))
                                        .color(theme::temperature_color(temp as u32))
                                        .size(15.0),
                                );
                            }
                        });
                    });

                    ui.add_space(4.0);

                    // Main content: Progress bars + Charts side by side
                    ui.horizontal(|ui| {
                        // Left side: Progress bars
                        ui.vertical(|ui| {
                            ui.set_width(220.0);
                            
                            // Utilization bar
                            ui.label(RichText::new(format!("Utilization {}%", dynamic_info.utilization)).color(CyberColors::TEXT_SECONDARY).size(13.0));
                            ui.add(
                                CyberProgressBar::new(dynamic_info.utilization as f32 / 100.0)
                                    .color(accel_color)
                                    .height(bar_height),
                            );
                            
                            ui.add_space(4.0);
                            
                            // VRAM bar
                            let mem_used_mb = dynamic_info.memory.used / 1024 / 1024;
                            let mem_total_mb = dynamic_info.memory.total / 1024 / 1024;
                            ui.label(RichText::new(format!("VRAM {}/{}MB", mem_used_mb, mem_total_mb)).color(CyberColors::TEXT_SECONDARY).size(13.0));
                            ui.add(
                                CyberProgressBar::new(dynamic_info.memory.utilization as f32 / 100.0)
                                    .color(CyberColors::MAGENTA)
                                    .height(bar_height),
                            );
                            
                            // Vendor/Driver info at bottom
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("{:?}", static_info.vendor))
                                    .color(accel_color)
                                    .size(12.0),
                            );
                            if let Some(ref driver) = static_info.driver_version {
                                ui.label(
                                    RichText::new(format!("Driver: {}", driver))
                                        .color(CyberColors::TEXT_MUTED)
                                        .size(12.0),
                                );
                            }
                        });

                        ui.add_space(12.0);

                        // Right side: Charts (expand to fill)
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            
                            if i < self.gpu_history.len() {
                                ui.add(
                                    SparklineChart::new(self.gpu_history[i].iter().cloned().collect())
                                        .color(accel_color)
                                        .height(chart_height)
                                        .title("Utilization")
                                        .unit("%")
                                        .max_value(100.0)
                                        .show_scale(true),
                                );
                            }
                            
                            ui.add_space(2.0);
                            
                            if i < self.gpu_temp_history.len() {
                                ui.add(
                                    SparklineChart::new(self.gpu_temp_history[i].iter().cloned().collect())
                                        .color(CyberColors::NEON_YELLOW)
                                        .height(chart_height)
                                        .title("Temperature")
                                        .unit("°C")
                                        .max_value(100.0)
                                        .show_scale(true),
                                );
                            }
                        });
                    });
                });
                
                // Minimal gap between device panes
                if i < device_count - 1 {
                    ui.add_space(2.0);
                }
            }
        });
    }

    fn draw_memory_tab(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            if let Some(ref mem) = self.memory_stats {
                let usage = mem.ram_usage_percent();
                let total_mb = mem.ram.total as f64 / 1024.0;
                let used_mb = mem.ram.used as f64 / 1024.0;
                let free_mb = mem.ram.free as f64 / 1024.0;
                let buffers_mb = mem.ram.buffers as f64 / 1024.0;
                let cached_mb = mem.ram.cached as f64 / 1024.0;
                let shared_mb = mem.ram.shared as f64 / 1024.0;
                // Available = free + buffers + cached (like free -h)
                let available_mb = free_mb + buffers_mb + cached_mb;

                ui.add(SectionHeader::new("Physical Memory").icon("💾"));

                // Get trend from history
                let mem_trend = self
                    .memory_history
                    .iter()
                    .rev()
                    .nth(1)
                    .map(|&prev| trend_indicator(usage, prev).0)
                    .unwrap_or("→");

                ui.add(
                    CyberProgressBar::new(usage / 100.0)
                        .with_threshold_color()
                        .with_trend(mem_trend)
                        .label(format!("{:.1} MB / {:.1} MB", used_mb, total_mb))
                        .height(32.0),
                );

                ui.add_space(8.0);

                // Memory breakdown like `free -h` output
                ui.add(SectionHeader::new("Memory Breakdown (free -h style)").icon("📈"));

                // Main row: total, used, free, shared, buff/cache, available
                ui.horizontal(|ui| {
                    ui.add(
                        MetricCard::new("Total", format!("{:.0}", total_mb))
                            .unit("MB")
                            .color(CyberColors::CYAN),
                    );
                    ui.add(
                        MetricCard::new("Used", format!("{:.0}", used_mb))
                            .unit("MB")
                            .color(threshold_color(usage)),
                    );
                    ui.add(
                        MetricCard::new("Free", format!("{:.0}", free_mb))
                            .unit("MB")
                            .color(CyberColors::THRESHOLD_OK),
                    );
                    ui.add(
                        MetricCard::new("Shared", format!("{:.0}", shared_mb))
                            .unit("MB")
                            .color(CyberColors::NEON_PURPLE),
                    );
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.add(
                        MetricCard::new("Buffers", format!("{:.0}", buffers_mb))
                            .unit("MB")
                            .color(CyberColors::NEON_ORANGE),
                    );
                    ui.add(
                        MetricCard::new("Cached", format!("{:.0}", cached_mb))
                            .unit("MB")
                            .color(CyberColors::NEON_YELLOW),
                    );
                    ui.add(
                        MetricCard::new("Available", format!("{:.0}", available_mb))
                            .unit("MB")
                            .color(CyberColors::THRESHOLD_OK),
                    );
                    ui.add(
                        MetricCard::new("Usage", format!("{:.1}", usage))
                            .unit("%")
                            .color(threshold_color(usage)),
                    );
                });

                // Visual breakdown bar (stacked)
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Memory Map: ").color(CyberColors::TEXT_PRIMARY));
                    // Show proportional bar
                    if total_mb > 0.0 {
                        let used_pct = (used_mb - buffers_mb - cached_mb).max(0.0) / total_mb;
                        let buffers_pct = buffers_mb / total_mb;
                        let cached_pct = cached_mb / total_mb;
                        let free_pct = free_mb / total_mb;

                        let _bar_width = ui.available_width() - 100.0;

                        ui.label(
                            RichText::new(format!("█{:.0}%", used_pct * 100.0))
                                .color(CyberColors::MAGENTA)
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("█{:.0}%", buffers_pct * 100.0))
                                .color(CyberColors::NEON_ORANGE)
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("█{:.0}%", cached_pct * 100.0))
                                .color(CyberColors::NEON_YELLOW)
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("█{:.0}%", free_pct * 100.0))
                                .color(CyberColors::THRESHOLD_OK)
                                .small(),
                        );

                        // Legend
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("(used/buffers/cache/free)")
                                    .color(CyberColors::TEXT_MUTED)
                                    .small(),
                            );
                        });
                    }
                });

                ui.add_space(16.0);

                // Memory history
                ui.add(
                    SparklineChart::new(self.memory_history.iter().cloned().collect())
                        .color(CyberColors::MAGENTA)
                        .height(150.0)
                        .title("Memory Usage History")
                        .unit("%")
                        .max_value(100.0)
                        .show_scale(true)
                        .show_min_max(true),
                );

                // Swap info
                ui.add_space(16.0);
                ui.add(SectionHeader::new("Swap Memory").icon("🔄"));

                let swap_usage = mem.swap_usage_percent();
                let swap_total_mb = mem.swap.total as f64 / 1024.0;
                let swap_used_mb = mem.swap.used as f64 / 1024.0;
                let swap_free_mb = swap_total_mb - swap_used_mb;
                let swap_cached_mb = mem.swap.cached as f64 / 1024.0;

                if swap_total_mb > 0.0 {
                    ui.add(
                        CyberProgressBar::new(swap_usage / 100.0)
                            .color(CyberColors::NEON_PURPLE)
                            .label(format!(
                                "Swap: {:.1} MB / {:.1} MB",
                                swap_used_mb, swap_total_mb
                            ))
                            .height(24.0),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add(
                            MetricCard::new("Swap Total", format!("{:.0}", swap_total_mb))
                                .unit("MB")
                                .color(CyberColors::CYAN),
                        );
                        ui.add(
                            MetricCard::new("Swap Used", format!("{:.0}", swap_used_mb))
                                .unit("MB")
                                .color(CyberColors::MAGENTA),
                        );
                        ui.add(
                            MetricCard::new("Swap Free", format!("{:.0}", swap_free_mb))
                                .unit("MB")
                                .color(CyberColors::NEON_GREEN),
                        );
                        if swap_cached_mb > 0.0 {
                            ui.add(
                                MetricCard::new("Swap Cached", format!("{:.0}", swap_cached_mb))
                                    .unit("MB")
                                    .color(CyberColors::NEON_YELLOW),
                            );
                        }
                    });
                } else {
                    ui.label(RichText::new("No swap configured").color(CyberColors::TEXT_MUTED));
                }
            } else {
                ui.label(
                    RichText::new("Unable to read memory statistics").color(CyberColors::ERROR),
                );
            }
        });
    }

    fn draw_processes_tab(&mut self, ui: &mut egui::Ui) {
        ui.add(SectionHeader::new("Running Processes (htop-style)").icon("📋"));

        // Task summary (htop-style: "Tasks: 150, 43 thr; 1 running")
        let running = self.process_list.iter().filter(|p| p.state == 'R').count();
        let sleeping = self
            .process_list
            .iter()
            .filter(|p| p.state == 'S' || p.state == 'I')
            .count();
        let disk_wait = self.process_list.iter().filter(|p| p.state == 'D').count();
        let zombie = self.process_list.iter().filter(|p| p.state == 'Z').count();
        let stopped = self.process_list.iter().filter(|p| p.state == 'T').count();
        let gpu_procs = self
            .process_list
            .iter()
            .filter(|p| p.total_gpu_memory_bytes > 0)
            .count();

        ui.horizontal(|ui| {
            ui.label(RichText::new("Tasks:").color(CyberColors::TEXT_PRIMARY));
            ui.label(
                RichText::new(format!("{}", self.process_list.len())).color(CyberColors::CYAN),
            );
            ui.separator();
            ui.label(RichText::new(format!("{} running", running)).color(CyberColors::NEON_GREEN));
            ui.label(
                RichText::new(format!("{} sleeping", sleeping)).color(CyberColors::TEXT_MUTED),
            );
            if disk_wait > 0 {
                ui.label(
                    RichText::new(format!("{} D-wait", disk_wait)).color(CyberColors::NEON_ORANGE),
                );
            }
            if zombie > 0 {
                ui.label(RichText::new(format!("{} zombie", zombie)).color(CyberColors::NEON_RED));
            }
            if stopped > 0 {
                ui.label(
                    RichText::new(format!("{} stopped", stopped)).color(CyberColors::NEON_PURPLE),
                );
            }
            ui.separator();
            if gpu_procs > 0 {
                ui.label(
                    RichText::new(format!("🎮 {} GPU", gpu_procs)).color(CyberColors::NEON_ORANGE),
                );
            }
        });

        ui.add_space(4.0);

        // Filter bar
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").color(CyberColors::TEXT_SECONDARY));
            ui.add(
                egui::TextEdit::singleline(&mut self.process_filter)
                    .hint_text("Filter processes...")
                    .desired_width(200.0),
            );

            ui.separator();

            // Sort options
            ui.label(RichText::new("Sort by:").color(CyberColors::TEXT_SECONDARY));
            if ui
                .selectable_label(self.process_sort_column == ProcessSortColumn::Name, "Name")
                .clicked()
            {
                self.process_sort_column = ProcessSortColumn::Name;
            }
            if ui
                .selectable_label(self.process_sort_column == ProcessSortColumn::Cpu, "CPU")
                .clicked()
            {
                self.process_sort_column = ProcessSortColumn::Cpu;
            }
            if ui
                .selectable_label(
                    self.process_sort_column == ProcessSortColumn::Memory,
                    "Memory",
                )
                .clicked()
            {
                self.process_sort_column = ProcessSortColumn::Memory;
            }
            if ui
                .selectable_label(self.process_sort_column == ProcessSortColumn::Pid, "PID")
                .clicked()
            {
                self.process_sort_column = ProcessSortColumn::Pid;
            }

            if ui
                .button(if self.process_sort_ascending {
                    "↑"
                } else {
                    "↓"
                })
                .clicked()
            {
                self.process_sort_ascending = !self.process_sort_ascending;
            }

            ui.label(
                RichText::new(format!("Total: {}", self.process_list.len()))
                    .color(CyberColors::TEXT_MUTED),
            );
        });

        ui.add_space(8.0);

        // Process table
        ScrollArea::vertical().show(ui, |ui| {
            let mut processes = self.process_list.clone();

            // Filter
            if !self.process_filter.is_empty() {
                let filter = self.process_filter.to_lowercase();
                processes.retain(|p| p.name.to_lowercase().contains(&filter));
            }

            // Sort
            match self.process_sort_column {
                ProcessSortColumn::Name => processes.sort_by(|a, b| {
                    if self.process_sort_ascending {
                        a.name.cmp(&b.name)
                    } else {
                        b.name.cmp(&a.name)
                    }
                }),
                ProcessSortColumn::Pid => processes.sort_by(|a, b| {
                    if self.process_sort_ascending {
                        a.pid.cmp(&b.pid)
                    } else {
                        b.pid.cmp(&a.pid)
                    }
                }),
                ProcessSortColumn::Cpu => processes.sort_by(|a, b| {
                    let cmp = a
                        .cpu_percent
                        .partial_cmp(&b.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal);
                    if self.process_sort_ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                }),
                ProcessSortColumn::Memory => processes.sort_by(|a, b| {
                    let cmp = a.memory_bytes.cmp(&b.memory_bytes);
                    if self.process_sort_ascending {
                        cmp
                    } else {
                        cmp.reverse()
                    }
                }),
            }

            // Table header (htop-style)
            ui.horizontal(|ui| {
                ui.set_min_height(24.0);
                ui.label(RichText::new("PID").color(CyberColors::CYAN).strong());
                ui.add_space(40.0);
                ui.label(RichText::new("ST").color(CyberColors::CYAN).strong()); // State
                ui.add_space(8.0);
                ui.label(RichText::new("PRI").color(CyberColors::CYAN).strong()); // Priority
                ui.add_space(8.0);
                ui.label(RichText::new("Name").color(CyberColors::CYAN).strong());
                ui.add_space(160.0);
                ui.label(RichText::new("CPU %").color(CyberColors::CYAN).strong());
                ui.add_space(30.0);
                ui.label(RichText::new("Memory").color(CyberColors::CYAN).strong());
                ui.add_space(40.0);
                ui.label(RichText::new("GPU Mem").color(CyberColors::CYAN).strong());
            });
            ui.separator();

            // Process rows (htop-style with state and priority)
            for process in processes.iter().take(100) {
                let cpu_color = theme::utilization_color(process.cpu_percent);
                let mem_mb = process.memory_bytes as f64 / 1024.0 / 1024.0;
                let gpu_mem_mb = process.total_gpu_memory_bytes as f64 / 1024.0 / 1024.0;

                // State color coding like htop
                let state_color = match process.state {
                    'R' => CyberColors::NEON_GREEN,        // Running
                    'S' | 'I' => CyberColors::TEXT_MUTED,  // Sleeping/Idle
                    'D' => CyberColors::NEON_ORANGE,       // Disk wait (uninterruptible)
                    'Z' => CyberColors::NEON_RED,          // Zombie
                    'T' | 't' => CyberColors::NEON_PURPLE, // Stopped/Traced
                    _ => CyberColors::TEXT_SECONDARY,
                };

                ui.horizontal(|ui| {
                    ui.set_min_height(20.0);
                    ui.label(
                        RichText::new(format!("{:>6}", process.pid))
                            .color(CyberColors::TEXT_MUTED)
                            .monospace(),
                    );
                    ui.add_space(20.0);
                    // State column
                    ui.label(
                        RichText::new(format!("{}", process.state))
                            .color(state_color)
                            .monospace(),
                    );
                    ui.add_space(8.0);
                    // Priority/nice column
                    let pri_str = process
                        .priority
                        .map(|p| format!("{:>3}", p))
                        .unwrap_or_else(|| "  -".to_string());
                    ui.label(
                        RichText::new(pri_str)
                            .color(CyberColors::TEXT_MUTED)
                            .monospace(),
                    );
                    ui.add_space(8.0);
                    // Name
                    ui.add_sized(
                        Vec2::new(200.0, 20.0),
                        egui::Label::new(
                            RichText::new(&process.name).color(CyberColors::TEXT_PRIMARY),
                        ),
                    );
                    // CPU
                    ui.label(
                        RichText::new(format!("{:>5.1}%", process.cpu_percent))
                            .color(cpu_color)
                            .monospace(),
                    );
                    ui.add_space(10.0);
                    // Memory
                    ui.label(
                        RichText::new(format!("{:>8.1} MB", mem_mb))
                            .color(CyberColors::MAGENTA)
                            .monospace(),
                    );
                    ui.add_space(20.0);
                    // GPU Memory (if using GPU)
                    if gpu_mem_mb > 0.1 {
                        ui.label(
                            RichText::new(format!("{:>6.0} MB", gpu_mem_mb))
                                .color(CyberColors::NEON_ORANGE)
                                .monospace(),
                        );
                    } else {
                        ui.label(
                            RichText::new("     -   ")
                                .color(CyberColors::TEXT_MUTED)
                                .monospace(),
                        );
                    }
                });
            }
        });
    }

    fn draw_network_tab(&mut self, ui: &mut egui::Ui) {
        // Clone the rates to avoid borrow conflict
        let rates = self.network_rates.clone();

        ScrollArea::vertical().show(ui, |ui| {
            ui.add(SectionHeader::new("Network Interfaces").icon("🌐"));

            // Show total bandwidth rates at the top
            let total_rx_rate: f64 = rates.values().map(|(rx, _)| rx).sum();
            let total_tx_rate: f64 = rates.values().map(|(_, tx)| tx).sum();

            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("Total Bandwidth:").color(CyberColors::TEXT_MUTED));
                ui.label(
                    RichText::new(format!("↓ {}/s", format_bytes(total_rx_rate)))
                        .color(CyberColors::NEON_GREEN)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("↑ {}/s", format_bytes(total_tx_rate)))
                        .color(CyberColors::NEON_ORANGE)
                        .strong(),
                );
            });
            ui.add_space(8.0);

            // Network charts - stacked vertically, left-aligned
            ui.add(
                SparklineChart::new(self.network_rx_history.iter().cloned().collect())
                    .color(CyberColors::NEON_GREEN)
                    .height(80.0)
                    .title("Download (Total MB)")
                    .unit("MB")
                    .show_scale(true)
                    .show_min_max(true),
            );

            ui.add_space(4.0);

            ui.add(
                SparklineChart::new(self.network_tx_history.iter().cloned().collect())
                    .color(CyberColors::NEON_ORANGE)
                    .height(80.0)
                    .title("Upload (Total MB)")
                    .unit("MB")
                    .show_scale(true)
                    .show_min_max(true),
            );

            ui.add_space(16.0);

            // Interface details
            if let Some(ref mut monitor) = self.network_monitor {
                if let Ok(interfaces) = monitor.interfaces() {
                    for iface in interfaces {
                        let iface_color = if iface.name.contains("eth")
                            || iface.name.contains("en")
                            || iface.name.contains("Ethernet")
                        {
                            CyberColors::NEON_BLUE
                        } else if iface.name.contains("wl") || iface.name.contains("Wi") {
                            CyberColors::NEON_PURPLE
                        } else {
                            CyberColors::CYAN
                        };

                        // Get bandwidth rates for this interface
                        let (rx_rate, tx_rate) =
                            rates.get(&iface.name).copied().unwrap_or((0.0, 0.0));

                        ui.add(SectionHeader::new(&iface.name).icon("📡"));

                        // Bandwidth rate row
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(RichText::new("Rate:").color(CyberColors::TEXT_MUTED));
                            ui.label(
                                RichText::new(format!("↓ {}/s", format_bytes(rx_rate)))
                                    .color(CyberColors::NEON_GREEN)
                                    .monospace(),
                            );
                            ui.label(
                                RichText::new(format!("↑ {}/s", format_bytes(tx_rate)))
                                    .color(CyberColors::NEON_ORANGE)
                                    .monospace(),
                            );
                            if let Some(speed) = iface.speed_mbps {
                                ui.separator();
                                ui.label(RichText::new("Link:").color(CyberColors::TEXT_MUTED));
                                ui.label(
                                    RichText::new(format!("{} Mbps", speed))
                                        .color(iface_color)
                                        .monospace(),
                                );
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add(
                                MetricCard::new(
                                    "Received",
                                    format!("{:.1}", iface.rx_bytes as f64 / 1024.0 / 1024.0),
                                )
                                .unit("MB")
                                .color(CyberColors::NEON_GREEN),
                            );

                            ui.add(
                                MetricCard::new(
                                    "Sent",
                                    format!("{:.1}", iface.tx_bytes as f64 / 1024.0 / 1024.0),
                                )
                                .unit("MB")
                                .color(CyberColors::NEON_ORANGE),
                            );

                            ui.add(
                                MetricCard::new("Packets In", iface.rx_packets)
                                    .color(iface_color),
                            );

                            ui.add(
                                MetricCard::new("Packets Out", iface.tx_packets)
                                    .color(iface_color),
                            );
                        });

                        // Status and IP addresses
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Status:").color(CyberColors::TEXT_SECONDARY));
                            ui.label(
                                RichText::new(if iface.is_up { "UP" } else { "DOWN" }).color(
                                    if iface.is_up {
                                        CyberColors::NEON_GREEN
                                    } else {
                                        CyberColors::ERROR
                                    },
                                ),
                            );

                            if !iface.ipv4_addresses.is_empty() {
                                ui.separator();
                                ui.label(RichText::new("IPv4:").color(CyberColors::TEXT_SECONDARY));
                                for ip in &iface.ipv4_addresses {
                                    ui.label(RichText::new(ip).color(iface_color).monospace());
                                }
                            }
                        });

                        ui.add_space(8.0);
                    }
                }
            } else {
                ui.label(
                    RichText::new("Unable to read network information").color(CyberColors::ERROR),
                );
            }
        });
    }

    fn draw_disk_tab(&mut self, ui: &mut egui::Ui) {
        // Trigger lazy loading of disk data
        self.start_disk_loading();

        // Show loading indicator
        if self.disk_loading {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.spinner();
                ui.label(
                    RichText::new("Loading disk information...")
                        .color(CyberColors::TEXT_SECONDARY)
                        .size(18.0),
                );
            });
            return;
        }

        if self.disks.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(RichText::new("💿").size(64.0));
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No Disks Detected")
                        .color(CyberColors::TEXT_SECONDARY)
                        .size(24.0),
                );
                ui.label(
                    RichText::new("Unable to enumerate storage devices")
                        .color(CyberColors::TEXT_MUTED),
                );
            });
            return;
        }

        ui.add(SectionHeader::new("Storage Devices").icon("💿"));
        ui.add_space(8.0);

        // Column widths - must match draw_disk_row exactly
        const COL_MODEL: f32 = 420.0;
        const COL_INTERFACE: f32 = 110.0;
        const COL_CAPACITY: f32 = 90.0;
        const COL_READ: f32 = 110.0;
        const COL_WRITE: f32 = 110.0;
        const COL_SPACING: f32 = 30.0;
        const HEADER_HEIGHT: f32 = 20.0;

        // Header row - use exact same allocation method as data rows for perfect alignment
        // Must account for: 1px stroke + 12px inner_margin from each card's Frame
        const CARD_LEFT_OFFSET: f32 = 13.0;
        
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = COL_SPACING;
            ui.add_space(CARD_LEFT_OFFSET);
            
            // Column 1: Device (left-aligned to match data)
            let (model_rect, _) = ui.allocate_exact_size(egui::vec2(COL_MODEL, HEADER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(model_rect) {
                ui.painter().text(
                    model_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    "Device",
                    egui::FontId::proportional(13.0),
                    CyberColors::TEXT_MUTED,
                );
            }
            
            // Column 2: Interface (centered to match data)
            let (iface_rect, _) = ui.allocate_exact_size(egui::vec2(COL_INTERFACE, HEADER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(iface_rect) {
                ui.painter().text(
                    iface_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Interface",
                    egui::FontId::proportional(13.0),
                    CyberColors::TEXT_MUTED,
                );
            }
            
            // Column 3: Capacity (centered to match data)
            let (cap_rect, _) = ui.allocate_exact_size(egui::vec2(COL_CAPACITY, HEADER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(cap_rect) {
                ui.painter().text(
                    cap_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Capacity",
                    egui::FontId::proportional(13.0),
                    CyberColors::TEXT_MUTED,
                );
            }
            
            // Column 4: Read (centered to match data)
            let (read_rect, _) = ui.allocate_exact_size(egui::vec2(COL_READ, HEADER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(read_rect) {
                ui.painter().text(
                    read_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📥 Read",
                    egui::FontId::proportional(13.0),
                    CyberColors::TEXT_MUTED,
                );
            }
            
            // Column 5: Write (centered to match data)
            let (write_rect, _) = ui.allocate_exact_size(egui::vec2(COL_WRITE, HEADER_HEIGHT), egui::Sense::hover());
            if ui.is_rect_visible(write_rect) {
                ui.painter().text(
                    write_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📤 Write",
                    egui::FontId::proportional(13.0),
                    CyberColors::TEXT_MUTED,
                );
            }
            
            // Health column (right side)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(CARD_LEFT_OFFSET);
                ui.label(RichText::new("Health").color(CyberColors::TEXT_MUTED).size(13.0));
            });
        });
        ui.add_space(4.0);

        // Disk list with scroll
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, disk) in self.disks.iter().enumerate() {
                    // Get cached data for this disk (or empty defaults if not yet cached)
                    let cached = self.cached_disk_data.get(i).cloned().unwrap_or_default();
                    Self::draw_disk_row_cached(ui, disk, i, &cached);
                    ui.add_space(6.0);
                }
            });
    }

    fn draw_disk_row_cached(ui: &mut egui::Ui, disk: &Box<dyn DiskDevice>, index: usize, cached: &CachedDiskData) {
        let disk_color = theme::neon_color_by_index(index);
        let disk_name = disk.name().to_string();
        let disk_type = disk.disk_type();

        let type_icon = match disk_type {
            crate::disk::DiskType::NvmeSsd => "⚡",
            crate::disk::DiskType::SataSsd => "💾",
            crate::disk::DiskType::SataHdd => "🔘",
            crate::disk::DiskType::Usb => "🔌",
            crate::disk::DiskType::Scsi => "📀",
            crate::disk::DiskType::Virtual => "☁",
            crate::disk::DiskType::Unknown => "?",
        };

        let format_bytes = |bytes: u64| -> String {
            let b = bytes as f64;
            if b >= 1e12 { format!("{:.2} TB", b / 1e12) }
            else if b >= 1e9 { format!("{:.1} GB", b / 1e9) }
            else if b >= 1e6 { format!("{:.0} MB", b / 1e6) }
            else if b >= 1e3 { format!("{:.0} KB", b / 1e3) }
            else { format!("{} B", bytes) }
        };

        // Use cached data instead of making I/O calls
        let info = &cached.info;
        let io_stats = &cached.io_stats;
        let health = &cached.health;
        
        let model_name = info.as_ref().map(|i| {
            if i.model.len() > 40 { format!("{}…", &i.model[..38]) } else { i.model.clone() }
        }).unwrap_or_else(|| disk_name.clone());
        let interface = info.as_ref().and_then(|i| i.interface_type.clone()).unwrap_or_else(|| "Unknown".to_string());
        let capacity = info.as_ref().map(|i| format_bytes(i.capacity)).unwrap_or_else(|| "N/A".to_string());
        let read_bytes = io_stats.as_ref().map(|io| format_bytes(io.read_bytes)).unwrap_or_else(|| "N/A".to_string());
        let write_bytes = io_stats.as_ref().map(|io| format_bytes(io.write_bytes)).unwrap_or_else(|| "N/A".to_string());
        
        egui::Frame::none()
            .fill(CyberColors::SURFACE)
            .stroke(egui::Stroke::new(1.0, disk_color.gamma_multiply(0.4)))
            .rounding(6.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                // Use fixed column positions via exact sizing
                const COL_MODEL: f32 = 420.0;
                const COL_INTERFACE: f32 = 110.0;
                const COL_CAPACITY: f32 = 90.0;
                const COL_READ: f32 = 110.0;
                const COL_WRITE: f32 = 110.0;
                const ROW_HEIGHT: f32 = 45.0;
                
                // Row 1: Use exact size allocation to guarantee column widths
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 30.0;
                    
                    // Column 1: Icon + Model - use exact size
                    let (model_rect, _) = ui.allocate_exact_size(egui::vec2(COL_MODEL, ROW_HEIGHT), egui::Sense::hover());
                    if ui.is_rect_visible(model_rect) {
                        // Draw icon
                        let icon_pos = model_rect.left_center() + egui::vec2(14.0, 0.0);
                        ui.painter().text(icon_pos, egui::Align2::LEFT_CENTER, type_icon, egui::FontId::proportional(28.0), CyberColors::TEXT_PRIMARY);
                        
                        // Draw model name
                        let name_pos = model_rect.left_center() + egui::vec2(50.0, -8.0);
                        ui.painter().text(name_pos, egui::Align2::LEFT_CENTER, &model_name, egui::FontId::proportional(18.0), disk_color);
                        
                        // Draw disk name
                        let disk_name_pos = model_rect.left_center() + egui::vec2(50.0, 10.0);
                        ui.painter().text(disk_name_pos, egui::Align2::LEFT_CENTER, &disk_name, egui::FontId::monospace(13.0), CyberColors::TEXT_MUTED);
                    }
                    
                    // Column 2: Interface - exact size, centered
                    let (iface_rect, _) = ui.allocate_exact_size(egui::vec2(COL_INTERFACE, ROW_HEIGHT), egui::Sense::hover());
                    if ui.is_rect_visible(iface_rect) {
                        ui.painter().text(iface_rect.center(), egui::Align2::CENTER_CENTER, &interface, egui::FontId::proportional(16.0), CyberColors::NEON_PURPLE);
                    }
                    
                    // Column 3: Capacity - exact size, centered
                    let (cap_rect, _) = ui.allocate_exact_size(egui::vec2(COL_CAPACITY, ROW_HEIGHT), egui::Sense::hover());
                    if ui.is_rect_visible(cap_rect) {
                        ui.painter().text(cap_rect.center(), egui::Align2::CENTER_CENTER, &capacity, egui::FontId::proportional(18.0), CyberColors::CYAN);
                    }
                    
                    // Column 4: Read - exact size, centered
                    let (read_rect, _) = ui.allocate_exact_size(egui::vec2(COL_READ, ROW_HEIGHT), egui::Sense::hover());
                    if ui.is_rect_visible(read_rect) {
                        ui.painter().text(read_rect.center(), egui::Align2::CENTER_CENTER, &read_bytes, egui::FontId::proportional(16.0), CyberColors::NEON_GREEN);
                    }
                    
                    // Column 5: Write - exact size, centered
                    let (write_rect, _) = ui.allocate_exact_size(egui::vec2(COL_WRITE, ROW_HEIGHT), egui::Sense::hover());
                    if ui.is_rect_visible(write_rect) {
                        ui.painter().text(write_rect.center(), egui::Align2::CENTER_CENTER, &write_bytes, egui::FontId::proportional(16.0), CyberColors::NEON_ORANGE);
                    }
                    
                    // Column 6: Health - right aligned, use remaining space
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(h) = &health {
                            let (text, color) = match h {
                                crate::disk::DiskHealth::Healthy => ("✓ Healthy", CyberColors::NEON_GREEN),
                                crate::disk::DiskHealth::Warning => ("⚠ Warning", CyberColors::NEON_ORANGE),
                                crate::disk::DiskHealth::Critical | crate::disk::DiskHealth::Failed => ("✗ Critical", CyberColors::NEON_RED),
                                crate::disk::DiskHealth::Unknown => ("Unknown", CyberColors::TEXT_MUTED),
                            };
                            egui::Frame::none()
                                .fill(color.gamma_multiply(0.15))
                                .rounding(4.0)
                                .inner_margin(egui::vec2(10.0, 4.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(text).color(color).size(14.0));
                                });
                        }
                    });
                });
                
                // Row 2: Partitions with aligned columns (use cached filesystem data)
                let filesystems = &cached.filesystems;
                if !filesystems.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    
                    for fs in filesystems.iter().take(3) {
                        let pct = fs.usage_percent();
                        let color = if pct > 90.0 { CyberColors::NEON_RED } 
                            else if pct > 75.0 { CyberColors::NEON_ORANGE } 
                            else { CyberColors::NEON_GREEN };
                        
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 20.0; // Even spacing
                            
                            // Mount point (fixed width 50px)
                            ui.allocate_ui(egui::vec2(50.0, 22.0), |ui| {
                                let mount = fs.mount_point.to_string_lossy();
                                ui.label(RichText::new(mount.as_ref()).color(CyberColors::TEXT_PRIMARY).size(15.0).monospace());
                            });
                            
                            // Filesystem type (fixed width 60px)
                            ui.allocate_ui(egui::vec2(60.0, 22.0), |ui| {
                                ui.label(RichText::new(&fs.fs_type).color(CyberColors::TEXT_MUTED).size(13.0));
                            });
                            
                            // Progress bar (fixed width 180px)
                            let bar_w = 180.0;
                            let bar_h = 18.0;
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_w, bar_h), egui::Sense::hover());
                            if ui.is_rect_visible(rect) {
                                ui.painter().rect_filled(rect, 3.0, CyberColors::BACKGROUND_DARK);
                                let w = rect.width() * pct / 100.0;
                                ui.painter().rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(w, rect.height())), 3.0, color.gamma_multiply(0.8));
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, 
                                    format!("{:.0}%", pct), egui::FontId::proportional(12.0), CyberColors::TEXT_PRIMARY);
                            }
                            
                            // Used / Total (fixed width)
                            ui.allocate_ui(egui::vec2(180.0, 22.0), |ui| {
                                ui.label(RichText::new(format!("{} / {}", format_bytes(fs.used_size), format_bytes(fs.total_size)))
                                    .color(CyberColors::TEXT_SECONDARY).size(14.0));
                            });
                        });
                        ui.add_space(2.0); // Space between partition rows
                    }
                    if filesystems.len() > 3 {
                        ui.label(RichText::new(format!("+{} more partitions", filesystems.len() - 3)).color(CyberColors::TEXT_MUTED).size(12.0));
                    }
                }
            });
    }

    fn draw_connections_tab(&mut self, ui: &mut egui::Ui) {
        ui.add(SectionHeader::new("Network Connections (netstat)").icon("📡"));

        // Filter bar
        ui.horizontal(|ui| {
            ui.label(RichText::new("🔍").color(CyberColors::TEXT_SECONDARY));
            ui.add(
                egui::TextEdit::singleline(&mut self.connection_filter)
                    .hint_text("Filter by address or process...")
                    .desired_width(200.0),
            );

            ui.separator();

            // Protocol filter
            ui.label(RichText::new("Protocol:").color(CyberColors::TEXT_SECONDARY));
            if ui
                .selectable_label(self.connection_protocol_filter.is_none(), "All")
                .clicked()
            {
                self.connection_protocol_filter = None;
            }
            if ui
                .selectable_label(
                    self.connection_protocol_filter == Some(Protocol::Tcp),
                    "TCP",
                )
                .clicked()
            {
                self.connection_protocol_filter = Some(Protocol::Tcp);
            }
            if ui
                .selectable_label(
                    self.connection_protocol_filter == Some(Protocol::Udp),
                    "UDP",
                )
                .clicked()
            {
                self.connection_protocol_filter = Some(Protocol::Udp);
            }

            ui.separator();

            // State filter
            ui.label(RichText::new("State:").color(CyberColors::TEXT_SECONDARY));
            if ui
                .selectable_label(self.connection_state_filter.is_none(), "All")
                .clicked()
            {
                self.connection_state_filter = None;
            }
            if ui
                .selectable_label(
                    self.connection_state_filter == Some(ConnectionState::Established),
                    "ESTABLISHED",
                )
                .clicked()
            {
                self.connection_state_filter = Some(ConnectionState::Established);
            }
            if ui
                .selectable_label(
                    self.connection_state_filter == Some(ConnectionState::Listen),
                    "LISTEN",
                )
                .clicked()
            {
                self.connection_state_filter = Some(ConnectionState::Listen);
            }

            ui.label(
                RichText::new(format!("Total: {}", self.connections.len()))
                    .color(CyberColors::TEXT_MUTED),
            );
        });

        ui.add_space(8.0);

        // Connection table
        ScrollArea::vertical().show(ui, |ui| {
            let mut connections = self.connections.clone();

            // Apply protocol filter
            if let Some(proto) = self.connection_protocol_filter {
                connections.retain(|c| {
                    c.protocol == proto
                        || (proto == Protocol::Tcp && c.protocol == Protocol::Tcp6)
                        || (proto == Protocol::Udp && c.protocol == Protocol::Udp6)
                });
            }

            // Apply state filter
            if let Some(state) = self.connection_state_filter {
                connections.retain(|c| c.state == state);
            }

            // Apply text filter
            if !self.connection_filter.is_empty() {
                let filter = self.connection_filter.to_lowercase();
                connections.retain(|c| {
                    c.local_address.to_lowercase().contains(&filter)
                        || c.remote_address
                            .as_ref()
                            .map(|r| r.to_lowercase().contains(&filter))
                            .unwrap_or(false)
                        || c.process_name
                            .as_ref()
                            .map(|p| p.to_lowercase().contains(&filter))
                            .unwrap_or(false)
                });
            }

            // Table header
            ui.horizontal(|ui| {
                ui.set_min_height(24.0);
                ui.add_sized(
                    Vec2::new(60.0, 20.0),
                    egui::Label::new(RichText::new("Proto").color(CyberColors::CYAN).strong()),
                );
                ui.add_sized(
                    Vec2::new(200.0, 20.0),
                    egui::Label::new(
                        RichText::new("Local Address")
                            .color(CyberColors::CYAN)
                            .strong(),
                    ),
                );
                ui.add_sized(
                    Vec2::new(200.0, 20.0),
                    egui::Label::new(
                        RichText::new("Remote Address")
                            .color(CyberColors::CYAN)
                            .strong(),
                    ),
                );
                ui.add_sized(
                    Vec2::new(100.0, 20.0),
                    egui::Label::new(RichText::new("State").color(CyberColors::CYAN).strong()),
                );
                ui.add_sized(
                    Vec2::new(60.0, 20.0),
                    egui::Label::new(RichText::new("PID").color(CyberColors::CYAN).strong()),
                );
                ui.label(RichText::new("Process").color(CyberColors::CYAN).strong());
            });
            ui.separator();

            // Connection rows
            for conn in connections.iter().take(200) {
                let proto_color = match conn.protocol {
                    Protocol::Tcp | Protocol::Tcp6 => CyberColors::NEON_BLUE,
                    Protocol::Udp | Protocol::Udp6 => CyberColors::NEON_PURPLE,
                };

                let state_color = match conn.state {
                    ConnectionState::Established => CyberColors::NEON_GREEN,
                    ConnectionState::Listen => CyberColors::CYAN,
                    ConnectionState::TimeWait | ConnectionState::CloseWait => {
                        CyberColors::NEON_YELLOW
                    }
                    ConnectionState::Stateless => CyberColors::TEXT_MUTED,
                    _ => CyberColors::NEON_ORANGE,
                };

                ui.horizontal(|ui| {
                    ui.set_min_height(18.0);

                    // Protocol
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::Label::new(
                            RichText::new(format!("{}", conn.protocol))
                                .color(proto_color)
                                .monospace(),
                        ),
                    );

                    // Local Address
                    ui.add_sized(
                        Vec2::new(200.0, 18.0),
                        egui::Label::new(
                            RichText::new(&conn.local_address)
                                .color(CyberColors::TEXT_PRIMARY)
                                .monospace(),
                        ),
                    );

                    // Remote Address
                    let remote = conn.remote_address.as_deref().unwrap_or("*");
                    ui.add_sized(
                        Vec2::new(200.0, 18.0),
                        egui::Label::new(
                            RichText::new(remote)
                                .color(CyberColors::TEXT_SECONDARY)
                                .monospace(),
                        ),
                    );

                    // State
                    ui.add_sized(
                        Vec2::new(100.0, 18.0),
                        egui::Label::new(
                            RichText::new(format!("{}", conn.state))
                                .color(state_color)
                                .monospace(),
                        ),
                    );

                    // PID
                    let pid_str = conn
                        .pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    ui.add_sized(
                        Vec2::new(60.0, 18.0),
                        egui::Label::new(
                            RichText::new(pid_str)
                                .color(CyberColors::TEXT_MUTED)
                                .monospace(),
                        ),
                    );

                    // Process name
                    let proc_name = conn.process_name.as_deref().unwrap_or("-");
                    ui.label(RichText::new(proc_name).color(CyberColors::MAGENTA));
                });
            }

            if connections.len() > 200 {
                ui.label(
                    RichText::new(format!(
                        "... and {} more connections",
                        connections.len() - 200
                    ))
                    .color(CyberColors::TEXT_MUTED),
                );
            }
        });
    }

    fn draw_system_info_tab(&mut self, ui: &mut egui::Ui) {
        // Trigger lazy loading of system info data
        self.start_system_info_loading();

        ScrollArea::vertical().show(ui, |ui| {
            ui.add(SectionHeader::new("System Information"));

            // Show loading indicator if still loading
            if self.system_info_loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading detailed system information...").color(CyberColors::TEXT_MUTED));
                });
                ui.add_space(8.0);
            }

            // Always show basic system info from environment
            egui::Grid::new("basic_system_info_grid")
                .num_columns(2)
                .spacing([40.0, 8.0])
                .show(ui, |ui| {
                    // Hostname
                    ui.label(RichText::new("Hostname:").color(CyberColors::TEXT_MUTED));
                    ui.label(RichText::new(&self.hostname).color(CyberColors::CYAN));
                    ui.end_row();

                    // OS from environment
                    ui.label(RichText::new("Platform:").color(CyberColors::TEXT_MUTED));
                    ui.label(RichText::new(std::env::consts::OS).color(CyberColors::NEON_GREEN));
                    ui.end_row();

                    // Architecture
                    ui.label(RichText::new("Architecture:").color(CyberColors::TEXT_MUTED));
                    ui.label(RichText::new(std::env::consts::ARCH).color(CyberColors::NEON_GREEN));
                    ui.end_row();

                    // Uptime
                    let uptime = self.start_time.elapsed();
                    let hours = uptime.as_secs() / 3600;
                    let mins = (uptime.as_secs() % 3600) / 60;
                    let secs = uptime.as_secs() % 60;
                    ui.label(RichText::new("App Uptime:").color(CyberColors::TEXT_MUTED));
                    ui.label(
                        RichText::new(format!("{:02}:{:02}:{:02}", hours, mins, secs))
                            .color(CyberColors::TEXT_PRIMARY),
                    );
                    ui.end_row();
                });

            // System Info Section from WMI (if available)
            if let Some(ref info) = self.system_info {
                ui.add_space(16.0);
                ui.add(SectionHeader::new("Operating System"));

                egui::Grid::new("system_info_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        // OS Information
                        ui.label(RichText::new("Operating System:").color(CyberColors::TEXT_MUTED));
                        ui.label(
                            RichText::new(format!("{} {}", info.os_name, info.os_version))
                                .color(CyberColors::CYAN),
                        );
                        ui.end_row();

                        if let Some(ref kernel) = info.kernel_version {
                            ui.label(RichText::new("Kernel:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(kernel).color(CyberColors::TEXT_PRIMARY));
                            ui.end_row();
                        }

                        if let Some(ref hostname) = info.hostname {
                            ui.label(
                                RichText::new("Computer Name:").color(CyberColors::TEXT_MUTED),
                            );
                            ui.label(RichText::new(hostname).color(CyberColors::TEXT_PRIMARY));
                            ui.end_row();
                        }
                    });

                ui.add_space(16.0);
                ui.add(SectionHeader::new("Hardware"));

                egui::Grid::new("hardware_info_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        if let Some(ref manufacturer) = info.manufacturer {
                            ui.label(RichText::new("Manufacturer:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(manufacturer).color(CyberColors::NEON_PURPLE));
                            ui.end_row();
                        }

                        if let Some(ref product) = info.product_name {
                            ui.label(RichText::new("Product:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(product).color(CyberColors::TEXT_PRIMARY));
                            ui.end_row();
                        }

                        if let Some(ref serial) = info.serial_number {
                            ui.label(RichText::new("Serial:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(serial).color(CyberColors::TEXT_MUTED));
                            ui.end_row();
                        }

                        // Motherboard
                        if let Some(ref vendor) = info.board_vendor {
                            ui.label(RichText::new("Board Vendor:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(vendor).color(CyberColors::NEON_ORANGE));
                            ui.end_row();
                        }

                        if let Some(ref name) = info.board_name {
                            ui.label(RichText::new("Board Model:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(name).color(CyberColors::TEXT_PRIMARY));
                            ui.end_row();
                        }

                        if let Some(ref version) = info.board_version {
                            ui.label(
                                RichText::new("Board Version:").color(CyberColors::TEXT_MUTED),
                            );
                            ui.label(RichText::new(version).color(CyberColors::TEXT_SECONDARY));
                            ui.end_row();
                        }

                        // CPU
                        if let Some(ref cpu_name) = info.cpu_name {
                            ui.label(RichText::new("CPU:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(cpu_name).color(CyberColors::CYAN));
                            ui.end_row();
                        }

                        if let (Some(cores), Some(threads)) = (info.cpu_cores, info.cpu_threads) {
                            ui.label(RichText::new("CPU Config:").color(CyberColors::TEXT_MUTED));
                            ui.label(
                                RichText::new(format!("{} Cores / {} Threads", cores, threads))
                                    .color(CyberColors::TEXT_PRIMARY),
                            );
                            ui.end_row();
                        }
                    });

                // BIOS/UEFI Section
                ui.add_space(16.0);
                ui.add(SectionHeader::new("BIOS / UEFI"));

                egui::Grid::new("bios_info_grid")
                    .num_columns(2)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        let bios = &info.bios;

                        ui.label(RichText::new("Firmware Type:").color(CyberColors::TEXT_MUTED));
                        ui.label(
                            RichText::new(format!("{:?}", bios.firmware_type))
                                .color(CyberColors::NEON_GREEN),
                        );
                        ui.end_row();

                        if let Some(ref vendor) = bios.vendor {
                            ui.label(RichText::new("BIOS Vendor:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(vendor).color(CyberColors::NEON_PURPLE));
                            ui.end_row();
                        }

                        if let Some(ref version) = bios.version {
                            ui.label(RichText::new("BIOS Version:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(version).color(CyberColors::TEXT_PRIMARY));
                            ui.end_row();
                        }

                        if let Some(ref date) = bios.release_date {
                            ui.label(RichText::new("Release Date:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(date).color(CyberColors::TEXT_SECONDARY));
                            ui.end_row();
                        }

                        if let Some(secure_boot) = bios.secure_boot {
                            ui.label(RichText::new("Secure Boot:").color(CyberColors::TEXT_MUTED));
                            let (text, color) = if secure_boot {
                                ("Enabled", CyberColors::NEON_GREEN)
                            } else {
                                ("Disabled", CyberColors::NEON_ORANGE)
                            };
                            ui.label(RichText::new(text).color(color));
                            ui.end_row();
                        }
                    });
            } else if !self.system_info_loading {
                // Only show error if we're done loading and still have no data
                ui.add_space(16.0);
                ui.label(
                    RichText::new("⚠ Detailed system information not available (WMI query failed)")
                        .color(CyberColors::NEON_ORANGE),
                );
            }

            // Motherboard Sensors Section
            ui.add_space(16.0);
            ui.add(SectionHeader::new("🌡️ System Temperatures"));

            // Collect all available temperatures from various sources
            let mut all_temps: Vec<(String, f32, &str)> = Vec::new();
            
            // Get motherboard sensor data
            let mut has_mb_sensors = false;
            for sensor_device in &self.motherboard_sensors {
                let temps = sensor_device.temperature_sensors().unwrap_or_default();
                if !temps.is_empty() {
                    has_mb_sensors = true;
                    for temp in temps {
                        all_temps.push((temp.label.clone(), temp.temperature, "Motherboard"));
                    }
                }
            }

            // Get GPU temperatures
            for (i, info) in self.gpu_dynamic_info.iter().enumerate() {
                if let Some(temp) = info.thermal.temperature {
                    let gpu_name = self.gpu_static_info.get(i)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| format!("GPU {}", i));
                    all_temps.push((gpu_name, temp as f32, "GPU"));
                }
            }

            // Get disk temperatures
            for disk in &self.disks {
                if let Ok(Some(temp)) = disk.temperature() {
                    let disk_name = disk.name().to_string();
                    all_temps.push((disk_name, temp, "Storage"));
                }
            }

            if all_temps.is_empty() {
                // No temperatures available at all
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ℹ️").size(16.0));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("No temperature sensors detected")
                                .color(CyberColors::TEXT_MUTED),
                        );
                        ui.label(
                            RichText::new("Windows WMI doesn't expose CPU temperatures on most systems.")
                                .color(CyberColors::TEXT_MUTED)
                                .small(),
                        );
                    });
                });
                ui.add_space(8.0);
                
                // LHM download link
                ui.horizontal(|ui| {
                    ui.label(RichText::new("💡").size(14.0));
                    ui.label(
                        RichText::new("For full sensor support, run ")
                            .color(CyberColors::TEXT_SECONDARY),
                    );
                    ui.hyperlink_to(
                        RichText::new("LibreHardwareMonitor").color(CyberColors::CYAN),
                        "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor",
                    );
                });
                ui.label(
                    RichText::new("Simon will auto-detect LHM sensors when it's running.")
                        .color(CyberColors::TEXT_MUTED)
                        .small(),
                );
            } else {
                // Show all temperatures in a nice grid
                egui::Grid::new("all_temps_grid")
                    .num_columns(3)
                    .spacing([40.0, 8.0])
                    .show(ui, |ui| {
                        for (name, temp, source) in &all_temps {
                            ui.label(RichText::new(name).color(CyberColors::TEXT_SECONDARY));
                            ui.label(
                                RichText::new(format!("{:.1}°C", temp))
                                    .color(theme::temperature_color(*temp as u32))
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(*source)
                                    .color(CyberColors::TEXT_MUTED)
                                    .small(),
                            );
                            ui.end_row();
                        }
                    });

                // Show note if we only have GPU temps (no motherboard)
                if !has_mb_sensors {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("💡").size(14.0));
                        ui.label(
                            RichText::new("CPU temps: Install ")
                                .color(CyberColors::TEXT_MUTED)
                                .small(),
                        );
                        ui.hyperlink_to(
                            RichText::new("LibreHardwareMonitor").color(CyberColors::CYAN).small(),
                            "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor",
                        );
                    });
                }
            }

            // Show voltages and fans if available (from motherboard sensors)
            for sensor_device in &self.motherboard_sensors {
                let voltages = sensor_device.voltage_rails().unwrap_or_default();
                let fans = sensor_device.fans().unwrap_or_default();

                if !voltages.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("⚡ Voltages")
                            .color(CyberColors::TEXT_MUTED)
                            .strong(),
                    );
                    egui::Grid::new(format!("volts_{}", sensor_device.name()))
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            for volt in &voltages {
                                ui.label(
                                    RichText::new(&volt.label).color(CyberColors::TEXT_SECONDARY),
                                );
                                ui.label(
                                    RichText::new(format!("{:.3}V", volt.voltage))
                                        .color(CyberColors::NEON_YELLOW),
                                );
                                ui.end_row();
                            }
                        });
                }

                if !fans.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("🌀 Fans")
                            .color(CyberColors::TEXT_MUTED)
                            .strong(),
                    );
                    egui::Grid::new(format!("fans_{}", sensor_device.name()))
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            for fan in &fans {
                                ui.label(
                                    RichText::new(&fan.label).color(CyberColors::TEXT_SECONDARY),
                                );
                                let (rpm_text, rpm_color) = match fan.rpm {
                                    Some(0) => ("Stopped".to_string(), CyberColors::TEXT_MUTED),
                                    Some(rpm) => (format!("{} RPM", rpm), CyberColors::NEON_GREEN),
                                    None => ("N/A".to_string(), CyberColors::TEXT_MUTED),
                                };
                                ui.label(
                                    RichText::new(rpm_text).color(rpm_color),
                                );
                                ui.end_row();
                            }
                        });
                }
            }

            // Storage Devices (SATA/NVMe) Section
            if !self.sata_devices.is_empty() {
                ui.add_space(16.0);
                ui.add(SectionHeader::new("💾 Storage Devices"));

                egui::Grid::new("sata_devices_grid")
                    .num_columns(5)
                    .spacing([15.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("Device").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Model").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Capacity").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Interface").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Type").color(CyberColors::CYAN).strong());
                        ui.end_row();

                        for device in &self.sata_devices {
                            // Device name
                            ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                            
                            // Model
                            ui.label(RichText::new(device.model.as_deref().unwrap_or("-"))
                                .color(CyberColors::TEXT_SECONDARY));
                            
                            // Capacity
                            let capacity = device.capacity_gb
                                .map(|gb| {
                                    if gb >= 1000.0 {
                                        format!("{:.1} TB", gb / 1000.0)
                                    } else {
                                        format!("{:.0} GB", gb)
                                    }
                                })
                                .unwrap_or_else(|| "-".to_string());
                            ui.label(RichText::new(capacity).color(CyberColors::NEON_YELLOW));
                            
                            // Interface
                            ui.label(RichText::new(device.interface_speed.as_deref().unwrap_or("-"))
                                .color(CyberColors::NEON_BLUE));
                            
                            // Media type
                            let (type_str, type_color) = match device.media_type {
                                motherboard::SataMediaType::Ssd => ("SSD", CyberColors::NEON_GREEN),
                                motherboard::SataMediaType::Hdd => ("HDD", CyberColors::NEON_ORANGE),
                                motherboard::SataMediaType::Unknown => ("Unknown", CyberColors::TEXT_MUTED),
                            };
                            ui.label(RichText::new(type_str).color(type_color));
                            ui.end_row();
                        }
                    });
            }

            // PCIe Devices Section
            if !self.pcie_devices.is_empty() {
                ui.add_space(16.0);
                ui.add(SectionHeader::new("🔌 PCIe Devices"));

                egui::Grid::new("pcie_devices_grid")
                    .num_columns(3)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("Class").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Device").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Vendor").color(CyberColors::CYAN).strong());
                        ui.end_row();

                        for device in &self.pcie_devices {
                            // Device class with color coding
                            let (class_str, class_color) = match device.device_class.as_deref() {
                                Some("Display") => ("Display", CyberColors::NEON_GREEN),
                                Some("Network") => ("Network", CyberColors::NEON_BLUE),
                                Some("Storage") => ("Storage", CyberColors::NEON_PURPLE),
                                Some("Audio") => ("Audio", CyberColors::NEON_ORANGE),
                                Some("USB") => ("USB", CyberColors::NEON_YELLOW),
                                Some(other) => (other, CyberColors::TEXT_SECONDARY),
                                None => ("Other", CyberColors::TEXT_MUTED),
                            };
                            ui.label(RichText::new(class_str).color(class_color));
                            
                            // Device name
                            ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                            
                            // Vendor
                            ui.label(RichText::new(device.vendor.as_deref().unwrap_or("-"))
                                .color(CyberColors::TEXT_SECONDARY));
                            ui.end_row();
                        }
                    });
            }

            // Drivers Section
            if !self.driver_info.is_empty() {
                ui.add_space(16.0);
                ui.add(SectionHeader::new("Installed Drivers"));

                egui::Grid::new("drivers_grid")
                    .num_columns(4)
                    .spacing([20.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("Type").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Name").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Version").color(CyberColors::CYAN).strong());
                        ui.label(RichText::new("Vendor").color(CyberColors::CYAN).strong());
                        ui.end_row();

                        for driver in &self.driver_info {
                            let type_color = match driver.driver_type {
                                crate::motherboard::DriverType::Gpu => CyberColors::NEON_GREEN,
                                crate::motherboard::DriverType::Network => CyberColors::NEON_BLUE,
                                crate::motherboard::DriverType::Storage => CyberColors::NEON_PURPLE,
                                _ => CyberColors::TEXT_SECONDARY,
                            };

                            ui.label(
                                RichText::new(format!("{}", driver.driver_type)).color(type_color),
                            );
                            ui.label(RichText::new(&driver.name).color(CyberColors::TEXT_PRIMARY));
                            ui.label(
                                RichText::new(&driver.version).color(CyberColors::NEON_YELLOW),
                            );
                            ui.label(
                                RichText::new(driver.vendor.as_deref().unwrap_or("-"))
                                    .color(CyberColors::TEXT_MUTED),
                            );
                            ui.end_row();
                        }
                    });
            }
        });
    }

    fn draw_peripherals_tab(&mut self, ui: &mut egui::Ui) {
        // Trigger background loading if not started (same as System tab)
        // Check for results from background loading
        if let Some(receiver) = self.system_info_receiver.take() {
            if let Ok(result) = receiver.try_recv() {
                self.system_info = result.system_info;
                self.motherboard_sensors = result.sensors;
                self.driver_info = result.drivers;
                self.pcie_devices = result.pcie_devices;
                self.sata_devices = result.sata_devices;
                self.system_temps = result.system_temps;
                self.peripherals = result.peripherals;
                self.system_info_loading = false;
            } else {
                // Put it back if no result yet
                self.system_info_receiver = Some(receiver);
            }
        }

        // Start background loading if not started yet
        if !self.system_info_tried && !self.system_info_loading {
            self.system_info_tried = true;
            self.system_info_loading = true;
            
            let (tx, rx) = channel();
            self.system_info_receiver = Some(rx);
            
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                
                let system_info = motherboard::get_system_info().ok();
                let sensors = motherboard::enumerate_sensors().unwrap_or_default();
                let drivers = motherboard::get_driver_versions().unwrap_or_default();
                let pcie_devices = motherboard::get_pcie_devices().unwrap_or_default();
                let sata_devices = motherboard::get_sata_devices().unwrap_or_default();
                let system_temps = motherboard::get_system_temperatures().ok();
                let peripherals = motherboard::get_peripherals().ok();
                
                let _ = tx.send(SystemInfoResult {
                    system_info,
                    sensors,
                    drivers,
                    pcie_devices,
                    sata_devices,
                    system_temps,
                    peripherals,
                });
            });
        }

        ScrollArea::vertical().show(ui, |ui| {
            ui.add(SectionHeader::new("🔌 Peripherals & Buses"));
            
            // Show loading indicator if peripherals not yet loaded
            if self.peripherals.is_none() && self.system_info_loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading peripheral information...").color(CyberColors::TEXT_MUTED));
                });
                ui.add_space(8.0);
            }
            
            if let Some(ref peripherals) = self.peripherals {
                // USB Devices Section
                if !peripherals.usb_devices.is_empty() {
                    ui.add_space(8.0);
                    ui.add(SectionHeader::new("🔗 USB Devices"));
                    
                    // Group by USB version
                    let mut usb3_devices: Vec<_> = peripherals.usb_devices.iter()
                        .filter(|d| matches!(d.usb_version, motherboard::UsbVersion::Usb3_0 | motherboard::UsbVersion::Usb3_1 | motherboard::UsbVersion::Usb3_2 | motherboard::UsbVersion::Usb4))
                        .collect();
                    let mut usb2_devices: Vec<_> = peripherals.usb_devices.iter()
                        .filter(|d| matches!(d.usb_version, motherboard::UsbVersion::Usb2_0))
                        .collect();
                    let mut other_usb: Vec<_> = peripherals.usb_devices.iter()
                        .filter(|d| matches!(d.usb_version, motherboard::UsbVersion::Usb1_1 | motherboard::UsbVersion::Unknown))
                        .collect();
                    
                    usb3_devices.sort_by(|a, b| a.name.cmp(&b.name));
                    usb2_devices.sort_by(|a, b| a.name.cmp(&b.name));
                    other_usb.sort_by(|a, b| a.name.cmp(&b.name));
                    
                    egui::Grid::new("usb_devices_grid")
                        .num_columns(4)
                        .spacing([15.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Version").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Device").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Class").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Status").color(CyberColors::CYAN).strong());
                            ui.end_row();
                            
                            // USB 3.x devices first (fastest)
                            for device in &usb3_devices {
                                let version_color = match device.usb_version {
                                    motherboard::UsbVersion::Usb4 => CyberColors::NEON_PURPLE,
                                    motherboard::UsbVersion::Usb3_2 => CyberColors::NEON_GREEN,
                                    motherboard::UsbVersion::Usb3_1 => CyberColors::NEON_GREEN,
                                    motherboard::UsbVersion::Usb3_0 => CyberColors::NEON_BLUE,
                                    _ => CyberColors::TEXT_SECONDARY,
                                };
                                ui.label(RichText::new(format!("{}", device.usb_version)).color(version_color));
                                ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(device.device_class.as_deref().unwrap_or("-")).color(CyberColors::TEXT_SECONDARY));
                                let status_color = if device.status.as_deref() == Some("OK") { CyberColors::NEON_GREEN } else { CyberColors::NEON_ORANGE };
                                ui.label(RichText::new(device.status.as_deref().unwrap_or("-")).color(status_color));
                                ui.end_row();
                            }
                            
                            // USB 2.0 devices
                            for device in &usb2_devices {
                                ui.label(RichText::new(format!("{}", device.usb_version)).color(CyberColors::NEON_YELLOW));
                                ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(device.device_class.as_deref().unwrap_or("-")).color(CyberColors::TEXT_SECONDARY));
                                let status_color = if device.status.as_deref() == Some("OK") { CyberColors::NEON_GREEN } else { CyberColors::NEON_ORANGE };
                                ui.label(RichText::new(device.status.as_deref().unwrap_or("-")).color(status_color));
                                ui.end_row();
                            }
                            
                            // Other USB devices
                            for device in &other_usb {
                                ui.label(RichText::new(format!("{}", device.usb_version)).color(CyberColors::TEXT_MUTED));
                                ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(device.device_class.as_deref().unwrap_or("-")).color(CyberColors::TEXT_SECONDARY));
                                let status_color = if device.status.as_deref() == Some("OK") { CyberColors::NEON_GREEN } else { CyberColors::NEON_ORANGE };
                                ui.label(RichText::new(device.status.as_deref().unwrap_or("-")).color(status_color));
                                ui.end_row();
                            }
                        });
                    
                    // USB summary
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!(
                            "Total: {} devices ({} USB 3.x, {} USB 2.0, {} other)",
                            peripherals.usb_devices.len(),
                            usb3_devices.len(),
                            usb2_devices.len(),
                            other_usb.len()
                        )).color(CyberColors::TEXT_MUTED).small());
                    });
                }
                
                // Display Outputs Section
                if !peripherals.display_outputs.is_empty() {
                    ui.add_space(16.0);
                    ui.add(SectionHeader::new("🖥️ Display Outputs"));
                    
                    egui::Grid::new("display_outputs_grid")
                        .num_columns(4)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Type").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Name").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Resolution").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Refresh").color(CyberColors::CYAN).strong());
                            ui.end_row();
                            
                            for output in &peripherals.display_outputs {
                                let type_color = match output.output_type {
                                    motherboard::DisplayOutputType::Hdmi => CyberColors::NEON_PURPLE,
                                    motherboard::DisplayOutputType::DisplayPort => CyberColors::NEON_GREEN,
                                    motherboard::DisplayOutputType::Thunderbolt => CyberColors::NEON_YELLOW,
                                    motherboard::DisplayOutputType::UsbC => CyberColors::NEON_BLUE,
                                    _ => CyberColors::TEXT_SECONDARY,
                                };
                                ui.label(RichText::new(format!("{}", output.output_type)).color(type_color));
                                ui.label(RichText::new(&output.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(output.resolution.as_deref().unwrap_or("-")).color(CyberColors::NEON_YELLOW));
                                let refresh = output.refresh_rate.map(|r| format!("{} Hz", r)).unwrap_or_else(|| "-".to_string());
                                ui.label(RichText::new(refresh).color(CyberColors::TEXT_SECONDARY));
                                ui.end_row();
                            }
                        });
                }
                
                // Audio Devices Section
                if !peripherals.audio_devices.is_empty() {
                    ui.add_space(16.0);
                    ui.add(SectionHeader::new("🔊 Audio Devices"));
                    
                    egui::Grid::new("audio_devices_grid")
                        .num_columns(4)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Type").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Device").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Manufacturer").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Status").color(CyberColors::CYAN).strong());
                            ui.end_row();
                            
                            for device in &peripherals.audio_devices {
                                let type_color = match device.device_type {
                                    motherboard::AudioDeviceType::Output => CyberColors::NEON_GREEN,
                                    motherboard::AudioDeviceType::Input => CyberColors::NEON_BLUE,
                                    motherboard::AudioDeviceType::OutputInput => CyberColors::NEON_PURPLE,
                                    motherboard::AudioDeviceType::Unknown => CyberColors::TEXT_MUTED,
                                };
                                ui.label(RichText::new(format!("{}", device.device_type)).color(type_color));
                                ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(device.manufacturer.as_deref().unwrap_or("-")).color(CyberColors::TEXT_SECONDARY));
                                let status_color = if device.status.as_deref() == Some("OK") { CyberColors::NEON_GREEN } else { CyberColors::NEON_ORANGE };
                                ui.label(RichText::new(device.status.as_deref().unwrap_or("-")).color(status_color));
                                ui.end_row();
                            }
                        });
                }
                
                // Network Ports Section
                if !peripherals.network_ports.is_empty() {
                    ui.add_space(16.0);
                    ui.add(SectionHeader::new("🌐 Network Ports"));
                    
                    egui::Grid::new("network_ports_grid")
                        .num_columns(4)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(RichText::new("Type").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Adapter").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Speed").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("MAC").color(CyberColors::CYAN).strong());
                            ui.end_row();
                            
                            for port in &peripherals.network_ports {
                                let type_color = match port.port_type {
                                    motherboard::NetworkPortType::Ethernet => CyberColors::NEON_BLUE,
                                    motherboard::NetworkPortType::WiFi => CyberColors::NEON_GREEN,
                                    motherboard::NetworkPortType::Bluetooth => CyberColors::NEON_PURPLE,
                                    motherboard::NetworkPortType::Thunderbolt => CyberColors::NEON_YELLOW,
                                    motherboard::NetworkPortType::Other => CyberColors::TEXT_MUTED,
                                };
                                ui.label(RichText::new(format!("{}", port.port_type)).color(type_color));
                                ui.label(RichText::new(&port.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(port.speed.as_deref().unwrap_or("-")).color(CyberColors::NEON_YELLOW));
                                ui.label(RichText::new(port.mac_address.as_deref().unwrap_or("-")).color(CyberColors::TEXT_MUTED).small());
                                ui.end_row();
                            }
                        });
                }
                
                // Bluetooth Devices Section (if any)
                if !peripherals.bluetooth_devices.is_empty() {
                    ui.add_space(16.0);
                    ui.add(SectionHeader::new("📶 Bluetooth Devices"));
                    
                    egui::Grid::new("bluetooth_devices_grid")
                        .num_columns(3)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Device").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Address").color(CyberColors::CYAN).strong());
                            ui.label(RichText::new("Status").color(CyberColors::CYAN).strong());
                            ui.end_row();
                            
                            for device in &peripherals.bluetooth_devices {
                                ui.label(RichText::new(&device.name).color(CyberColors::TEXT_PRIMARY));
                                ui.label(RichText::new(device.address.as_deref().unwrap_or("-")).color(CyberColors::TEXT_MUTED));
                                let status = if device.connected { "Connected" } else if device.paired { "Paired" } else { "Available" };
                                let status_color = if device.connected { CyberColors::NEON_GREEN } else { CyberColors::TEXT_SECONDARY };
                                ui.label(RichText::new(status).color(status_color));
                                ui.end_row();
                            }
                        });
                }
            } else if !self.system_info_loading {
                ui.add_space(16.0);
                ui.label(RichText::new("⚠ Peripheral information not available").color(CyberColors::NEON_ORANGE));
            }
        });
    }

    fn draw_network_tools_tab(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Header
                ui.add(SectionHeader::new("🔧 Network Diagnostic Tools"));
                ui.label(
                    RichText::new("nmap • traceroute • ping • netcat style utilities")
                        .color(CyberColors::TEXT_SECONDARY),
                );
                ui.add_space(10.0);

                // Target Host Input
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Target:").color(CyberColors::CYAN));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.nettools_target_host)
                            .desired_width(200.0)
                            .hint_text("hostname or IP"),
                    );
                    ui.add_space(20.0);

                    // Ping button
                    if ui
                        .button(RichText::new("🔔 Ping").color(CyberColors::NEON_GREEN))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        match network_tools::ping(&host, 4) {
                            Ok(result) => {
                                self.nettools_ping_result = Some(result);
                                self.nettools_operation = "Ping complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Ping failed: {}", e);
                            }
                        }
                    }

                    // Traceroute button
                    if ui
                        .button(RichText::new("🗺️ Traceroute").color(CyberColors::NEON_BLUE))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        match network_tools::traceroute(&host, 30) {
                            Ok(result) => {
                                self.nettools_traceroute_result = Some(result);
                                self.nettools_operation = "Traceroute complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Traceroute failed: {}", e);
                            }
                        }
                    }

                    // DNS Lookup button
                    if ui
                        .button(RichText::new("📖 DNS").color(CyberColors::NEON_YELLOW))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        match network_tools::dns_lookup(&host) {
                            Ok(addrs) => {
                                self.nettools_dns_results = addrs;
                                self.nettools_operation = "DNS lookup complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("DNS lookup failed: {}", e);
                            }
                        }
                    }
                });

                // Port Scan Section
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Port Range:").color(CyberColors::CYAN));
                    let mut start = self.nettools_port_range_start as i32;
                    let mut end = self.nettools_port_range_end as i32;
                    ui.add(egui::DragValue::new(&mut start).range(1..=65535).prefix("Start: "));
                    ui.add(egui::DragValue::new(&mut end).range(1..=65535).prefix("End: "));
                    self.nettools_port_range_start = start as u16;
                    self.nettools_port_range_end = end as u16;

                    // Common ports button (parallel scan)
                    if ui
                        .button(RichText::new("🔍 Scan Common").color(CyberColors::NEON_PURPLE))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        let ports = network_tools::common_ports();
                        match network_tools::parallel_scan(&host, &ports, std::time::Duration::from_secs(1), 50) {
                            Ok(results) => {
                                self.nettools_port_scan_results = results;
                                self.nettools_operation = "Port scan complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Port scan failed: {}", e);
                            }
                        }
                    }

                    // Scan range button (parallel)
                    if ui
                        .button(RichText::new("🔎 Scan Range").color(CyberColors::NEON_ORANGE))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        let start = self.nettools_port_range_start;
                        let end = self.nettools_port_range_end;
                        let ports: Vec<u16> = (start..=end).collect();
                        match network_tools::parallel_scan(&host, &ports, std::time::Duration::from_secs(1), 100) {
                            Ok(results) => {
                                self.nettools_port_scan_results = results;
                                self.nettools_operation = "Port scan complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Port scan failed: {}", e);
                            }
                        }
                    }
                });

                // Nmap-style scan section
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nmap-Style Scans:").color(CyberColors::CYAN));
                    
                    // Quick scan button
                    if ui
                        .button(RichText::new("⚡ Quick Scan").color(CyberColors::NEON_GREEN))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        match network_tools::quick_scan(&host) {
                            Ok(result) => {
                                self.nettools_nmap_result = Some(result);
                                self.nettools_operation = "Nmap scan complete".to_string();
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Nmap scan failed: {}", e);
                            }
                        }
                    }

                    // Full scan button  
                    if ui
                        .button(RichText::new("🔬 Full Scan").color(CyberColors::NEON_BLUE))
                        .clicked()
                        && !self.nettools_is_running
                    {
                        let host = self.nettools_target_host.clone();
                        match network_tools::full_scan(&host, std::time::Duration::from_millis(500)) {
                            Ok(result) => {
                                let duration = result.scan_duration_secs;
                                self.nettools_nmap_result = Some(result);
                                self.nettools_operation = format!("Nmap scan complete in {:.2}s", duration);
                            }
                            Err(e) => {
                                self.nettools_operation = format!("Nmap scan failed: {}", e);
                            }
                        }
                    }
                });

                // Status line
                if !self.nettools_operation.is_empty() {
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new(&self.nettools_operation)
                            .color(CyberColors::TEXT_SECONDARY)
                            .italics(),
                    );
                }

                ui.add_space(15.0);
                ui.separator();

                // Results Section
                ui.columns(2, |columns| {
                    // Left column: Ping & DNS results
                    columns[0].add(SectionHeader::new("📡 Ping Results"));
                    if let Some(ref result) = self.nettools_ping_result {
                        columns[0].horizontal(|ui| {
                            let status_color = if result.is_reachable {
                                CyberColors::NEON_GREEN
                            } else {
                                CyberColors::NEON_RED
                            };
                            let status_text = if result.is_reachable {
                                "✓ REACHABLE"
                            } else {
                                "✗ UNREACHABLE"
                            };
                            ui.label(RichText::new(&result.host).color(CyberColors::CYAN));
                            ui.label(RichText::new(status_text).color(status_color).strong());
                        });

                        if let Some(ref ip) = result.ip_address {
                            columns[0].label(
                                RichText::new(format!("  IP: {}", ip)).color(CyberColors::TEXT_SECONDARY),
                            );
                        }

                        columns[0].label(
                            RichText::new(format!(
                                "  Packets: {} sent, {} received, {:.0}% loss",
                                result.packets_sent, result.packets_received, result.packet_loss_percent
                            ))
                            .color(CyberColors::TEXT_PRIMARY),
                        );

                        if result.is_reachable {
                            columns[0].label(
                                RichText::new(format!(
                                    "  RTT: min={:.2}ms avg={:.2}ms max={:.2}ms",
                                    result.rtt_min_ms, result.rtt_avg_ms, result.rtt_max_ms
                                ))
                                .color(CyberColors::NEON_YELLOW),
                            );

                            // RTT visualization
                            columns[0].add_space(5.0);
                            let rtt_data: Vec<f32> = result.ping_times.iter()
                                .filter_map(|t| t.map(|v| v as f32))
                                .collect();
                            if !rtt_data.is_empty() {
                                columns[0].add(SparklineChart::new(rtt_data).color(CyberColors::CYAN));
                            }
                        }
                    } else {
                        columns[0].label(
                            RichText::new("No ping results yet").color(CyberColors::TEXT_MUTED),
                        );
                    }

                    // DNS Results
                    columns[0].add_space(10.0);
                    columns[0].add(SectionHeader::new("📖 DNS Results"));
                    if !self.nettools_dns_results.is_empty() {
                        for addr in &self.nettools_dns_results {
                            let addr_color = if addr.is_ipv4() {
                                CyberColors::NEON_GREEN
                            } else {
                                CyberColors::NEON_BLUE
                            };
                            columns[0].label(
                                RichText::new(format!("  → {}", addr)).color(addr_color),
                            );
                        }
                    } else {
                        columns[0].label(
                            RichText::new("No DNS results yet").color(CyberColors::TEXT_MUTED),
                        );
                    }

                    // Right column: Traceroute results
                    columns[1].add(SectionHeader::new("🗺️ Traceroute Results"));
                    if let Some(ref result) = self.nettools_traceroute_result {
                        columns[1].label(
                            RichText::new(format!(
                                "Route to {} ({} hops)",
                                result.target,
                                result.hops.len()
                            ))
                            .color(CyberColors::CYAN),
                        );

                        let status_color = if result.destination_reached {
                            CyberColors::NEON_GREEN
                        } else {
                            CyberColors::NEON_YELLOW
                        };
                        let status_text = if result.destination_reached {
                            "✓ Destination reached"
                        } else {
                            "⚠ Destination not reached"
                        };
                        columns[1].label(RichText::new(status_text).color(status_color));

                        columns[1].add_space(5.0);
                        ScrollArea::vertical()
                            .id_salt("traceroute_scroll")
                            .max_height(200.0)
                            .show(&mut columns[1], |ui| {
                                for hop in &result.hops {
                                    let addr = hop.address.as_deref().unwrap_or("*");
                                    let rtt = hop
                                        .rtt_ms
                                        .map(|r| format!("{:.2}ms", r))
                                        .unwrap_or_else(|| "*".to_string());

                                    let addr_color = if hop.responded {
                                        CyberColors::NEON_GREEN
                                    } else {
                                        CyberColors::TEXT_MUTED
                                    };

                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format!("{:>2}", hop.ttl))
                                                .color(CyberColors::CYAN),
                                        );
                                        ui.label(
                                            RichText::new(format!("{:>15}", addr))
                                                .color(addr_color)
                                                .monospace(),
                                        );
                                        ui.label(
                                            RichText::new(format!("{:>10}", rtt))
                                                .color(CyberColors::NEON_YELLOW)
                                                .monospace(),
                                        );
                                        if let Some(ref hostname) = hop.hostname {
                                            ui.label(
                                                RichText::new(hostname)
                                                    .color(CyberColors::TEXT_SECONDARY),
                                            );
                                        }
                                    });
                                }
                            });
                    } else {
                        columns[1].label(
                            RichText::new("No traceroute results yet").color(CyberColors::TEXT_MUTED),
                        );
                    }
                });

                // Port Scan Results
                ui.add_space(15.0);
                ui.separator();
                ui.add(SectionHeader::new("🔍 Port Scan Results"));

                if !self.nettools_port_scan_results.is_empty() {
                    // Summary
                    let open_count = self.nettools_port_scan_results.iter()
                        .filter(|p| p.status == PortStatus::Open)
                        .count();
                    let closed_count = self.nettools_port_scan_results.iter()
                        .filter(|p| p.status == PortStatus::Closed)
                        .count();
                    let filtered_count = self.nettools_port_scan_results.iter()
                        .filter(|p| p.status == PortStatus::Filtered)
                        .count();

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Scanned {} ports: ", self.nettools_port_scan_results.len()))
                                .color(CyberColors::TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(format!("{} open", open_count))
                                .color(CyberColors::NEON_GREEN),
                        );
                        ui.label(
                            RichText::new(format!("{} closed", closed_count))
                                .color(CyberColors::NEON_RED),
                        );
                        ui.label(
                            RichText::new(format!("{} filtered", filtered_count))
                                .color(CyberColors::NEON_YELLOW),
                        );
                    });

                    ui.add_space(5.0);

                    // Show open/filtered ports in a grid (nmap style output)
                    ScrollArea::vertical()
                        .id_salt("port_scan_scroll")
                        .max_height(250.0)
                        .show(ui, |ui| {
                            egui::Grid::new("port_scan_grid")
                                .num_columns(4)
                                .spacing([20.0, 4.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    // Header
                                    ui.label(RichText::new("PORT").color(CyberColors::CYAN).strong());
                                    ui.label(RichText::new("STATE").color(CyberColors::CYAN).strong());
                                    ui.label(RichText::new("SERVICE").color(CyberColors::CYAN).strong());
                                    ui.label(RichText::new("CONNECT").color(CyberColors::CYAN).strong());
                                    ui.end_row();

                                    for result in &self.nettools_port_scan_results {
                                        // Only show open/filtered ports (like nmap default)
                                        if result.status != PortStatus::Open && result.status != PortStatus::Filtered {
                                            continue;
                                        }

                                        let status_color = match result.status {
                                            PortStatus::Open => CyberColors::NEON_GREEN,
                                            PortStatus::Closed => CyberColors::NEON_RED,
                                            PortStatus::Filtered => CyberColors::NEON_YELLOW,
                                            PortStatus::Error => CyberColors::TEXT_MUTED,
                                        };

                                        ui.label(
                                            RichText::new(format!("{}/tcp", result.port))
                                                .color(CyberColors::TEXT_PRIMARY),
                                        );
                                        ui.label(
                                            RichText::new(format!("{}", result.status))
                                                .color(status_color),
                                        );
                                        ui.label(
                                            RichText::new(result.service.as_deref().unwrap_or("-"))
                                                .color(CyberColors::TEXT_SECONDARY),
                                        );
                                        ui.label(
                                            RichText::new(
                                                result
                                                    .connect_time_ms
                                                    .map(|t| format!("{:.1}ms", t))
                                                    .unwrap_or_else(|| "-".to_string()),
                                            )
                                            .color(CyberColors::NEON_YELLOW),
                                        );
                                        ui.end_row();
                                    }
                                });
                        });
                } else {
                    ui.label(
                        RichText::new("No port scan results yet. Use 'Scan Common' or 'Scan Range' to scan ports.")
                            .color(CyberColors::TEXT_MUTED),
                    );
                }

                // Nmap Scan Results Section
                ui.add_space(15.0);
                ui.separator();
                ui.add(SectionHeader::new("🎯 Nmap-Style Scan Results"));

                if let Some(ref result) = self.nettools_nmap_result {
                    // Host info
                    ui.horizontal(|ui| {
                        let status_color = if result.is_up {
                            CyberColors::NEON_GREEN
                        } else {
                            CyberColors::NEON_RED
                        };
                        let status_text = if result.is_up { "UP" } else { "DOWN" };
                        
                        ui.label(RichText::new(&result.host).color(CyberColors::CYAN));
                        ui.label(RichText::new(format!("({})", status_text)).color(status_color));
                        if let Some(latency) = result.latency_ms {
                            ui.label(RichText::new(format!("{:.2}ms latency", latency)).color(CyberColors::NEON_YELLOW));
                        }
                    });

                    // IP addresses
                    if !result.ip_addresses.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("IP(s):").color(CyberColors::TEXT_MUTED));
                            for ip in &result.ip_addresses {
                                ui.label(RichText::new(ip).color(CyberColors::NEON_GREEN));
                            }
                        });
                    }

                    // Hostname
                    if let Some(ref hostname) = result.hostname {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Hostname:").color(CyberColors::TEXT_MUTED));
                            ui.label(RichText::new(hostname).color(CyberColors::TEXT_PRIMARY));
                        });
                    }

                    // OS fingerprint
                    if let Some(ref os) = result.os_fingerprint {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("OS Guess:").color(CyberColors::TEXT_MUTED));
                            let os_text = match (&os.os_family, &os.os_gen) {
                                (Some(family), Some(gen)) => format!("{} {}", family, gen),
                                (Some(family), None) => family.clone(),
                                _ => "Unknown".to_string(),
                            };
                            ui.label(RichText::new(os_text).color(CyberColors::NEON_PURPLE));
                            ui.label(RichText::new(format!("({}% confidence)", os.confidence)).color(CyberColors::TEXT_SECONDARY));
                        });
                    }

                    ui.add_space(5.0);
                    ui.label(RichText::new(format!("Scan completed in {:.2}s", result.scan_duration_secs)).color(CyberColors::TEXT_SECONDARY));

                    // Services table
                    if !result.services.is_empty() {
                        ui.add_space(10.0);
                        ui.label(RichText::new(format!("{} open port(s) detected:", result.services.len())).color(CyberColors::CYAN));

                        ScrollArea::vertical()
                            .id_salt("nmap_services_scroll")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                egui::Grid::new("nmap_services_grid")
                                    .num_columns(4)
                                    .spacing([20.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        // Header
                                        ui.label(RichText::new("PORT").color(CyberColors::CYAN).strong());
                                        ui.label(RichText::new("SERVICE").color(CyberColors::CYAN).strong());
                                        ui.label(RichText::new("VERSION").color(CyberColors::CYAN).strong());
                                        ui.label(RichText::new("BANNER").color(CyberColors::CYAN).strong());
                                        ui.end_row();

                                        for svc in &result.services {
                                            ui.label(RichText::new(format!("{}/tcp", svc.port)).color(CyberColors::TEXT_PRIMARY));
                                            ui.label(RichText::new(&svc.service).color(CyberColors::NEON_GREEN));
                                            
                                            let version = match (&svc.product, &svc.version) {
                                                (Some(p), Some(v)) => format!("{} {}", p, v),
                                                (Some(p), None) => p.clone(),
                                                (None, Some(v)) => v.clone(),
                                                _ => "-".to_string(),
                                            };
                                            ui.label(RichText::new(version).color(CyberColors::NEON_PURPLE));
                                            
                                            let banner = svc.banner.as_ref()
                                                .map(|b| if b.len() > 40 { format!("{}...", &b[..40]) } else { b.clone() })
                                                .unwrap_or_else(|| "-".to_string());
                                            ui.label(RichText::new(banner).color(CyberColors::TEXT_SECONDARY));
                                            ui.end_row();
                                        }
                                    });
                            });
                    } else {
                        ui.label(RichText::new("No open ports found on scanned target.").color(CyberColors::TEXT_MUTED));
                    }
                } else {
                    ui.label(
                        RichText::new("No nmap scan results yet. Use 'Quick Scan' or 'Full Scan' for service detection.")
                            .color(CyberColors::TEXT_MUTED),
                    );
                }

                // Packet Capture (tcpdump-style) Section
                ui.add_space(15.0);
                ui.separator();
                ui.add(SectionHeader::new("📦 Packet Capture (tcpdump)"));

                // Check if capture tools are available
                let capture_available = network_tools::is_capture_available();
                
                if !capture_available {
                    ui.label(
                        RichText::new("⚠️ No packet capture tool found. Install Wireshark (tshark) or tcpdump.")
                            .color(CyberColors::WARNING),
                    );
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Protocol:").color(CyberColors::CYAN));
                        egui::ComboBox::from_id_salt("capture_protocol")
                            .selected_text(format!("{}", self.nettools_capture_protocol))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::All, "All");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Tcp, "TCP");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Udp, "UDP");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Icmp, "ICMP");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Http, "HTTP");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Https, "HTTPS");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Dns, "DNS");
                                ui.selectable_value(&mut self.nettools_capture_protocol, network_tools::CaptureProtocol::Ssh, "SSH");
                            });

                        ui.add_space(10.0);
                        ui.label(RichText::new("Packets:").color(CyberColors::CYAN));
                        let mut count = self.nettools_capture_count as i32;
                        ui.add(egui::DragValue::new(&mut count).range(10..=1000));
                        self.nettools_capture_count = count as u32;

                        ui.add_space(10.0);
                        if ui
                            .button(RichText::new("📡 Capture").color(CyberColors::NEON_GREEN))
                            .clicked()
                            && !self.nettools_is_running
                        {
                            let config = network_tools::CaptureConfig {
                                protocol: self.nettools_capture_protocol,
                                host_filter: if self.nettools_target_host.is_empty() || self.nettools_target_host == "8.8.8.8" {
                                    None
                                } else {
                                    Some(self.nettools_target_host.clone())
                                },
                                packet_count: self.nettools_capture_count,
                                timeout_secs: 30,
                                ..Default::default()
                            };
                            
                            match network_tools::capture_packets(&config) {
                                Ok(result) => {
                                    self.nettools_capture_result = Some(result);
                                    self.nettools_operation = "Capture complete".to_string();
                                }
                                Err(e) => {
                                    self.nettools_operation = format!("Capture failed: {}", e);
                                }
                            }
                        }
                    });

                    // Capture results
                    if let Some(ref result) = self.nettools_capture_result {
                        ui.add_space(10.0);
                        
                        // Summary stats
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} packets", result.total_packets)).color(CyberColors::NEON_GREEN));
                            ui.label(RichText::new(format!("({:.1} pkt/s)", result.packets_per_sec)).color(CyberColors::TEXT_SECONDARY));
                            ui.label(RichText::new(format!("{} bytes", result.total_bytes)).color(CyberColors::CYAN));
                            ui.label(RichText::new(format!("in {:.2}s", result.duration_secs)).color(CyberColors::TEXT_MUTED));
                        });

                        // Protocol breakdown
                        if !result.protocol_stats.is_empty() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Protocols:").color(CyberColors::TEXT_MUTED));
                                for (proto, count) in &result.protocol_stats {
                                    ui.label(RichText::new(format!("{}: {}", proto, count)).color(CyberColors::NEON_YELLOW));
                                }
                            });
                        }

                        // Top talkers
                        ui.columns(2, |cols| {
                            cols[0].label(RichText::new("Top Sources:").color(CyberColors::CYAN).small());
                            for (addr, count) in result.top_sources.iter().take(5) {
                                cols[0].label(RichText::new(format!("  {} ({})", addr, count)).color(CyberColors::TEXT_SECONDARY).small());
                            }

                            cols[1].label(RichText::new("Top Destinations:").color(CyberColors::CYAN).small());
                            for (addr, count) in result.top_destinations.iter().take(5) {
                                cols[1].label(RichText::new(format!("  {} ({})", addr, count)).color(CyberColors::TEXT_SECONDARY).small());
                            }
                        });

                        // Packet table
                        ui.add_space(5.0);
                        ScrollArea::vertical()
                            .id_salt("capture_packets_scroll")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                egui::Grid::new("capture_packets_grid")
                                    .num_columns(6)
                                    .spacing([10.0, 2.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        // Header
                                        ui.label(RichText::new("#").color(CyberColors::CYAN).small());
                                        ui.label(RichText::new("TIME").color(CyberColors::CYAN).small());
                                        ui.label(RichText::new("SOURCE").color(CyberColors::CYAN).small());
                                        ui.label(RichText::new("DEST").color(CyberColors::CYAN).small());
                                        ui.label(RichText::new("PROTO").color(CyberColors::CYAN).small());
                                        ui.label(RichText::new("LEN").color(CyberColors::CYAN).small());
                                        ui.end_row();

                                        for pkt in result.packets.iter().take(100) {
                                            ui.label(RichText::new(format!("{}", pkt.number)).color(CyberColors::TEXT_MUTED).small());
                                            ui.label(RichText::new(&pkt.timestamp).color(CyberColors::TEXT_SECONDARY).small());
                                            ui.label(RichText::new(&pkt.source).color(CyberColors::NEON_GREEN).small());
                                            ui.label(RichText::new(&pkt.destination).color(CyberColors::NEON_BLUE).small());
                                            ui.label(RichText::new(&pkt.protocol).color(CyberColors::NEON_YELLOW).small());
                                            ui.label(RichText::new(format!("{}", pkt.length)).color(CyberColors::TEXT_PRIMARY).small());
                                            ui.end_row();
                                        }
                                    });
                            });
                    } else {
                        ui.label(
                            RichText::new("No capture results yet. Click 'Capture' to start packet capture.")
                                .color(CyberColors::TEXT_MUTED),
                        );
                        ui.label(
                            RichText::new("Note: Requires administrator/root privileges.")
                                .color(CyberColors::WARNING)
                                .small(),
                        );
                    }
                }

                // Help/Info section
                ui.add_space(15.0);
                ui.separator();
                ui.collapsing(RichText::new("ℹ️ About Network Tools").color(CyberColors::CYAN), |ui| {
                    ui.label(RichText::new(
                        "This tab provides network diagnostic tools similar to popular CLI utilities:\n\n\
                        • Ping - ICMP echo test (like 'ping' command)\n\
                        • Traceroute - Path tracing with hop-by-hop latency (like 'traceroute/tracert')\n\
                        • DNS - Domain name resolution (like 'nslookup/dig')\n\
                        • Port Scan - TCP connect scan (like 'nmap -sT')\n\
                        • Nmap Scan - Service detection with banner grabbing\n\
                        • Packet Capture - Network traffic capture (like 'tcpdump/tshark')\n\n\
                        Note: Some operations may require administrator privileges or be blocked by firewalls."
                    ).color(CyberColors::TEXT_SECONDARY));
                });
            });
    }

    fn draw_ai_assistant_tab(&mut self, ui: &mut egui::Ui) {
        // Show loading state while agent is being initialized in background
        // But timeout after 3 seconds to show the UI anyway
        let loading_timeout = self.agent_loading && self.agent_loading_start.elapsed().as_secs() < 3;
        
        if loading_timeout {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.spinner();
                ui.add_space(10.0);
                ui.label(
                    RichText::new("🔍 Detecting AI backends...")
                        .color(CyberColors::CYAN)
                        .size(18.0),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Checking for Ollama, OpenAI, Anthropic, LM Studio...")
                        .color(CyberColors::TEXT_SECONDARY),
                );
            });
            return;
        }
        
        // If we timed out, mark loading as done
        if self.agent_loading && self.agent_loading_start.elapsed().as_secs() >= 3 {
            self.agent_loading = false;
        }

        let agent_available = self.agent.is_some() && self.silicon_monitor.is_some();

        // Wrap content in ScrollArea like other tabs
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);

            // Header with provider/model selection
            let mut refresh_ollama = false;
            
            ui.horizontal(|ui| {
                ui.add(SectionHeader::new("🤖 AI System Assistant"));
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Model dropdown - use detected models for Ollama, predefined for others
                    let display_model = if self.ai_selected_model.is_empty() {
                        "Select model...".to_string()
                    } else {
                        self.ai_selected_model.clone()
                    };
                
                // Refresh button for Ollama models
                if matches!(self.ai_selected_backend, AiBackendSelection::Ollama) {
                    if ui.button(RichText::new("🔄").size(14.0)).on_hover_text("Refresh model list").clicked() {
                        refresh_ollama = true;
                    }
                }
                
                egui::ComboBox::from_id_salt("ai_model_select")
                    .selected_text(RichText::new(&display_model).color(CyberColors::NEON_GREEN))
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        // Models based on selected backend
                        match self.ai_selected_backend {
                            AiBackendSelection::Ollama => {
                                // Use actually detected Ollama models
                                for model in &self.ai_ollama_models {
                                    ui.selectable_value(&mut self.ai_selected_model, model.clone(), model);
                                }
                            }
                            AiBackendSelection::OpenAi => {
                                for model in ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4", "gpt-3.5-turbo"] {
                                    ui.selectable_value(&mut self.ai_selected_model, model.to_string(), model);
                                }
                            }
                            AiBackendSelection::Anthropic => {
                                for model in ["claude-3-5-sonnet-latest", "claude-3-opus", "claude-3-sonnet", "claude-3-haiku"] {
                                    ui.selectable_value(&mut self.ai_selected_model, model.to_string(), model);
                                }
                            }
                            AiBackendSelection::GitHub => {
                                for model in ["gpt-4o", "gpt-4o-mini", "o1-preview", "o1-mini"] {
                                    ui.selectable_value(&mut self.ai_selected_model, model.to_string(), model);
                                }
                            }
                            AiBackendSelection::LmStudio => {
                                ui.selectable_value(&mut self.ai_selected_model, "local-model".to_string(), "local-model");
                            }
                        }
                    });
                
                ui.label(RichText::new("Model:").color(CyberColors::TEXT_SECONDARY).size(12.0));
                
                ui.add_space(12.0);
                
                // Provider dropdown
                let provider_name = match self.ai_selected_backend {
                    AiBackendSelection::Ollama => "🦙 Ollama",
                    AiBackendSelection::OpenAi => "🤖 OpenAI",
                    AiBackendSelection::Anthropic => "🧠 Anthropic",
                    AiBackendSelection::GitHub => "🐙 GitHub",
                    AiBackendSelection::LmStudio => "📦 LM Studio",
                };
                egui::ComboBox::from_id_salt("ai_provider_select")
                    .selected_text(RichText::new(provider_name).color(CyberColors::CYAN))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::Ollama, "🦙 Ollama");
                        ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::OpenAi, "🤖 OpenAI");
                        ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::Anthropic, "🧠 Anthropic");
                        ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::GitHub, "🐙 GitHub");
                        ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::LmStudio, "📦 LM Studio");
                    });
                
                ui.label(RichText::new("Provider:").color(CyberColors::TEXT_SECONDARY).size(12.0));
            });
        });

        // Handle deferred refresh after the UI block
        if refresh_ollama {
            self.refresh_ollama_models();
        }

        ui.add_space(8.0);

        // Show connection warning if agent not available
        if !agent_available {
            egui::Frame::none()
                .fill(CyberColors::SURFACE)
                .stroke(egui::Stroke::new(1.0, CyberColors::NEON_YELLOW))
                .rounding(6.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠").color(CyberColors::NEON_YELLOW).size(16.0));
                        ui.label(RichText::new("AI backend not connected.").color(CyberColors::NEON_YELLOW));
                        if ui.button(RichText::new("🔄 Retry").color(CyberColors::CYAN)).clicked() {
                            self.retry_agent_connection();
                        }
                        if ui.button(RichText::new("⚙ Setup").color(CyberColors::TEXT_SECONDARY)).clicked() {
                            // Show setup panel in a collapsible section
                        }
                    });
                });
            ui.add_space(8.0);
        }

        // Chat history area - fills available space
        let chat_height = ui.available_height() - 80.0; // Leave room for input area
        
        egui::Frame::none()
            .fill(CyberColors::BACKGROUND)
            .stroke(egui::Stroke::new(1.0, CyberColors::BORDER))
            .rounding(6.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2(ui.available_width(), chat_height.max(300.0)));
                
                if self.agent_history.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("👋 Welcome to the AI Assistant!")
                                .color(CyberColors::CYAN)
                                .size(20.0),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Ask questions about your system's performance, GPU status, or get optimization suggestions.")
                                .color(CyberColors::TEXT_SECONDARY)
                                .size(14.0),
                        );
                        ui.add_space(24.0);
                        
                        // Example questions
                        egui::Frame::none()
                            .fill(CyberColors::SURFACE)
                            .rounding(6.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new("💡 Try asking:").color(CyberColors::TEXT_PRIMARY).size(13.0));
                                ui.add_space(8.0);
                                let examples = [
                                    "What is my GPU utilization?",
                                    "Is my system running hot?",
                                    "How can I optimize performance?",
                                    "What's using my GPU memory?",
                                ];
                                for example in examples {
                                    ui.label(RichText::new(format!("  • {}", example)).color(CyberColors::CYAN_DIM).size(12.0));
                                }
                            });
                    });
                } else {
                    // Scroll area for messages
                    let scroll_height = chat_height - 30.0;
                    egui::ScrollArea::vertical()
                        .max_height(scroll_height.max(250.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                    // Calculate max bubble width - allow wider bubbles
                    let max_bubble_width = (ui.available_width() - 60.0).min(800.0);
                    
                    for entry in self.agent_history.iter() {
                        let is_user = entry.role == ChatRole::User;
                        
                        let (bg_color, border_color, text_color, icon) = if is_user {
                            (CyberColors::SURFACE, CyberColors::CYAN_DIM, CyberColors::TEXT_PRIMARY, "👤")
                        } else {
                            (CyberColors::BACKGROUND_DARK, CyberColors::NEON_GREEN, CyberColors::TEXT_PRIMARY, "🤖")
                        };

                        // Message bubble - left aligned for assistant, right padding for user
                        if is_user {
                            ui.add_space(40.0); // Indent user messages
                        }
                        
                        egui::Frame::none()
                            .fill(bg_color)
                            .stroke(egui::Stroke::new(1.0, border_color))
                            .inner_margin(10.0)
                            .rounding(8.0)
                            .show(ui, |ui| {
                                ui.set_max_width(max_bubble_width);
                                
                                // Header with icon and role
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(icon).size(14.0));
                                    ui.label(
                                        RichText::new(if is_user { "You" } else { "Assistant" })
                                            .color(if is_user { CyberColors::CYAN } else { CyberColors::NEON_GREEN })
                                            .strong()
                                            .size(12.0),
                                    );
                                    
                                    // Metadata on the right
                                    if !is_user {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let meta = if entry.from_cache {
                                                "⚡ cached".to_string()
                                            } else if let Some(ms) = entry.inference_time_ms {
                                                format!("⏱ {}ms", ms)
                                            } else {
                                                String::new()
                                            };
                                            if !meta.is_empty() {
                                                ui.label(RichText::new(meta).color(CyberColors::TEXT_MUTED).size(10.0));
                                            }
                                        });
                                    }
                                });
                                
                                ui.add_space(4.0);
                                        
                                // Message content with text wrapping
                                ui.label(
                                    RichText::new(&entry.content)
                                        .color(text_color)
                                        .size(13.0),
                                );
                            });
                        
                        ui.add_space(8.0);
                    }
                    }); // End ScrollArea
                }
            });

        ui.add_space(8.0);

        // Input area with improved styling
        egui::Frame::none()
            .fill(CyberColors::SURFACE)
            .stroke(egui::Stroke::new(1.0, CyberColors::BORDER))
            .rounding(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.agent_query)
                            .hint_text("Ask about your system...")
                            .desired_width(ui.available_width() - 100.0)
                            .font(egui::TextStyle::Body),
                    );

                    let enter_pressed =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    let send_enabled = !self.agent_is_processing && !self.agent_query.trim().is_empty();
                    let send_btn = ui.add_enabled(
                        send_enabled,
                        egui::Button::new(
                            RichText::new(if self.agent_is_processing { "⏳" } else { "➤ Send" })
                                .color(if send_enabled { CyberColors::CYAN } else { CyberColors::TEXT_MUTED })
                                .size(14.0)
                        )
                        .min_size(Vec2::new(70.0, 28.0)),
                    );

                    if (enter_pressed || send_btn.clicked())
                        && !self.agent_is_processing
                        && !self.agent_query.trim().is_empty()
                    {
                        self.send_agent_query();
                    }
                });
            });

        // Bottom toolbar
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("🗑️ Clear").color(CyberColors::TEXT_MUTED).size(12.0)).clicked() {
                self.agent_history.clear();
            }
            
            ui.add_space(8.0);
            
            if self.agent_is_processing {
                ui.spinner();
                ui.label(
                    RichText::new("Thinking...")
                        .color(CyberColors::CYAN)
                        .italics()
                        .size(12.0),
                );
            } else {
                ui.label(
                    RichText::new(format!("{} messages", self.agent_history.len()))
                        .color(CyberColors::TEXT_MUTED)
                        .size(11.0),
                );
            }
        });
        }); // End ScrollArea
    }

    fn send_agent_query(&mut self) {
        let query = self.agent_query.trim().to_string();
        if query.is_empty() || self.agent_is_processing {
            return;
        }

        // Add user message to history
        self.agent_history.push_back(AgentChatEntry {
            role: ChatRole::User,
            content: query.clone(),
            timestamp: std::time::Instant::now(),
            inference_time_ms: None,
            from_cache: false,
        });

        self.agent_query.clear();
        self.agent_is_processing = true;

        // Get tool context from AI Data API (runs on UI thread to avoid Send issues)
        let tool_context = if let Some(ref mut api) = self.ai_data_api {
            api.auto_query(&query)
        } else {
            String::new()
        };

        // Get the agent config and monitor for background thread
        // We need to create fresh instances in the background thread since Agent uses &mut self
        let config = self.agent.as_ref().map(|a| a.config().clone());
        
        if let Some(config) = config {
            let (tx, rx) = channel();
            self.agent_response_receiver = Some(rx);

            // Spawn background thread for agent query
            std::thread::spawn(move || {
                let result = (|| -> Result<AgentResponse, String> {
                    // Create fresh agent and monitor in background thread
                    let mut agent = crate::agent::Agent::new(config)
                        .map_err(|e| format!("Failed to create agent: {}", e))?;
                    let monitor = crate::SiliconMonitor::new()
                        .map_err(|e| format!("Failed to create monitor: {}", e))?;
                    
                    // Enhance the query with tool context if available
                    let enhanced_query = if !tool_context.is_empty() {
                        format!(
                            "{}\n\n---\n\n## User Question\n{}", 
                            tool_context,
                            query
                        )
                    } else {
                        query
                    };
                    
                    let response = agent.ask(&enhanced_query, &monitor)
                        .map_err(|e| format!("{}", e))?;
                    
                    Ok(AgentResponse {
                        response: response.response,
                        inference_time_ms: response.inference_time_ms,
                        from_cache: response.from_cache,
                    })
                })();
                
                let _ = tx.send(result);
            });
        } else {
            // No agent available
            self.agent_history.push_back(AgentChatEntry {
                role: ChatRole::Assistant,
                content: "Error: AI Agent not available".to_string(),
                timestamp: std::time::Instant::now(),
                inference_time_ms: None,
                from_cache: false,
            });
            self.agent_is_processing = false;
        }
    }

    /// Draw the AI setup panel when no backend is available
    fn draw_ai_setup_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        
        // Show status message if any
        if let Some((msg, is_error)) = &self.ai_status_message {
            let color = if *is_error { CyberColors::NEON_RED } else { CyberColors::NEON_GREEN };
            ui.horizontal(|ui| {
                ui.label(RichText::new(if *is_error { "❌" } else { "✓" }).color(color));
                ui.label(RichText::new(msg.as_str()).color(color));
            });
            ui.add_space(10.0);
        }

        // Detected backends section
        let available = crate::agent::AgentConfig::list_available_backends();
        if !available.is_empty() {
            egui::Frame::none()
                .fill(CyberColors::SURFACE)
                .rounding(8.0)
                .inner_margin(15.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("✓ Available Backends").color(CyberColors::NEON_GREEN).size(16.0));
                    ui.add_space(5.0);
                    for backend in &available {
                        ui.label(RichText::new(format!("  • {:?}", backend)).color(CyberColors::CYAN));
                    }
                    ui.add_space(10.0);
                    if ui.button(RichText::new("🔄 Retry Connection").color(CyberColors::CYAN)).clicked() {
                        self.retry_agent_connection();
                    }
                });
            ui.add_space(15.0);
        }

        // Setup options
        egui::Frame::none()
            .fill(CyberColors::SURFACE)
            .rounding(8.0)
            .inner_margin(15.0)
            .show(ui, |ui| {
                ui.label(RichText::new("🔧 Configure AI Backend").color(CyberColors::CYAN).size(18.0));
                ui.add_space(15.0);

                // Backend selection tabs - read current value first to avoid borrow issues
                let current_backend = self.ai_selected_backend;
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::Ollama, 
                        RichText::new("🦙 Ollama").color(if current_backend == AiBackendSelection::Ollama { CyberColors::CYAN } else { CyberColors::TEXT_SECONDARY }));
                    ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::OpenAi, 
                        RichText::new("🤖 OpenAI").color(if current_backend == AiBackendSelection::OpenAi { CyberColors::CYAN } else { CyberColors::TEXT_SECONDARY }));
                    ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::Anthropic, 
                        RichText::new("🧠 Anthropic").color(if current_backend == AiBackendSelection::Anthropic { CyberColors::CYAN } else { CyberColors::TEXT_SECONDARY }));
                    ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::GitHub, 
                        RichText::new("🐙 GitHub").color(if current_backend == AiBackendSelection::GitHub { CyberColors::CYAN } else { CyberColors::TEXT_SECONDARY }));
                    ui.selectable_value(&mut self.ai_selected_backend, AiBackendSelection::LmStudio, 
                        RichText::new("📦 LM Studio").color(if current_backend == AiBackendSelection::LmStudio { CyberColors::CYAN } else { CyberColors::TEXT_SECONDARY }));
                });
                ui.add_space(15.0);
                ui.separator();
                ui.add_space(10.0);

                match self.ai_selected_backend {
                    AiBackendSelection::Ollama => {
                        ui.label(RichText::new("Ollama - Local AI (Recommended)").color(CyberColors::TEXT_PRIMARY).size(14.0));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Run AI models locally on your machine. Free and private.").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            if self.ai_ollama_starting {
                                ui.spinner();
                                ui.label(RichText::new("Starting Ollama...").color(CyberColors::CYAN));
                            } else {
                                if ui.button(RichText::new("▶ Start Ollama").color(CyberColors::NEON_GREEN).size(14.0)).clicked() {
                                    self.start_ollama();
                                }
                                if ui.button(RichText::new("📥 Install Ollama").color(CyberColors::TEXT_SECONDARY)).clicked() {
                                    let _ = open::that("https://ollama.com/download");
                                }
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(RichText::new("After starting Ollama, click 'Retry Connection' above.").color(CyberColors::TEXT_MUTED).small());
                    }
                    AiBackendSelection::OpenAi => {
                        ui.label(RichText::new("OpenAI API").color(CyberColors::TEXT_PRIMARY).size(14.0));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Use GPT models via OpenAI API. Requires API key.").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("API Key:").color(CyberColors::TEXT_SECONDARY));
                            ui.add(egui::TextEdit::singleline(&mut self.ai_api_key_input)
                                .password(true)
                                .hint_text("sk-...")
                                .desired_width(300.0));
                        });
                        ui.add_space(10.0);
                        if ui.button(RichText::new("💾 Set API Key").color(CyberColors::CYAN)).clicked() {
                            self.set_api_key("OPENAI_API_KEY");
                        }
                        ui.add_space(5.0);
                        if ui.link(RichText::new("Get an API key from OpenAI →").color(CyberColors::TEXT_MUTED).small()).clicked() {
                            let _ = open::that("https://platform.openai.com/api-keys");
                        }
                    }
                    AiBackendSelection::Anthropic => {
                        ui.label(RichText::new("Anthropic Claude API").color(CyberColors::TEXT_PRIMARY).size(14.0));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Use Claude models via Anthropic API. Requires API key.").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("API Key:").color(CyberColors::TEXT_SECONDARY));
                            ui.add(egui::TextEdit::singleline(&mut self.ai_api_key_input)
                                .password(true)
                                .hint_text("sk-ant-...")
                                .desired_width(300.0));
                        });
                        ui.add_space(10.0);
                        if ui.button(RichText::new("💾 Set API Key").color(CyberColors::CYAN)).clicked() {
                            self.set_api_key("ANTHROPIC_API_KEY");
                        }
                        ui.add_space(5.0);
                        if ui.link(RichText::new("Get an API key from Anthropic →").color(CyberColors::TEXT_MUTED).small()).clicked() {
                            let _ = open::that("https://console.anthropic.com/settings/keys");
                        }
                    }
                    AiBackendSelection::GitHub => {
                        ui.label(RichText::new("GitHub Models").color(CyberColors::TEXT_PRIMARY).size(14.0));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Use AI models via GitHub. Requires GitHub token.").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(15.0);
                        
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Token:").color(CyberColors::TEXT_SECONDARY));
                            ui.add(egui::TextEdit::singleline(&mut self.ai_api_key_input)
                                .password(true)
                                .hint_text("ghp_...")
                                .desired_width(300.0));
                        });
                        ui.add_space(10.0);
                        if ui.button(RichText::new("💾 Set Token").color(CyberColors::CYAN)).clicked() {
                            self.set_api_key("GITHUB_TOKEN");
                        }
                        ui.add_space(5.0);
                        if ui.link(RichText::new("Create a GitHub token →").color(CyberColors::TEXT_MUTED).small()).clicked() {
                            let _ = open::that("https://github.com/settings/tokens");
                        }
                    }
                    AiBackendSelection::LmStudio => {
                        ui.label(RichText::new("LM Studio - Local AI").color(CyberColors::TEXT_PRIMARY).size(14.0));
                        ui.add_space(5.0);
                        ui.label(RichText::new("Run local models with LM Studio's OpenAI-compatible API.").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(15.0);
                        
                        ui.label(RichText::new("1. Download and install LM Studio").color(CyberColors::TEXT_SECONDARY));
                        ui.label(RichText::new("2. Download a model (e.g., Llama 3.2, Mistral)").color(CyberColors::TEXT_SECONDARY));
                        ui.label(RichText::new("3. Start the local server on port 1234").color(CyberColors::TEXT_SECONDARY));
                        ui.add_space(10.0);
                        
                        if ui.button(RichText::new("📥 Download LM Studio").color(CyberColors::CYAN)).clicked() {
                            let _ = open::that("https://lmstudio.ai/");
                        }
                        ui.add_space(5.0);
                        ui.label(RichText::new("After starting the server, click 'Retry Connection' above.").color(CyberColors::TEXT_MUTED).small());
                    }
                }
            });
    }

    /// Start Ollama in the background
    fn start_ollama(&mut self) {
        self.ai_ollama_starting = true;
        self.ai_status_message = Some(("Starting Ollama...".to_string(), false));
        
        // Try to start Ollama
        #[cfg(target_os = "windows")]
        {
            // On Windows, try to start Ollama from common locations
            let ollama_paths = [
                std::env::var("LOCALAPPDATA").ok().map(|p| format!("{}\\Ollama\\ollama.exe", p)),
                Some("C:\\Program Files\\Ollama\\ollama.exe".to_string()),
                Some("ollama".to_string()), // Try PATH
            ];
            
            for path in ollama_paths.into_iter().flatten() {
                if let Ok(_) = std::process::Command::new(&path)
                    .arg("serve")
                    .spawn()
                {
                    self.ai_status_message = Some(("Ollama started! Wait a few seconds and click 'Retry Connection'.".to_string(), false));
                    self.ai_ollama_starting = false;
                    return;
                }
            }
            self.ai_status_message = Some(("Could not start Ollama. Please install it from ollama.com".to_string(), true));
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(_) = std::process::Command::new("ollama")
                .arg("serve")
                .spawn()
            {
                self.ai_status_message = Some(("Ollama started! Wait a few seconds and click 'Retry Connection'.".to_string(), false));
            } else {
                self.ai_status_message = Some(("Could not start Ollama. Please install it from ollama.com".to_string(), true));
            }
        }
        
        self.ai_ollama_starting = false;
    }

    /// Set an API key as environment variable and retry connection
    fn set_api_key(&mut self, env_var: &str) {
        let key = self.ai_api_key_input.trim();
        if key.is_empty() {
            self.ai_status_message = Some(("Please enter an API key".to_string(), true));
            return;
        }
        
        // Set environment variable for this process
        std::env::set_var(env_var, key);
        self.ai_api_key_input.clear();
        self.ai_status_message = Some((format!("{} set! Retrying connection...", env_var), false));
        
        // Retry connection
        self.retry_agent_connection();
    }

    /// Retry agent connection with current configuration
    fn retry_agent_connection(&mut self) {
        self.agent_loading = true;
        self.ai_status_message = None;
        
        let (tx, rx) = channel();
        self.agent_receiver = Some(rx);
        
        std::thread::spawn(move || {
            let agent = crate::agent::AgentConfig::auto_detect()
                .ok()
                .and_then(|config| crate::agent::Agent::new(config).ok());
            let _ = tx.send(agent);
        });
    }

    /// Draw the settings window
    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        egui::Window::new("⚙ Settings")
            .collapsible(true)
            .resizable(true)
            .default_width(350.0)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 50.0))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                
                // Close button in corner
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Appearance").color(CyberColors::CYAN));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
                
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Color Theme Selection
                ui.label(RichText::new("Color Theme").color(CyberColors::TEXT_PRIMARY));
                ui.add_space(4.0);
                
                egui::ComboBox::from_id_salt("theme_selector")
                    .selected_text(self.settings.color_theme.name())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for theme in ColorTheme::all() {
                            let is_selected = self.settings.color_theme == *theme;
                            ui.horizontal(|ui| {
                                // Color preview swatch
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 2.0, theme.accent_color());
                                
                                if ui.selectable_label(is_selected, theme.name()).clicked() {
                                    self.settings.color_theme = *theme;
                                }
                            });
                        }
                    });

                ui.add_space(16.0);
                
                // Graph Line Thickness
                ui.label(RichText::new("Graph Line Thickness").color(CyberColors::TEXT_PRIMARY));
                ui.add_space(4.0);
                
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.settings.graph_line_thickness, 1.0..=5.0)
                            .step_by(0.5)
                            .suffix(" px")
                    );
                });

                // Preview line
                ui.add_space(8.0);
                let preview_rect = ui.available_rect_before_wrap();
                let preview_height = 30.0;
                let preview_width = preview_rect.width().min(200.0);
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(preview_width, preview_height),
                    egui::Sense::hover(),
                );
                let rect = response.rect;
                
                // Draw preview background
                painter.rect_filled(rect, 4.0, CyberColors::SURFACE);
                
                // Draw sample sine wave with current thickness
                let points: Vec<egui::Pos2> = (0..50)
                    .map(|i| {
                        let t = i as f32 / 49.0;
                        let x = rect.left() + t * rect.width();
                        let y = rect.center().y + (t * 6.0 * std::f32::consts::PI).sin() * 10.0;
                        egui::Pos2::new(x, y)
                    })
                    .collect();
                
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(
                        self.settings.graph_line_thickness,
                        self.settings.color_theme.accent_color(),
                    ),
                ));

                ui.add_space(16.0);
                
                // Show Grid Lines
                ui.checkbox(
                    &mut self.settings.show_grid_lines,
                    RichText::new("Show Grid Lines").color(CyberColors::TEXT_PRIMARY),
                );

                ui.add_space(8.0);
                
                // Animation Speed
                ui.label(RichText::new("Animation Speed").color(CyberColors::TEXT_PRIMARY));
                ui.add_space(4.0);
                ui.add(
                    egui::Slider::new(&mut self.settings.animation_speed, 0.5..=2.0)
                        .step_by(0.1)
                        .suffix("x")
                );

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Reset to defaults button
                ui.horizontal(|ui| {
                    if ui.button("Reset to Defaults").clicked() {
                        self.settings = AppSettings::default();
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            self.show_settings = false;
                        }
                    });
                });
                
                ui.add_space(8.0);
            });
    }

    /// Detect locally installed Ollama models by running `ollama list`
    fn detect_ollama_models() -> Vec<String> {
        // Try to run `ollama list` and parse output
        match std::process::Command::new("ollama")
            .arg("list")
            .output()
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut models = Vec::new();
                
                // Skip header line (NAME, ID, SIZE, MODIFIED)
                for line in stdout.lines().skip(1) {
                    // First column is the model name
                    if let Some(name) = line.split_whitespace().next() {
                        if !name.is_empty() {
                            models.push(name.to_string());
                        }
                    }
                }
                
                if models.is_empty() {
                    // Fallback if parsing failed
                    vec!["llama3.2".to_string()]
                } else {
                    models
                }
            }
            _ => {
                // Ollama not installed or not running - use defaults
                vec!["llama3.2".to_string(), "llama3.1".to_string(), "mistral".to_string()]
            }
        }
    }

    /// Refresh the list of Ollama models (can be called when user switches to Ollama)
    fn refresh_ollama_models(&mut self) {
        self.ai_ollama_models = Self::detect_ollama_models();
        // Select first model if current selection is not in the list
        if !self.ai_ollama_models.contains(&self.ai_selected_model) {
            if let Some(first) = self.ai_ollama_models.first() {
                self.ai_selected_model = first.clone();
            }
        }
    }
}

/// Format bytes as human-readable string (B, KB, MB, GB)
fn format_bytes(bytes: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes / KB)
    } else {
        format!("{:.0} B", bytes)
    }
}











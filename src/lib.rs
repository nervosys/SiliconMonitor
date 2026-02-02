//! # Silicon Monitor (simon)
//!
//! A comprehensive, cross-platform Rust library for hardware monitoring. Silicon Monitor provides
//! a unified API for monitoring CPUs, GPUs (NVIDIA/AMD/Intel), memory, disks, motherboards,
//! processes, and network interfaces across Windows, Linux, and macOS.
//!
//! ## Features
//!
//! - **Multi-Vendor GPU Support**: NVIDIA (NVML), AMD (sysfs), Intel (i915/xe)
//! - **Process Monitoring**: System-wide process tracking with GPU attribution
//! - **Network Monitoring**: Interface statistics with bandwidth rate calculation
//! - **Cross-Platform**: Unified API across Windows, Linux, and macOS
//! - **Zero-Cost Abstractions**: Built in Rust for maximum performance and safety
//! - **Terminal UI**: Beautiful TUI for real-time monitoring
//!
//! ## Quick Start
//!
//! ### GPU Monitoring
//!
//! ```no_run
//! use simon::gpu::GpuCollection;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Auto-detect all available GPUs
//! let gpus = GpuCollection::auto_detect()?;
//!
//! // Get snapshot of all GPUs
//! for (idx, info) in gpus.snapshot_all()?.iter().enumerate() {
//!     println!("GPU {}: {} ({})",
//!         idx,
//!         info.static_info.name,
//!         info.static_info.vendor
//!     );
//!     println!("  Utilization: {}%", info.dynamic_info.utilization);
//!     println!("  Memory: {} / {} MB",
//!         info.dynamic_info.memory.used / 1024 / 1024,
//!         info.dynamic_info.memory.total / 1024 / 1024
//!     );
//!     if let Some(temp) = info.dynamic_info.thermal.temperature {
//!         println!("  Temperature: {}°C", temp);
//!     }
//!     if let Some(power) = info.dynamic_info.power.draw {
//!         println!("  Power: {:.1}W", power as f32 / 1000.0);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Process Monitoring with GPU Attribution
//!
//! ```no_run
//! use simon::{ProcessMonitor, GpuCollection};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let gpus = GpuCollection::auto_detect()?;
//! let mut monitor = ProcessMonitor::with_gpus(gpus)?;
//!
//! // Get processes sorted by GPU memory usage
//! let gpu_procs = monitor.processes_by_gpu_memory()?;
//! for proc in gpu_procs.iter().take(10) {
//!     println!("{} (PID {}): {} MB GPU memory",
//!         proc.name,
//!         proc.pid,
//!         proc.total_gpu_memory_bytes / 1024 / 1024
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Network Monitoring
//!
//! ```no_run
//! use simon::NetworkMonitor;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut monitor = NetworkMonitor::new()?;
//!
//! // Get all active interfaces
//! for iface in monitor.active_interfaces()? {
//!     let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, &iface);
//!     println!("{}: ↓{:.2} MB/s ↑{:.2} MB/s",
//!         iface.name,
//!         rx_rate / 1_000_000.0,
//!         tx_rate / 1_000_000.0
//!     );
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### AI Data API - Tool-Based System Access
//!
//! The AI Data API provides a comprehensive tool-based interface for AI systems
//! to query hardware monitoring data:
//!
//! ```no_run
//! use simon::ai_api::{AiDataApi, ToolCategory};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut api = AiDataApi::new()?;
//!
//! // List available tools (35+ tools across 8 categories)
//! for tool in api.list_tools_by_category(ToolCategory::Gpu) {
//!     println!("{}: {}", tool.name, tool.description);
//! }
//!
//! // Call a tool
//! let result = api.call_tool("get_gpu_status", serde_json::json!({}))?;
//! if result.success {
//!     println!("{}", serde_json::to_string_pretty(&result.data)?);
//! }
//!
//! // Get tools in OpenAI function calling format
//! let openai_tools = api.tools_as_openai_functions();
//!
//! // Get tools in Anthropic tools format
//! let anthropic_tools = api.tools_as_anthropic_tools();
//! # Ok(())
//! # }
//! ```
//!
//! ## Feature Flags
//!
//! - `nvidia` - NVIDIA GPU support via NVML (requires CUDA toolkit or NVIDIA driver)
//! - `amd` - AMD GPU support via sysfs/DRM (Linux only, requires amdgpu driver)
//! - `intel` - Intel GPU support via i915/xe drivers (Linux only)
//! - `cli` - Command-line interface and TUI
//! - `full` - All features enabled
//!
//! ## Platform Support
//!
//! | Platform | CPU | Memory | Disk | GPU (NVIDIA) | GPU (AMD) | GPU (Intel) | Network | Processes |
//! |----------|-----|--------|------|--------------|-----------|-------------|---------|-----------|
//! | Linux    | ✅  | ✅     | ✅   | ✅           | ✅        | ✅          | ✅      | ✅        |
//! | Windows  | ✅  | ✅     | ✅   | ✅           | 🚧        | 🚧          | 🚧      | 🚧        |
//! | macOS    | ✅  | ✅     | ✅   | ❌           | ❌        | ❌          | 🚧      | 🚧        |
//!
//! ## Examples
//!
//! See the `examples/` directory for comprehensive examples:
//!
//! - `gpu_monitor.rs` - Multi-vendor GPU monitoring
//! - `process_monitor.rs` - Process tracking with GPU attribution
//! - `network_monitor.rs` - Network interface statistics
//! - `tui.rs` - Interactive terminal UI
//! - `all_gpus.rs` - Unified multi-vendor GPU example
//! - `ai_api_demo.rs` - AI Data API demonstration with tool-based system access

pub mod audio; // Audio device monitoring and control
pub mod bluetooth; // Bluetooth device monitoring
pub mod display; // Display/monitor information
pub mod usb; // USB device enumeration
pub mod agent; // AI agent for system analysis and predictions
pub mod ai_api; // AI Data API for full system visibility with tool-based access
pub mod ai_workload; // AI training and inference workload monitoring
pub mod bandwidth; // Network bandwidth testing (iperf-style)
pub mod boot_config; // Boot configuration and startup management
pub mod config; // Configuration management with TOML persistence
pub mod connections; // Network connection monitoring (netstat-like)
pub mod consent; // User consent management for ethical data collection
pub mod core;
pub mod cpufreq; // CPU frequency scaling and governor control
pub mod disk; // Disk/storage monitoring
pub mod error;
pub mod fan_control; // Advanced fan monitoring and control
pub mod gpu; // GPU abstraction layer
pub mod health; // System health scoring and alerts
pub mod hwmon; // Hardware monitoring (temperatures, voltages, fans) - native implementation
pub mod memory_management; // Memory and swap management (jetson_stats style)
pub mod motherboard; // Motherboard sensors, BIOS, system information
pub mod network_monitor; // Network interface monitoring
pub mod network_tools; // Network diagnostic tools (ping, traceroute, port scan) - nmap/netcat style
pub mod observability; // Full system observability API with MCP-like permissions for external AI access
pub mod platform;
pub mod power_supply; // Battery and power supply monitoring
pub mod process_monitor; // Unified process monitoring with GPU attribution
pub mod sandbox; // Sandbox and VM detection for ethical data collection
pub mod services; // System service monitoring and control
pub mod silicon; // New: Unified silicon monitoring (CPU, NPU, I/O, network)
pub mod stats;
pub mod system_stats; // System-wide stats (load avg, vmstat, uptime) - Linux/BSD style
pub mod tsdb; // Time-series database for recording metrics
pub mod utils;

// Unified backend for CLI, TUI, and GUI
pub mod backend;

#[cfg(feature = "cli")]
pub mod tui; // Terminal UI

#[cfg(feature = "cli")]
pub use tui::{AcceleratorInfo, AcceleratorType};

#[cfg(feature = "gui")]
pub mod gui; // Graphical UI


// Re-export new hardware monitors
pub use audio::{AudioDevice, AudioDeviceType, AudioMonitor, AudioState};
pub use bluetooth::{BluetoothAdapter, BluetoothDevice, BluetoothDeviceType, BluetoothMonitor, BluetoothState};
pub use display::{DisplayConnection, DisplayInfo, DisplayMonitor, HdrMode};
pub use usb::{UsbClass, UsbDevice, UsbMonitor, UsbSpeed};
// Re-export main types
pub use core::{
    cpu::CpuStats,
    engine::{EngineInfo, EngineStats},
    fan::FanStats,
    gpu::{GpuInfo as CoreGpuInfo, GpuStats, GpuType},
    memory::MemoryStats,
    power::PowerStats,
    process::{ProcessInfo, ProcessStats},
    temperature::TemperatureStats,
};
pub use error::{Error, Result, SimonError};
pub use stats::{Simon, Snapshot};

// Re-export unified GPU interface (legacy)
pub use gpu::{
    Gpu, GpuClocks, GpuCollection, GpuDynamicInfo, GpuEngines, GpuInfo, GpuMemory, GpuPower,
    GpuProcess as LegacyGpuProcess, GpuProcessType, GpuStaticInfo, GpuThermal, GpuVendor,
    PcieLinkInfo,
};

// Re-export new unified GPU traits (preferred)
pub use gpu::traits;
// Note: GpuError conflicts with crate::Error::GpuError variant
pub use gpu::{Clocks, Device, Memory, Power, Temperature, Utilization, Vendor};

// Re-export process monitor
pub use process_monitor::{
    CategoryStats, ProcessCategory, ProcessGpuType, ProcessMonitor, ProcessMonitorInfo,
};

// Re-export network monitor
pub use network_monitor::{NetworkInterfaceInfo, NetworkMonitor};

// Re-export connection monitor (netstat-like)
pub use connections::{ConnectionInfo, ConnectionMonitor, ConnectionState, Protocol};

// Re-export AI workload monitoring
pub use ai_workload::{
    AiFramework, AiWorkload, AiWorkloadMonitor, CloudProvider, DistributedConfig, InferenceMetrics,
    TpuConfig, TrainingMetrics, WorkloadType,
};

// Re-export AI Data API for full system visibility
pub use ai_api::{AiDataApi, ToolCall, ToolCategory, ToolDefinition, ToolResult};

// Re-export Observability API for external AI access with MCP-like permissions
pub use observability::{
    // API types
    ApiResponse, ObservabilityApi, ObservabilityError, RequestContext, ResponseMeta,
    // Context types
    SystemContext, MinimalContext, SystemContextBuilder, HardwareContext, SoftwareContext,
    // Permission types
    ApiConfig, ApiKey, Capability, Permission, PermissionChecker, Scope,
    // Event types
    EventManager, SystemEvent, EventCategory, EventSeverity, EventFilter,
    // Metric types
    MetricCollector, MetricSnapshot, MetricValue, MetricStats,
    // Server types
    ServerConfig, RequestHandler, HttpRequest, HttpResponse, OpenApiSpec,
    // Streaming types
    StreamManager, StreamMessage, StreamChannel, Subscription,
};

// Re-export AI agent
pub use agent::{Agent, AgentConfig, AgentResponse, ModelSize, Query, QueryType, SystemState};

// Re-export configuration management
pub use config::{ChartConfig, Config, GeneralConfig, GpuConfig, ProcessConfig};

// Re-export consent management
pub use consent::{
    is_offline_mode, is_telemetry_disabled, ConsentConfig, ConsentManager, ConsentRecord,
    ConsentScope,
};

// Re-export sandbox detection
pub use sandbox::{SandboxDetector, SandboxInfo};

// Re-export system-wide stats (Linux/BSD style)
pub use system_stats::{CpuTime, LoadAverage, SystemStats, VmStats};

// Re-export network diagnostic tools (nmap/traceroute/ping/tcpdump style)
pub use network_tools::{
    // Tcpdump-style packet capture
    capture_dns,
    capture_http,
    capture_packets,
    capture_tcp,
    // Ping/Traceroute
    check_connectivity,
    check_port,
    // Port scanning
    common_ports,
    dns_lookup,
    // Nmap-style scanning
    full_scan,
    get_service_name,
    grab_banner,
    is_capture_available,
    latency_test,
    list_capture_interfaces,
    nmap_scan,
    parallel_scan,
    parallel_scan_with_banners,
    ping,
    quick_capture,
    quick_scan,
    reverse_dns,
    scan_port_range,
    scan_ports,
    scan_ports_with_timeout,
    traceroute,
    CaptureConfig,
    CaptureProtocol,
    CaptureResult,
    CapturedPacket,
    // Utility
    NetworkTools,
    NmapScanResult,
    OsFingerprint,
    PingResult,
    PortScanResult,
    PortStatus,
    ServiceInfo,
    TracerouteHop,
    TracerouteResult,
};

// Re-export power supply / battery monitoring
pub use power_supply::{
    battery_percent, is_on_ac_power, is_on_battery, power_summary, BatteryHealth, ChargingStatus,
    PowerSupplyInfo, PowerSupplyMonitor, PowerSupplyType,
};

// Re-export system health monitoring
pub use health::{
    has_critical_issues, health_score, quick_health_check, HealthCheck, HealthStatus,
    HealthThresholds, SystemHealth,
};

// Re-export bandwidth testing (iperf-style)
pub use bandwidth::{
    bandwidth_test, loopback_test, memory_bandwidth_test, quick_bandwidth_estimate,
    BandwidthConfig, BandwidthResult, MemoryBandwidthResult, DEFAULT_BUFFER_SIZE, DEFAULT_PORT,
};

// Re-export fan control
pub use fan_control::{
    fan_summary, list_fans, list_thermal_zones, FanControlMode, FanCurve, FanCurvePoint, FanInfo,
    FanMonitor, FanProfile, FanSummary, FanType, ThermalZone, TripPoint,
};

// Re-export CPU frequency scaling
pub use cpufreq::{
    available_governors, cpufreq_summary, list_cpus, CpuFreqInfo, CpuFreqMonitor, CpuFreqPolicy,
    CpuFreqSummary, CpuIdleState, EnergyPreference, Governor, TurboStatus,
};

// Re-export system services monitoring
pub use services::{
    common_services, get_services_status, is_service_running, service_summary,
    ServiceInfo as SystemServiceInfo, ServiceMonitor, ServiceStatus, ServiceSummary, ServiceType,
    StartupType,
};

// Re-export memory management (jetson_stats style)
pub use memory_management::{
    format_bytes, memory_summary, MemoryInfo, MemoryMonitor, MemoryPressure, MemorySummary,
    ProcessMemory, SwapDevice, SwapInfo, SwapType, ZramInfo,
};

// Re-export boot configuration
pub use boot_config::{
    boot_summary, format_uptime, BootInfo, BootMonitor, BootSummary, BootTime, BootType,
    KernelParams, StartupItem, StartupItemStatus, StartupItemType,
};

// Re-export unified backend
pub use backend::{
    AcceleratorState, BackendConfig, CpuState, DiskState, FullSystemState, HistoryBuffer,
    MemoryState, MonitoringBackend, NetworkState, ProcessState, SystemInfoState,
    DEFAULT_HISTORY_SIZE, DEFAULT_UPDATE_INTERVAL,
};

// Re-export time-series database for metrics recording
pub use tsdb::{
    format_size, parse_size, DatabaseStats, MetricSample, MetricsRecorder, ProcessSnapshot,
    SystemSnapshot, TimeSeriesDb,
};

/// Main entry point for unified silicon monitoring
pub struct SiliconMonitor {
    gpus: GpuCollection,
}

impl SiliconMonitor {
    /// Create new silicon monitor with auto-detection
    pub fn new() -> Result<Self> {
        let gpus = GpuCollection::auto_detect()
            .map_err(|e| SimonError::InitializationError(e.to_string()))?;
        Ok(Self { gpus })
    }

    /// Get all GPU information snapshots
    pub fn snapshot_gpus(&self) -> Result<Vec<GpuInfo>> {
        self.gpus
            .snapshot_all()
            .map_err(|e| SimonError::Other(e.to_string()))
    }

    /// Get GPU collection
    pub fn gpus(&self) -> &GpuCollection {
        &self.gpus
    }

    /// Get mutable GPU collection
    pub fn gpus_mut(&mut self) -> &mut GpuCollection {
        &mut self.gpus
    }

    /// Get number of detected GPUs
    pub fn gpu_count(&self) -> usize {
        self.gpus.len()
    }
}

impl Default for SiliconMonitor {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            gpus: GpuCollection::new(),
        })
    }
}

// Backward compatibility alias
pub type GpuInterface = SiliconMonitor;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

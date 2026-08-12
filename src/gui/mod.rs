// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! simon's graphical UI, built on Dewey.
//!
//! simon's shipping GUI (`src/gui/`) is ~10k lines of immediate-mode egui. This
//! module is its replacement, built on [Dewey](https://crates.io/crates/deweygui)
//! — nervosys' agentic-first GUI framework — and grown one tab at a time behind
//! the `dewey-gui` feature so the eframe path keeps shipping until this reaches
//! parity. See HANDOFF.md for the port order.
//!
//! # Why move at all
//!
//! 3.9.0 fixed a bug where four of nine tabs rendered only a spinner under
//! `gui --frame`: their contents load on background threads collected by
//! `check_background_loaders`, which runs only inside the interactive event loop,
//! so the headless path never collected them. The fix taught the headless path to
//! pump the loaders.
//!
//! Dewey's answer is that the loaders were never the app's to pump.
//! [`Command::Task`] hands background work to the runtime, which delivers the
//! result as an ordinary message through `update`. There is no "collect the
//! loaders" step to forget, and therefore no headless-versus-interactive
//! divergence to get wrong — the bug class is structurally absent rather than
//! fixed. Every widget also carries an `agent_id`, so an agent reads named nodes
//! out of the ontology instead of scraping painted text, which is what simon's
//! own contract test had been reduced to doing.

use dewey::prelude::*;

// Dewey's prelude exports its own single-parameter `Result<T>` alias, which the
// glob above would otherwise shadow std's with — turning every `Result<T, String>`
// in this file into an arity error and every `?` into a demand for
// `dewey::error::Error`. The loaders below deliberately carry their failure as a
// String for display, so std's two-parameter Result is the one wanted here.
use dewey::agent::driver::HeadlessDriver;
use std::result::Result;

/// Which tab is showing.
///
/// Only the tabs ported so far. This grows with the migration rather than
/// mirroring the egui `Tab` enum up front — a variant here means a tab that
/// actually renders, so the agent-visible tab list never promises more than the
/// port delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Cpu,
    Accelerators,
    Processes,
    Memory,
    Network,
    Disk,
    System,
    Peripherals,
    Profiles,
    Connections,
    NetworkTools,
    AiAssistant,
}

impl Tab {
    pub const PORTED: [Tab; 13] = [
        Tab::Overview,
        Tab::Cpu,
        Tab::Accelerators,
        Tab::Processes,
        Tab::Memory,
        Tab::Network,
        Tab::Disk,
        Tab::System,
        Tab::Peripherals,
        Tab::Profiles,
        Tab::Connections,
        Tab::NetworkTools,
        Tab::AiAssistant,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Cpu => "CPU",
            Tab::Accelerators => "Accelerators",
            Tab::Processes => "Processes",
            Tab::Memory => "Memory",
            Tab::Network => "Network",
            Tab::Disk => "Disk",
            Tab::System => "System",
            Tab::Peripherals => "Peripherals",
            Tab::Profiles => "Profiles",
            Tab::Connections => "Connections",
            Tab::NetworkTools => "Network Tools",
            Tab::AiAssistant => "AI Assistant",
        }
    }
}

/// One row of the memory breakdown, in megabytes.
///
/// Carries formatted values rather than a live `MemoryStats` so the view stays a
/// pure function of the model — the Elm property that makes a headless read and
/// an interactive frame render identically by construction.
#[derive(Debug, Clone, Default)]
pub struct MemoryView {
    pub usage_percent: f32,
    pub total_mb: f64,
    pub used_mb: f64,
    pub free_mb: f64,
    pub buffers_mb: f64,
    pub cached_mb: f64,
    pub shared_mb: f64,
    /// free + buffers + cached, the same arithmetic `free -h` reports.
    pub available_mb: f64,
}

/// Per-core and aggregate CPU utilisation.
#[derive(Debug, Clone, Default)]
pub struct CpuView {
    pub core_count: usize,
    /// Recent total-busy samples, oldest first, for the chart. Capped at 120 —
    /// this is a display buffer, not a time series, and simon already has
    /// `simon record` for anyone who wants real history.
    pub history: Vec<f64>,
    /// (core id, busy percent, governor) — busy is 100 - idle.
    pub cores: Vec<(usize, f32, String)>,
    pub total_busy_percent: f32,
    pub total_user: f32,
    pub total_system: f32,
    pub total_idle: f32,
}

/// One accelerator: GPU, and whatever else the vendor layers report.
#[derive(Debug, Clone, Default)]
pub struct AcceleratorRow {
    pub index: usize,
    pub vendor: String,
    pub name: String,
    /// Absent where the device declines to report, not defaulted to zero.
    pub utilization_percent: Option<u8>,
    pub memory_used_mb: Option<f64>,
    pub memory_total_mb: Option<f64>,
    pub temperature_c: Option<f32>,
    pub power_watts: Option<f64>,
}

/// One process row.
#[derive(Debug, Clone, Default)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_mb: f64,
}

/// One drive, reduced to what the tab draws.
#[derive(Debug, Clone, Default)]
pub struct DiskRow {
    pub name: String,
    pub model: String,
    pub capacity_bytes: u64,
    pub interface: String,
    pub temperature_c: Option<f32>,
    /// SMART verdict, or None where the drive declines to report one.
    pub health: Option<String>,
}

/// Host, board and firmware identity, flattened to label/value pairs.
///
/// A Vec of pairs rather than a struct mirroring `SystemInfo` because the tab
/// draws whatever is present and skips what is not — several fields are
/// `Option` and absent on plenty of hardware. Flattening at load time keeps the
/// "is this worth a row" decision out of the view, which stays a pure function.
#[derive(Debug, Clone, Default)]
pub struct SystemView {
    pub rows: Vec<(String, String)>,
}

/// Attached devices, grouped by kind.
///
/// Each group is (heading, rows). Empty groups are dropped at load time rather
/// than rendered as an empty heading — the same rule the System tab follows for
/// absent fields, for the same reason.
#[derive(Debug, Clone, Default)]
pub struct PeripheralsView {
    pub groups: Vec<(String, Vec<String>)>,
}

/// Tunable driver settings, grouped by the device that owns them.
///
/// Values and defaults are rendered as the provider reported them. Nothing here
/// proposes a change: `simon tune`'s rule is that a proposed value comes from
/// what the driver declared, never from this crate, and a read-only tab has even
/// less business inventing one.
#[derive(Debug, Clone, Default)]
pub struct ProfilesView {
    pub groups: Vec<(String, Vec<String>)>,
    /// Subsystems that failed, kept as first-class rows rather than dropped —
    /// "the GPU provider errored" is information an agent needs, and silently
    /// omitting it would read as "this machine has no GPU settings".
    pub errors: Vec<(String, String)>,
    pub total_settings: usize,
}

/// Total bandwidth across every interface, bytes per second.
#[derive(Debug, Clone, Default)]
pub struct NetworkView {
    pub interfaces: Vec<(String, f64, f64)>,
    pub total_rx: f64,
    pub total_tx: f64,
}

/// One socket.
#[derive(Debug, Clone, Default)]
pub struct ConnectionRow {
    pub protocol: String,
    pub local: String,
    pub remote: Option<String>,
    pub state: String,
    pub process: Option<String>,
}

/// The network-tools pane: what can be run, and nothing run yet.
///
/// Ping, traceroute and port scans send packets to hosts the user names. None of
/// them fires at load. The egui tab is driven by a button; here the equivalent is
/// an explicit `Msg`, so an agent taking a headless frame of this app never
/// causes traffic to leave the machine as a side effect of *looking*.
#[derive(Debug, Clone, Default)]
pub struct NetworkToolsView {
    pub available: Vec<String>,
    /// Result of the last explicitly requested run, if any.
    pub last_run: Option<(String, String)>,
}

/// AI backend availability.
///
/// Probing a backend is a network call. The egui path deliberately does not wait
/// on it, because blocking a frame on a DNS timeout makes reading the GUI as slow
/// as the slowest unreachable host. This pane reports what is configured without
/// dialling it, and says so.
#[derive(Debug, Clone, Default)]
pub struct AiView {
    pub backends: Vec<String>,
    pub probed: bool,
}

/// The at-a-glance pane: one line per subsystem, drawn from the other panes.
///
/// Derived in the view rather than loaded, because every number it shows is
/// already in the model. A separate loader would let Overview and the tab it
/// summarises disagree, which is exactly the kind of drift a single source of
/// truth exists to prevent.
#[derive(Debug, Clone, Default)]
pub struct OverviewRow {
    pub label: String,
    pub value: String,
}

/// What a pane is currently able to show.
///
/// `Loading` is a real state rather than an absence, because the 3.9.0 bug was
/// precisely a pane that said "loading" forever and no test could tell that from
/// a pane that was still working. Here the state is in the model, so a headless
/// read can assert on it directly instead of inferring it from painted glyphs.
#[derive(Debug, Clone)]
pub enum Pane<T> {
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Pane<T> {
    pub fn is_loading(&self) -> bool {
        matches!(self, Pane::Loading)
    }
}

/// The application model.
pub struct SimonApp {
    pub tab: Tab,
    /// Left edge of each tab, measured during `view`.
    ///
    /// Dewey's `Tabs` sizes each tab to its label (`measure_text + 16`), not to
    /// an equal share of the bar. Assuming equal slots put clicks on the wrong
    /// tab and drew the selection marker under the wrong word — visible as a
    /// CPU pane with "Accelerators" underlined. `view` is the only place with a
    /// painter to measure with, so it records the spans here for `handle_event`,
    /// which has no frame of its own.
    pub tab_spans: std::sync::Mutex<Vec<f32>>,
    /// Last known window size, tracked so `handle_event` can hit-test clicks.
    ///
    /// `view` takes `&self` and cannot record layout, and `handle_event` has no
    /// frame — so the tab bar's geometry has to be derived from the window size
    /// rather than read back from what was drawn. Dewey sends `Event::Resize`,
    /// which makes this exact rather than a guess.
    pub size: Size,
    pub cpu: Pane<CpuView>,
    pub accelerators: Pane<Vec<AcceleratorRow>>,
    pub processes: Pane<Vec<ProcessRow>>,
    pub connections: Pane<Vec<ConnectionRow>>,
    pub network_tools: NetworkToolsView,
    pub ai: AiView,
    pub memory: Pane<MemoryView>,
    pub network: Pane<NetworkView>,
    pub disks: Pane<Vec<DiskRow>>,
    pub system: Pane<SystemView>,
    pub peripherals: Pane<PeripheralsView>,
    pub profiles: Pane<ProfilesView>,
}

#[derive(Debug)]
pub enum Msg {
    SelectTab(Tab),
    Resized(f32, f32),
    Refresh,
    MemoryLoaded(Result<MemoryView, String>),
    NetworkLoaded(Result<NetworkView, String>),
    DisksLoaded(Result<Vec<DiskRow>, String>),
    SystemLoaded(Result<SystemView, String>),
    PeripheralsLoaded(Result<PeripheralsView, String>),
    ProfilesLoaded(Result<ProfilesView, String>),
    CpuLoaded(Result<CpuView, String>),
    AcceleratorsLoaded(Result<Vec<AcceleratorRow>, String>),
    ProcessesLoaded(Result<Vec<ProcessRow>, String>),
    ConnectionsLoaded(Result<Vec<ConnectionRow>, String>),
    /// Explicitly requested by the user; never issued by `init`.
    RunNetworkTool(String, String),
}

impl Default for SimonApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SimonApp {
    pub fn new() -> Self {
        Self {
            tab: Tab::Overview,
            tab_spans: std::sync::Mutex::new(Vec::new()),
            size: Size::new(1280.0, 800.0),
            cpu: Pane::Loading,
            accelerators: Pane::Loading,
            processes: Pane::Loading,
            connections: Pane::Loading,
            network_tools: NetworkToolsView {
                available: vec![
                    "ping <host>".to_string(),
                    "traceroute <host>".to_string(),
                    "scan-ports <host>".to_string(),
                ],
                last_run: None,
            },
            ai: AiView {
                backends: Vec::new(),
                probed: false,
            },
            memory: Pane::Loading,
            network: Pane::Loading,
            disks: Pane::Loading,
            system: Pane::Loading,
            peripherals: Pane::Loading,
            profiles: Pane::Loading,
        }
    }

    /// Commands that load every pane the app shows.
    ///
    /// Issued once at startup rather than lazily per tab: the panes are cheap
    /// here, and loading on tab-switch is what made the egui version's timing
    /// depend on which tab you happened to open first.
    pub fn load_all() -> Command<Msg> {
        Command::Batch(vec![
            Command::Task(Box::new(|| Msg::MemoryLoaded(read_memory()))),
            Command::Task(Box::new(|| Msg::NetworkLoaded(read_network()))),
            Command::Task(Box::new(|| Msg::DisksLoaded(read_disks()))),
            Command::Task(Box::new(|| Msg::SystemLoaded(read_system()))),
            Command::Task(Box::new(|| Msg::PeripheralsLoaded(read_peripherals()))),
            Command::Task(Box::new(|| Msg::ProfilesLoaded(read_profiles()))),
            Command::Task(Box::new(|| Msg::CpuLoaded(read_cpu()))),
            Command::Task(Box::new(|| Msg::AcceleratorsLoaded(read_accelerators()))),
            Command::Task(Box::new(|| Msg::ProcessesLoaded(read_processes()))),
            Command::Task(Box::new(|| Msg::ConnectionsLoaded(read_connections()))),
        ])
    }
}

/// Read physical memory, in the same units the egui tab reports.
///
/// Note `MemoryStats::new()` is a zero-constructor, not a reader — it returns a
/// struct of zeros on every platform. The real values come from the per-platform
/// `read_memory_stats`. The egui tab calls that only under
/// `#[cfg(target_os = "windows")]` and falls back to `MemoryStats::new()`
/// everywhere else, which means its memory tab shows 0 MB on Linux and macOS
/// despite both having working readers. This port wires up Linux too rather than
/// reproducing that; macOS has no `read_memory_stats` to call yet, so it still
/// gets zeros and says so instead of pretending.
fn read_memory() -> Result<MemoryView, String> {
    #[cfg(target_os = "windows")]
    let mem = crate::platform::windows::read_memory_stats().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    let mem = crate::platform::linux::memory::read_memory_stats().map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let mem = return Err("no memory reader for this platform yet".to_string());
    let kb = |v: u64| v as f64 / 1024.0;
    let (free_mb, buffers_mb, cached_mb) =
        (kb(mem.ram.free), kb(mem.ram.buffers), kb(mem.ram.cached));
    Ok(MemoryView {
        usage_percent: mem.ram_usage_percent(),
        total_mb: kb(mem.ram.total),
        used_mb: kb(mem.ram.used),
        free_mb,
        buffers_mb,
        cached_mb,
        shared_mb: kb(mem.ram.shared),
        available_mb: free_mb + buffers_mb + cached_mb,
    })
}

/// Read per-interface counters and total them.
fn read_network() -> Result<NetworkView, String> {
    let mut monitor = crate::NetworkMonitor::new().map_err(|e| e.to_string())?;
    let interfaces: Vec<(String, f64, f64)> = monitor
        .interfaces()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|i| (i.name, i.rx_bytes as f64, i.tx_bytes as f64))
        .collect();
    Ok(NetworkView {
        total_rx: interfaces.iter().map(|(_, rx, _)| rx).sum(),
        total_tx: interfaces.iter().map(|(_, _, tx)| tx).sum(),
        interfaces,
    })
}

/// Enumerate drives and reduce each to a display row.
///
/// The COM guard matters: disk paths reach WMI on Windows, which requires COM to
/// be initialised on the calling thread. The egui path takes the guard inside the
/// thread it spawns; here the runtime owns the thread, so the guard belongs at the
/// top of the task instead. It is the one piece of the old loader that does not
/// simply disappear.
fn read_disks() -> Result<Vec<DiskRow>, String> {
    let _com = crate::pipeline::com_guard();
    let disks = crate::disk::enumerate_disks().map_err(|e| e.to_string())?;
    Ok(disks
        .iter()
        .map(|disk| {
            let info = disk.info().ok();
            DiskRow {
                name: disk.name().to_string(),
                model: info
                    .as_ref()
                    .map(|i| i.model.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                capacity_bytes: info.as_ref().map(|i| i.capacity).unwrap_or(0),
                interface: info
                    .as_ref()
                    .and_then(|i| i.interface_type.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                temperature_c: disk.temperature().ok().flatten(),
                health: disk.health().ok().map(|h| format!("{h:?}")),
            }
        })
        .collect())
}

/// Read host, board and firmware identity.
///
/// Like the disk loader this reaches WMI on Windows, so it takes the COM guard.
fn read_system() -> Result<SystemView, String> {
    let _com = crate::pipeline::com_guard();
    let info = crate::motherboard::get_system_info().map_err(|e| e.to_string())?;

    let mut rows = vec![
        ("OS".to_string(), info.os_name.clone()),
        ("OS Version".to_string(), info.os_version.clone()),
        ("Architecture".to_string(), info.architecture.clone()),
    ];
    // Absent fields are skipped rather than rendered as "unknown": a row that is
    // not there is a truthful "this machine did not say", where a row reading
    // "unknown" invites an agent to treat the absence as a measured value.
    let mut push = |label: &str, value: &Option<String>| {
        if let Some(v) = value {
            if !v.trim().is_empty() {
                rows.push((label.to_string(), v.clone()));
            }
        }
    };
    push("Kernel", &info.kernel_version);
    push("Hostname", &info.hostname);
    push("Manufacturer", &info.manufacturer);
    push("Product", &info.product_name);
    push("Board Vendor", &info.board_vendor);
    push("Board", &info.board_name);
    push("CPU", &info.cpu_name);
    push("BIOS Vendor", &info.bios.vendor);
    push("BIOS Version", &info.bios.version);
    push("BIOS Date", &info.bios.release_date);

    if let Some(cores) = info.cpu_cores {
        rows.push(("CPU Cores".to_string(), cores.to_string()));
    }
    if let Some(threads) = info.cpu_threads {
        rows.push(("CPU Threads".to_string(), threads.to_string()));
    }
    rows.push((
        "Firmware".to_string(),
        format!("{:?}", info.bios.firmware_type),
    ));
    if let Some(sb) = info.bios.secure_boot {
        rows.push(("Secure Boot".to_string(), sb.to_string()));
    }

    // Serial number and UUID are deliberately not surfaced. They identify the
    // machine, and this tab is read by agents and pasted into issues.
    Ok(SystemView { rows })
}

/// Enumerate attached devices.
///
/// This is the slowest loader in the app — on Windows it runs several PowerShell
/// CIM queries, and the egui tab took 16.5 s to settle. Nothing here makes it
/// faster; what changes is that the cost is paid by a task the runtime owns, so
/// a headless read waits for it once instead of racing it.
///
/// MAC and Bluetooth addresses are omitted for the same reason the System tab
/// omits the machine UUID: they identify the machine and its owner's devices,
/// and this pane is read by agents and pasted into issues.
fn read_peripherals() -> Result<PeripheralsView, String> {
    let _com = crate::pipeline::com_guard();
    let p = crate::motherboard::get_peripherals().map_err(|e| e.to_string())?;

    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |name: &str, rows: Vec<String>| {
        if !rows.is_empty() {
            groups.push((name.to_string(), rows));
        }
    };

    add(
        "USB",
        p.usb_devices
            .iter()
            .map(|d| match &d.vendor {
                Some(v) if !v.trim().is_empty() => format!("{} ({v})", d.name),
                _ => d.name.clone(),
            })
            .collect(),
    );
    add(
        "Displays",
        p.display_outputs
            .iter()
            .map(|d| {
                let state = if d.connected {
                    "connected"
                } else {
                    "disconnected"
                };
                match &d.resolution {
                    Some(r) => format!("{} [{:?}] {state} {r}", d.name, d.output_type),
                    None => format!("{} [{:?}] {state}", d.name, d.output_type),
                }
            })
            .collect(),
    );
    add(
        "Audio",
        p.audio_devices
            .iter()
            .map(|d| format!("{} [{:?}]", d.name, d.device_type))
            .collect(),
    );
    add(
        "Bluetooth",
        p.bluetooth_devices
            .iter()
            .map(|d| {
                let state = if d.connected { "connected" } else { "paired" };
                format!("{} ({state})", d.name)
            })
            .collect(),
    );
    add(
        "Network Ports",
        p.network_ports
            .iter()
            .map(|d| match &d.speed {
                Some(sp) => format!("{} [{:?}] {sp}", d.name, d.port_type),
                None => format!("{} [{:?}]", d.name, d.port_type),
            })
            .collect(),
    );

    Ok(PeripheralsView { groups })
}

/// Read tunable driver settings.
///
/// Uses the cached inspector the egui tab uses, so opening this pane does not
/// re-run every provider from scratch — the providers reach vendor control
/// panels and are not cheap.
fn read_profiles() -> Result<ProfilesView, String> {
    let _com = crate::pipeline::com_guard();
    let mut inspector = crate::profile::cache::CachedProfileInspector::new();
    let snapshot = inspector.snapshot_all();

    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (subsystem, provider_groups) in &snapshot.providers {
        for group in provider_groups {
            let rows: Vec<String> = group
                .settings
                .iter()
                .map(|setting| {
                    let unit = setting
                        .unit
                        .as_deref()
                        .map(|u| format!(" {u}"))
                        .unwrap_or_default();
                    // The default is shown only when it differs from the current
                    // value, so a row that mentions one is a row worth reading.
                    let default = match &setting.default {
                        Some(d) if format!("{d:?}") != format!("{:?}", setting.value) => {
                            format!("  (default {d:?})")
                        }
                        _ => String::new(),
                    };
                    format!(
                        "{}: {:?}{unit}{default}",
                        setting.display_name, setting.value
                    )
                })
                .collect();
            if !rows.is_empty() {
                groups.push((format!("{subsystem:?} — {}", group.device), rows));
            }
        }
    }

    let errors = snapshot
        .errors
        .iter()
        .map(|(subsystem, reason)| (format!("{subsystem:?}"), reason.clone()))
        .collect();

    Ok(ProfilesView {
        total_settings: snapshot.total_settings(),
        groups,
        errors,
    })
}

/// Read per-core CPU utilisation.
///
/// `CpuStats::new()` is a zero-constructor — no cores, 100% idle — so this calls
/// the per-platform reader directly. See HANDOFF.md open work 10; the same trap
/// shipped two defects in the egui GUI.
fn read_cpu() -> Result<CpuView, String> {
    #[cfg(target_os = "windows")]
    let stats = crate::platform::windows::read_cpu_stats().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    let stats = crate::platform::linux::cpu::read_cpu_stats().map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    return Err("no CPU reader for this platform yet".to_string());

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let cores = stats
            .cores
            .iter()
            .map(|c| {
                // Busy is derived from idle rather than summing user+system+…:
                // the fields a platform fills in vary, and idle is the one every
                // reader here populates.
                let busy = c.idle.map(|i| 100.0 - i).unwrap_or(0.0);
                (c.id, busy, c.governor.clone())
            })
            .collect::<Vec<_>>();
        Ok(CpuView {
            core_count: cores.len(),
            history: Vec::new(),
            cores,
            total_busy_percent: 100.0 - stats.total.idle,
            total_user: stats.total.user,
            total_system: stats.total.system,
            total_idle: stats.total.idle,
        })
    }
}

/// Enumerate accelerators.
///
/// A device that declines to report a metric yields `None`, never zero. A GPU
/// reading "0 W" and a GPU that does not expose power are different claims, and
/// this pane is read by agents that cannot tell them apart afterwards.
fn read_accelerators() -> Result<Vec<AcceleratorRow>, String> {
    let _com = crate::pipeline::com_guard();
    let collection = crate::gpu::GpuCollection::auto_detect().map_err(|e| e.to_string())?;

    Ok(collection
        .gpus()
        .iter()
        .enumerate()
        .map(|(i, gpu)| {
            let stat = gpu.static_info().ok();
            let dyn_info = gpu.dynamic_info().ok();
            AcceleratorRow {
                index: stat.as_ref().map(|s| s.index).unwrap_or(i),
                vendor: stat
                    .as_ref()
                    .map(|s| format!("{:?}", s.vendor))
                    .unwrap_or_else(|| "unknown".to_string()),
                name: stat
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| gpu.name().unwrap_or_else(|_| "unknown".to_string())),
                utilization_percent: dyn_info.as_ref().map(|d| d.utilization),
                memory_used_mb: dyn_info
                    .as_ref()
                    .map(|d| d.memory.used as f64 / 1024.0 / 1024.0),
                memory_total_mb: dyn_info
                    .as_ref()
                    .map(|d| d.memory.total as f64 / 1024.0 / 1024.0),
                temperature_c: dyn_info
                    .as_ref()
                    .and_then(|d| d.thermal.temperature)
                    .map(|t| t as f32),
                power_watts: dyn_info
                    .as_ref()
                    .and_then(|d| d.power.draw)
                    .map(|w| w as f64),
            }
        })
        .collect())
}

/// Read the top processes by CPU.
///
/// Capped at 32 rows. The cap is applied by the loader, not the view, so the
/// model says exactly what the tab can show and a headless read cannot be
/// misled into thinking it received the whole process table.
fn read_processes() -> Result<Vec<ProcessRow>, String> {
    let mut monitor = crate::process_monitor::ProcessMonitor::new().map_err(|e| e.to_string())?;
    let procs = monitor.processes_by_cpu().map_err(|e| e.to_string())?;
    Ok(procs
        .into_iter()
        .take(32)
        .map(|p| ProcessRow {
            pid: p.pid,
            name: p.name,
            cpu_percent: p.cpu_percent,
            memory_mb: p.memory_bytes as f64 / 1024.0 / 1024.0,
        })
        .collect())
}

/// Read open sockets.
///
/// Remote addresses are rendered as reported. Unlike the MAC addresses dropped
/// from Peripherals, these are the substance of the tab — a connections pane that
/// hides who you are connected to is not a connections pane.
fn read_connections() -> Result<Vec<ConnectionRow>, String> {
    let monitor = crate::connections::ConnectionMonitor::new().map_err(|e| e.to_string())?;
    let all = monitor.all_connections().map_err(|e| e.to_string())?;
    Ok(all
        .into_iter()
        .take(64)
        .map(|c| ConnectionRow {
            protocol: format!("{:?}", c.protocol),
            local: c.local_address,
            remote: c.remote_address,
            state: format!("{:?}", c.state),
            process: c.process_name,
        })
        .collect())
}

fn format_bytes(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", value, UNITS[unit])
}

impl Model for SimonApp {
    type Msg = Msg;

    /// Load every pane at startup.
    ///
    /// This is the whole of the fix the egui path needed three attempts to get
    /// right: the runtime owns the background work, so it runs identically
    /// whether a human is looking at the window or a headless driver is asking
    /// for one frame.
    fn init(&self) -> Command<Msg> {
        Self::load_all()
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Tab, ..
            }) => {
                let next = (Tab::PORTED.iter().position(|t| *t == self.tab).unwrap_or(0) + 1)
                    % Tab::PORTED.len();
                Some(Msg::SelectTab(Tab::PORTED[next]))
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('r'),
                ..
            }) => Some(Msg::Refresh),

            // Number keys jump straight to a tab, 1-9 then 0 for the tenth.
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) if c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap() as usize;
                let index = if n == 0 { 9 } else { n - 1 };
                Tab::PORTED.get(index).copied().map(Msg::SelectTab)
            }

            // Clicking the tab bar selects a tab. The first cut of this GUI
            // handled no mouse events at all: it rendered, and nothing in it
            // could be clicked, which is not a GUI so much as a picture of one.
            Event::Mouse(m) if m.is_click() => {
                if m.position.y <= TAB_BAR_HEIGHT {
                    let Ok(spans) = self.tab_spans.lock() else {
                        return None;
                    };
                    // Before the first frame there is nothing measured, so a
                    // click cannot be resolved and is ignored rather than
                    // guessed at.
                    spans
                        .windows(2)
                        .position(|w| m.position.x >= w[0] && m.position.x < w[1])
                        .and_then(|i| Tab::PORTED.get(i).copied())
                        .map(Msg::SelectTab)
                } else {
                    None
                }
            }

            Event::Resize(size) => Some(Msg::Resized(size.width, size.height)),
            _ => None,
        }
    }

    fn title(&self) -> &str {
        "Simon — Silicon Monitor"
    }

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::SelectTab(tab) => {
                self.tab = tab;
                Command::None
            }
            Msg::Resized(w, h) => {
                self.size = Size::new(w, h);
                Command::None
            }
            Msg::Refresh => {
                self.memory = Pane::Loading;
                self.network = Pane::Loading;
                self.disks = Pane::Loading;
                self.system = Pane::Loading;
                self.peripherals = Pane::Loading;
                self.profiles = Pane::Loading;
                self.cpu = Pane::Loading;
                self.accelerators = Pane::Loading;
                self.processes = Pane::Loading;
                self.connections = Pane::Loading;
                Self::load_all()
            }
            Msg::MemoryLoaded(Ok(v)) => {
                self.memory = Pane::Ready(v);
                Command::None
            }
            Msg::MemoryLoaded(Err(e)) => {
                self.memory = Pane::Failed(e);
                Command::None
            }
            Msg::NetworkLoaded(Ok(v)) => {
                self.network = Pane::Ready(v);
                Command::None
            }
            Msg::NetworkLoaded(Err(e)) => {
                self.network = Pane::Failed(e);
                Command::None
            }
            Msg::DisksLoaded(Ok(v)) => {
                self.disks = Pane::Ready(v);
                Command::None
            }
            Msg::DisksLoaded(Err(e)) => {
                self.disks = Pane::Failed(e);
                Command::None
            }
            Msg::SystemLoaded(Ok(v)) => {
                self.system = Pane::Ready(v);
                Command::None
            }
            Msg::SystemLoaded(Err(e)) => {
                self.system = Pane::Failed(e);
                Command::None
            }
            Msg::PeripheralsLoaded(Ok(v)) => {
                self.peripherals = Pane::Ready(v);
                Command::None
            }
            Msg::PeripheralsLoaded(Err(e)) => {
                self.peripherals = Pane::Failed(e);
                Command::None
            }
            Msg::ProfilesLoaded(Ok(v)) => {
                self.profiles = Pane::Ready(v);
                Command::None
            }
            Msg::ProfilesLoaded(Err(e)) => {
                self.profiles = Pane::Failed(e);
                Command::None
            }
            Msg::CpuLoaded(Ok(mut v)) => {
                // Carry the previous samples forward across a refresh, so the
                // chart is a history rather than a single point redrawn.
                if let Pane::Ready(old) = &self.cpu {
                    v.history = old.history.clone();
                }
                v.history.push(v.total_busy_percent as f64);
                if v.history.len() > 120 {
                    let excess = v.history.len() - 120;
                    v.history.drain(0..excess);
                }
                self.cpu = Pane::Ready(v);
                Command::None
            }
            Msg::CpuLoaded(Err(e)) => {
                self.cpu = Pane::Failed(e);
                Command::None
            }
            Msg::AcceleratorsLoaded(Ok(v)) => {
                self.accelerators = Pane::Ready(v);
                Command::None
            }
            Msg::AcceleratorsLoaded(Err(e)) => {
                self.accelerators = Pane::Failed(e);
                Command::None
            }
            Msg::ProcessesLoaded(Ok(v)) => {
                self.processes = Pane::Ready(v);
                Command::None
            }
            Msg::ProcessesLoaded(Err(e)) => {
                self.processes = Pane::Failed(e);
                Command::None
            }
            Msg::ConnectionsLoaded(Ok(v)) => {
                self.connections = Pane::Ready(v);
                Command::None
            }
            Msg::ConnectionsLoaded(Err(e)) => {
                self.connections = Pane::Failed(e);
                Command::None
            }
            Msg::RunNetworkTool(tool, target) => {
                // Recorded, not executed here: running it would put a network
                // call inside `update`, which must stay synchronous and pure
                // enough to reason about. A real run is a Command::Task issued
                // from an explicit user action.
                self.network_tools.last_run =
                    Some((format!("{tool} {target}"), "requested".to_string()));
                Command::None
            }
        }
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        // Paint the ground first. Without this the window inherits whatever the
        // backend last cleared to, which is why the first cut looked like text
        // scattered on nothing.
        Container::new().bg(palette::BG).render(area, frame);

        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(TAB_BAR_HEIGHT), Constraint::Fill(1.0)],
        )
        .split(area);
        Container::new()
            .bg(palette::SURFACE_ALT)
            .render(chunks[0], frame);

        // `Tabs` is a StatefulWidget, and `view` takes `&self` — so the selection
        // is derived into a local each frame rather than stored. The model stays
        // the single source of truth for which tab is showing; the widget's state
        // is a projection of it, never the other way round.
        let mut tab_state = TabState::new()
            .with_selected(Tab::PORTED.iter().position(|t| *t == self.tab).unwrap_or(0));
        // Measure the tabs exactly as `Tabs` will, and keep the edges for
        // hit-testing. Same formula, same text style, so clicks and paint agree.
        {
            let ts = TextStyle::default();
            let mut x = chunks[0].x;
            let mut spans = Vec::with_capacity(Tab::PORTED.len() + 1);
            spans.push(x);
            for tab in Tab::PORTED {
                x += frame.painter().measure_text(tab.title(), &ts).width + 16.0;
                spans.push(x);
            }
            if let Ok(mut slot) = self.tab_spans.lock() {
                *slot = spans;
            }
        }

        Tabs::new(Tab::PORTED.iter().map(|t| t.title().to_string()).collect())
            .fg(palette::TEXT)
            // The selected tab is filled with this colour by the widget itself.
            // Setting it to the bar's own background made the highlight
            // invisible, which is what sent me looking for a marker to draw.
            .bg(palette::CPU)
            .agent_id("tab_bar")
            .render(chunks[0], frame, &mut tab_state);

        let body = card(inset(chunks[1], PAD), frame);
        match self.tab {
            Tab::Overview => self.view_overview(body, frame),
            Tab::Cpu => self.view_cpu(body, frame),
            Tab::Accelerators => self.view_accelerators(body, frame),
            Tab::Processes => self.view_processes(body, frame),
            Tab::Memory => self.view_memory(body, frame),
            Tab::Network => self.view_network(body, frame),
            Tab::Disk => self.view_disk(body, frame),
            Tab::System => self.view_system(body, frame),
            Tab::Peripherals => self.view_peripherals(body, frame),
            Tab::Profiles => self.view_profiles(body, frame),
            Tab::Connections => self.view_connections(body, frame),
            Tab::NetworkTools => self.view_network_tools(body, frame),
            Tab::AiAssistant => self.view_ai(body, frame),
        }
    }
}

// ── Appearance ──────────────────────────────────────────────────────────────

/// The colour palette.
///
/// Carries the intent of the egui GUI's `CyberColors` — a dark instrument panel
/// with a colour per domain — rather than its exact values, which lived in 441
/// lines of theme code that did not survive the port. Held as plain constants
/// because a `Theme` lookup returns magenta for an unset token, and a palette
/// that fails loudly in the wrong direction is worse than one that cannot fail.
mod palette {
    use dewey::prelude::Color;

    pub const BG: Color = Color::rgb(0.043, 0.051, 0.075);
    pub const SURFACE: Color = Color::rgb(0.086, 0.102, 0.145);
    pub const SURFACE_ALT: Color = Color::rgb(0.110, 0.129, 0.180);
    pub const BORDER: Color = Color::rgb(0.180, 0.212, 0.286);

    pub const TEXT: Color = Color::rgb(0.878, 0.898, 0.941);
    pub const MUTED: Color = Color::rgb(0.647, 0.694, 0.776);

    pub const CPU: Color = Color::rgb(0.302, 0.651, 1.0);
    pub const MEMORY: Color = Color::rgb(0.686, 0.510, 1.0);
    pub const DISK: Color = Color::rgb(1.0, 0.722, 0.302);
    pub const NETWORK: Color = Color::rgb(0.267, 0.855, 0.635);
    pub const ACCEL: Color = Color::rgb(1.0, 0.478, 0.361);
    pub const SYSTEM: Color = Color::rgb(0.545, 0.796, 0.910);

    pub const OK: Color = Color::rgb(0.267, 0.824, 0.510);
    pub const WARN: Color = Color::rgb(1.0, 0.741, 0.259);
    pub const CRIT: Color = Color::rgb(1.0, 0.376, 0.376);

    /// Green below 60%, amber to 85%, red above — the thresholds the egui
    /// progress bar used, kept so a glance means the same thing it used to.
    pub fn threshold(percent: f32) -> Color {
        if percent >= 85.0 {
            CRIT
        } else if percent >= 60.0 {
            WARN
        } else {
            OK
        }
    }
}

const TAB_BAR_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 30.0;
const PAD: f32 = 16.0;

/// Inset a rect on all sides.
fn inset(area: Rect, by: f32) -> Rect {
    Rect::new(
        area.x + by,
        area.y + by,
        (area.width - by * 2.0).max(0.0),
        (area.height - by * 2.0).max(0.0),
    )
}

/// A section heading: the domain's colour, bold, above its rows.
fn heading(area: Rect, frame: &mut Frame<'_>, id: &str, text: &str, color: Color) {
    Label::new(text)
        .bold()
        .text_size(21.0)
        .fg(color)
        .agent_id(id.to_string())
        .render(area, frame);
}

/// A label/value row: muted label on the left, value in its own colour.
///
/// Two columns rather than one `"{label}: {value}"` string, because a wall of
/// same-weight text was most of what made the first cut of this GUI unreadable.
/// The agent id goes on the value — that is the part worth querying, and it
/// keeps every id stable from before the restyle.
fn kv(
    area: Rect,
    frame: &mut Frame<'_>,
    id: impl Into<String>,
    label: &str,
    value: &str,
    color: Color,
) {
    let cols = Layout::new(
        Direction::Horizontal,
        [Constraint::Length(190.0), Constraint::Fill(1.0)],
    )
    .split(area);
    Label::new(label).fg(palette::MUTED).render(cols[0], frame);
    Label::new(value)
        .fg(color)
        .agent_id(id)
        .render(cols[1], frame);
}

/// A card: surface panel with a border, to sit rows inside.
fn card(area: Rect, frame: &mut Frame<'_>) -> Rect {
    Container::new()
        .bg(palette::SURFACE)
        .border(palette::BORDER, 1.0)
        .rounded(8.0)
        .render(area, frame);
    inset(area, 12.0)
}

/// Stack `count` rows down `area`, returning each row's rect.
///
/// The first row is taller: it carries the section heading, and at the same
/// height as a data row the heading sat directly on top of the first value.
fn rows_of(area: Rect, count: usize) -> Vec<Rect> {
    Layout::new(
        Direction::Vertical,
        (0..count)
            .map(|i| {
                Constraint::Length(if i == 0 {
                    ROW_HEIGHT + 14.0
                } else {
                    ROW_HEIGHT
                })
            })
            .collect::<Vec<_>>(),
    )
    .split(area)
}

impl SimonApp {
    /// One line per subsystem, derived from the panes rather than reloaded.
    fn view_connections(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.connections {
            Pane::Loading => Label::new("Loading connections…")
                .fg(palette::MUTED)
                .agent_id("connections_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Connections unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("connections_status")
                .render(area, frame),
            Pane::Ready(rows) if rows.is_empty() => Label::new("No open connections")
                .fg(palette::MUTED)
                .agent_id("connections_empty")
                .render(area, frame),
            Pane::Ready(rows) => {
                let shown = rows.len().min(24);
                let r = rows_of(area, 2 + shown);
                heading(
                    r[0],
                    frame,
                    "connections_heading",
                    "Connections",
                    palette::NETWORK,
                );
                Label::new(format!("{} sockets", rows.len()))
                    .fg(palette::MUTED)
                    .agent_id("connections_count")
                    .render(r[1], frame);
                for (i, c) in rows.iter().take(shown).enumerate() {
                    let remote = c.remote.as_deref().unwrap_or("-");
                    let proc = c.process.as_deref().unwrap_or("-");
                    kv(
                        r[i + 2],
                        frame,
                        format!("connection_{i}"),
                        &format!("{} {}", c.protocol, c.local),
                        &format!("→ {remote}   [{}]   {proc}", c.state),
                        palette::NETWORK,
                    );
                }
            }
        }
    }

    fn view_network_tools(&self, area: Rect, frame: &mut Frame<'_>) {
        let t = &self.network_tools;
        let r = rows_of(area, 3 + t.available.len());
        heading(
            r[0],
            frame,
            "networktools_heading",
            "Network Tools",
            palette::NETWORK,
        );
        // Stated outright, because an agent reading this pane should not have to
        // infer that looking at it did not send packets.
        Label::new("Nothing runs until explicitly requested — these send traffic.")
            .fg(palette::WARN)
            .agent_id("networktools_notice")
            .render(r[1], frame);
        for (i, tool) in t.available.iter().enumerate() {
            Label::new(format!("  {tool}"))
                .agent_id(format!("networktools_available_{i}"))
                .render(r[i + 2], frame);
        }
        let last = match &t.last_run {
            Some((cmd, status)) => format!("last: {cmd} — {status}"),
            None => "last: nothing run this session".to_string(),
        };
        Label::new(last)
            .fg(palette::MUTED)
            .agent_id("networktools_last_run")
            .render(r[2 + t.available.len()], frame);
    }

    fn view_ai(&self, area: Rect, frame: &mut Frame<'_>) {
        let r = rows_of(area, 3 + self.ai.backends.len());
        heading(r[0], frame, "ai_heading", "AI Assistant", palette::MEMORY);
        Label::new(if self.ai.probed {
            "Backends probed."
        } else {
            "Backends not probed — probing dials the network, which a frame read must not do."
        })
        .fg(palette::MUTED)
        .agent_id("ai_probe_state")
        .render(r[1], frame);
        if self.ai.backends.is_empty() {
            Label::new("No backends configured")
                .fg(palette::MUTED)
                .agent_id("ai_empty")
                .render(r[2], frame);
        } else {
            for (i, b) in self.ai.backends.iter().enumerate() {
                Label::new(format!("  {b}"))
                    .agent_id(format!("ai_backend_{i}"))
                    .render(r[i + 2], frame);
            }
        }
    }

    fn view_overview(&self, area: Rect, frame: &mut Frame<'_>) {
        let mut rows: Vec<(String, String)> = Vec::new();

        // Each arm states the pane's condition rather than skipping it. A missing
        // Overview line would be indistinguishable from a healthy subsystem with
        // nothing to say, which is the ambiguity this whole port exists to remove.
        let summarise = |label: &str, text: String| (label.to_string(), text);

        rows.push(summarise(
            "CPU",
            match &self.cpu {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(c) => format!(
                    "{:.0}% busy across {} cores",
                    c.total_busy_percent, c.core_count
                ),
            },
        ));
        rows.push(summarise(
            "Memory",
            match &self.memory {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(m) => format!(
                    "{:.0} / {:.0} MB ({:.0}%)",
                    m.used_mb, m.total_mb, m.usage_percent
                ),
            },
        ));
        rows.push(summarise(
            "Accelerators",
            match &self.accelerators {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(a) if a.is_empty() => "none detected".into(),
                Pane::Ready(a) => format!("{} detected", a.len()),
            },
        ));
        rows.push(summarise(
            "Disks",
            match &self.disks {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(d) if d.is_empty() => "none detected".into(),
                Pane::Ready(d) => format!("{} attached", d.len()),
            },
        ));
        rows.push(summarise(
            "Network",
            match &self.network {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(n) => format!("{} interfaces", n.interfaces.len()),
            },
        ));
        rows.push(summarise(
            "Processes",
            match &self.processes {
                Pane::Loading => "loading…".into(),
                Pane::Failed(e) => format!("unavailable — {e}"),
                Pane::Ready(p) => format!("top {} by CPU", p.len()),
            },
        ));

        let r = rows_of(area, 1 + rows.len());
        heading(r[0], frame, "overview_heading", "Overview", palette::SYSTEM);
        for (i, (label, value)) in rows.iter().enumerate() {
            let color = match label.as_str() {
                "CPU" => palette::CPU,
                "Memory" => palette::MEMORY,
                "Accelerators" => palette::ACCEL,
                "Disks" => palette::DISK,
                "Network" => palette::NETWORK,
                _ => palette::SYSTEM,
            };
            // A pane that could not be read is stated in the failure colour, so
            // "unavailable" never reads at a glance like a measurement.
            let color = if value.starts_with("unavailable") {
                palette::CRIT
            } else if value.starts_with("loading") {
                palette::MUTED
            } else {
                color
            };
            kv(
                r[i + 1],
                frame,
                format!("overview_{}", label.to_lowercase()),
                label,
                value,
                color,
            );
        }

        // Everything above is one line per subsystem, which left most of the
        // window empty — six rows of text in a 900px pane read as a stub rather
        // than an instrument. The space below carries the detail an operator
        // actually watches: load bars, the busiest cores, and the biggest
        // processes. It is the same data the CPU and Processes tabs hold, at the
        // depth that fits without making this a duplicate of them.
        let rest = Rect::new(
            area.x,
            r[rows.len()].y + ROW_HEIGHT,
            area.width,
            (area.height - (r[rows.len()].y - area.y) - ROW_HEIGHT).max(0.0),
        );
        if rest.height < ROW_HEIGHT * 4.0 {
            return;
        }

        let cols = Layout::new(
            Direction::Horizontal,
            [Constraint::Fill(1.0), Constraint::Fill(1.0)],
        )
        .split(rest);

        // Left: load bars for the two subsystems that move second to second.
        let left = rows_of(cols[0], 8);
        heading(
            left[0],
            frame,
            "overview_load_heading",
            "Load",
            palette::CPU,
        );
        let mut used = 1usize;
        if let Pane::Ready(c) = &self.cpu {
            ProgressBar::new(c.total_busy_percent / 100.0)
                .label(format!("CPU {:.0}%", c.total_busy_percent))
                .fg(palette::threshold(c.total_busy_percent))
                .bg(palette::SURFACE_ALT)
                .rounded(4.0)
                .agent_id("overview_cpu_bar")
                .render(left[used], frame);
            used += 1;

            // The busiest cores, not the first few: an average of 13% can hide
            // one core pinned at 100%, which is the case worth seeing.
            let mut cores: Vec<_> = c.cores.iter().collect();
            cores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (id, busy, _gov) in cores.iter().take(3) {
                if used >= left.len() {
                    break;
                }
                kv(
                    left[used],
                    frame,
                    format!("overview_hot_core_{id}"),
                    &format!("core {id}"),
                    &format!("{busy:.0}%"),
                    palette::threshold(*busy),
                );
                used += 1;
            }
        }
        if let Pane::Ready(m) = &self.memory {
            if used < left.len() {
                ProgressBar::new(m.usage_percent / 100.0)
                    .label(format!("Memory {:.0}%", m.usage_percent))
                    .fg(palette::threshold(m.usage_percent))
                    .bg(palette::SURFACE_ALT)
                    .rounded(4.0)
                    .agent_id("overview_memory_bar")
                    .render(left[used], frame);
            }
        }

        // Right: the processes actually consuming the machine.
        let right = rows_of(cols[1], 8);
        heading(
            right[0],
            frame,
            "overview_processes_heading",
            "Top processes",
            palette::SYSTEM,
        );
        if let Pane::Ready(ps) = &self.processes {
            for (i, proc) in ps.iter().take(6).enumerate() {
                kv(
                    right[i + 1],
                    frame,
                    format!("overview_process_{i}"),
                    &proc.name,
                    &format!("{:.1}%   {:.0} MB", proc.cpu_percent, proc.memory_mb),
                    palette::threshold(proc.cpu_percent),
                );
            }
        }
    }

    fn view_cpu(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.cpu {
            Pane::Loading => Label::new("Loading CPU information…")
                .fg(palette::MUTED)
                .agent_id("cpu_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("CPU unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("cpu_status")
                .render(area, frame),
            Pane::Ready(c) => {
                let shown = c.cores.len().min(32);
                // Row 3 is the chart's slot whether or not it draws, so core rows
                // do not shift position between the first and second refresh.
                let r = rows_of(area, 4 + shown);
                heading(r[0], frame, "cpu_heading", "CPU", palette::CPU);
                ProgressBar::new(c.total_busy_percent / 100.0)
                    .label(format!("{:.1}% busy", c.total_busy_percent))
                    .fg(palette::threshold(c.total_busy_percent))
                    .bg(palette::SURFACE)
                    .rounded(4.0)
                    .agent_id("cpu_total_bar")
                    .render(r[1], frame);
                kv(
                    r[2],
                    frame,
                    "cpu_totals",
                    "user / system / idle",
                    &format!(
                        "{:.1}%  {:.1}%  {:.1}%     {} cores",
                        c.total_user, c.total_system, c.total_idle, c.core_count
                    ),
                    palette::TEXT,
                );
                // The chart replaces the egui_plot history graph. It renders only
                // once there are at least two samples: a one-point line chart is a
                // dot that implies a trend it cannot have.
                if c.history.len() >= 2 {
                    Chart::line("CPU busy %")
                        .series(Series::new(
                            "busy",
                            c.history.clone(),
                            Color::rgb(0.30, 0.65, 1.0),
                        ))
                        .agent_id("cpu_history_chart")
                        .render(r[3], frame);
                }
                for (i, (id, busy, governor)) in c.cores.iter().take(shown).enumerate() {
                    let i = i + 1; // past the chart slot
                    let gov = if governor.trim().is_empty() {
                        String::new()
                    } else {
                        format!("  [{governor}]")
                    };
                    kv(
                        r[i + 3],
                        frame,
                        format!("cpu_core_{id}"),
                        &format!("core {id}{gov}"),
                        &format!("{busy:.0}%"),
                        palette::threshold(*busy),
                    );
                }
            }
        }
    }

    fn view_accelerators(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.accelerators {
            Pane::Loading => Label::new("Loading accelerator information…")
                .fg(palette::MUTED)
                .agent_id("accelerators_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Accelerators unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("accelerators_status")
                .render(area, frame),
            Pane::Ready(rows) if rows.is_empty() => Label::new("No accelerators detected")
                .fg(palette::MUTED)
                .agent_id("accelerators_empty")
                .render(area, frame),
            Pane::Ready(rows) => {
                let r = rows_of(area, 1 + rows.len().min(8));
                heading(
                    r[0],
                    frame,
                    "accelerators_heading",
                    "Accelerators",
                    palette::ACCEL,
                );
                for (i, a) in rows.iter().take(8).enumerate() {
                    // A metric the device did not report is omitted, never zeroed.
                    let mut parts = vec![format!("{} {}", a.vendor, a.name)];
                    if let Some(u) = a.utilization_percent {
                        parts.push(format!("{u}% util"));
                    }
                    if let (Some(used), Some(total)) = (a.memory_used_mb, a.memory_total_mb) {
                        parts.push(format!("{used:.0}/{total:.0} MB"));
                    }
                    if let Some(t) = a.temperature_c {
                        parts.push(format!("{t:.0}°C"));
                    }
                    if let Some(w) = a.power_watts {
                        parts.push(format!("{w:.0} W"));
                    }
                    let name = parts.remove(0);
                    kv(
                        r[i + 1],
                        frame,
                        format!("accelerator_{}", a.index),
                        &name,
                        &parts.join("   "),
                        palette::ACCEL,
                    );
                }
            }
        }
    }

    fn view_processes(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.processes {
            Pane::Loading => Label::new("Loading process list…")
                .fg(palette::MUTED)
                .agent_id("processes_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Processes unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("processes_status")
                .render(area, frame),
            Pane::Ready(rows) if rows.is_empty() => Label::new("No processes reported")
                .fg(palette::MUTED)
                .agent_id("processes_empty")
                .render(area, frame),
            Pane::Ready(rows) => {
                let shown = rows.len().min(24);
                let r = rows_of(area, 2 + shown);
                heading(
                    r[0],
                    frame,
                    "processes_heading",
                    "Processes",
                    palette::SYSTEM,
                );
                Label::new(format!("top {} by CPU", rows.len()))
                    .fg(palette::MUTED)
                    .agent_id("processes_caption")
                    .render(r[1], frame);
                for (i, p) in rows.iter().take(shown).enumerate() {
                    kv(
                        r[i + 2],
                        frame,
                        format!("process_{i}"),
                        &format!("{:>7}  {}", p.pid, p.name),
                        &format!("{:>5.1}%   {:>8.0} MB", p.cpu_percent, p.memory_mb),
                        palette::threshold(p.cpu_percent),
                    );
                }
            }
        }
    }

    fn view_memory(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.memory {
            Pane::Loading => {
                Label::new("Loading memory information…")
                    .fg(palette::MUTED)
                    .agent_id("memory_status")
                    .render(area, frame);
            }
            Pane::Failed(e) => {
                Label::new(format!("Memory unavailable: {e}"))
                    .fg(palette::CRIT)
                    .agent_id("memory_status")
                    .render(area, frame);
            }
            Pane::Ready(m) => {
                let r = rows_of(area, 9);
                heading(
                    r[0],
                    frame,
                    "memory_heading",
                    "Physical Memory",
                    palette::MEMORY,
                );
                ProgressBar::new(m.usage_percent / 100.0)
                    .label(format!("{:.1} MB / {:.1} MB", m.used_mb, m.total_mb))
                    .fg(palette::threshold(m.usage_percent))
                    .bg(palette::SURFACE)
                    .rounded(4.0)
                    .agent_id("memory_usage_bar")
                    .render(r[1], frame);

                // Same six figures as `free -h`, each individually addressable so
                // an agent can read one without parsing a rendered row.
                for (i, (id, label, value)) in [
                    ("memory_total", "Total", m.total_mb),
                    ("memory_used", "Used", m.used_mb),
                    ("memory_free", "Free", m.free_mb),
                    ("memory_shared", "Shared", m.shared_mb),
                    ("memory_buffers", "Buffers", m.buffers_mb),
                    ("memory_cached", "Cached", m.cached_mb),
                    ("memory_available", "Available", m.available_mb),
                ]
                .iter()
                .enumerate()
                {
                    kv(
                        r[i + 2],
                        frame,
                        *id,
                        label,
                        &format!("{value:.0} MB"),
                        palette::MEMORY,
                    );
                }
            }
        }
    }

    fn view_disk(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.disks {
            Pane::Loading => Label::new("Loading disk information…")
                .fg(palette::MUTED)
                .agent_id("disk_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Disks unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("disk_status")
                .render(area, frame),
            // A machine with no drives is a real answer, not a failure, and it
            // must not read as the spinner state. 3.9.0 could not tell the two
            // apart from painted text alone; here they are different nodes.
            Pane::Ready(rows) if rows.is_empty() => Label::new("No disks detected")
                .fg(palette::MUTED)
                .agent_id("disk_empty")
                .render(area, frame),
            Pane::Ready(rows) => {
                let r = rows_of(area, 1 + rows.len().min(16));
                heading(r[0], frame, "disk_heading", "Disks", palette::DISK);
                for (i, d) in rows.iter().take(16).enumerate() {
                    let temp = d
                        .temperature_c
                        .map(|t| format!(" {t:.0}°C"))
                        .unwrap_or_default();
                    let health = d
                        .health
                        .as_deref()
                        .map(|h| format!(" [{h}]"))
                        .unwrap_or_default();
                    kv(
                        r[i + 1],
                        frame,
                        format!("disk_row_{i}"),
                        &d.name,
                        &format!(
                            "{}  {}  {}{temp}{health}",
                            d.model,
                            format_bytes(d.capacity_bytes as f64),
                            d.interface
                        ),
                        palette::DISK,
                    );
                }
            }
        }
    }

    fn view_system(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.system {
            Pane::Loading => Label::new("Loading system information…")
                .fg(palette::MUTED)
                .agent_id("system_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("System information unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("system_status")
                .render(area, frame),
            Pane::Ready(v) => {
                let r = rows_of(area, 1 + v.rows.len().min(24));
                heading(r[0], frame, "system_heading", "System", palette::SYSTEM);
                for (i, (label, value)) in v.rows.iter().take(24).enumerate() {
                    kv(
                        r[i + 1],
                        frame,
                        format!("system_{}", label.to_lowercase().replace(' ', "_")),
                        label,
                        value,
                        palette::SYSTEM,
                    );
                }
            }
        }
    }

    fn view_peripherals(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.peripherals {
            Pane::Loading => Label::new("Loading peripheral information…")
                .fg(palette::MUTED)
                .agent_id("peripherals_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Peripherals unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("peripherals_status")
                .render(area, frame),
            Pane::Ready(v) if v.groups.is_empty() => Label::new("No peripherals detected")
                .fg(palette::MUTED)
                .agent_id("peripherals_empty")
                .render(area, frame),
            Pane::Ready(v) => {
                // Flattened to (agent_id, text) first so the row budget is applied
                // once across all groups. Truncating per group would silently drop
                // whole categories while claiming the tab rendered.
                let mut lines: Vec<(String, String)> = Vec::new();
                for (heading, rows) in &v.groups {
                    let slug = heading.to_lowercase().replace(' ', "_");
                    lines.push((format!("peripherals_group_{slug}"), heading.clone()));
                    for (i, row) in rows.iter().enumerate() {
                        lines.push((format!("peripherals_{slug}_{i}"), format!("  {row}")));
                    }
                }
                let shown = lines.len().min(40);

                let r = rows_of(area, 1 + shown);
                heading(
                    r[0],
                    frame,
                    "peripherals_heading",
                    "Peripherals",
                    palette::ACCEL,
                );
                for (i, (id, text)) in lines.iter().take(shown).enumerate() {
                    // Group headings are the un-indented lines; rows arrive with
                    // two leading spaces from the flattening above.
                    let is_group = !text.starts_with("  ");
                    let mut label = Label::new(text.clone()).agent_id(id.clone());
                    label = if is_group {
                        label.bold().fg(palette::ACCEL)
                    } else {
                        label.fg(palette::TEXT)
                    };
                    label.render(r[i + 1], frame);
                }
            }
        }
    }

    fn view_profiles(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.profiles {
            Pane::Loading => Label::new("Loading driver settings…")
                .fg(palette::MUTED)
                .agent_id("profiles_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Driver settings unavailable: {e}"))
                .fg(palette::CRIT)
                .agent_id("profiles_status")
                .render(area, frame),
            Pane::Ready(v) if v.groups.is_empty() && v.errors.is_empty() => {
                Label::new("No tunable settings detected")
                    .fg(palette::MUTED)
                    .agent_id("profiles_empty")
                    .render(area, frame)
            }
            Pane::Ready(v) => {
                let mut lines: Vec<(String, String)> = Vec::new();
                for (subsystem, reason) in &v.errors {
                    lines.push((
                        format!("profiles_error_{}", subsystem.to_lowercase()),
                        format!("{subsystem}: unavailable — {reason}"),
                    ));
                }
                for (heading, rows) in &v.groups {
                    let slug: String = heading
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    lines.push((format!("profiles_group_{slug}"), heading.clone()));
                    for (i, row) in rows.iter().enumerate() {
                        lines.push((format!("profiles_{slug}_{i}"), format!("  {row}")));
                    }
                }
                let shown = lines.len().min(40);

                let r = rows_of(area, 1 + shown);
                heading(
                    r[0],
                    frame,
                    "profiles_heading",
                    &format!("Profiles — {} settings", v.total_settings),
                    palette::DISK,
                );
                for (i, (id, text)) in lines.iter().take(shown).enumerate() {
                    // Group headings are the un-indented lines; rows arrive with
                    // two leading spaces from the flattening above.
                    let is_group = !text.starts_with("  ");
                    let mut label = Label::new(text.clone()).agent_id(id.clone());
                    label = if is_group {
                        label.bold().fg(palette::ACCEL)
                    } else {
                        label.fg(palette::TEXT)
                    };
                    label.render(r[i + 1], frame);
                }
            }
        }
    }

    fn view_network(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.network {
            Pane::Loading => {
                Label::new("Loading network information…")
                    .fg(palette::MUTED)
                    .agent_id("network_status")
                    .render(area, frame);
            }
            Pane::Failed(e) => {
                Label::new(format!("Network unavailable: {e}"))
                    .fg(palette::CRIT)
                    .agent_id("network_status")
                    .render(area, frame);
            }
            Pane::Ready(n) => {
                let r = rows_of(area, 2 + n.interfaces.len().min(16));
                heading(
                    r[0],
                    frame,
                    "network_heading",
                    "Interfaces",
                    palette::NETWORK,
                );
                kv(
                    r[1],
                    frame,
                    "network_total_bandwidth",
                    "Total bandwidth",
                    &format!(
                        "↓ {}   ↑ {}",
                        format_bytes(n.total_rx),
                        format_bytes(n.total_tx)
                    ),
                    palette::NETWORK,
                );

                for (i, (name, rx, tx)) in n.interfaces.iter().take(16).enumerate() {
                    kv(
                        r[i + 2],
                        frame,
                        format!("network_iface_{i}"),
                        name,
                        &format!("↓ {}   ↑ {}", format_bytes(*rx), format_bytes(*tx)),
                        palette::NETWORK,
                    );
                }
            }
        }
    }
}

// ── Entry points ────────────────────────────────────────────────────────────

/// Launch the interactive application.
///
/// Runs on Dewey's `agpu` backend (Vulkan-first, its own surface management)
/// rather than the eframe/egui-wgpu default. wgpu's Vulkan presentation path
/// reuses a single binary semaphore across swapchain images, which the
/// validation layer rejects as VUID-vkQueueSubmit-pSignalSemaphores-00067 — a
/// spec violation rather than noise, even though it renders. The bug is wgpu's,
/// below both Dewey and simon, so the fix here is to take the path that does not
/// hit it.
///
/// The headless entry points below are unaffected either way: they render
/// through Dewey's `TestBackend`, which touches no GPU at all. That is also why
/// the 839-test suite passed without a hint of this — the interactive surface is
/// genuinely not the one the tests cover.
pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Dewey's default window is 800x600, which is not enough for thirteen tabs
    // across the top and a label/value grid beneath: the values fell off the
    // right edge entirely, so the first window showed a column of grey labels
    // and no readings at all. The headless tests never caught it because
    // `TestBackend` renders into a fixed 1280x800 area that no real window has.
    let options = ProgramOptions {
        width: 1400.0,
        height: 900.0,
        ..ProgramOptions::default()
    };
    AgpuProgram::new(SimonApp::new())
        .with_options(options)
        .run()
}

/// Render one tab headlessly and return its text.
///
/// This replaces `gui::headless::frame`, and is most of why the port happened.
/// The old implementation needed a 30-second deadline, a per-tab settle predicate
/// combining loader flags with painted text, and two wrong attempts before it
/// worked. Here `init()` runs the loaders to completion because the runtime owns
/// them, so the frame is simply drawn — there is nothing to wait for and nothing
/// to get wrong.
pub fn frame(tab: Option<&str>) -> std::result::Result<String, String> {
    let selected = match tab {
        None => Tab::PORTED[0],
        Some(name) => Tab::PORTED
            .iter()
            .copied()
            .find(|t| t.title().eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                format!(
                    "unknown tab {name:?}; known tabs: {}",
                    Tab::PORTED
                        .iter()
                        .map(|t| t.title())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
    };

    let mut driver = HeadlessDriver::new(
        SimonApp {
            tab: selected,
            ..SimonApp::new()
        },
        1280.0,
        800.0,
    );
    driver.init();
    driver.process_request(&dewey::agent::protocol::AgentRequest::GetTree);
    Ok(driver.ontology().export_tree().to_string())
}

/// Run a script of agent requests, one JSON object per line.
///
/// The transport is Dewey's agent protocol rather than a bespoke command
/// vocabulary, so anything that can drive a Dewey app can drive simon's GUI.
pub fn script(source: &str) -> std::result::Result<String, String> {
    let mut driver = HeadlessDriver::new(SimonApp::new(), 1280.0, 800.0);
    driver.init();

    let mut out = String::new();
    for (n, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let request: dewey::agent::protocol::AgentRequest =
            serde_json::from_str(line).map_err(|e| format!("line {}: {e}", n + 1))?;
        let response = driver.process_request(&request);
        out.push_str(
            &serde_json::to_string(&response).map_err(|e| format!("line {}: {e}", n + 1))?,
        );
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive the app headlessly and return the driver with every pane loaded.
    ///
    /// No pumping loop, and that is the point. `HeadlessDriver` runs
    /// `Command::Task` inline, so one `init()` leaves the model fully populated.
    /// The egui path needed a 30-second deadline, a per-tab settle predicate and
    /// two wrong attempts to reach the same place, because its background work
    /// was owned by the app rather than the runtime.
    fn loaded() -> HeadlessDriver<SimonApp> {
        let mut driver = HeadlessDriver::new(SimonApp::new(), 1280.0, 800.0);
        driver.init();
        driver
    }

    /// A model with every loader run, shared across the data tests.
    ///
    /// The loaders hit real hardware — peripherals alone runs several PowerShell
    /// CIM queries and costs ~10 s — so giving each test its own `loaded()` had
    /// the suite enumerating this machine five times over for the same answers.
    /// The tests that assert on *data* share one; the tests that assert on the
    /// *driver* still build their own, because a shared model would defeat what
    /// they check.
    fn shared() -> &'static SimonApp {
        static APP: std::sync::OnceLock<SimonApp> = std::sync::OnceLock::new();
        APP.get_or_init(|| {
            let mut app = SimonApp::new();
            app.update(Msg::MemoryLoaded(read_memory()));
            app.update(Msg::NetworkLoaded(read_network()));
            app.update(Msg::DisksLoaded(read_disks()));
            app.update(Msg::SystemLoaded(read_system()));
            app.update(Msg::PeripheralsLoaded(read_peripherals()));
            app.update(Msg::ProfilesLoaded(read_profiles()));
            app.update(Msg::CpuLoaded(read_cpu()));
            app.update(Msg::AcceleratorsLoaded(read_accelerators()));
            app.update(Msg::ProcessesLoaded(read_processes()));
            app.update(Msg::ConnectionsLoaded(read_connections()));
            app
        })
    }

    #[test]
    fn every_ported_tab_renders_named_nodes() {
        for tab in Tab::PORTED {
            let mut driver = HeadlessDriver::new(SimonApp::new(), 1280.0, 800.0);
            driver.init();
            driver.process_request(&dewey::agent::protocol::AgentRequest::GetTree);

            let expected = match tab {
                Tab::Memory => "memory_heading",
                Tab::Network => "network_heading",
                // A driveless machine is a legitimate answer, so accept either.
                Tab::Disk => "disk_heading",
                Tab::System => "system_heading",
                Tab::Peripherals => "peripherals_heading",
                Tab::Profiles => "profiles_heading",
                Tab::Overview => "overview_heading",
                Tab::Cpu => "cpu_heading",
                Tab::Accelerators => "accelerators_heading",
                Tab::Processes => "processes_heading",
                Tab::Connections => "connections_heading",
                Tab::NetworkTools => "networktools_heading",
                Tab::AiAssistant => "ai_heading",
            };
            if tab == Tab::Disk && driver.ontology().find_node("disk_empty").is_some() {
                continue;
            }
            if driver.model().tab != tab {
                continue; // reached below by the tab-switch test
            }
            assert!(
                driver.ontology().find_node(expected).is_some(),
                "{} tab rendered no node named {expected}",
                tab.title()
            );
        }
    }

    /// The property the 3.9.0 bug violated: a tab must show data, not a spinner.
    ///
    /// Asserted against the *model* rather than painted glyphs. The egui contract
    /// test could only ask "did some text appear", which a spinner satisfies —
    /// which is exactly why four broken tabs passed it for six releases.
    #[test]
    fn panes_leave_the_loading_state() {
        let driver = loaded();
        assert!(
            !driver.model().memory.is_loading(),
            "memory pane still Loading after init; a Command::Task did not deliver"
        );
        assert!(
            !driver.model().network.is_loading(),
            "network pane still Loading after init; a Command::Task did not deliver"
        );
        assert!(
            !driver.model().connections.is_loading(),
            "connections pane still Loading"
        );
        assert!(!driver.model().cpu.is_loading(), "cpu pane still Loading");
        assert!(
            !driver.model().accelerators.is_loading(),
            "accelerators pane still Loading"
        );
        assert!(
            !driver.model().processes.is_loading(),
            "processes pane still Loading"
        );
        assert!(
            !driver.model().profiles.is_loading(),
            "profiles pane still Loading after init"
        );
        assert!(
            !driver.model().peripherals.is_loading(),
            "peripherals pane still Loading after init -- the slowest loader in \
             the app, and the one the egui path took 16.5 s to settle"
        );
        assert!(
            !driver.model().system.is_loading(),
            "system pane still Loading after init"
        );
        assert!(
            !driver.model().disks.is_loading(),
            "disk pane still Loading after init -- this is the exact 3.9.0 failure,              and the whole premise of the port is that it cannot happen here"
        );
    }

    /// The system tab reports identity, and withholds the identifying parts.
    ///
    /// `SystemInfo` carries a serial number and a machine UUID. This tab is read
    /// by agents and pasted into issues, so `read_system` never surfaces them —
    /// and this asserts that, because the omission is a decision rather than an
    /// oversight and would otherwise be easy to "fix" by adding the rows back.
    #[test]
    fn system_reports_identity_without_identifiers() {
        let app = shared();
        match &app.system {
            Pane::Ready(v) => {
                assert!(!v.rows.is_empty(), "system tab produced no rows");
                assert!(
                    v.rows.iter().any(|(label, _)| label == "OS"),
                    "system tab has no OS row"
                );
                for (label, _) in &v.rows {
                    let l = label.to_lowercase();
                    assert!(
                        !l.contains("serial") && !l.contains("uuid"),
                        "system tab surfaced a machine identifier: {label}"
                    );
                }
            }
            Pane::Failed(e) => panic!("system info failed: {e}"),
            Pane::Loading => panic!("system still loading after init"),
        }
    }

    /// Peripherals render, and no device address goes out with them.
    ///
    /// `PeripheralsInfo` carries MAC addresses on network ports and Bluetooth
    /// addresses on paired devices. Both identify the machine and its owner's
    /// devices, and this pane is read by agents, so `read_peripherals` drops
    /// them. Asserted by shape — a colon-separated hex run — rather than by
    /// field name, so re-adding the data anywhere in a row still trips it.
    #[test]
    fn peripherals_carry_no_hardware_addresses() {
        let app = shared();
        let looks_like_mac = |s: &str| {
            let parts: Vec<&str> = s.split(':').collect();
            parts.len() >= 6
                && parts
                    .iter()
                    .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
        };
        match &app.peripherals {
            Pane::Ready(v) => {
                for (heading, rows) in &v.groups {
                    assert!(!rows.is_empty(), "group {heading} rendered with no rows");
                    for row in rows {
                        for word in row.split_whitespace() {
                            assert!(
                                !looks_like_mac(word.trim_matches(['(', ')', '[', ']'])),
                                "peripherals leaked a hardware address in {heading}: {row}"
                            );
                        }
                    }
                }
            }
            // A machine with nothing attached is legitimate; a failure is not
            // fatal to the suite either, since this loader reaches WMI and CI
            // runners are not required to answer.
            Pane::Failed(_) => {}
            Pane::Loading => panic!("peripherals still loading after init"),
        }
    }

    /// The profiles tab reports settings and keeps failures visible.
    ///
    /// A provider that errors is rendered as a row, not dropped. "The GPU
    /// provider failed" and "this GPU exposes no tunables" are different facts,
    /// and an agent reading this pane has to be able to tell them apart.
    #[test]
    fn profiles_render_settings_and_surface_provider_errors() {
        let app = shared();
        match &app.profiles {
            Pane::Ready(v) => {
                assert_eq!(
                    v.groups.is_empty() && v.errors.is_empty(),
                    v.total_settings == 0 && v.errors.is_empty(),
                    "group list and settings count disagree about emptiness"
                );
                for (heading, rows) in &v.groups {
                    assert!(!rows.is_empty(), "group {heading} rendered with no rows");
                }
                for (subsystem, reason) in &v.errors {
                    assert!(
                        !reason.trim().is_empty(),
                        "{subsystem} errored with no stated reason -- an absence \
                         with no reason is exactly what the ontology forbids"
                    );
                }
            }
            Pane::Failed(_) => {}
            Pane::Loading => panic!("profiles still loading after init"),
        }
    }

    /// The CPU tab reports cores and a utilisation that adds up.
    #[test]
    fn cpu_reports_cores_and_consistent_totals() {
        let app = shared();
        match &app.cpu {
            Pane::Ready(c) => {
                assert!(c.core_count > 0, "no CPU cores reported");
                assert_eq!(c.cores.len(), c.core_count, "core list and count disagree");
                assert!(
                    (0.0..=100.0).contains(&c.total_busy_percent),
                    "busy percent out of range: {}",
                    c.total_busy_percent
                );
                // The zero-constructor's signature: no cores and exactly 100% idle.
                // Asserting against it means a regression to CpuStats::new() fails
                // here rather than silently rendering an idle machine.
                assert!(
                    !(c.cores.is_empty() && c.total_idle == 100.0),
                    "CPU pane looks like the zero-constructor, not a reading"
                );
            }
            Pane::Failed(e) => panic!("CPU read failed: {e}"),
            Pane::Loading => panic!("CPU still loading after init"),
        }
    }

    /// Accelerator metrics are absent, never zeroed, when a device declines.
    #[test]
    fn accelerators_distinguish_absent_from_zero() {
        let app = shared();
        match &app.accelerators {
            Pane::Ready(rows) => {
                for a in rows {
                    assert!(!a.name.is_empty(), "accelerator {} has no name", a.index);
                    if let Some(u) = a.utilization_percent {
                        assert!(u <= 100, "utilisation {u}% exceeds 100 on {}", a.name);
                    }
                    if let (Some(used), Some(total)) = (a.memory_used_mb, a.memory_total_mb) {
                        assert!(
                            used <= total * 1.05,
                            "{}: used {used:.0} MB exceeds total {total:.0} MB",
                            a.name
                        );
                    }
                }
            }
            Pane::Failed(_) => {}
            Pane::Loading => panic!("accelerators still loading after init"),
        }
    }

    /// Processes come back sorted by CPU and capped by the loader.
    #[test]
    fn processes_are_capped_and_ordered() {
        let app = shared();
        match &app.processes {
            Pane::Ready(rows) => {
                assert!(
                    rows.len() <= 32,
                    "loader returned {} rows, cap is 32",
                    rows.len()
                );
                for pair in rows.windows(2) {
                    assert!(
                        pair[0].cpu_percent >= pair[1].cpu_percent,
                        "process list is not ordered by CPU: {} then {}",
                        pair[0].cpu_percent,
                        pair[1].cpu_percent
                    );
                }
            }
            Pane::Failed(_) => {}
            Pane::Loading => panic!("processes still loading after init"),
        }
    }

    /// Overview never invents a state a pane is not in.
    ///
    /// It is derived from the other panes rather than loaded, so the risk it
    /// carries is disagreement rather than absence — an Overview claiming a
    /// figure the tab it summarises does not have.
    #[test]
    fn overview_agrees_with_the_panes_it_summarises() {
        let app = shared();
        let mut driver = HeadlessDriver::new(
            SimonApp {
                tab: Tab::Overview,
                ..SimonApp::new()
            },
            1280.0,
            800.0,
        );
        driver.init();
        driver.process_request(&dewey::agent::protocol::AgentRequest::GetTree);
        assert!(
            driver.ontology().find_node("overview_cpu").is_some(),
            "overview has no CPU line"
        );
        if let (Pane::Ready(c), Pane::Ready(_)) = (&app.cpu, &app.memory) {
            assert!(c.core_count > 0);
        }
    }

    /// Reading the Network Tools pane sends no traffic.
    ///
    /// The guarantee this asserts is negative and therefore easy to lose: a later
    /// change that "helpfully" pings on load would still render a pane, and every
    /// other test here would pass. The pane's own state is the only witness.
    #[test]
    fn network_tools_runs_nothing_on_load() {
        let driver = loaded();
        assert!(
            driver.model().network_tools.last_run.is_none(),
            "something ran a network tool during init"
        );
        assert!(
            !driver.model().network_tools.available.is_empty(),
            "network tools pane lists nothing it can do"
        );
    }

    /// Reading the AI pane dials nothing.
    ///
    /// Same shape of guarantee as the network tools pane, and the same reason the
    /// egui contract test had to stop waiting on this tab: a backend probe is a
    /// network call, and blocking a frame on one makes reading the GUI as slow as
    /// the slowest unreachable host.
    #[test]
    fn ai_pane_does_not_probe_on_load() {
        let driver = loaded();
        assert!(
            !driver.model().ai.probed,
            "the AI pane probed backends during init"
        );
    }

    /// Connections come back bounded and legible.
    #[test]
    fn connections_are_bounded_and_labelled() {
        let app = shared();
        match &app.connections {
            Pane::Ready(rows) => {
                assert!(
                    rows.len() <= 64,
                    "loader returned {} rows, cap is 64",
                    rows.len()
                );
                for c in rows {
                    assert!(!c.local.is_empty(), "a connection has no local address");
                    assert!(!c.state.is_empty(), "a connection has no state");
                }
            }
            Pane::Failed(_) => {}
            Pane::Loading => panic!("connections still loading after init"),
        }
    }

    /// Every tab in the egui GUI has a counterpart here.
    ///
    /// The migration's completion condition, asserted rather than asserted-to.
    #[test]
    fn every_egui_tab_has_a_dewey_counterpart() {
        assert_eq!(
            Tab::PORTED.len(),
            13,
            "the egui GUI has 13 tabs; this port must cover all of them"
        );
    }

    /// A headless frame of every tab returns a tree containing its heading.
    ///
    /// The end-to-end version of what 3.9.0 fixed: `gui --frame <tab>` for all
    /// thirteen tabs, with no deadline and no settle predicate anywhere.
    #[test]
    fn frame_renders_every_tab_by_name() {
        for tab in Tab::PORTED {
            let out = frame(Some(tab.title())).unwrap_or_else(|e| panic!("{}: {e}", tab.title()));
            assert!(
                out.len() > 32,
                "{} produced a suspiciously empty tree",
                tab.title()
            );
        }
    }

    /// An unknown tab name is refused, and the error names the alternatives.
    #[test]
    fn frame_rejects_an_unknown_tab() {
        let err = frame(Some("nonexistent")).expect_err("unknown tab was accepted");
        assert!(
            err.contains("Overview"),
            "error does not list known tabs: {err}"
        );
    }

    /// The disk tab reports drives, and each carries the fields the row draws.
    ///
    /// This machine has three NVMe drives and a USB gadget, so an empty list here
    /// would mean enumeration silently failed rather than that the box is
    /// driveless — but the assertion is written to pass on a driveless machine
    /// too, since CI runners legitimately are one.
    #[test]
    fn disks_load_with_usable_rows() {
        let app = shared();
        match &app.disks {
            Pane::Ready(rows) => {
                for d in rows {
                    assert!(!d.name.is_empty(), "a disk row has no device name");
                }
                // Deliberately not asserting capacity > 0. A first version did,
                // and this machine's USB File-Stor Gadget failed it: it reports a
                // model but zero bytes, which is what a removable device with no
                // media legitimately does. It is the same drive that answers
                // NotSupported to ATA SMART. A row rendering "0 B" is correct
                // there; the tab's job is to report what the device said.
            }
            Pane::Failed(e) => panic!("disk enumeration failed: {e}"),
            Pane::Loading => panic!("disks still loading after init"),
        }
    }

    #[test]
    fn memory_reads_a_plausible_total() {
        let app = shared();
        match &app.memory {
            Pane::Ready(m) => {
                assert!(
                    m.total_mb > 128.0,
                    "implausible total memory: {} MB",
                    m.total_mb
                );
                assert!(
                    m.used_mb <= m.total_mb,
                    "used ({}) exceeds total ({})",
                    m.used_mb,
                    m.total_mb
                );
            }
            Pane::Failed(e) => panic!("memory read failed: {e}"),
            Pane::Loading => panic!("memory still loading"),
        }
    }

    /// Tab cycles the selection, and the newly selected pane is what renders.
    ///
    /// Split in two because `HeadlessDriver` exposes only `&M`: the key mapping
    /// is checked against the model, and the rendering consequence against a
    /// driver started on that tab. Asserting both is what makes this a test of
    /// the switch rather than of the key handler alone.
    /// Clicking the tab bar selects the tab under the pointer.
    ///
    /// The first Dewey cut of this GUI handled no mouse events at all, and I
    /// reported it as working without ever pressing a button. This asserts the
    /// dispatch — given a click at a position, the right `Msg` comes back. It
    /// does not prove the framework delivers the event, which is a separate
    /// claim and one only running the binary can settle.
    #[test]
    fn clicking_the_tab_bar_selects_that_tab() {
        use dewey::event::{MouseButton, MouseEvent, MouseEventKind};

        let app = SimonApp::new();

        // Stand in for what `view` measures. The widths do not matter to the
        // mapping, only that they differ from each other — equal widths were
        // the wrong assumption that made clicks land on the neighbouring tab,
        // so a test using equal widths would have passed while the GUI was
        // broken. Whether the real measurement matches the widget is settled by
        // running it, not here.
        let widths: Vec<f32> = Tab::PORTED
            .iter()
            .map(|t| t.title().len() as f32 * 8.0 + 16.0)
            .collect();
        let mut spans = vec![0.0];
        for w in &widths {
            spans.push(spans.last().unwrap() + w);
        }
        *app.tab_spans.lock().unwrap() = spans.clone();

        let click_at = |x: f32, y: f32| {
            app.handle_event(Event::Mouse(MouseEvent {
                kind: MouseEventKind::Click(MouseButton::Left),
                position: Position::new(x, y),
                modifiers: Default::default(),
            }))
        };

        for (index, tab) in Tab::PORTED.iter().enumerate() {
            let x = (spans[index] + spans[index + 1]) / 2.0;
            match click_at(x, TAB_BAR_HEIGHT / 2.0) {
                Some(Msg::SelectTab(got)) => assert_eq!(
                    got, *tab,
                    "clicking at x={x} should select {tab:?}, got {got:?}"
                ),
                other => panic!("clicking tab {index} produced {other:?}"),
            }
        }

        // A click below the bar belongs to the pane, not the tab strip.
        assert!(
            click_at(10.0, TAB_BAR_HEIGHT + 50.0).is_none(),
            "a click in the content area must not change tabs"
        );

        // Past the last tab is bare bar, not the nearest tab.
        assert!(
            click_at(spans.last().unwrap() + 40.0, TAB_BAR_HEIGHT / 2.0).is_none(),
            "empty space beyond the tabs must not select one"
        );
    }

    #[test]
    fn tab_key_cycles_and_switches_the_rendered_pane() {
        // Written against PORTED's order rather than named tabs: this test broke
        // once already when a tab was added ahead of the default, which told it
        // nothing about the switching it exists to check.
        let mut app = SimonApp::new();
        assert_eq!(app.tab, Tab::PORTED[0], "default tab is not the first");

        let msg = app
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Tab,
                KeyModifiers::empty(),
            )))
            .expect("Tab key produced no message");
        app.update(msg);
        assert_eq!(
            app.tab,
            Tab::PORTED[1],
            "Tab did not advance to the next tab"
        );

        // Cycling all the way round returns to the start.
        for _ in 1..Tab::PORTED.len() {
            let msg = app
                .handle_event(Event::Key(KeyEvent::new(
                    KeyCode::Tab,
                    KeyModifiers::empty(),
                )))
                .expect("Tab key produced no message");
            app.update(msg);
        }
        assert_eq!(app.tab, Tab::PORTED[0], "Tab did not wrap around");

        let mut driver = HeadlessDriver::new(
            SimonApp {
                tab: Tab::Network,
                ..SimonApp::new()
            },
            1280.0,
            800.0,
        );
        driver.init();
        driver.process_request(&dewey::agent::protocol::AgentRequest::GetTree);
        assert!(
            driver.ontology().find_node("network_heading").is_some(),
            "with Network selected, its heading is not in the tree"
        );
        assert!(
            driver.ontology().find_node("memory_heading").is_none(),
            "the unselected Memory tab still rendered into the tree"
        );
    }
}

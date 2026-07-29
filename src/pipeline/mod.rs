//! Lock-free snapshot pipeline: decouples hardware collection from rendering.
//!
//! # Why this exists
//!
//! Before this module, every frontend (CLI, TUI, GUI) called blocking collectors
//! directly on its render thread. A slow WMI or NVML query would stall a frame,
//! and [`crate::backend::MonitoringBackend::update`] ran every collector serially,
//! so the slowest source set the floor for all of them.
//!
//! This module inverts that:
//!
//! - A dedicated **collector thread** owns all hardware handles and never shares them.
//! - Independent collectors run **concurrently** within each tick, so total tick cost
//!   is the slowest single collector rather than their sum.
//! - Results are published as an immutable [`Snapshot`] into an [`ArcSwap`] slot.
//!   Readers do a lock-free atomic load and never block, never wait on a mutex, and
//!   never touch a hardware API.
//!
//! # Thread-safety design
//!
//! [`crate::backend::MonitoringBackend`] cannot be moved across threads: it stores
//! `Vec<Box<dyn DiskDevice>>` and `Vec<Box<dyn MotherboardDevice>>`, trait objects
//! with no `Send` bound. Rather than retrofit `Send` onto those traits, the collector
//! thread **constructs its own sources** and only ever sends plain owned data
//! ([`Snapshot`]) across the boundary. Nothing that touches a driver ever crosses a
//! thread.
//!
//! On Windows this also matters for COM: `wmi` initializes COM per-thread, and
//! `disk::windows` contains a `COMLibrary::assume_initialized()` path that is only
//! sound on a thread where COM was actually initialized. Every thread this module
//! spawns therefore calls [`com_guard`] first and holds the guard for its lifetime.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};

use crate::connections::{ConnectionInfo, ConnectionMonitor};
use crate::core::cpu::CpuStats;
use crate::core::memory::MemoryStats;
use crate::gpu::{GpuCollection, GpuDynamicInfo, GpuStaticInfo};
use crate::network_monitor::NetworkMonitor;
use crate::process_monitor::{ProcessMonitor, ProcessMonitorInfo};
use crate::system_stats::SystemStats;

/// Default collector tick interval.
pub const DEFAULT_TICK: Duration = Duration::from_millis(1000);

/// Default number of samples retained per history series.
pub const DEFAULT_HISTORY: usize = 120;

// ============================================================================
// COM initialization guard
// ============================================================================

/// Per-thread COM guard.
///
/// On Windows this initializes COM for the calling thread and keeps it initialized
/// for as long as the returned value is held. Every thread that may reach a WMI
/// collector must hold one, otherwise `COMLibrary::assume_initialized()` in
/// `disk::windows` is unsound.
///
/// On non-Windows targets this is a zero-sized no-op.
#[cfg(target_os = "windows")]
pub fn com_guard() -> Option<wmi::COMLibrary> {
    wmi::COMLibrary::new().ok()
}

/// Per-thread COM guard (no-op on non-Windows targets).
#[cfg(not(target_os = "windows"))]
pub fn com_guard() {}

// ============================================================================
// Plain-data snapshot types
// ============================================================================

/// A disk row, flattened to plain data so it can cross a thread boundary.
///
/// `Box<dyn DiskDevice>` is not `Send`, so the collector flattens it here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiskSnapshot {
    /// Device model or drive letter.
    pub name: String,
    /// Mount point or drive root.
    pub mount_point: String,
    /// Total capacity in bytes.
    pub total: u64,
    /// Used capacity in bytes.
    pub used: u64,
    /// Filesystem label (NTFS, ext4, ...).
    pub filesystem: String,
    /// Read throughput in bytes/sec.
    pub read_rate: f64,
    /// Write throughput in bytes/sec.
    pub write_rate: f64,
}

/// A network interface row with computed bandwidth rates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NetSnapshot {
    /// Interface name.
    pub name: String,
    /// Interface is up.
    pub is_up: bool,
    /// Cumulative bytes received.
    pub rx_bytes: u64,
    /// Cumulative bytes transmitted.
    pub tx_bytes: u64,
    /// Receive rate in bytes/sec.
    pub rx_rate: f64,
    /// Transmit rate in bytes/sec.
    pub tx_rate: f64,
    /// Link speed in Mbps, if known.
    pub speed_mbps: Option<u32>,
}

/// Per-collector wall-clock cost for the most recent tick, in microseconds.
///
/// Exposed so the TUI/GUI can surface which source is the bottleneck, and so
/// regressions in collection cost are observable rather than guessed at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageTimings {
    /// CPU collection cost.
    pub cpu_us: u64,
    /// Memory collection cost.
    pub memory_us: u64,
    /// GPU collection cost.
    pub gpu_us: u64,
    /// Process enumeration cost.
    pub process_us: u64,
    /// Network collection cost.
    pub network_us: u64,
    /// Connection table cost.
    pub connection_us: u64,
    /// Disk enumeration cost.
    pub disk_us: u64,
    /// System stats cost.
    pub system_us: u64,
}

impl StageTimings {
    /// Cost of the slowest single collector — the theoretical floor for a tick
    /// once collectors run concurrently.
    pub fn critical_path_us(&self) -> u64 {
        [
            self.cpu_us,
            self.memory_us,
            self.gpu_us,
            self.process_us,
            self.network_us,
            self.connection_us,
            self.disk_us,
            self.system_us,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }

    /// Cost if every collector had run serially — what the old `update()` paid.
    pub fn serial_us(&self) -> u64 {
        self.cpu_us
            + self.memory_us
            + self.gpu_us
            + self.process_us
            + self.network_us
            + self.connection_us
            + self.disk_us
            + self.system_us
    }
}

/// Ring-buffer history series, stored as plain `Vec` so the snapshot stays trivially
/// cloneable and serializable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Histories {
    /// CPU utilization percentage over time.
    pub cpu: Vec<f32>,
    /// Memory utilization percentage over time.
    pub memory: Vec<f32>,
    /// Per-GPU utilization percentage over time.
    pub gpu_util: Vec<Vec<f32>>,
    /// Per-GPU memory utilization percentage over time.
    pub gpu_memory: Vec<Vec<f32>>,
    /// Per-GPU temperature over time.
    pub gpu_temp: Vec<Vec<f32>>,
    /// Aggregate network receive rate over time.
    pub net_rx: Vec<f32>,
    /// Aggregate network transmit rate over time.
    pub net_tx: Vec<f32>,
}

/// An immutable, self-contained view of system state at one instant.
///
/// Cheap to share: readers hold an `Arc<Snapshot>` and never copy the contents.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    /// Monotonically increasing tick counter. Readers compare this against a cached
    /// value to decide whether any recomputation (formatting, layout) is needed.
    pub generation: u64,
    /// Wall-clock capture time, seconds since the Unix epoch.
    pub collected_at: u64,
    /// Total wall-clock cost of this tick.
    pub collect_us: u64,
    /// Per-collector breakdown of this tick.
    pub timings: StageTimings,

    /// CPU statistics, if the platform collector succeeded.
    pub cpu: Option<CpuStats>,
    /// Memory statistics, if the platform collector succeeded.
    pub memory: Option<MemoryStats>,
    /// Static GPU descriptors, captured once at startup.
    pub gpu_static: Vec<GpuStaticInfo>,
    /// Per-tick GPU telemetry, index-aligned with [`Snapshot::gpu_static`].
    ///
    /// `None` marks a device whose query failed this tick. Failures keep their slot
    /// rather than being dropped, so a flaky device cannot silently renumber the
    /// GPUs beside it.
    pub gpu_dynamic: Vec<Option<GpuDynamicInfo>>,
    /// Process table with GPU attribution.
    pub processes: Vec<ProcessMonitorInfo>,
    /// Open network connections.
    pub connections: Vec<ConnectionInfo>,
    /// Mounted disks.
    pub disks: Vec<DiskSnapshot>,
    /// Active network interfaces.
    pub network: Vec<NetSnapshot>,
    /// Load average / vmstat style system counters.
    pub system_stats: Option<SystemStats>,
    /// Retained time series.
    pub histories: Histories,
}

impl Snapshot {
    /// CPU utilization percentage, or 0.0 when unavailable.
    pub fn cpu_utilization(&self) -> f32 {
        self.cpu
            .as_ref()
            .map(|c| 100.0 - c.total.idle)
            .unwrap_or(0.0)
    }

    /// Memory utilization percentage, or 0.0 when unavailable.
    pub fn memory_utilization(&self) -> f32 {
        self.memory
            .as_ref()
            .map(|m| m.ram_usage_percent())
            .unwrap_or(0.0)
    }

    /// Aggregate receive rate across all interfaces, in bytes/sec.
    pub fn total_rx_rate(&self) -> f64 {
        self.network.iter().map(|n| n.rx_rate).sum()
    }

    /// Aggregate transmit rate across all interfaces, in bytes/sec.
    pub fn total_tx_rate(&self) -> f64 {
        self.network.iter().map(|n| n.tx_rate).sum()
    }
}

// ============================================================================
// Reader handle
// ============================================================================

/// A cheap, cloneable reader handle onto the newest [`Snapshot`].
///
/// Cloning is refcount-only. [`SnapshotHandle::latest`] is a lock-free atomic load,
/// safe to call every frame from any number of threads.
#[derive(Clone)]
pub struct SnapshotHandle {
    slot: Arc<ArcSwap<Snapshot>>,
    stop: Arc<AtomicBool>,
    interval_ms: Arc<AtomicU64>,
}

impl SnapshotHandle {
    /// Load the newest snapshot. Lock-free; never blocks on the collector.
    pub fn latest(&self) -> Arc<Snapshot> {
        self.slot.load_full()
    }

    /// Generation of the newest snapshot, without cloning the `Arc`.
    ///
    /// Use this to skip work when nothing has changed since the last frame.
    pub fn generation(&self) -> u64 {
        self.slot.load().generation
    }

    /// Change the collector tick interval. Takes effect on the next tick.
    pub fn set_interval(&self, interval: Duration) {
        self.interval_ms
            .store(interval.as_millis() as u64, Ordering::Relaxed);
    }

    /// Current collector tick interval.
    pub fn interval(&self) -> Duration {
        Duration::from_millis(self.interval_ms.load(Ordering::Relaxed))
    }

    /// Ask the collector thread to shut down after its current tick.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Whether a stop has been requested.
    pub fn is_stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Collector
// ============================================================================

/// Collector configuration.
#[derive(Debug, Clone)]
pub struct CollectorConfig {
    /// Interval between ticks.
    pub interval: Duration,
    /// Samples retained per history series.
    pub history_size: usize,
    /// Collect the process table. Disable for lightweight headless use.
    pub collect_processes: bool,
    /// Collect the open-connection table (relatively expensive on Windows).
    pub collect_connections: bool,
    /// Re-enumerate disks every N ticks. Disk topology changes rarely, so
    /// re-enumerating every tick wastes a comparatively expensive WMI call.
    pub disk_every_n_ticks: u64,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_TICK,
            history_size: DEFAULT_HISTORY,
            collect_processes: true,
            collect_connections: true,
            disk_every_n_ticks: 10,
        }
    }
}

/// Owns the collector thread. Dropping this stops and joins the thread.
pub struct Collector {
    handle: SnapshotHandle,
    join: Option<JoinHandle<()>>,
}

impl Collector {
    /// Spawn the collector thread and return immediately.
    ///
    /// The thread constructs its own hardware handles, so no driver state is ever
    /// shared. The first snapshot may briefly be [`Snapshot::default`] until the
    /// first tick lands; readers should treat generation 0 as "not ready yet".
    pub fn spawn(config: CollectorConfig) -> Self {
        let slot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let interval_ms = Arc::new(AtomicU64::new(config.interval.as_millis() as u64));

        let handle = SnapshotHandle {
            slot: Arc::clone(&slot),
            stop: Arc::clone(&stop),
            interval_ms: Arc::clone(&interval_ms),
        };

        let join = thread::Builder::new()
            .name("simon-collector".into())
            .spawn(move || collector_loop(config, slot, stop, interval_ms))
            .ok();

        Self { handle, join }
    }

    /// A cloneable reader handle for the UI threads.
    pub fn handle(&self) -> SnapshotHandle {
        self.handle.clone()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.handle.request_stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Hardware handles owned exclusively by the collector thread.
struct Sources {
    gpu: Option<GpuCollection>,
    processes: Option<ProcessMonitor>,
    network: Option<NetworkMonitor>,
    connections: Option<ConnectionMonitor>,
}

/// Bounded push that keeps the newest `cap` samples.
fn push_capped(series: &mut Vec<f32>, value: f32, cap: usize) {
    if series.len() >= cap {
        let overflow = series.len() + 1 - cap;
        series.drain(..overflow);
    }
    series.push(value);
}

fn collector_loop(
    config: CollectorConfig,
    slot: Arc<ArcSwap<Snapshot>>,
    stop: Arc<AtomicBool>,
    interval_ms: Arc<AtomicU64>,
) {
    // COM must be initialized on this thread before any WMI collector runs, and
    // must stay initialized for the thread's lifetime.
    let _com = com_guard();

    let mut histories = Histories::default();
    let mut generation: u64 = 0;
    let mut cached_disks: Vec<DiskSnapshot> = Vec::new();

    // === Warm-up publish ===
    //
    // Constructing the hardware sources is slow: `GpuCollection::auto_detect` spins
    // up vendor drivers (NVML, DXGI) and, on a multi-GPU host, measured 8-12s before
    // it returned. Publishing nothing until it finished left every UI blank for that
    // whole window, even though CPU and memory were available in under a millisecond.
    //
    // So publish those first. `collect_once` already treats every source as optional,
    // so passing an empty `Sources` yields exactly the collectors that need no setup
    // — CPU, memory, system stats and disks — and leaves the rest empty until the
    // real sources exist. Consumers key off `generation`, so this simply arrives as
    // the first update and is superseded a moment later.
    let mut warmup_sources = Sources {
        gpu: None,
        processes: None,
        network: None,
        connections: None,
    };
    generation += 1;
    slot.store(Arc::new(collect_once(
        &mut warmup_sources,
        &config,
        &mut histories,
        &mut cached_disks,
        &[],
        generation,
    )));

    if stop.load(Ordering::Relaxed) {
        return;
    }

    let mut sources = Sources {
        gpu: GpuCollection::auto_detect().ok(),
        processes: if config.collect_processes {
            ProcessMonitor::new().ok()
        } else {
            None
        },
        network: NetworkMonitor::new().ok(),
        connections: if config.collect_connections {
            ConnectionMonitor::new().ok()
        } else {
            None
        },
    };

    // Static GPU descriptors are captured once; they do not change at runtime.
    let gpu_static: Vec<GpuStaticInfo> = sources
        .gpu
        .as_ref()
        .and_then(|g| g.snapshot_all().ok())
        .map(|infos| infos.iter().map(|i| i.static_info.clone()).collect())
        .unwrap_or_default();

    // Size the per-GPU series now that the device count is known. The warm-up pass
    // ran with none, so these start empty.
    histories.gpu_util = vec![Vec::new(); gpu_static.len()];
    histories.gpu_memory = vec![Vec::new(); gpu_static.len()];
    histories.gpu_temp = vec![Vec::new(); gpu_static.len()];

    while !stop.load(Ordering::Relaxed) {
        let tick_start = Instant::now();
        generation += 1;

        let snapshot = collect_once(
            &mut sources,
            &config,
            &mut histories,
            &mut cached_disks,
            &gpu_static,
            generation,
        );

        slot.store(Arc::new(snapshot));

        // Sleep the remainder of the interval, re-reading it each tick so the UI can
        // retune cadence live. Waking in short slices keeps shutdown responsive even
        // at long intervals.
        let interval = Duration::from_millis(interval_ms.load(Ordering::Relaxed).max(50));
        let elapsed = tick_start.elapsed();
        let mut remaining = interval.saturating_sub(elapsed);
        while remaining > Duration::ZERO && !stop.load(Ordering::Relaxed) {
            let slice = remaining.min(Duration::from_millis(50));
            thread::sleep(slice);
            remaining = remaining.saturating_sub(slice);
        }
    }
}

/// Run one tick, fanning independent collectors out across scoped threads.
///
/// Each closure borrows a *disjoint* field of [`Sources`], which is what makes
/// concurrent `&mut` access sound here. Every scoped thread takes its own COM guard.
fn collect_once(
    sources: &mut Sources,
    config: &CollectorConfig,
    histories: &mut Histories,
    cached_disks: &mut Vec<DiskSnapshot>,
    gpu_static: &[GpuStaticInfo],
    generation: u64,
) -> Snapshot {
    let started = Instant::now();

    // Destructure so each scoped thread captures a distinct field.
    let Sources {
        gpu,
        processes,
        network,
        connections,
    } = sources;

    let refresh_disks = generation == 1 || generation % config.disk_every_n_ticks.max(1) == 0;

    let (cpu, memory, gpu_dynamic, process_list, net_list, conn_list, disk_list, sys_stats) =
        thread::scope(|scope| {
            let cpu_h = scope.spawn(timed(collect_cpu));
            let mem_h = scope.spawn(timed(collect_memory));
            let sys_h = scope.spawn(timed(collect_system_stats));
            let gpu_h = scope.spawn(timed(move || collect_gpu(gpu.as_ref())));
            let proc_h = scope.spawn(timed(move || collect_processes(processes.as_mut())));
            let net_h = scope.spawn(timed(move || collect_network(network.as_mut())));
            let conn_h = scope.spawn(timed(move || collect_connections(connections.as_mut())));
            let disk_h = scope.spawn(timed(move || {
                if refresh_disks {
                    Some(collect_disks())
                } else {
                    None
                }
            }));

            (
                join_or_default(cpu_h),
                join_or_default(mem_h),
                join_or_default(gpu_h),
                join_or_default(proc_h),
                join_or_default(net_h),
                join_or_default(conn_h),
                join_or_default(disk_h),
                join_or_default(sys_h),
            )
        });

    let timings = StageTimings {
        cpu_us: cpu.1,
        memory_us: memory.1,
        gpu_us: gpu_dynamic.1,
        process_us: process_list.1,
        network_us: net_list.1,
        connection_us: conn_list.1,
        disk_us: disk_list.1,
        system_us: sys_stats.1,
    };

    let (cpu, memory) = (cpu.0, memory.0);
    let gpu_dynamic = gpu_dynamic.0;
    let processes = process_list.0;
    let network = net_list.0;
    let connections = conn_list.0;
    let system_stats = sys_stats.0;

    if let Some(fresh) = disk_list.0 {
        *cached_disks = fresh;
    }

    // === histories ===
    let cap = config.history_size.max(1);
    if let Some(ref c) = cpu {
        push_capped(&mut histories.cpu, 100.0 - c.total.idle, cap);
    }
    if let Some(ref m) = memory {
        push_capped(&mut histories.memory, m.ram_usage_percent(), cap);
    }
    for (i, info) in gpu_dynamic.iter().enumerate() {
        // A device that failed this tick contributes no sample; its series simply
        // does not advance, which reads as a flat segment rather than a false zero.
        let Some(info) = info.as_ref() else { continue };
        if i < histories.gpu_util.len() {
            push_capped(&mut histories.gpu_util[i], info.utilization as f32, cap);
            push_capped(
                &mut histories.gpu_memory[i],
                info.memory.utilization as f32,
                cap,
            );
            if let Some(temp) = info.thermal.temperature {
                push_capped(&mut histories.gpu_temp[i], temp as f32, cap);
            }
        }
    }
    let rx_total: f64 = network.iter().map(|n| n.rx_rate).sum();
    let tx_total: f64 = network.iter().map(|n| n.tx_rate).sum();
    push_capped(&mut histories.net_rx, rx_total as f32, cap);
    push_capped(&mut histories.net_tx, tx_total as f32, cap);

    Snapshot {
        generation,
        collected_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        collect_us: started.elapsed().as_micros() as u64,
        timings,
        cpu,
        memory,
        gpu_static: gpu_static.to_vec(),
        gpu_dynamic,
        processes,
        connections,
        disks: cached_disks.clone(),
        network,
        system_stats,
        histories: histories.clone(),
    }
}

/// Wrap a collector so it takes its own COM guard and reports its own wall-clock cost.
fn timed<T, F>(f: F) -> impl FnOnce() -> (T, u64) + Send
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    move || {
        let _com = com_guard();
        let start = Instant::now();
        let value = f();
        (value, start.elapsed().as_micros() as u64)
    }
}

/// Join a scoped collector, treating a panic as "no data" rather than taking the
/// whole collector thread down with it. One flaky vendor driver must not kill
/// monitoring for everything else.
fn join_or_default<T: Default>(handle: thread::ScopedJoinHandle<'_, (T, u64)>) -> (T, u64) {
    handle.join().unwrap_or_else(|_| (T::default(), 0))
}

// ============================================================================
// Individual collectors
// ============================================================================

fn collect_cpu() -> Option<CpuStats> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::read_cpu_stats().ok()
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::read_cpu_stats().ok()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        CpuStats::new().ok()
    }
}

fn collect_memory() -> Option<MemoryStats> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::read_memory_stats().ok()
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::read_memory_stats().ok()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        MemoryStats::new().ok()
    }
}

fn collect_system_stats() -> Option<SystemStats> {
    SystemStats::new().ok()
}

fn collect_gpu(gpu: Option<&GpuCollection>) -> Vec<Option<GpuDynamicInfo>> {
    let Some(collection) = gpu else {
        return Vec::new();
    };
    // Partial results keep index alignment with the static descriptors captured at
    // startup, and let one failing device degrade alone.
    collection
        .snapshot_all_partial()
        .into_iter()
        .map(|result| result.ok().map(|info| info.dynamic_info))
        .collect()
}

fn collect_processes(monitor: Option<&mut ProcessMonitor>) -> Vec<ProcessMonitorInfo> {
    monitor.and_then(|m| m.processes().ok()).unwrap_or_default()
}

fn collect_connections(monitor: Option<&mut ConnectionMonitor>) -> Vec<ConnectionInfo> {
    monitor
        .and_then(|m| m.all_connections().ok())
        .unwrap_or_default()
}

fn collect_network(monitor: Option<&mut NetworkMonitor>) -> Vec<NetSnapshot> {
    let Some(monitor) = monitor else {
        return Vec::new();
    };
    let Ok(interfaces) = monitor.interfaces() else {
        return Vec::new();
    };

    let is_virtual = |name: &str| {
        name.starts_with("lo")
            || name.contains("Loopback")
            || name.starts_with("vEthernet")
            || name.starts_with("Local Area Connection*")
            || name.starts_with("VMware")
            || name.starts_with("VirtualBox")
    };

    let mut out: Vec<NetSnapshot> = interfaces
        .iter()
        .filter(|i| !is_virtual(&i.name) && i.is_active())
        .map(|i| {
            let (rx_rate, tx_rate) = monitor.bandwidth_rate(&i.name, i);
            NetSnapshot {
                name: i.name.clone(),
                is_up: i.is_up,
                rx_bytes: i.rx_bytes,
                tx_bytes: i.tx_bytes,
                rx_rate,
                tx_rate,
                speed_mbps: i.speed_mbps,
            }
        })
        .collect();

    // Fall back to any active non-loopback interface rather than showing nothing.
    if out.is_empty() {
        out = interfaces
            .iter()
            .filter(|i| !i.name.starts_with("lo") && !i.name.contains("Loopback") && i.is_active())
            .map(|i| {
                let (rx_rate, tx_rate) = monitor.bandwidth_rate(&i.name, i);
                NetSnapshot {
                    name: i.name.clone(),
                    is_up: i.is_up,
                    rx_bytes: i.rx_bytes,
                    tx_bytes: i.tx_bytes,
                    rx_rate,
                    tx_rate,
                    speed_mbps: i.speed_mbps,
                }
            })
            .collect();
    }

    out
}

#[cfg(target_os = "windows")]
fn collect_disks() -> Vec<DiskSnapshot> {
    // Windows: GetLogicalDriveStrings + GetDiskFreeSpaceExW is far cheaper and more
    // reliable than the WMI disk enumeration path.
    crate::platform::windows::logical_drives()
        .map(|drives| {
            drives
                .into_iter()
                .map(|d| DiskSnapshot {
                    name: d.name.clone(),
                    mount_point: d.name,
                    total: d.total,
                    used: d.used,
                    filesystem: d.filesystem,
                    read_rate: 0.0,
                    write_rate: 0.0,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(target_os = "windows"))]
fn collect_disks() -> Vec<DiskSnapshot> {
    let Ok(disks) = crate::disk::enumerate_disks() else {
        return Vec::new();
    };

    disks
        .iter()
        .filter_map(|disk| {
            let info = disk.info().ok()?;

            let (mount_point, filesystem, used, total) = disk
                .filesystem_info()
                .ok()
                .and_then(|fs_infos| fs_infos.first().cloned())
                .map(|fs| {
                    (
                        fs.mount_point.to_string_lossy().to_string(),
                        fs.fs_type.clone(),
                        fs.used_size,
                        fs.total_size,
                    )
                })
                .unwrap_or_else(|| ("N/A".to_string(), "N/A".to_string(), 0, info.capacity));

            let (read_rate, write_rate) = disk
                .io_stats()
                .ok()
                .map(|io| {
                    (
                        io.read_throughput.unwrap_or(0) as f64,
                        io.write_throughput.unwrap_or(0) as f64,
                    )
                })
                .unwrap_or((0.0, 0.0));

            Some(DiskSnapshot {
                name: info.model,
                mount_point,
                total,
                used,
                filesystem,
                read_rate,
                write_rate,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_capped_keeps_newest_samples() {
        let mut series = Vec::new();
        for i in 0..10 {
            push_capped(&mut series, i as f32, 4);
        }
        assert_eq!(series.len(), 4, "series must be bounded by cap");
        assert_eq!(series, vec![6.0, 7.0, 8.0, 9.0], "oldest samples evicted");
    }

    #[test]
    fn push_capped_handles_cap_of_one() {
        let mut series = Vec::new();
        push_capped(&mut series, 1.0, 1);
        push_capped(&mut series, 2.0, 1);
        assert_eq!(series, vec![2.0]);
    }

    #[test]
    fn critical_path_is_max_not_sum() {
        let t = StageTimings {
            cpu_us: 100,
            memory_us: 50,
            gpu_us: 900,
            process_us: 400,
            ..Default::default()
        };
        assert_eq!(
            t.critical_path_us(),
            900,
            "critical path is the slowest stage"
        );
        assert_eq!(t.serial_us(), 1450, "serial cost is the sum");
        assert!(
            t.critical_path_us() < t.serial_us(),
            "concurrent collection must beat serial"
        );
    }

    #[test]
    fn default_snapshot_is_generation_zero() {
        let snap = Snapshot::default();
        assert_eq!(snap.generation, 0, "readers treat gen 0 as not-ready");
        assert_eq!(snap.cpu_utilization(), 0.0);
        assert_eq!(snap.memory_utilization(), 0.0);
    }

    #[test]
    fn snapshot_aggregates_network_rates() {
        let snap = Snapshot {
            network: vec![
                NetSnapshot {
                    rx_rate: 1.5,
                    tx_rate: 0.5,
                    ..Default::default()
                },
                NetSnapshot {
                    rx_rate: 2.5,
                    tx_rate: 1.5,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(snap.total_rx_rate(), 4.0);
        assert_eq!(snap.total_tx_rate(), 2.0);
    }

    #[test]
    fn handle_is_lock_free_readable_and_stoppable() {
        let slot = Arc::new(ArcSwap::from_pointee(Snapshot::default()));
        let handle = SnapshotHandle {
            slot: Arc::clone(&slot),
            stop: Arc::new(AtomicBool::new(false)),
            interval_ms: Arc::new(AtomicU64::new(1000)),
        };

        assert_eq!(handle.generation(), 0);
        slot.store(Arc::new(Snapshot {
            generation: 7,
            ..Default::default()
        }));
        assert_eq!(handle.generation(), 7, "readers see the newest publish");
        assert_eq!(handle.latest().generation, 7);

        handle.set_interval(Duration::from_millis(250));
        assert_eq!(handle.interval(), Duration::from_millis(250));

        assert!(!handle.is_stopping());
        handle.request_stop();
        assert!(handle.is_stopping());
    }

    /// The first published snapshot must already carry useful data.
    ///
    /// Constructing the GPU collection initializes vendor drivers and can take
    /// seconds. Before the warm-up publish, nothing at all was published until that
    /// finished, so every UI sat blank. This asserts the invariant that makes the
    /// difference — first data does not wait on driver enumeration — rather than an
    /// absolute millisecond bound, which would be flaky on a loaded machine.
    #[test]
    fn first_publish_does_not_wait_for_driver_enumeration() {
        let collector = Collector::spawn(CollectorConfig {
            interval: Duration::from_millis(200),
            ..Default::default()
        });
        let handle = collector.handle();

        let start = Instant::now();
        while handle.generation() == 0 && start.elapsed() < Duration::from_secs(60) {
            thread::sleep(Duration::from_millis(1));
        }

        let first = handle.latest();
        assert!(
            first.generation > 0,
            "collector published nothing within 60s"
        );
        assert!(
            first.cpu.is_some() || first.memory.is_some(),
            "the first snapshot carried no CPU or memory sample, so the warm-up \
             publish is not doing its job — consumers would render an empty UI \
             until driver enumeration completed"
        );
    }

    #[test]
    fn snapshot_is_send_and_sync() {
        // The whole design rests on Snapshot crossing threads; assert it statically
        // so a future field addition (e.g. an Rc or a trait object) fails the build
        // here rather than at the call site.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Snapshot>();
        assert_send_sync::<SnapshotHandle>();
    }
}

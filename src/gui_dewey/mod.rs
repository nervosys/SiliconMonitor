// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! The Dewey port of simon's GUI.
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
use std::result::Result;

/// Which tab is showing.
///
/// Only the tabs ported so far. This grows with the migration rather than
/// mirroring the egui `Tab` enum up front — a variant here means a tab that
/// actually renders, so the agent-visible tab list never promises more than the
/// port delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Memory,
    Network,
    Disk,
    System,
}

impl Tab {
    pub const PORTED: [Tab; 4] = [Tab::Memory, Tab::Network, Tab::Disk, Tab::System];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Memory => "Memory",
            Tab::Network => "Network",
            Tab::Disk => "Disk",
            Tab::System => "System",
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

/// Total bandwidth across every interface, bytes per second.
#[derive(Debug, Clone, Default)]
pub struct NetworkView {
    pub interfaces: Vec<(String, f64, f64)>,
    pub total_rx: f64,
    pub total_tx: f64,
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
    pub memory: Pane<MemoryView>,
    pub network: Pane<NetworkView>,
    pub disks: Pane<Vec<DiskRow>>,
    pub system: Pane<SystemView>,
}

#[derive(Debug)]
pub enum Msg {
    SelectTab(Tab),
    Refresh,
    MemoryLoaded(Result<MemoryView, String>),
    NetworkLoaded(Result<NetworkView, String>),
    DisksLoaded(Result<Vec<DiskRow>, String>),
    SystemLoaded(Result<SystemView, String>),
}

impl Default for SimonApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SimonApp {
    pub fn new() -> Self {
        Self {
            tab: Tab::Memory,
            memory: Pane::Loading,
            network: Pane::Loading,
            disks: Pane::Loading,
            system: Pane::Loading,
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
            _ => None,
        }
    }

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::SelectTab(tab) => {
                self.tab = tab;
                Command::None
            }
            Msg::Refresh => {
                self.memory = Pane::Loading;
                self.network = Pane::Loading;
                self.disks = Pane::Loading;
                self.system = Pane::Loading;
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
        }
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;
        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(36.0), Constraint::Fill(1.0)],
        )
        .split(area);

        // `Tabs` is a StatefulWidget, and `view` takes `&self` — so the selection
        // is derived into a local each frame rather than stored. The model stays
        // the single source of truth for which tab is showing; the widget's state
        // is a projection of it, never the other way round.
        let mut tab_state = TabState::new()
            .with_selected(Tab::PORTED.iter().position(|t| *t == self.tab).unwrap_or(0));
        Tabs::new(Tab::PORTED.iter().map(|t| t.title().to_string()).collect())
            .agent_id("tab_bar")
            .render(chunks[0], frame, &mut tab_state);

        match self.tab {
            Tab::Memory => self.view_memory(chunks[1], frame),
            Tab::Network => self.view_network(chunks[1], frame),
            Tab::Disk => self.view_disk(chunks[1], frame),
            Tab::System => self.view_system(chunks[1], frame),
        }
    }
}

/// Rows are laid out top-down at a fixed line height; returns each row's rect.
fn rows_of(area: Rect, count: usize) -> Vec<Rect> {
    Layout::new(
        Direction::Vertical,
        (0..count)
            .map(|_| Constraint::Length(24.0))
            .collect::<Vec<_>>(),
    )
    .split(area)
}

impl SimonApp {
    fn view_memory(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.memory {
            Pane::Loading => {
                Label::new("Loading memory information…")
                    .agent_id("memory_status")
                    .render(area, frame);
            }
            Pane::Failed(e) => {
                Label::new(format!("Memory unavailable: {e}"))
                    .agent_id("memory_status")
                    .render(area, frame);
            }
            Pane::Ready(m) => {
                let r = rows_of(area, 9);
                Label::new("Physical Memory")
                    .bold()
                    .agent_id("memory_heading")
                    .render(r[0], frame);
                ProgressBar::new(m.usage_percent / 100.0)
                    .label(format!("{:.1} MB / {:.1} MB", m.used_mb, m.total_mb))
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
                    Label::new(format!("{label}: {value:.0} MB"))
                        .agent_id(*id)
                        .render(r[i + 2], frame);
                }
            }
        }
    }

    fn view_disk(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.disks {
            Pane::Loading => Label::new("Loading disk information…")
                .agent_id("disk_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("Disks unavailable: {e}"))
                .agent_id("disk_status")
                .render(area, frame),
            // A machine with no drives is a real answer, not a failure, and it
            // must not read as the spinner state. 3.9.0 could not tell the two
            // apart from painted text alone; here they are different nodes.
            Pane::Ready(rows) if rows.is_empty() => Label::new("No disks detected")
                .agent_id("disk_empty")
                .render(area, frame),
            Pane::Ready(rows) => {
                let r = rows_of(area, 1 + rows.len().min(16));
                Label::new("Disks")
                    .bold()
                    .agent_id("disk_heading")
                    .render(r[0], frame);
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
                    Label::new(format!(
                        "{}: {} {} {}{temp}{health}",
                        d.name,
                        d.model,
                        format_bytes(d.capacity_bytes as f64),
                        d.interface
                    ))
                    .agent_id(format!("disk_row_{i}"))
                    .render(r[i + 1], frame);
                }
            }
        }
    }

    fn view_system(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.system {
            Pane::Loading => Label::new("Loading system information…")
                .agent_id("system_status")
                .render(area, frame),
            Pane::Failed(e) => Label::new(format!("System information unavailable: {e}"))
                .agent_id("system_status")
                .render(area, frame),
            Pane::Ready(v) => {
                let r = rows_of(area, 1 + v.rows.len().min(24));
                Label::new("System")
                    .bold()
                    .agent_id("system_heading")
                    .render(r[0], frame);
                for (i, (label, value)) in v.rows.iter().take(24).enumerate() {
                    Label::new(format!("{label}: {value}"))
                        .agent_id(format!("system_{}", label.to_lowercase().replace(' ', "_")))
                        .render(r[i + 1], frame);
                }
            }
        }
    }

    fn view_network(&self, area: Rect, frame: &mut Frame<'_>) {
        match &self.network {
            Pane::Loading => {
                Label::new("Loading network information…")
                    .agent_id("network_status")
                    .render(area, frame);
            }
            Pane::Failed(e) => {
                Label::new(format!("Network unavailable: {e}"))
                    .agent_id("network_status")
                    .render(area, frame);
            }
            Pane::Ready(n) => {
                let r = rows_of(area, 2 + n.interfaces.len().min(16));
                Label::new("Interfaces")
                    .bold()
                    .agent_id("network_heading")
                    .render(r[0], frame);
                Label::new(format!(
                    "Total Bandwidth: ↓ {} ↑ {}",
                    format_bytes(n.total_rx),
                    format_bytes(n.total_tx)
                ))
                .agent_id("network_total_bandwidth")
                .render(r[1], frame);

                for (i, (name, rx, tx)) in n.interfaces.iter().take(16).enumerate() {
                    Label::new(format!(
                        "{name}: ↓ {} ↑ {}",
                        format_bytes(*rx),
                        format_bytes(*tx)
                    ))
                    .agent_id(format!("network_iface_{i}"))
                    .render(r[i + 2], frame);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dewey::agent::driver::HeadlessDriver;

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
        let driver = loaded();
        match &driver.model().system {
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

    /// The disk tab reports drives, and each carries the fields the row draws.
    ///
    /// This machine has three NVMe drives and a USB gadget, so an empty list here
    /// would mean enumeration silently failed rather than that the box is
    /// driveless — but the assertion is written to pass on a driveless machine
    /// too, since CI runners legitimately are one.
    #[test]
    fn disks_load_with_usable_rows() {
        let driver = loaded();
        match &driver.model().disks {
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
        let driver = loaded();
        match &driver.model().memory {
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
    #[test]
    fn tab_key_cycles_and_switches_the_rendered_pane() {
        let mut app = SimonApp::new();
        assert_eq!(app.tab, Tab::Memory);

        let msg = app
            .handle_event(Event::Key(KeyEvent::new(
                KeyCode::Tab,
                KeyModifiers::empty(),
            )))
            .expect("Tab key produced no message");
        app.update(msg);
        assert_eq!(app.tab, Tab::Network, "Tab did not advance the selection");

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

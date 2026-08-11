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
/// Only the two tabs ported so far. This grows with the migration rather than
/// mirroring the egui `Tab` enum up front — a variant here means a tab that
/// actually renders, so the agent-visible tab list never promises more than the
/// port delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Memory,
    Network,
}

impl Tab {
    pub const PORTED: [Tab; 2] = [Tab::Memory, Tab::Network];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Memory => "Memory",
            Tab::Network => "Network",
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
}

#[derive(Debug)]
pub enum Msg {
    SelectTab(Tab),
    Refresh,
    MemoryLoaded(Result<MemoryView, String>),
    NetworkLoaded(Result<NetworkView, String>),
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
        }
    }
}

/// Rows are laid out top-down at a fixed line height; returns each row's rect.
fn rows(area: Rect, count: usize) -> Vec<Rect> {
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
                let r = rows(area, 9);
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
                let r = rows(area, 2 + n.interfaces.len().min(16));
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
            };
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

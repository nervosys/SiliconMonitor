//! Terminal User Interface (TUI) for Silicon Monitor
//!
//! This module provides an interactive terminal dashboard for real-time hardware monitoring.
//! It displays CPU, GPU, memory, disk, and system information using the ratatui library.

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, prelude::Backend, Terminal};
use std::io;
use std::time::{Duration, Instant};

mod app;
mod ui;

use app::PeripheralCache;
pub use app::{AcceleratorInfo, AcceleratorType, App};

/// Run the TUI application
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal FIRST for immediate visual feedback
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app with fast initialization (slow components load in background)
    let mut app = App::new_fast()?;

    // Draw initial "Loading..." state immediately
    terminal.draw(|f| ui::draw(f, &app))?;

    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// Main application loop with unified tick-based timing
fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    // Unified tick system - base tick is 250ms (4 ticks/sec)
    // Data updates happen on tick multiples; rendering is on-demand (dirty flag).
    const TICK_MS: u64 = 250;
    let tick_duration = Duration::from_millis(TICK_MS);

    // Update frequencies in ticks
    const FAST_UPDATE_TICKS: u64 = 2; // Every 2 ticks (500ms) - CPU, GPU, Memory, Network
    const PROCESS_TICKS: u64 = 4; // Every 4 ticks (1s)
    const DISK_TICKS: u64 = 20; // Every 20 ticks (5s)
    const INIT_CHECK_TICKS: u64 = 1; // Every tick (250ms)

    let mut tick_count: u64 = 0;
    let mut last_tick = Instant::now();
    let mut needs_render = true; // Start dirty so initial frame draws

    // Peripheral refresh runs on a background thread to avoid blocking input
    let peripheral_data = std::sync::Arc::new(std::sync::Mutex::new(PeripheralCache::default()));
    {
        let pd = std::sync::Arc::clone(&peripheral_data);
        std::thread::Builder::new()
            .name("peripheral-refresh".into())
            .spawn(move || loop {
                let mut cache = PeripheralCache::default();
                cache.refresh();
                if let Ok(mut shared) = pd.lock() {
                    *shared = cache;
                }
                std::thread::sleep(Duration::from_secs(10));
            })
            .ok();
    }
    let mut last_peripheral_sync = Instant::now();

    // Create monitor for agent queries (lazy - only used when agent is active)
    let monitor = crate::SiliconMonitor::new()?;

    // Minimum interval between event-driven renders to avoid overwhelming the terminal
    const MIN_RENDER_INTERVAL_MS: u64 = 16; // ~60 FPS cap
    let mut last_render = Instant::now() - Duration::from_millis(MIN_RENDER_INTERVAL_MS);

    loop {
        // Calculate time until next tick
        let elapsed = last_tick.elapsed();
        let timeout = tick_duration.saturating_sub(elapsed);

        // --- Event phase: drain ALL pending key events before rendering ---
        // Use a short poll for the first check (respects tick timeout),
        // then drain remaining events with zero-timeout polls.
        let mut events_processed = false;
        if crossterm::event::poll(timeout)? {
            loop {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press {
                        events_processed = true;
                        // Handle agent input mode separately
                        if app.agent_input_mode {
                            match key.code {
                                KeyCode::Char(c) => app.agent_input_char(c),
                                KeyCode::Backspace => app.agent_input_backspace(),
                                KeyCode::Enter => app.submit_agent_query(&monitor),
                                KeyCode::Esc => app.toggle_agent_input(),
                                _ => {}
                            }
                        } else {
                            use crate::tui::app::ViewMode;
                            match app.view_mode {
                                ViewMode::Main => match key.code {
                                    KeyCode::Char('q') => return Ok(()),
                                    KeyCode::Esc => return Ok(()),
                                    KeyCode::Tab => {
                                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                                            app.previous_tab();
                                        } else {
                                            app.next_tab();
                                        }
                                    }
                                    KeyCode::BackTab => app.previous_tab(),
                                    KeyCode::Char('1') => app.set_tab(0),
                                    KeyCode::Char('2') => app.set_tab(1),
                                    KeyCode::Char('3') => app.set_tab(2),
                                    KeyCode::Char('4') => app.set_tab(3),
                                    KeyCode::Char('5') => app.set_tab(4),
                                    KeyCode::Char('6') => app.set_tab(5),
                                    KeyCode::Char('7') => app.set_tab(6),
                                    KeyCode::Char('8') => app.set_tab(7),
                                    KeyCode::Left => app.previous_tab(),
                                    KeyCode::Right => app.next_tab(),
                                    KeyCode::Up => app.select_process_up(),
                                    KeyCode::Down => app.select_process_down(),
                                    KeyCode::PageUp => app.scroll_page_up(),
                                    KeyCode::PageDown => app.scroll_page_down(),
                                    KeyCode::Home => app.scroll_to_top(),
                                    KeyCode::End => app.scroll_to_bottom(),
                                    KeyCode::Enter => app.open_process_detail(),
                                    KeyCode::Char('p') | KeyCode::Char('P') => {
                                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                                            app.previous_process_mode();
                                        } else {
                                            app.next_process_mode();
                                        }
                                    }
                                    KeyCode::Char('t') | KeyCode::Char('T') => {
                                        app.open_theme_picker()
                                    }
                                    KeyCode::Char('r') => app.reset_stats(),
                                    KeyCode::Char('a') | KeyCode::Char('A') => {
                                        app.toggle_agent_input()
                                    }
                                    KeyCode::Char('c') | KeyCode::Char('C') => {
                                        if app.selected_tab == 7 {
                                            app.clear_agent_history();
                                        }
                                    }
                                    KeyCode::F(12) => {
                                        if let Err(e) = app.save_config() {
                                            app.set_status_message(format!(
                                                "Failed to save config: {}",
                                                e
                                            ));
                                        }
                                    }
                                    _ => {}
                                },
                                ViewMode::ProcessDetail => match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                                        app.close_overlay()
                                    }
                                    KeyCode::Up => app.select_process_up(),
                                    KeyCode::Down => app.select_process_down(),
                                    _ => {}
                                },
                                ViewMode::ThemeSelection => match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => app.close_overlay(),
                                    KeyCode::Up => app.theme_picker_prev(),
                                    KeyCode::Down => app.theme_picker_next(),
                                    KeyCode::Enter => app.apply_selected_theme(),
                                    _ => {}
                                },
                            }
                        }
                    }
                }
                // Check if more events are queued (non-blocking)
                if !crossterm::event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        if events_processed {
            needs_render = true;
        }

        // --- Render phase: only when dirty, rate-limited ---
        if needs_render && last_render.elapsed() >= Duration::from_millis(MIN_RENDER_INTERVAL_MS) {
            terminal.draw(|f| ui::draw(f, app))?;
            last_render = Instant::now();
            needs_render = false;
        }

        // --- Tick phase: data updates ---
        if last_tick.elapsed() >= tick_duration {
            tick_count = tick_count.wrapping_add(1);
            last_tick = Instant::now();

            // Check for background initialization completion
            if tick_count % INIT_CHECK_TICKS == 0 {
                app.check_background_init();
            }

            // Fast updates (CPU, GPU, Memory, Network)
            if tick_count % FAST_UPDATE_TICKS == 0 {
                let _ = app.update_fast();
                needs_render = true;
            }

            // Process updates
            if tick_count % PROCESS_TICKS == 0 {
                let _ = app.update_processes_only();
                needs_render = true;
            }

            // Disk updates (expensive)
            if tick_count % DISK_TICKS == 0 {
                let _ = app.update_disks_only();
                needs_render = true;
            }

            // Sync peripheral data from background thread (cheap lock check)
            if last_peripheral_sync.elapsed() >= Duration::from_secs(2) {
                if let Ok(data) = peripheral_data.try_lock() {
                    if app.peripheral_cache.audio_info != data.audio_info
                        || app.peripheral_cache.usb_info != data.usb_info
                    {
                        app.peripheral_cache = data.clone();
                        needs_render = true;
                    }
                }
                last_peripheral_sync = Instant::now();
            }

            // Mark render needed on ticks that didn't update data
            // (for sparkline animation, clock updates, etc.) - but less frequently
            if tick_count % 4 == 0 {
                needs_render = true;
            }
        }
    }
}

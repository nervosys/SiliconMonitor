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
    // Unified tick system - base tick is 100ms (10 ticks/sec)
    const TICK_MS: u64 = 100;
    let tick_duration = Duration::from_millis(TICK_MS);

    // Update frequencies in ticks (multiples of base tick for consistency)
    const RENDER_TICKS: u64 = 1; // Every tick (100ms = 10 FPS)
    const FAST_UPDATE_TICKS: u64 = 5; // Every 5 ticks (500ms)
    const PROCESS_TICKS: u64 = 10; // Every 10 ticks (1s)
    const DISK_TICKS: u64 = 50; // Every 50 ticks (5s)
    const INIT_CHECK_TICKS: u64 = 2; // Check background init every 200ms

    let mut tick_count: u64 = 0;
    let mut last_tick = Instant::now();

    // Create monitor for agent queries (lazy - only used when agent is active)
    let monitor = crate::SiliconMonitor::new()?;

    loop {
        // Calculate time until next tick
        let elapsed = last_tick.elapsed();
        let timeout = tick_duration.saturating_sub(elapsed);

        // Poll for events with timeout
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
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
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Tab => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.previous_process_mode();
                                } else {
                                    app.next_process_mode();
                                }
                            }
                            KeyCode::BackTab => app.previous_process_mode(),
                            KeyCode::Char('1') => app.set_tab(0),
                            KeyCode::Char('2') => app.set_tab(1),
                            KeyCode::Char('3') => app.set_tab(2),
                            KeyCode::Char('4') => app.set_tab(3),
                            KeyCode::Char('5') => app.set_tab(4),
                            KeyCode::Char('6') => app.set_tab(5),
                            KeyCode::Left => app.previous_tab(),
                            KeyCode::Right => app.next_tab(),
                            KeyCode::Up => app.scroll_up(),
                            KeyCode::Down => app.scroll_down(),
                            KeyCode::PageUp => app.scroll_page_up(),
                            KeyCode::PageDown => app.scroll_page_down(),
                            KeyCode::Home => app.scroll_to_top(),
                            KeyCode::End => app.scroll_to_bottom(),
                            KeyCode::Char('r') => app.reset_stats(),
                            KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_agent_input(),
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                if app.selected_tab == 5 {
                                    app.clear_agent_history();
                                }
                            }
                            KeyCode::F(12) => {
                                if let Err(e) = app.save_config() {
                                    app.set_status_message(format!("Failed to save config: {}", e));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check if a tick has elapsed
        if last_tick.elapsed() >= tick_duration {
            tick_count = tick_count.wrapping_add(1);
            last_tick = Instant::now();

            // Check for background initialization completion
            if tick_count % INIT_CHECK_TICKS == 0 {
                app.check_background_init();
            }

            // Render UI
            if tick_count % RENDER_TICKS == 0 {
                terminal.draw(|f| ui::draw(f, app))?;
            }

            // Fast updates (CPU, GPU, Memory, Network)
            if tick_count % FAST_UPDATE_TICKS == 0 {
                let _ = app.update_fast();
            }

            // Process updates
            if tick_count % PROCESS_TICKS == 0 {
                let _ = app.update_processes_only();
            }

            // Disk updates (expensive)
            if tick_count % DISK_TICKS == 0 {
                let _ = app.update_disks_only();
            }
        }
    }
}

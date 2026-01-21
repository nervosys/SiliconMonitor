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
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new()?;
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

/// Main application loop
fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    let render_rate = Duration::from_millis(100); // Render at 10 FPS for smooth UI
    let fast_update_rate = Duration::from_millis(500); // Fast metrics (CPU/GPU/Memory)
    let slow_update_rate = Duration::from_secs(1); // Process updates (every 1s)
    let disk_update_rate = Duration::from_secs(5); // Disk updates (every 5s - expensive)

    let mut last_render = Instant::now();
    let mut last_fast_update = Instant::now();
    let mut last_slow_update = Instant::now();
    let mut last_disk_update = Instant::now();

    // Create monitor for agent queries
    let monitor = crate::SiliconMonitor::new()?;

    loop {
        // Render UI (fast - 10 FPS)
        if last_render.elapsed() >= render_rate {
            terminal.draw(|f| ui::draw(f, app))?;
            last_render = Instant::now();
        }

        // Calculate timeout for event polling
        let timeout = render_rate
            .checked_sub(last_render.elapsed())
            .unwrap_or_else(|| Duration::from_millis(10));

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
                                // Tab cycles forward through process display modes
                                if key.modifiers.contains(KeyModifiers::SHIFT) {
                                    app.previous_process_mode();
                                } else {
                                    app.next_process_mode();
                                }
                            }
                            KeyCode::BackTab => {
                                // Shift+Tab (BackTab) cycles backward
                                app.previous_process_mode();
                            }
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

        // Fast updates (CPU, GPU, Memory, Network) - every 500ms
        if last_fast_update.elapsed() >= fast_update_rate {
            app.update_fast()?;
            last_fast_update = Instant::now();
        }

        // Process updates - every 1 second
        if last_slow_update.elapsed() >= slow_update_rate {
            app.update_processes_only()?;
            last_slow_update = Instant::now();
        }

        // Disk updates - every 5 seconds (expensive operation)
        if last_disk_update.elapsed() >= disk_update_rate {
            app.update_disks_only()?;
            last_disk_update = Instant::now();
        }
    }
}

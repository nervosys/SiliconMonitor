//! UI rendering functions
//!
//! Glances-inspired single-screen layout with color-coded thresholds:
//! - Header: Title and system info with quicklook summary
//! - Hardware section: CPU, GPU, RAM, Disk, Network with trend indicators
//! - Process section: Sortable process list with color coding
//! - Footer: Help and controls
//!
//! Color thresholds (Glances-style):
//! - OK (Green): 0-50%
//! - CAREFUL (Cyan): 50-70%
//! - WARNING (Yellow): 70-90%
//! - CRITICAL (Red): 90-100%

#[allow(unused_imports)]
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Sparkline, Table, Tabs},
    Frame,
};

use super::app::App;

// ═══════════════════════════════════════════════════════════════════════════════
// GLANCES-STYLE COLOR SYSTEM
// ═══════════════════════════════════════════════════════════════════════════════

/// Glances-style threshold colors
mod glances_colors {
    use ratatui::style::Color;

    /// OK status - safe level (0-50%)
    pub const OK: Color = Color::Green;
    /// CAREFUL status - watch level (50-70%)
    pub const CAREFUL: Color = Color::Cyan;
    /// WARNING status - attention needed (70-90%)
    pub const WARNING: Color = Color::Yellow;
    /// CRITICAL status - urgent (90-100%)
    pub const CRITICAL: Color = Color::Red;
    /// Title/header color
    pub const TITLE: Color = Color::Cyan;
    /// Separator/border color
    pub const SEPARATOR: Color = Color::DarkGray;
    /// Inactive/disabled color
    pub const INACTIVE: Color = Color::DarkGray;
}

/// Get color based on percentage threshold (Glances-style)
/// - 0-50%: Green (OK)
/// - 50-70%: Cyan (CAREFUL)
/// - 70-90%: Yellow (WARNING)
/// - 90-100%: Red (CRITICAL)
fn threshold_color(percent: f32) -> Color {
    match percent {
        p if p >= 90.0 => glances_colors::CRITICAL,
        p if p >= 70.0 => glances_colors::WARNING,
        p if p >= 50.0 => glances_colors::CAREFUL,
        _ => glances_colors::OK,
    }
}

/// Safely clamp a percentage value to 0-100 range for gauge widgets
/// Handles NaN, infinity, and out-of-range values
fn safe_percent(value: f32) -> u16 {
    if value.is_nan() || value.is_infinite() || value < 0.0 {
        0
    } else if value > 100.0 {
        100
    } else {
        value as u16
    }
}

/// Get trend indicator arrow based on value change
/// Returns (arrow, color) tuple
fn trend_indicator(current: f32, previous: f32) -> (&'static str, Color) {
    let delta = current - previous;
    if delta.abs() < 0.5 {
        ("→", Color::DarkGray)
    } else if delta > 0.0 {
        ("↑", Color::Red)
    } else {
        ("↓", Color::Green)
    }
}

/// Format bytes to human-readable with auto unit (Glances-style)
fn auto_unit(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    match bytes {
        b if b >= TB => format!("{:.1}T", b as f64 / TB as f64),
        b if b >= GB => format!("{:.1}G", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1}M", b as f64 / MB as f64),
        b if b >= KB => format!("{:.1}K", b as f64 / KB as f64),
        _ => format!("{}B", bytes),
    }
}

/// Main drawing function - nvtop-style single screen layout with bar gauges
/// Order: CPU(s), Accelerators (GPU/NPU/FPGA/etc.), RAM, Disk(s), Network
pub fn draw(f: &mut Frame, app: &App) {
    // Calculate dynamic constraints based on hardware and available space
    let cpu_section_height: u16 = 3; // 1 CPU bar
    let accelerator_section_height: u16 = if app.accelerators.is_empty() {
        0
    } else {
        (app.accelerators.len() * 3) as u16 // 3 lines per accelerator (compact bar style)
    };
    let ram_section_height: u16 = 3; // 1 RAM bar
    let disk_section_height: u16 = 3; // 1 Disk bar (aggregated)
    let network_section_height: u16 = 3; // 1 Network bar

    let hardware_height = cpu_section_height
        + accelerator_section_height
        + ram_section_height
        + disk_section_height
        + network_section_height;

    // Calculate remaining space for process list
    let total_height = f.area().height;
    let used_height = 3 + hardware_height + 3; // header + hardware + footer
    let process_height = total_height.saturating_sub(used_height).max(5);

    // Build constraints dynamically
    let mut constraints = vec![Constraint::Length(3)]; // Header
    constraints.push(Constraint::Length(cpu_section_height)); // CPU

    if accelerator_section_height > 0 {
        constraints.push(Constraint::Length(accelerator_section_height)); // Accelerators
    }

    constraints.push(Constraint::Length(ram_section_height)); // RAM
    constraints.push(Constraint::Length(disk_section_height)); // Disk
    constraints.push(Constraint::Length(network_section_height)); // Network
    constraints.push(Constraint::Length(process_height)); // Process list (dynamic)
    constraints.push(Constraint::Length(3)); // Footer

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.area());

    let mut chunk_idx = 0;
    draw_nvtop_header(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    // Draw in order: CPU, Accelerators, RAM, Disk, Network
    draw_cpu_bar(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    if accelerator_section_height > 0 {
        draw_accelerators(f, app, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    draw_memory_bar(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    draw_disk_bar(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    draw_network_bar(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    draw_nvtop_processes(f, app, chunks[chunk_idx]);
    chunk_idx += 1;

    draw_nvtop_footer(f, app, chunks[chunk_idx]);

    // Draw overlays based on view mode
    use super::app::ViewMode;
    match app.view_mode {
        ViewMode::Main => {}
        ViewMode::ProcessDetail => draw_process_detail_overlay(f, app),
        ViewMode::ThemeSelection => draw_theme_picker_overlay(f, app),
    }
}

/// Draw header with Glances-style quicklook summary
fn draw_nvtop_header(f: &mut Frame, app: &App, area: Rect) {
    let uptime_secs = app.system_info.uptime.as_secs();
    let days = uptime_secs / 86400;
    let hours = (uptime_secs % 86400) / 3600;
    let minutes = (uptime_secs % 3600) / 60;

    // Format uptime like Glances
    let uptime_str = if days > 0 {
        format!("{}d {:02}:{:02}", days, hours, minutes)
    } else {
        format!("{:02}:{:02}", hours, minutes)
    };

    // CPU with threshold color
    let cpu_color = threshold_color(app.cpu_info.utilization);
    let cpu_span = Span::styled(
        format!("{:.0}%", app.cpu_info.utilization),
        Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
    );

    // Memory with threshold color
    let mem_percent = if app.memory_info.total > 0 {
        (app.memory_info.used as f64 / app.memory_info.total as f64) * 100.0
    } else {
        0.0
    };
    let mem_color = threshold_color(mem_percent as f32);
    let mem_span = Span::styled(
        format!("{:.0}%", mem_percent),
        Style::default().fg(mem_color).add_modifier(Modifier::BOLD),
    );

    // Swap with threshold color
    let swap_percent = if app.memory_info.swap_total > 0 {
        (app.memory_info.swap_used as f64 / app.memory_info.swap_total as f64) * 100.0
    } else {
        0.0
    };
    let swap_color = threshold_color(swap_percent as f32);
    let swap_span = Span::styled(
        format!("{:.0}%", swap_percent),
        Style::default().fg(swap_color).add_modifier(Modifier::BOLD),
    );

    // Network rates
    let net_rx = format_bandwidth(app.network_info.total_rx_rate);
    let net_tx = format_bandwidth(app.network_info.total_tx_rate);

    // Process count
    let proc_count = app.processes.len();

    let header_text = vec![
        Span::styled(
            "Silicon Monitor",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(glances_colors::SEPARATOR)),
        Span::raw(format!(
            "{}@{}",
            app.system_info.hostname, app.system_info.os
        )),
        Span::styled(" │ ", Style::default().fg(glances_colors::SEPARATOR)),
        Span::styled("⏱", Style::default().fg(Color::White)),
        Span::raw(format!(" {} ", uptime_str)),
        Span::styled(" │ ", Style::default().fg(glances_colors::SEPARATOR)),
        // Quicklook style: CPU MEM SWAP NET
        Span::styled("CPU:", Style::default().fg(Color::White)),
        cpu_span,
        Span::raw(" "),
        Span::styled("MEM:", Style::default().fg(Color::White)),
        mem_span,
        Span::raw(" "),
        Span::styled("SWAP:", Style::default().fg(Color::White)),
        swap_span,
        Span::styled(" │ ", Style::default().fg(glances_colors::SEPARATOR)),
        Span::styled("NET:", Style::default().fg(Color::White)),
        Span::styled(
            format!("↓{} ↑{}", net_rx, net_tx),
            Style::default().fg(glances_colors::OK),
        ),
        Span::styled(" │ ", Style::default().fg(glances_colors::SEPARATOR)),
        Span::styled("GPU:", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}", app.accelerators.len()),
            Style::default().fg(glances_colors::TITLE),
        ),
        Span::raw(" "),
        Span::styled("PROC:", Style::default().fg(Color::White)),
        Span::styled(
            format!("{}", proc_count),
            Style::default().fg(glances_colors::TITLE),
        ),
    ];

    let header = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(header, area);
}

/// Draw all accelerators (GPUs, NPUs, FPGAs, etc.) with detailed metrics
fn draw_accelerators(f: &mut Frame, app: &App, area: Rect) {
    if app.accelerators.is_empty() {
        let message = if app.is_loading() {
            "Detecting accelerators..."
        } else {
            "No accelerators detected"
        };
        let no_accel = Paragraph::new(message)
            .block(Block::default().borders(Borders::ALL).title("Accelerators"))
            .alignment(Alignment::Center);
        f.render_widget(no_accel, area);
        return;
    }

    // Split area for each accelerator
    let accel_count = app.accelerators.len();
    let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Ratio(1, accel_count as u32))
        .take(accel_count)
        .collect();

    let accel_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, accel) in app.accelerators.iter().enumerate() {
        draw_single_accelerator(f, accel, idx, accel_chunks[idx]);
    }
}

/// Draw a single accelerator with all its metrics (Glances-style compact format)
fn draw_single_accelerator(
    f: &mut Frame,
    accel: &super::app::AcceleratorInfo,
    idx: usize,
    area: Rect,
) {
    let type_str = format!("{}", accel.accel_type);

    // Build PCIe info for title
    let pcie_str = match (accel.pcie_gen, accel.pcie_width) {
        (Some(gen), Some(width)) => format!(" │ PCIe Gen{}x{}", gen, width),
        _ => String::new(),
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(
            "{} {} │ {} ({}){}",
            type_str, idx, accel.name, accel.vendor, pcie_str
        ),
        Style::default()
            .fg(glances_colors::TITLE)
            .add_modifier(Modifier::BOLD),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Memory percentage for threshold color
    let mem_percent = if accel.memory_total > 0 {
        (accel.memory_used as f64 / accel.memory_total as f64) * 100.0
    } else {
        0.0
    };

    // Build fan speed string
    let fan_str = match (accel.fan_speed_rpm, accel.fan_speed_percent) {
        (Some(rpm), _) => format!(" │ FAN:{}RPM", rpm),
        (None, Some(pct)) => format!(" │ FAN:{:.0}%", pct),
        (None, None) => String::new(),
    };

    // Build encoder/decoder string if active
    let enc_dec_str = {
        let enc = accel.encoder_util.filter(|&u| u > 0.5);
        let dec = accel.decoder_util.filter(|&u| u > 0.5);
        match (enc, dec) {
            (Some(e), Some(d)) => format!(" │ ENC:{:.0}% DEC:{:.0}%", e, d),
            (Some(e), None) => format!(" │ ENC:{:.0}%", e),
            (None, Some(d)) => format!(" │ DEC:{:.0}%", d),
            (None, None) => String::new(),
        }
    };

    // Compact: All key metrics with Glances-style formatting
    let accel_util_label = format!(
        "{}: {:.0}% @ {} MHz │ MEM: {}/{} ({:.0}%) @ {} MHz │ {:.0}°C │ {:.0}/{:.0}W{}{}",
        type_str,
        accel.utilization,
        accel.clock_core.unwrap_or(0),
        auto_unit(accel.memory_used),
        auto_unit(accel.memory_total),
        mem_percent,
        accel.clock_memory.unwrap_or(0),
        accel.temperature.unwrap_or(0.0),
        accel.power.unwrap_or(0.0),
        accel.power_limit.unwrap_or(0.0),
        fan_str,
        enc_dec_str
    );

    let accel_color = threshold_color(accel.utilization);

    let accel_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(accel_color)
                .add_modifier(Modifier::BOLD),
        )
        .percent(safe_percent(accel.utilization))
        .label(accel_util_label);
    f.render_widget(accel_gauge, inner);
}

/// Draw all GPU bars with detailed metrics (nvtop style) - DEPRECATED, use draw_accelerators
#[allow(dead_code)]
fn draw_nvtop_gpus(f: &mut Frame, app: &App, area: Rect) {
    if app.gpu_info.is_empty() {
        let no_gpu = Paragraph::new("No GPUs detected")
            .block(Block::default().borders(Borders::ALL).title("GPUs"))
            .alignment(Alignment::Center);
        f.render_widget(no_gpu, area);
        return;
    }

    // Split area for each GPU
    let gpu_count = app.gpu_info.len();
    let constraints: Vec<Constraint> = std::iter::repeat(Constraint::Ratio(1, gpu_count as u32))
        .take(gpu_count)
        .collect();

    let gpu_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, gpu) in app.gpu_info.iter().enumerate() {
        draw_single_gpu(f, gpu, idx, gpu_chunks[idx]);
    }
}

/// Draw a single GPU with all its metrics (Glances-style compact format) - DEPRECATED, use draw_single_accelerator
#[allow(dead_code)]
fn draw_single_gpu(f: &mut Frame, gpu: &super::app::GpuInfo, idx: usize, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!("GPU {} │ {} ({})", idx, gpu.name, gpu.vendor),
        Style::default()
            .fg(glances_colors::TITLE)
            .add_modifier(Modifier::BOLD),
    ));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Memory percentage for threshold color
    let mem_percent = if gpu.memory_total > 0 {
        (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0
    } else {
        0.0
    };

    // Compact: All key metrics with Glances-style formatting
    let gpu_util_label = format!(
        "GPU: {:.0}% @ {} MHz │ MEM: {}/{} ({:.0}%) @ {} MHz │ {:.0}°C │ {:.0}/{:.0}W",
        gpu.utilization,
        gpu.clock_graphics.unwrap_or(0),
        auto_unit(gpu.memory_used),
        auto_unit(gpu.memory_total),
        mem_percent,
        gpu.clock_memory.unwrap_or(0),
        gpu.temperature.unwrap_or(0.0),
        gpu.power.unwrap_or(0.0),
        gpu.power_limit.unwrap_or(0.0)
    );

    let gpu_color = threshold_color(gpu.utilization);

    let gpu_gauge = Gauge::default()
        .gauge_style(Style::default().fg(gpu_color).add_modifier(Modifier::BOLD))
        .percent(safe_percent(gpu.utilization))
        .label(gpu_util_label);
    f.render_widget(gpu_gauge, inner);
}

/// Draw system monitoring graphs (DEPRECATED - bars now drawn individually in order)
#[allow(dead_code)]
fn draw_system_graphs(f: &mut Frame, app: &App, area: Rect) {
    let graph_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // CPU bar
            Constraint::Length(3), // RAM bar
            Constraint::Length(3), // Disk bar
            Constraint::Length(3), // Network bar
        ])
        .split(area);

    draw_cpu_bar(f, app, graph_chunks[0]);
    draw_memory_bar(f, app, graph_chunks[1]);
    draw_disk_bar(f, app, graph_chunks[2]);
    draw_network_bar(f, app, graph_chunks[3]);
}

/// Draw CPU utilization bar gauge with Glances-style formatting and per-core mini-bars
fn draw_cpu_bar(f: &mut Frame, app: &App, area: Rect) {
    // Get previous CPU value for trend indicator
    let prev_cpu = app
        .cpu_history
        .iter()
        .rev()
        .nth(1)
        .map(|&v| v as f32)
        .unwrap_or(app.cpu_info.utilization);
    let (trend_arrow, _trend_color) = trend_indicator(app.cpu_info.utilization, prev_cpu);

    // Build per-core mini display (compact, shows as colored blocks)
    let per_core_display: String = if !app.cpu_info.per_core_usage.is_empty() {
        // Show first 16 cores with mini-bars, then ... if more
        let cores_to_show = app.cpu_info.per_core_usage.len().min(16);
        let mini_bars: String = app.cpu_info.per_core_usage[..cores_to_show]
            .iter()
            .map(|&u| {
                // Use block characters for mini utilization bars
                match u as u32 {
                    0..=12 => '▁',
                    13..=25 => '▂',
                    26..=37 => '▃',
                    38..=50 => '▄',
                    51..=62 => '▅',
                    63..=75 => '▆',
                    76..=87 => '▇',
                    _ => '█',
                }
            })
            .collect();

        if app.cpu_info.per_core_usage.len() > 16 {
            format!("[{}…]", mini_bars)
        } else {
            format!("[{}]", mini_bars)
        }
    } else {
        String::new()
    };

    let cpu_label = format!(
        "CPU {} {:.0}% │ {} cores @ {} MHz │ {:.0}°C {}",
        trend_arrow,
        app.cpu_info.utilization,
        app.cpu_info.cores,
        app.cpu_info.frequency.unwrap_or(0),
        app.cpu_info.temperature.unwrap_or(0.0),
        per_core_display
    );

    let cpu_color = threshold_color(app.cpu_info.utilization);

    let cpu_gauge = Gauge::default()
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                "CPU",
                Style::default()
                    .fg(glances_colors::TITLE)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .gauge_style(Style::default().fg(cpu_color).add_modifier(Modifier::BOLD))
        .percent(safe_percent(app.cpu_info.utilization))
        .label(cpu_label);

    f.render_widget(cpu_gauge, area);
}

/// Draw memory utilization bar gauge with Glances-style formatting
fn draw_memory_bar(f: &mut Frame, app: &App, area: Rect) {
    let mem_percent = (app.memory_info.used as f64 / app.memory_info.total.max(1) as f64) * 100.0;

    // Get previous memory value for trend indicator
    let prev_mem = app
        .memory_history
        .iter()
        .rev()
        .nth(1)
        .map(|&v| v as f32)
        .unwrap_or(mem_percent as f32);
    let (trend_arrow, _) = trend_indicator(mem_percent as f32, prev_mem);

    let mem_label = format!(
        "MEM {} {:.0}% │ {}/{} │ SWAP: {}",
        trend_arrow,
        mem_percent,
        auto_unit(app.memory_info.used),
        auto_unit(app.memory_info.total),
        auto_unit(app.memory_info.swap_used)
    );

    let mem_color = threshold_color(mem_percent as f32);

    let mem_gauge = Gauge::default()
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                "Memory",
                Style::default()
                    .fg(glances_colors::TITLE)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .gauge_style(Style::default().fg(mem_color).add_modifier(Modifier::BOLD))
        .percent(safe_percent(mem_percent as f32))
        .label(mem_label);

    f.render_widget(mem_gauge, area);
}

/// Draw disk usage bar gauge with Glances-style auto units
fn draw_disk_bar(f: &mut Frame, app: &App, area: Rect) {
    let total_space: u64 = app.disk_info.iter().map(|d| d.total).sum();
    let used_space: u64 = app.disk_info.iter().map(|d| d.used).sum();
    let total_read: f64 = app.disk_info.iter().map(|d| d.read_rate).sum();
    let total_write: f64 = app.disk_info.iter().map(|d| d.write_rate).sum();
    let disk_percent = if total_space > 0 {
        (used_space as f64 / total_space as f64) * 100.0
    } else {
        0.0
    };

    // Build disk list string with Glances-style formatting
    let disk_list: Vec<String> = app
        .disk_info
        .iter()
        .take(3)
        .map(|d| {
            let percent = if d.total > 0 {
                (d.used as f64 / d.total as f64) * 100.0
            } else {
                0.0
            };
            format!(
                "{}:{:.0}%",
                d.name.chars().take(20).collect::<String>(),
                percent
            )
        })
        .collect();

    // Format I/O rates
    let io_str = if total_read > 0.0 || total_write > 0.0 {
        format!(
            " │ R:{}/s W:{}/s",
            auto_unit(total_read as u64),
            auto_unit(total_write as u64)
        )
    } else {
        String::new()
    };

    let disk_label = if !disk_list.is_empty() {
        format!(
            "DISK {:.0}% │ {}/{}{}│ {}",
            disk_percent,
            auto_unit(used_space),
            auto_unit(total_space),
            io_str,
            disk_list.join(" ")
        )
    } else {
        format!(
            "DISK {:.0}% │ {}/{}{} │ No disks",
            disk_percent,
            auto_unit(used_space),
            auto_unit(total_space),
            io_str
        )
    };

    let disk_color = threshold_color(disk_percent as f32);

    let disk_gauge = Gauge::default()
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                "Disk",
                Style::default()
                    .fg(glances_colors::TITLE)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .gauge_style(Style::default().fg(disk_color).add_modifier(Modifier::BOLD))
        .percent(safe_percent(disk_percent as f32))
        .label(disk_label);

    f.render_widget(disk_gauge, area);
}

/// Draw network bar gauge with Glances-style formatting
/// Draw network bar gauge with Glances-style formatting and real bandwidth data
fn draw_network_bar(f: &mut Frame, app: &App, area: Rect) {
    let net_info = &app.network_info;

    // Format bandwidth rates
    let rx_rate = format_bandwidth(net_info.total_rx_rate);
    let tx_rate = format_bandwidth(net_info.total_tx_rate);

    // Build interface list (top 3 by activity)
    let mut active_ifaces: Vec<_> = net_info
        .interfaces
        .iter()
        .filter(|i| i.is_up && (i.rx_rate > 0.0 || i.tx_rate > 0.0))
        .collect();
    active_ifaces.sort_by(|a, b| {
        let a_total = a.rx_rate + a.tx_rate;
        let b_total = b.rx_rate + b.tx_rate;
        b_total
            .partial_cmp(&a_total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let iface_list: String = if !active_ifaces.is_empty() {
        active_ifaces
            .iter()
            .take(2)
            .map(|i| {
                let speed = i
                    .speed_mbps
                    .map(|s| format!(" {}Mb", s))
                    .unwrap_or_default();
                format!(
                    "{}{}↓{} ↑{}",
                    i.name.chars().take(8).collect::<String>(),
                    speed,
                    format_bandwidth(i.rx_rate),
                    format_bandwidth(i.tx_rate)
                )
            })
            .collect::<Vec<_>>()
            .join(" │ ")
    } else if !net_info.interfaces.is_empty() {
        // Show first interface even if inactive
        net_info
            .interfaces
            .iter()
            .take(2)
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        "No interfaces".to_string()
    };

    let net_label = format!(
        "NET ↓{} ↑{} │ Total: ↓{} ↑{} │ {}",
        rx_rate,
        tx_rate,
        auto_unit(net_info.total_rx_bytes),
        auto_unit(net_info.total_tx_bytes),
        iface_list
    );

    // Calculate a visual gauge based on activity (normalize to something reasonable)
    // Use logarithmic scale for better visualization
    let max_rate = (net_info.total_rx_rate + net_info.total_tx_rate).max(1.0);
    let gauge_percent = ((max_rate.log10() + 3.0) * 10.0).clamp(0.0, 100.0) as u16;

    // Color based on activity
    let net_color =
        if net_info.total_rx_rate > 10_000_000.0 || net_info.total_tx_rate > 10_000_000.0 {
            glances_colors::WARNING // > 10 MB/s
        } else if net_info.total_rx_rate > 1_000_000.0 || net_info.total_tx_rate > 1_000_000.0 {
            glances_colors::CAREFUL // > 1 MB/s
        } else {
            glances_colors::OK
        };

    let net_gauge = Gauge::default()
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                "Network",
                Style::default()
                    .fg(glances_colors::TITLE)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .gauge_style(Style::default().fg(net_color).add_modifier(Modifier::BOLD))
        .percent(gauge_percent)
        .label(net_label);

    f.render_widget(net_gauge, area);
}

/// Format bandwidth to human-readable with auto unit (B/s, KB/s, MB/s, GB/s)
fn format_bandwidth(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.1}G/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1}M/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.0}K/s", bytes_per_sec / KB)
    } else if bytes_per_sec > 0.0 {
        format!("{:.0}B/s", bytes_per_sec)
    } else {
        "0B/s".to_string()
    }
}

/// Draw CPU utilization graph with sparkline (DEPRECATED - use draw_cpu_bar)
#[allow(dead_code)]
fn draw_cpu_graph(f: &mut Frame, app: &App, area: Rect) {
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25), // Info
            Constraint::Min(0),     // Graph
        ])
        .split(area);

    // CPU info
    let cpu_text = vec![
        Line::from(format!("CPU: {:.0}%", app.cpu_info.utilization)),
        Line::from(format!("{} cores", app.cpu_info.cores,)),
        Line::from(format!("@ {} MHz", app.cpu_info.frequency.unwrap_or(0))),
    ];
    let cpu_info = Paragraph::new(cpu_text)
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .style(Style::default().fg(Color::White));
    f.render_widget(cpu_info, inner_chunks[0]);

    // CPU history sparkline
    let cpu_data: Vec<u64> = app.cpu_history.iter().copied().collect();
    if !cpu_data.is_empty() {
        let sparkline = Sparkline::default()
            .block(Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM))
            .data(&cpu_data)
            .style(Style::default().fg(usage_color(app.cpu_info.utilization)));
        f.render_widget(sparkline, inner_chunks[1]);
    }
}

/// Draw memory utilization graph with sparkline (DEPRECATED - use draw_memory_bar)
#[allow(dead_code)]
fn draw_memory_graph(f: &mut Frame, app: &App, area: Rect) {
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25), // Info
            Constraint::Min(0),     // Graph
        ])
        .split(area);

    // Memory info
    let mem_used_gb = app.memory_info.used as f64 / (1024.0 * 1024.0 * 1024.0);
    let mem_total_gb = app.memory_info.total as f64 / (1024.0 * 1024.0 * 1024.0);
    let mem_percent = (mem_used_gb / mem_total_gb) * 100.0;

    let mem_text = vec![
        Line::from(format!("RAM: {:.0}%", mem_percent)),
        Line::from(format!("{:.1} GB", mem_used_gb)),
        Line::from(format!("/ {:.1} GB", mem_total_gb)),
    ];
    let mem_info = Paragraph::new(mem_text)
        .block(Block::default().borders(Borders::ALL).title("Memory"))
        .style(Style::default().fg(Color::White));
    f.render_widget(mem_info, inner_chunks[0]);

    // Memory history sparkline
    let mem_data: Vec<u64> = app.memory_history.iter().copied().collect();
    if !mem_data.is_empty() {
        let sparkline = Sparkline::default()
            .block(Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM))
            .data(&mem_data)
            .style(Style::default().fg(usage_color(mem_percent as f32)));
        f.render_widget(sparkline, inner_chunks[1]);
    }
}

/// Draw disk I/O information (DEPRECATED - use draw_disk_bar)
#[allow(dead_code)]
fn draw_disk_graph(f: &mut Frame, app: &App, area: Rect) {
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25), // Info
            Constraint::Min(0),     // Stats
        ])
        .split(area);

    // Disk summary
    let _total_disks = app.disk_info.len();
    let total_space: u64 = app.disk_info.iter().map(|d| d.total).sum();
    let used_space: u64 = app.disk_info.iter().map(|d| d.used).sum();
    let total_gb = total_space as f64 / (1024.0 * 1024.0 * 1024.0);
    let used_gb = used_space as f64 / (1024.0 * 1024.0 * 1024.0);
    let disk_percent = if total_space > 0 {
        (used_gb / total_gb) * 100.0
    } else {
        0.0
    };

    let disk_text = vec![
        Line::from(format!("Disk: {:.0}%", disk_percent)),
        Line::from(format!("{:.1} GB", used_gb)),
        Line::from(format!("/ {:.1} GB", total_gb)),
    ];
    let disk_info = Paragraph::new(disk_text)
        .block(Block::default().borders(Borders::ALL).title("Disk"))
        .style(Style::default().fg(Color::White));
    f.render_widget(disk_info, inner_chunks[0]);

    // Disk list spanning full width
    if !app.disk_info.is_empty() {
        let disk_items: Vec<Span> = app
            .disk_info
            .iter()
            .map(|disk| {
                let used = disk.used as f64 / (1024.0 * 1024.0 * 1024.0);
                let total = disk.total as f64 / (1024.0 * 1024.0 * 1024.0);
                let percent = if total > 0.0 {
                    (used / total) * 100.0
                } else {
                    0.0
                };
                Span::styled(
                    format!(" {}: {:.0}% ", disk.name, percent),
                    Style::default().fg(usage_color(percent as f32)),
                )
            })
            .collect();

        let disk_list = Paragraph::new(Line::from(disk_items))
            .block(Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM))
            .style(Style::default().fg(Color::White));
        f.render_widget(disk_list, inner_chunks[1]);
    } else {
        let no_disk = Paragraph::new("No disks detected")
            .block(Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(no_disk, inner_chunks[1]);
    }
}

/// Draw network information (DEPRECATED - use draw_network_bar)
#[allow(dead_code)]
fn draw_network_graph(f: &mut Frame, _app: &App, area: Rect) {
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25), // Info
            Constraint::Min(0),     // Stats
        ])
        .split(area);

    // Network summary (placeholder for now)
    let net_text = vec![
        Line::from("Network: N/A"),
        Line::from(""),
        Line::from("(Win: Not impl)"),
    ];
    let net_info = Paragraph::new(net_text)
        .block(Block::default().borders(Borders::ALL).title("Network"))
        .style(Style::default().fg(Color::White));
    f.render_widget(net_info, inner_chunks[0]);

    // Network placeholder
    let net_placeholder = Paragraph::new("Network monitoring requires Linux/macOS")
        .block(Block::default().borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM))
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(net_placeholder, inner_chunks[1]);
}

/// Draw GPU processes table (nvtop style)
fn draw_nvtop_processes(f: &mut Frame, app: &App, area: Rect) {
    let mode_name = app.process_mode_name();
    // Use cached process order for smooth scrolling (no sorting during render)
    let processes = app.get_processes_cached();
    let has_gpus = !app.accelerators.is_empty();
    let scroll_pos = app.get_scroll_position();
    let total_count = processes.len();
    let visible_count = 25;

    // Calculate which display row is selected (for highlight bar)
    let selected_display_idx = if app.selected_process_idx >= scroll_pos && app.selected_process_idx < scroll_pos + visible_count {
        Some(app.selected_process_idx - scroll_pos)
    } else {
        None
    };

    // Selection highlight style (Catppuccin Mocha surface0)
    let highlight_style = Style::default().bg(Color::Rgb(69, 71, 90));

    // Get total GPU memory for computing percentages
    let total_gpu_memory: u64 = app.accelerators.iter().map(|a| a.memory_total).sum();

    // Determine columns based on mode - Glances-style headers
    let (header, rows, widths) = match app.process_display_mode {
        super::app::ProcessDisplayMode::All | super::app::ProcessDisplayMode::Cpu => {
            // Include GPU columns if GPUs are present (nvtop-style comprehensive view)
            let header = if has_gpus {
                Row::new(vec![
                    Span::styled(
                        "PID",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "USER",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "S",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "COMMAND",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "CPU%",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "MEM",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "DEV",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "GPU%",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "GMEM",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "GM%",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "E/D",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "TY",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                ])
                .bottom_margin(1)
            } else {
                Row::new(vec![
                    Span::styled(
                        "PID",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "USER",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "S",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "COMMAND",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "CPU%",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "MEM",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "THR",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                    Span::styled(
                        "I/O",
                        Style::default()
                            .fg(glances_colors::TITLE)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    ),
                ])
                .bottom_margin(1)
            };

            let rows: Vec<Row> = processes
                .iter()
                .skip(scroll_pos)
                .take(visible_count)
                .enumerate()
                .map(|(display_idx, p)| {
                    let is_selected = selected_display_idx == Some(display_idx);
                    // Use Glances threshold colors
                    let cpu_color = threshold_color(p.cpu_percent);
                    let gpu_color = threshold_color(p.gpu_usage_percent.unwrap_or(0.0));
                    let enc_color = threshold_color(p.encoder_usage_percent.unwrap_or(0.0));

                    // Calculate GPU memory percentage from total GPU memory
                    let gpu_mem_percent = if total_gpu_memory > 0 && p.total_gpu_memory_bytes > 0 {
                        Some(
                            (p.total_gpu_memory_bytes as f64 / total_gpu_memory as f64 * 100.0)
                                as f32,
                        )
                    } else {
                        p.gpu_memory_percentage
                    };
                    let gpu_mem_pct_color = threshold_color(gpu_mem_percent.unwrap_or(0.0));

                    // Color GPU mem based on usage
                    let gpu_mem_color = if p.total_gpu_memory_bytes > 1024 * 1024 * 1024 {
                        glances_colors::WARNING // > 1GB
                    } else if p.total_gpu_memory_bytes > 256 * 1024 * 1024 {
                        glances_colors::CAREFUL // > 256MB
                    } else if p.total_gpu_memory_bytes > 0 {
                        glances_colors::OK
                    } else {
                        Color::DarkGray
                    };

                    // Process state color
                    let state_color = match p.state {
                        'R' => glances_colors::OK,       // Running
                        'S' => Color::White,             // Sleeping
                        'D' => glances_colors::WARNING,  // Disk sleep
                        'Z' => glances_colors::CRITICAL, // Zombie
                        'T' => glances_colors::CAREFUL,  // Stopped
                        _ => Color::DarkGray,
                    };

                    // GPU process type display
                    let gpu_type_str = match p.gpu_process_type {
                        crate::process_monitor::ProcessGpuType::Graphical => "G",
                        crate::process_monitor::ProcessGpuType::Compute => "C",
                        crate::process_monitor::ProcessGpuType::GraphicalCompute => "M",
                        crate::process_monitor::ProcessGpuType::Unknown => "-",
                    };

                    if has_gpus {
                        // Format GPU indices (e.g., "0", "0,1", or "-" if none)
                        let gpu_idx_str = if p.gpu_indices.is_empty() {
                            "-".to_string()
                        } else {
                            p.gpu_indices
                                .iter()
                                .map(|i| i.to_string())
                                .collect::<Vec<_>>()
                                .join(",")
                        };

                        // Format ENC/DEC combined (show both if available)
                        let enc_dec_str = match (p.encoder_usage_percent, p.decoder_usage_percent) {
                            (Some(e), Some(d)) if e > 0.1 || d > 0.1 => {
                                format!("{:.0}/{:.0}", e, d)
                            }
                            (Some(e), None) if e > 0.1 => format!("{:.0}/-", e),
                            (None, Some(d)) if d > 0.1 => format!("-/{:.0}", d),
                            _ => "-/-".to_string(),
                        };

                        Row::new(vec![
                            Span::styled(
                                format!("{:>7}", p.pid),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!(
                                    "{:<8}",
                                    p.user
                                        .as_deref()
                                        .unwrap_or("?")
                                        .chars()
                                        .take(8)
                                        .collect::<String>()
                                ),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(format!("{}", p.state), Style::default().fg(state_color)),
                            Span::styled(
                                p.name.chars().take(18).collect::<String>(),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!("{:>5.1}%", p.cpu_percent),
                                Style::default().fg(cpu_color),
                            ),
                            Span::styled(
                                format!("{:>6}", auto_unit(p.memory_bytes)),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                gpu_idx_str,
                                Style::default().fg(if p.gpu_indices.is_empty() {
                                    Color::DarkGray
                                } else {
                                    Color::Magenta
                                }),
                            ),
                            Span::styled(
                                p.gpu_usage_percent
                                    .map(|u| format!("{:>4.0}%", u))
                                    .unwrap_or_else(|| "   -".to_string()),
                                Style::default().fg(gpu_color),
                            ),
                            Span::styled(
                                if p.total_gpu_memory_bytes > 0 {
                                    format!("{:>5}", auto_unit(p.total_gpu_memory_bytes))
                                } else {
                                    "    -".to_string()
                                },
                                Style::default().fg(gpu_mem_color),
                            ),
                            Span::styled(
                                gpu_mem_percent
                                    .map(|u| format!("{:>4.1}%", u))
                                    .unwrap_or_else(|| "   -".to_string()),
                                Style::default().fg(gpu_mem_pct_color),
                            ),
                            Span::styled(
                                enc_dec_str,
                                Style::default().fg(
                                    if p.encoder_usage_percent.is_some()
                                        || p.decoder_usage_percent.is_some()
                                    {
                                        enc_color
                                    } else {
                                        Color::DarkGray
                                    },
                                ),
                            ),
                            Span::styled(
                                gpu_type_str.to_string(),
                                Style::default().fg(if p.is_gpu_process() {
                                    glances_colors::CAREFUL
                                } else {
                                    glances_colors::INACTIVE
                                }),
                            ),
                        ]).style(if is_selected { highlight_style } else { Style::default() })
                    } else {
                        // Total I/O (read + write) for display
                        let total_io = p.io_read_bytes + p.io_write_bytes;
                        let io_color = if total_io > 1024 * 1024 * 1024 {
                            glances_colors::WARNING // > 1GB total I/O
                        } else if total_io > 100 * 1024 * 1024 {
                            glances_colors::CAREFUL // > 100MB
                        } else if total_io > 0 {
                            glances_colors::OK
                        } else {
                            Color::DarkGray
                        };

                        // Thread count coloring
                        let thread_color = if p.thread_count > 100 {
                            glances_colors::WARNING
                        } else if p.thread_count > 20 {
                            glances_colors::CAREFUL
                        } else if p.thread_count > 0 {
                            Color::White
                        } else {
                            Color::DarkGray
                        };

                        Row::new(vec![
                            Span::styled(
                                format!("{:>7}", p.pid),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!(
                                    "{:<8}",
                                    p.user
                                        .as_deref()
                                        .unwrap_or("?")
                                        .chars()
                                        .take(8)
                                        .collect::<String>()
                                ),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(format!("{}", p.state), Style::default().fg(state_color)),
                            Span::styled(
                                p.name.chars().take(25).collect::<String>(),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                format!("{:>5.1}%", p.cpu_percent),
                                Style::default().fg(cpu_color),
                            ),
                            Span::styled(
                                format!("{:>6}", auto_unit(p.memory_bytes)),
                                Style::default().fg(Color::White),
                            ),
                            Span::styled(
                                if p.thread_count > 0 {
                                    format!("{:>4}", p.thread_count)
                                } else {
                                    "   -".to_string()
                                },
                                Style::default().fg(thread_color),
                            ),
                            Span::styled(
                                if total_io > 0 {
                                    format!("{:>6}", auto_unit(total_io))
                                } else {
                                    "     -".to_string()
                                },
                                Style::default().fg(io_color),
                            ),
                        ]).style(if is_selected { highlight_style } else { Style::default() })
                    }
                })
                .collect();

            let widths = if has_gpus {
                vec![
                    Constraint::Length(8), // PID
                    Constraint::Length(9), // USER
                    Constraint::Length(2), // S (state)
                    Constraint::Min(10),   // COMMAND
                    Constraint::Length(7), // CPU%
                    Constraint::Length(7), // MEM
                    Constraint::Length(4), // DEV (GPU index)
                    Constraint::Length(6), // GPU%
                    Constraint::Length(6), // GMEM
                    Constraint::Length(6), // GM% (GPU mem percentage)
                    Constraint::Length(6), // E/D (enc/dec combined)
                    Constraint::Length(2), // TY (type)
                ]
            } else {
                vec![
                    Constraint::Length(8), // PID
                    Constraint::Length(9), // USER
                    Constraint::Length(2), // S (state)
                    Constraint::Min(15),   // COMMAND
                    Constraint::Length(7), // CPU%
                    Constraint::Length(7), // MEM
                    Constraint::Length(5), // THR (threads)
                    Constraint::Length(7), // I/O
                ]
            };

            (header, rows, widths)
        }
        super::app::ProcessDisplayMode::Gpu(gpu_idx) => {
            let header = Row::new(vec![
                Span::styled(
                    "PID",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "USER",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "COMMAND",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "GPU MEM",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "GPU%",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "TYPE",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ])
            .bottom_margin(1);

            let rows: Vec<Row> = processes
                .iter()
                .skip(scroll_pos)
                .take(visible_count)
                .enumerate()
                .map(|(display_idx, p)| {
                    let is_selected = selected_display_idx == Some(display_idx);
                    let gpu_mem = p
                        .gpu_memory_per_device
                        .get(&gpu_idx)
                        .map(|&m| auto_unit(m))
                        .unwrap_or_else(|| "0B".to_string());

                    let gpu_usage = p
                        .gpu_usage_percent
                        .map(|u| format!("{:>5.1}%", u))
                        .unwrap_or_else(|| "  N/A".to_string());

                    // Use Glances threshold colors for GPU usage
                    let gpu_color = threshold_color(p.gpu_usage_percent.unwrap_or(0.0));

                    let proc_type = format!("{:?}", p.gpu_process_type);

                    Row::new(vec![
                        Span::styled(format!("{:>7}", p.pid), Style::default().fg(Color::White)),
                        Span::styled(
                            format!(
                                "{:<10}",
                                p.user
                                    .as_deref()
                                    .unwrap_or("?")
                                    .chars()
                                    .take(10)
                                    .collect::<String>()
                            ),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(p.name.clone(), Style::default().fg(Color::White)),
                        Span::styled(format!("{:>7}", gpu_mem), Style::default().fg(gpu_color)),
                        Span::styled(gpu_usage, Style::default().fg(gpu_color)),
                        Span::styled(proc_type, Style::default().fg(glances_colors::INACTIVE)),
                    ]).style(if is_selected { highlight_style } else { Style::default() })
                })
                .collect();

            let widths = vec![
                Constraint::Length(8),  // PID
                Constraint::Length(11), // User
                Constraint::Min(15),    // Name (flexible)
                Constraint::Length(10), // GPU Mem
                Constraint::Length(8),  // GPU%
                Constraint::Length(10), // Type
            ];

            (header, rows, widths)
        }
        super::app::ProcessDisplayMode::Npu(_) => {
            let header = Row::new(vec![Span::styled(
                "No NPU processes available",
                Style::default().fg(glances_colors::INACTIVE),
            )]);
            let widths = vec![Constraint::Percentage(100)];
            (header, vec![], widths)
        }
        super::app::ProcessDisplayMode::Accelerator(accel_idx) => {
            let header = Row::new(vec![
                Span::styled(
                    "PID",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "USER",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "COMMAND",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "ACCEL MEM",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "ACCEL%",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
                Span::styled(
                    "TYPE",
                    Style::default()
                        .fg(glances_colors::TITLE)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ])
            .bottom_margin(1);

            let rows: Vec<Row> = processes
                .iter()
                .skip(scroll_pos)
                .take(visible_count)
                .enumerate()
                .map(|(display_idx, p)| {
                    let is_selected = selected_display_idx == Some(display_idx);
                    let accel_mem = p
                        .gpu_memory_per_device
                        .get(&accel_idx)
                        .map(|&m| auto_unit(m))
                        .unwrap_or_else(|| "0B".to_string());

                    let accel_usage = p
                        .gpu_usage_percent
                        .map(|u| format!("{:>5.1}%", u))
                        .unwrap_or_else(|| "  N/A".to_string());

                    let accel_color = threshold_color(p.gpu_usage_percent.unwrap_or(0.0));

                    let proc_type = format!("{:?}", p.gpu_process_type);

                    Row::new(vec![
                        Span::styled(format!("{:>7}", p.pid), Style::default().fg(Color::White)),
                        Span::styled(
                            format!(
                                "{:<10}",
                                p.user
                                    .as_deref()
                                    .unwrap_or("?")
                                    .chars()
                                    .take(10)
                                    .collect::<String>()
                            ),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(p.name.clone(), Style::default().fg(Color::White)),
                        Span::styled(
                            format!("{:>9}", accel_mem),
                            Style::default().fg(accel_color),
                        ),
                        Span::styled(accel_usage, Style::default().fg(accel_color)),
                        Span::styled(proc_type, Style::default().fg(glances_colors::INACTIVE)),
                    ]).style(if is_selected { highlight_style } else { Style::default() })
                })
                .collect();

            let widths = vec![
                Constraint::Length(8),  // PID
                Constraint::Length(11), // User
                Constraint::Min(15),    // Name (flexible)
                Constraint::Length(10), // Accel Mem
                Constraint::Length(8),  // Accel%
                Constraint::Length(10), // Type
            ];

            (header, rows, widths)
        }
    };

    // Build the title showing scroll position if scrolled
    let title = if scroll_pos > 0 {
        format!(
            "Processes - {} ({}-{} of {})",
            mode_name,
            scroll_pos + 1,
            (scroll_pos + visible_count).min(total_count),
            total_count
        )
    } else {
        format!(
            "Processes - {} ({} of {})",
            mode_name,
            visible_count.min(total_count),
            total_count
        )
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .column_spacing(1);

    f.render_widget(table, area);
}

/// Draw footer with controls (Glances-style hotkey display)
fn draw_nvtop_footer(f: &mut Frame, _app: &App, area: Rect) {
    let help_text = vec![
        Span::styled(
            "q",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Quit  "),
        Span::styled(
            "Tab",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Process  "),
        Span::styled(
            "r",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Reset  "),
        Span::styled(
            "↑↓",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Select  "),
        Span::styled(
            "Enter",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Detail  "),
        Span::styled(
            "t",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Theme  "),
        Span::styled(
            "PgUp/Dn",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Page  "),
        Span::styled(
            "Home/End",
            Style::default()
                .fg(glances_colors::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" Top/Bot  "),
        Span::styled("│", Style::default().fg(glances_colors::SEPARATOR)),
        Span::raw(" "),
        Span::styled("OK", Style::default().fg(glances_colors::OK)),
        Span::raw(":0-50% "),
        Span::styled("CAREFUL", Style::default().fg(glances_colors::CAREFUL)),
        Span::raw(":50-70% "),
        Span::styled("WARNING", Style::default().fg(glances_colors::WARNING)),
        Span::raw(":70-90% "),
        Span::styled("CRITICAL", Style::default().fg(glances_colors::CRITICAL)),
        Span::raw(":90%+"),
    ];

    let help = Paragraph::new(Line::from(help_text))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(help, area);
}

// Keep old functions for potential reuse, but mark as unused for now
#[allow(dead_code)]
fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = app.tabs.iter().map(|t| Line::from(*t)).collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Silicon Monitor"),
        )
        .select(app.selected_tab)
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Cyan)
                .fg(Color::Black),
        );

    f.render_widget(tabs, area);
}

#[allow(dead_code)]
fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    match app.selected_tab {
        0 => draw_overview(f, app, area),
        1 => draw_cpu(f, app, area),
        2 => draw_gpu(f, app, area),
        3 => draw_memory(f, app, area),
        4 => draw_peripherals(f, app, area),
        5 => draw_system(f, app, area),
        6 => draw_agent(f, app, area),
        _ => {}
    }
}

#[allow(dead_code)]
fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    // CPU Overview
    let cpu_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("CPU - {}", app.cpu_info.name));

    let cpu_gauge = Gauge::default()
        .block(cpu_block)
        .gauge_style(
            Style::default()
                .fg(cpu_color(app.cpu_info.utilization))
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .percent(safe_percent(app.cpu_info.utilization))
        .label(format!(
            "{:.1}% | {} cores | {:.0}°C",
            app.cpu_info.utilization,
            app.cpu_info.cores,
            app.cpu_info.temperature.unwrap_or(0.0)
        ));

    f.render_widget(cpu_gauge, chunks[0]);

    // Memory Overview
    let mem_percent = (app.memory_info.used as f64 / app.memory_info.total.max(1) as f64) * 100.0;
    let mem_block = Block::default().borders(Borders::ALL).title("Memory");

    let mem_gauge = Gauge::default()
        .block(mem_block)
        .gauge_style(
            Style::default()
                .fg(usage_color(mem_percent as f32))
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .percent(safe_percent(mem_percent as f32))
        .label(format!(
            "{:.1} GB / {:.1} GB ({:.0}%)",
            app.memory_info.used as f64 / (1024.0 * 1024.0 * 1024.0),
            app.memory_info.total as f64 / (1024.0 * 1024.0 * 1024.0),
            mem_percent
        ));

    f.render_widget(mem_gauge, chunks[1]);

    // GPU Overview
    if !app.gpu_info.is_empty() {
        let gpu = &app.gpu_info[0];
        let gpu_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("GPU - {} ({})", gpu.name, gpu.vendor));

        let gpu_gauge = Gauge::default()
            .block(gpu_block)
            .gauge_style(
                Style::default()
                    .fg(usage_color(gpu.utilization))
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .percent(safe_percent(gpu.utilization))
            .label(format!(
                "{:.0}% | {:.0}°C | {:.0}W / {:.0}W",
                gpu.utilization,
                gpu.temperature.unwrap_or(0.0),
                gpu.power.unwrap_or(0.0),
                gpu.power_limit.unwrap_or(0.0)
            ));

        f.render_widget(gpu_gauge, chunks[2]);
    } else {
        let no_gpu = Paragraph::new("No GPUs detected")
            .block(Block::default().borders(Borders::ALL).title("GPU"))
            .alignment(Alignment::Center);
        f.render_widget(no_gpu, chunks[2]);
    }
}

#[allow(dead_code)]
fn draw_cpu(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // CPU Info
    let info_text = vec![
        Line::from(format!("Name: {}", app.cpu_info.name)),
        Line::from(format!(
            "Cores: {} ({} threads)",
            app.cpu_info.cores, app.cpu_info.threads
        )),
        Line::from(format!("Utilization: {:.1}%", app.cpu_info.utilization)),
        Line::from(format!(
            "Temperature: {:.1}°C",
            app.cpu_info.temperature.unwrap_or(0.0)
        )),
        Line::from(format!(
            "Frequency: {} MHz",
            app.cpu_info.frequency.unwrap_or(0)
        )),
    ];

    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("CPU Information"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(info, chunks[0]);

    // CPU History Graph
    let sparkline_data: Vec<u64> = app.cpu_history.iter().copied().collect();
    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("CPU History (60s)"),
        )
        .data(&sparkline_data)
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(sparkline, chunks[1]);
}

#[allow(dead_code)]
fn draw_gpu(f: &mut Frame, app: &App, area: Rect) {
    if app.gpu_info.is_empty() {
        let no_gpu = Paragraph::new("No GPUs detected")
            .block(Block::default().borders(Borders::ALL).title("GPU"))
            .alignment(Alignment::Center);
        f.render_widget(no_gpu, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let gpu = &app.gpu_info[0];

    // GPU Info - safely calculate memory percentage with clamping
    let mem_percent = if gpu.memory_total > 0 {
        ((gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0).clamp(0.0, 100.0) as u16
    } else {
        0
    };
    let info_text = vec![
        Line::from(format!("Name: {}", gpu.name)),
        Line::from(format!("Vendor: {}", gpu.vendor)),
        Line::from(format!("Utilization: {:.0}%", gpu.utilization)),
        Line::from(format!(
            "Temperature: {:.0}°C",
            gpu.temperature.unwrap_or(0.0)
        )),
        Line::from(format!(
            "Power: {:.0}W / {:.0}W",
            gpu.power.unwrap_or(0.0),
            gpu.power_limit.unwrap_or(0.0)
        )),
        Line::from(format!(
            "Memory: {:.1} GB / {:.1} GB ({:.0}%)",
            gpu.memory_used as f64 / (1024.0 * 1024.0 * 1024.0),
            gpu.memory_total as f64 / (1024.0 * 1024.0 * 1024.0),
            mem_percent
        )),
        Line::from(format!(
            "Graphics Clock: {} MHz",
            gpu.clock_graphics
                .map(|c| c.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        )),
        Line::from(format!(
            "Memory Clock: {} MHz",
            gpu.clock_memory
                .map(|c| c.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        )),
    ];

    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("GPU Information"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(info, chunks[0]);

    // GPU History
    if !app.gpu_histories.is_empty() {
        let sparkline_data: Vec<u64> = app.gpu_histories[0].iter().copied().collect();
        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("GPU Utilization History (60s)"),
            )
            .data(&sparkline_data)
            .style(Style::default().fg(Color::Green));

        f.render_widget(sparkline, chunks[1]);
    }
}

#[allow(dead_code)]
fn draw_memory(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Memory Info
    let used_gb = app.memory_info.used as f64 / (1024.0 * 1024.0 * 1024.0);
    let total_gb = app.memory_info.total as f64 / (1024.0 * 1024.0 * 1024.0);
    let avail_gb = app.memory_info.available as f64 / (1024.0 * 1024.0 * 1024.0);
    let swap_used_gb = app.memory_info.swap_used as f64 / (1024.0 * 1024.0 * 1024.0);
    let swap_total_gb = app.memory_info.swap_total as f64 / (1024.0 * 1024.0 * 1024.0);

    let info_text = vec![
        Line::from(format!("Total: {:.2} GB", total_gb)),
        Line::from(format!("Used: {:.2} GB", used_gb)),
        Line::from(format!("Available: {:.2} GB", avail_gb)),
        Line::from(format!("Usage: {:.1}%", (used_gb / total_gb) * 100.0)),
        Line::from(""),
        Line::from(format!("Swap Total: {:.2} GB", swap_total_gb)),
        Line::from(format!("Swap Used: {:.2} GB", swap_used_gb)),
    ];

    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Memory Information"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(info, chunks[0]);

    // Memory History
    let sparkline_data: Vec<u64> = app.memory_history.iter().copied().collect();
    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Memory Usage History (60s)"),
        )
        .data(&sparkline_data)
        .style(Style::default().fg(Color::Magenta));

    f.render_widget(sparkline, chunks[1]);
}

#[allow(dead_code)]
fn draw_system(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // System Info
    let uptime_secs = app.system_info.uptime.as_secs();
    let days = uptime_secs / 86400;
    let hours = (uptime_secs % 86400) / 3600;
    let minutes = (uptime_secs % 3600) / 60;

    let mut info_lines = vec![
        Line::from(format!("Hostname: {}", app.system_info.hostname)),
        Line::from(format!("OS: {}", app.system_info.os)),
        Line::from(format!("Kernel: {}", app.system_info.kernel)),
        Line::from(format!("Uptime: {}d {}h {}m", days, hours, minutes)),
    ];

    if let Some(ref manufacturer) = app.system_info.manufacturer {
        info_lines.push(Line::from(format!("Manufacturer: {}", manufacturer)));
    }
    if let Some(ref model) = app.system_info.model {
        info_lines.push(Line::from(format!("Model: {}", model)));
    }

    let info = Paragraph::new(info_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("System Information"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(info, chunks[0]);

    // Disk Info
    let disk_items: Vec<ListItem> = app
        .disk_info
        .iter()
        .map(|disk| {
            let used_gb = disk.used as f64 / (1024.0 * 1024.0 * 1024.0);
            let total_gb = disk.total as f64 / (1024.0 * 1024.0 * 1024.0);
            let percent = (used_gb / total_gb) * 100.0;

            ListItem::new(format!(
                "{}: {:.1} GB / {:.1} GB ({:.0}%) - {}",
                disk.name, used_gb, total_gb, percent, disk.mount_point
            ))
        })
        .collect();

    let disks = List::new(disk_items)
        .block(Block::default().borders(Borders::ALL).title("Disks"))
        .style(Style::default().fg(Color::White));

    f.render_widget(disks, chunks[1]);
}

#[allow(dead_code)]
fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    // Check if there's a status message to display
    if let Some(status_msg) = app.get_status_message() {
        let status = Paragraph::new(Line::from(vec![Span::styled(
            status_msg,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        f.render_widget(status, area);
    } else if app.agent_input_mode {
        // Show agent input mode
        let input_text = format!("> {}", app.agent_input);
        let input = Paragraph::new(Line::from(vec![
            Span::styled(
                "Agent Query: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(&input_text),
            Span::styled(
                "█",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::SLOW_BLINK),
            ),
        ]))
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);
        f.render_widget(input, area);
    } else {
        let help_text = vec![
            Span::raw("Press "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to quit | "),
            Span::styled("</", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to switch tabs | "),
            Span::styled("r", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to reset graphs | "),
            Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" for agent | "),
            Span::styled("F12", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to save config"),
        ];

        let help = Paragraph::new(Line::from(help_text))
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);

        f.render_widget(help, area);
    }
}

#[allow(dead_code)]
fn draw_agent(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Info/help
            Constraint::Min(0),    // Conversation history
        ])
        .split(area);

    // Agent info header
    let info_lines = if let Some(ref _agent) = app.agent {
        let cache_stats = app.agent_cache_stats().unwrap_or_default();
        vec![
            Line::from(vec![Span::styled(
                "[AI Agent Active]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(format!("Model: Medium (500M params) | {}", cache_stats)),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to ask a question | "),
                Span::styled("c", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to clear history"),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![Span::styled(
                "❌ AI Agent Unavailable",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("Agent failed to initialize. Check error logs."),
        ]
    };

    let info = Paragraph::new(info_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("AI Agent - Natural Language Queries"),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(info, chunks[0]);

    // Conversation history
    if app.agent_history.is_empty() {
        let help_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "No queries yet. Try asking:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("  * What's my GPU temperature?"),
            Line::from("  * How much power am I using?"),
            Line::from("  * Show GPU utilization"),
            Line::from("  * Is my system healthy?"),
            Line::from("  * Compare GPU temperatures"),
            Line::from("  * What's my memory usage?"),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(" to start asking questions"),
            ]),
        ];

        let help = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Examples"))
            .alignment(Alignment::Left);

        f.render_widget(help, chunks[1]);
    } else {
        // Show conversation history (most recent first)
        let history_items: Vec<ListItem> = app
            .agent_history
            .iter()
            .rev() // Show newest first
            .enumerate()
            .flat_map(|(i, response)| {
                let time_str = format!(
                    "[{}ms{}]",
                    response.inference_time_ms,
                    if response.from_cache { ", cached" } else { "" }
                );

                vec![
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("Q{}: ", app.agent_history.len() - i),
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(&response.query),
                        Span::styled(
                            format!(" {}", time_str),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])),
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            "A:  ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(&response.response, Style::default().fg(Color::White)),
                    ])),
                    ListItem::new(Line::from("")), // Spacer
                ]
            })
            .collect();

        let history = List::new(history_items)
            .block(Block::default().borders(Borders::ALL).title(format!(
                "Conversation History ({} queries)",
                app.agent_history.len()
            )))
            .style(Style::default().fg(Color::White));

        f.render_widget(history, chunks[1]);
    }
}

#[allow(dead_code)]
fn cpu_color(utilization: f32) -> Color {
    if utilization < 40.0 {
        Color::Green
    } else if utilization < 70.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Get color based on usage percentage (Glances-style thresholds)
fn usage_color(percent: f32) -> Color {
    threshold_color(percent)
}

#[allow(dead_code)]
fn draw_peripherals(f: &mut Frame, _app: &App, area: Rect) {
    use crate::audio::AudioMonitor;
    use crate::bluetooth::BluetoothMonitor;
    use crate::display::DisplayMonitor;
    use crate::usb::UsbMonitor;
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);
    
    // Audio section
    let audio_block = Block::default()
        .borders(Borders::ALL)
        .title("Audio Devices")
        .border_style(Style::default().fg(glances_colors::TITLE));
    
    let audio_info = if let Ok(monitor) = AudioMonitor::new() {
        let devices = monitor.devices();
        if devices.is_empty() {
            "No audio devices detected".to_string()
        } else {
            format!("{} audio device(s) | Volume: {:.0}% | Muted: {}",
                devices.len(), monitor.master_volume().unwrap_or(100) as f32, if monitor.is_muted() { "Yes" } else { "No" })
        }
    } else {
        "Audio monitoring not available".to_string()
    };
    let audio_para = Paragraph::new(audio_info).block(audio_block);
    f.render_widget(audio_para, chunks[0]);
    
    // Bluetooth section
    let bt_block = Block::default()
        .borders(Borders::ALL)
        .title("Bluetooth")
        .border_style(Style::default().fg(glances_colors::TITLE));
    
    let bt_info = if let Ok(monitor) = BluetoothMonitor::new() {
        let adapters = monitor.adapters().len();
        let devices = monitor.devices().len();
        if monitor.is_available() {
            format!("{} adapter(s) | {} device(s) paired", adapters, devices)
        } else {
            "Bluetooth not available".to_string()
        }
    } else {
        "Bluetooth monitoring not available".to_string()
    };
    let bt_para = Paragraph::new(bt_info).block(bt_block);
    f.render_widget(bt_para, chunks[1]);
    
    // Display section
    let display_block = Block::default()
        .borders(Borders::ALL)
        .title("Displays")
        .border_style(Style::default().fg(glances_colors::TITLE));
    
    let display_info = if let Ok(monitor) = DisplayMonitor::new() {
        let displays = monitor.displays();
        if displays.is_empty() {
            "No displays detected".to_string()
        } else {
            let info: Vec<String> = displays.iter().map(|d| {
                format!("{}: {}x{} @ {:.0}Hz", d.name.as_deref().unwrap_or("Display"), d.width, d.height, d.refresh_rate)
            }).collect();
            info.join(" | ")
        }
    } else {
        "Display monitoring not available".to_string()
    };
    let display_para = Paragraph::new(display_info).block(display_block);
    f.render_widget(display_para, chunks[2]);
    
    // USB section
    let usb_block = Block::default()
        .borders(Borders::ALL)
        .title("USB Devices")
        .border_style(Style::default().fg(glances_colors::TITLE));
    
    let usb_info = if let Ok(monitor) = UsbMonitor::new() {
        let devices = monitor.devices();
        if devices.is_empty() {
            "No USB devices detected".to_string()
        } else {
            format!("{} USB device(s) connected", devices.len())
        }
    } else {
        "USB monitoring not available".to_string()
    };
    let usb_para = Paragraph::new(usb_info).block(usb_block);
    f.render_widget(usb_para, chunks[3]);
}
/// Draw process detail overlay
fn draw_process_detail_overlay(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 50, f.area());
    
    // Semi-transparent background
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(137, 180, 250)))
        .title(" Process Detail (Enter/Esc to close) ")
        .title_style(Style::default().fg(Color::Rgb(137, 180, 250)).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(30, 30, 46)));
    
    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);
    
    if let Some(process) = app.get_selected_process() {
        let lines = vec![
            Line::from(vec![
                Span::styled("PID: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(format!("{}", process.pid)),
            ]),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(&process.name),
            ]),
            Line::from(vec![
                Span::styled("User: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(process.user.as_deref().unwrap_or("unknown")),
            ]),
            Line::from(vec![
                Span::styled("CPU: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(format!("{:.1}%", process.cpu_percent)),
            ]),
            Line::from(vec![
                Span::styled("Memory: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(auto_unit(process.memory_bytes)),
            ]),
            Line::from(vec![
                Span::styled("State: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(format!("{}", process.state)),
            ]),
            Line::from(vec![
                Span::styled("Threads: ", Style::default().fg(Color::Rgb(166, 227, 161))),
                Span::raw(format!("{}", process.thread_count)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("GPU Usage: ", Style::default().fg(Color::Rgb(250, 179, 135))),
                Span::raw(process.gpu_usage_percent.map(|u| format!("{:.1}%", u)).unwrap_or_else(|| "-".to_string())),
            ]),
            Line::from(vec![
                Span::styled("GPU Memory: ", Style::default().fg(Color::Rgb(250, 179, 135))),
                Span::raw(if process.total_gpu_memory_bytes > 0 { auto_unit(process.total_gpu_memory_bytes) } else { "-".to_string() }),
            ]),
            Line::from(vec![
                Span::styled("Encoder: ", Style::default().fg(Color::Rgb(250, 179, 135))),
                Span::raw(process.encoder_usage_percent.map(|u| format!("{:.1}%", u)).unwrap_or_else(|| "-".to_string())),
            ]),
            Line::from(vec![
                Span::styled("Decoder: ", Style::default().fg(Color::Rgb(250, 179, 135))),
                Span::raw(process.decoder_usage_percent.map(|u| format!("{:.1}%", u)).unwrap_or_else(|| "-".to_string())),
            ]),
        ];
        let para = Paragraph::new(lines);
        f.render_widget(para, inner);
    }
}

/// Draw theme picker overlay
fn draw_theme_picker_overlay(f: &mut Frame, app: &App) {
    use super::app::ColorTheme;
    
    let themes = ColorTheme::all();
    let height = (themes.len() + 4) as u16;
    let area = centered_rect(40, height.min(20), f.area());
    
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(137, 180, 250)))
        .title(" Select Theme (Enter to apply, Esc to cancel) ")
        .title_style(Style::default().fg(Color::Rgb(137, 180, 250)).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(30, 30, 46)));
    
    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);
    
    let items: Vec<ListItem> = themes
        .iter()
        .enumerate()
        .map(|(idx, theme)| {
            let is_selected = idx == app.selected_theme_idx;
            let is_current = *theme == app.color_theme;
            let name = if is_current {
                format!("{} (current)", theme.name())
            } else {
                theme.name().to_string()
            };
            let style = if is_selected {
                Style::default().bg(Color::Rgb(69, 71, 90)).fg(Color::Rgb(205, 214, 244))
            } else {
                Style::default().fg(Color::Rgb(166, 173, 200))
            };
            ListItem::new(name).style(style)
        })
        .collect();
    
    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Helper to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}



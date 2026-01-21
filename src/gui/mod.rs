//! Silicon Monitor GUI - Cyber-themed hardware monitoring dashboard
//!
//! A modern, visually appealing GUI inspired by GNOME System Monitor and Glances,
//! with a cyberpunk aesthetic featuring neon colors, dark backgrounds, and
//! real-time hardware metrics visualization.

use eframe::egui;

mod app;
mod theme;
mod widgets;

pub use app::SiliconMonitorApp;

/// Run the Silicon Monitor GUI application
pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Silicon Monitor")
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Silicon Monitor",
        options,
        Box::new(|cc| Ok(Box::new(SiliconMonitorApp::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    // Simple default icon - cyan colored "S" pattern
    let size = 32;
    let mut rgba = vec![0u8; size * size * 4];

    // Create a simple "S" shape with cyan color
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let in_border = x < 2 || x >= size - 2 || y < 2 || y >= size - 2;
            let in_s = (y < size / 3 && (x > size / 4 || y < 4))
                || (y >= size / 3 && y < 2 * size / 3 && x > size / 4 && x < 3 * size / 4)
                || (y >= 2 * size / 3 && (x < 3 * size / 4 || y >= size - 4));

            if in_border || in_s {
                rgba[idx] = 0; // R
                rgba[idx + 1] = 255; // G (cyan)
                rgba[idx + 2] = 255; // B (cyan)
                rgba[idx + 3] = 255; // A
            }
        }
    }

    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

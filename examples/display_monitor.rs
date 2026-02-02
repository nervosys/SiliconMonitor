//! Display monitoring example
//!
//! Demonstrates how to enumerate displays and get their properties.
//!
//! Run with: cargo run --example display_monitor

use simon::display::{DisplayConnection, DisplayMonitor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Display Monitor Example ===\n");

    let monitor = DisplayMonitor::new()?;

    let displays = monitor.displays();
    println!("Found {} display(s):\n", displays.len());

    for display in displays {
        let primary = if display.is_primary { " (Primary)" } else { "" };
        let connection_icon = match display.connection {
            DisplayConnection::Hdmi => "📺",
            DisplayConnection::DisplayPort => "🖥️",
            DisplayConnection::Dvi => "🖥️",
            DisplayConnection::Vga => "🖥️",
            DisplayConnection::Internal | DisplayConnection::Edp => "💻",
            DisplayConnection::Usb => "🔌",
            DisplayConnection::Wireless => "📶",
            DisplayConnection::Unknown => "❓",
        };

        let name = display.name.as_deref().unwrap_or("Unknown Display");
        println!("{} {}{}", connection_icon, name, primary);
        println!("  ID: {}", display.id);
        println!("  Resolution: {}x{}", display.width, display.height);
        println!("  Refresh Rate: {} Hz", display.refresh_rate);
        
        if let Some(scale) = display.scale_factor {
            println!("  Scale Factor: {:.0}%", scale * 100.0);
        }
        
        if let (Some(w), Some(h)) = (display.physical_width_mm, display.physical_height_mm) {
            let diagonal_mm = ((w as f64).powi(2) + (h as f64).powi(2)).sqrt();
            let diagonal_inches = diagonal_mm / 25.4;
            println!("  Physical Size: {}mm x {}mm ({:.1}\" diagonal)", w, h, diagonal_inches);
        }
        
        if let Some(bits) = display.bits_per_pixel {
            println!("  Color Depth: {} bits", bits);
        }
        
        println!("  Connection: {:?}", display.connection);
        println!();
    }

    // Summary info
    if !displays.is_empty() {
        let total_pixels: u64 = displays.iter()
            .map(|d| d.width as u64 * d.height as u64)
            .sum();
        let primary_count = displays.iter().filter(|d| d.is_primary).count();
        
        println!("Summary:");
        println!("  Total Displays: {}", displays.len());
        println!("  Primary Displays: {}", primary_count);
        println!("  Total Pixels: {} ({:.2}M)", total_pixels, total_pixels as f64 / 1_000_000.0);
    }

    Ok(())
}

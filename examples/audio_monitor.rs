//! Audio device monitoring example
//!
//! Demonstrates how to enumerate audio devices and check their status.
//!
//! Run with: cargo run --example audio_monitor

use simon::audio::AudioMonitor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Audio Monitor Example ===\n");

    let monitor = AudioMonitor::new()?;

    // Get master volume
    if let Some(volume) = monitor.master_volume() {
        println!("Master Volume: {}%", volume);
    } else {
        println!("Master Volume: Not available");
    }

    // Check mute status
    if let Some(muted) = monitor.is_muted() {
        println!("Muted: {}", if muted { "Yes" } else { "No" });
    }

    println!();

    // List all audio devices
    let devices = monitor.devices();
    println!("Found {} audio device(s):\n", devices.len());

    for device in devices {
        let direction = if device.is_output { "Output" } else { "Input" };
        let default = if device.is_default { " (Default)" } else { "" };
        let enabled = if device.is_enabled { "" } else { " [Disabled]" };

        println!("  {} {}{}{}", 
            if device.is_output { "🔊" } else { "🎤" },
            device.name,
            default,
            enabled
        );
        println!("    ID: {}", device.id);
        println!("    Type: {}", direction);
        if let Some(vol) = device.volume {
            println!("    Volume: {}%", vol);
        }
        println!();
    }

    Ok(())
}

//! Example: USB device event monitoring
//!
//! Demonstrates polling for USB device connect/disconnect events.
//!
//! Run with: cargo run --example usb_events --features cli

use simonlib::usb::{UsbEvent, UsbMonitor};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("USB Device Event Monitor");
    println!("========================");
    println!("Watching for USB device connect/disconnect events...");
    println!("Plug in or remove a USB device to see events.");
    println!("Press Ctrl+C to exit.\n");

    let mut monitor = UsbMonitor::new()?;

    // Initial device list
    println!("Currently connected devices:");
    for device in monitor.devices() {
        println!(
            "  {:04x}:{:04x} - {} ({:?})",
            device.vendor_id,
            device.product_id,
            device.product.as_deref().unwrap_or("Unknown"),
            device.speed
        );
    }
    println!();

    // Poll for events
    loop {
        thread::sleep(Duration::from_secs(1));

        let events = monitor.poll_events()?;

        for event in events {
            match event {
                UsbEvent::Connected(device) => {
                    println!(
                        "[+] CONNECTED: {:04x}:{:04x} - {} ({:?})",
                        device.vendor_id,
                        device.product_id,
                        device.product.as_deref().unwrap_or("Unknown"),
                        device.speed
                    );
                }
                UsbEvent::Disconnected(device) => {
                    println!(
                        "[-] DISCONNECTED: {:04x}:{:04x} - {} ({:?})",
                        device.vendor_id,
                        device.product_id,
                        device.product.as_deref().unwrap_or("Unknown"),
                        device.speed
                    );
                }
            }
        }
    }
}

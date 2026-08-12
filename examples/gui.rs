//! Silicon Monitor GUI Example
//!
//! Launches the graphical user interface for hardware monitoring.
//!
//! # Usage
//! ```bash
//! cargo run --release --features "gui nvidia" --example gui
//! ```

fn main() -> Result<(), eframe::Error> {
    println!("⚡ Starting Silicon Monitor GUI...");
    simonlib::gui::run()
}

//! Continuous monitoring example using Silicon Monitor

use simonlib::gpu::GpuCollection;
use std::error::Error;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Continuous GPU Monitoring ===");
    println!("Press Ctrl+C to stop\n");

    let gpus = GpuCollection::auto_detect()?;

    if gpus.len() == 0 {
        println!("No GPUs detected!");
        return Ok(());
    }

    loop {
        // Get snapshot of all GPUs
        let snapshots = gpus.snapshot_all()?;

        // Clear line and print stats
        print!("\r");

        // Print concise GPU stats
        let gpu_info: Vec<String> = snapshots
            .iter()
            .enumerate()
            .map(|(idx, info)| {
                let clocks = info.dynamic_info.clocks.graphics.unwrap_or(0);
                let power = info
                    .dynamic_info
                    .power
                    .draw
                    .map(|p| format!("{:.1}W", p as f64 / 1000.0))
                    .unwrap_or_else(|| "N/A".to_string());
                format!(
                    "GPU{}: {}%@{}MHz {}",
                    idx, info.dynamic_info.utilization, clocks, power
                )
            })
            .collect();

        print!("{}", gpu_info.join(" | "));

        use std::io::{self, Write};
        io::stdout().flush()?;

        // Sleep for interval
        thread::sleep(Duration::from_millis(500));
    }
}

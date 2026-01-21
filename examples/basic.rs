//! Simple example using Silicon Monitor's new unified API

use simonlib::gpu::GpuCollection;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("=== Silicon Monitor - Basic Example ===\n");

    // Auto-detect all available GPUs
    let gpus = GpuCollection::auto_detect()?;

    println!("Found {} GPU(s)\n", gpus.len());

    // Get snapshot of all GPUs
    println!("=== GPU Information ===");
    for (idx, info) in gpus.snapshot_all()?.iter().enumerate() {
        println!("GPU {}: {}", idx, info.static_info.name);
        println!("  Vendor: {:?}", info.static_info.vendor);
        println!(
            "  Memory: {} / {} MB",
            info.dynamic_info.memory.used / 1024 / 1024,
            info.dynamic_info.memory.total / 1024 / 1024
        );
        println!("  Utilization: {}%", info.dynamic_info.utilization);
        if let Some(temp) = info.dynamic_info.thermal.temperature {
            println!("  Temperature: {}°C", temp);
        }
        if let Some(power_draw) = info.dynamic_info.power.draw {
            println!("  Power: {:.1}W", power_draw as f64 / 1000.0);
        }

        if !info.dynamic_info.processes.is_empty() {
            println!("  Processes: {}", info.dynamic_info.processes.len());
        }
        println!();
    }

    Ok(())
}

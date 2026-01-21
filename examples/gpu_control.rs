//! GPU control example (Jetson only)

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    {
        use simonlib::{core::gpu::GpuStats, SiliconMonitor};

        let mut stats = SiliconMonitor::new()?;
        let snapshot = stats.snapshot()?;

        println!("=== GPU Control Example (Jetson) ===\n");

        // Print current GPU status
        for (name, gpu) in &snapshot.gpus {
            println!("GPU: {}", name);
            println!("  Current Load: {:.1}%", gpu.status.load);

            if let Some(scaling_3d) = gpu.status.scaling_3d {
                println!(
                    "  3D Scaling: {}",
                    if scaling_3d { "Enabled" } else { "Disabled" }
                );
            }

            if let Some(railgate) = gpu.status.railgate {
                println!(
                    "  Railgate: {}",
                    if railgate { "Enabled" } else { "Disabled" }
                );
            }
        }

        // Example: Toggle 3D scaling (requires root permissions)
        println!("\n=== Attempting to toggle 3D scaling ===");
        println!("Note: This requires root permissions on Jetson devices");

        let mut gpu_stats = GpuStats::new();

        // This would toggle 3D scaling (commented out for safety)
        // Uncomment and run with sudo to test
        /*
        if let Some((name, gpu)) = snapshot.gpus.iter().next() {
            if let Some(current_scaling) = gpu.status.scaling_3d {
                println!("Toggling 3D scaling for GPU: {}", name);
                match gpu_stats.set_3d_scaling(name, !current_scaling) {
                    Ok(_) => println!("Successfully toggled 3D scaling"),
                    Err(e) => println!("Error toggling 3D scaling: {}", e),
                }
            }
        }
        */

        println!("\nUncomment the code in examples/gpu_control.rs to test GPU control");
    }

    #[cfg(not(target_os = "linux"))]
    {
        println!("GPU control is only available on Linux Jetson devices");
    }

    Ok(())
}

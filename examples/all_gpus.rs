//! Unified GPU Monitoring Example
//!
//! This example demonstrates monitoring all GPU vendors (NVIDIA, AMD, Intel)
//! using Silicon Monitor's unified Device trait interface.
//!
//! # Usage
//!
//! ```bash
//! # Monitor all GPU types
//! cargo run --example all_gpus --features nvidia,amd,intel
//!
//! # Monitor specific vendor
//! cargo run --example all_gpus --features nvidia
//! cargo run --example all_gpus --features amd
//! cargo run --example all_gpus --features intel
//! ```

use simonlib::gpu::traits::{Device, Vendor};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("═══════════════════════════════════════════════════════════");
    println!("       Silicon Monitor - Unified GPU Monitoring");
    println!("═══════════════════════════════════════════════════════════\n");

    let mut all_devices: Vec<Box<dyn Device>> = Vec::new();
    // Annotated because every insertion sits behind a vendor feature: with none
    // enabled there is nothing for inference to work from and the example fails to
    // compile, which only shows up on a build that is not `--all-features`.
    let mut vendor_counts: std::collections::HashMap<Vendor, usize> =
        std::collections::HashMap::new();

    // Enumerate NVIDIA GPUs
    #[cfg(feature = "nvidia")]
    {
        print!("[SCAN] Scanning for NVIDIA GPUs... ");
        match simonlib::gpu::nvidia_new::enumerate() {
            Ok(devices) => {
                let count = devices.len();
                println!("[OK] Found {}", count);
                vendor_counts.insert(Vendor::Nvidia, count);
                for device in devices {
                    all_devices.push(Box::new(device) as Box<dyn Device>);
                }
            }
            Err(e) => {
                println!("⚠️  None ({})", e);
                vendor_counts.insert(Vendor::Nvidia, 0);
            }
        }
    }

    // Enumerate AMD GPUs
    #[cfg(feature = "amd")]
    {
        print!("[SCAN] Scanning for AMD GPUs... ");
        match simonlib::gpu::amd_rocm::enumerate() {
            Ok(mut devices) => {
                let count = devices.len();
                println!("[OK] Found {}", count);
                vendor_counts.insert(Vendor::Amd, count);
                all_devices.append(&mut devices);
            }
            Err(e) => {
                println!("⚠️  None ({})", e);
                vendor_counts.insert(Vendor::Amd, 0);
            }
        }
    }

    // Enumerate Intel GPUs
    #[cfg(feature = "intel")]
    {
        print!("[SCAN] Scanning for Intel GPUs... ");
        match simonlib::gpu::intel_levelzero::enumerate() {
            Ok(mut devices) => {
                let count = devices.len();
                println!("[OK] Found {}", count);
                vendor_counts.insert(Vendor::Intel, count);
                all_devices.append(&mut devices);
            }
            Err(e) => {
                println!("⚠️  None ({})", e);
                vendor_counts.insert(Vendor::Intel, 0);
            }
        }
    }

    println!();

    if all_devices.is_empty() {
        println!("[ERROR] No GPUs detected!");
        println!("\nThis could mean:");
        println!("  - No supported GPUs are installed");
        println!("  - GPU drivers are not loaded");
        println!("  - Insufficient permissions to access GPU devices");
        return Ok(());
    }

    println!(
        "[INFO] Summary: {} total GPU(s) detected",
        all_devices.len()
    );
    for (vendor, count) in &vendor_counts {
        if *count > 0 {
            println!("   {} {} GPU(s)", count, vendor_name(*vendor));
        }
    }
    println!();

    // Monitor each GPU
    for (i, device) in all_devices.iter().enumerate() {
        println!("┌─────────────────────────────────────────────────────────┐");
        println!(
            "│ GPU #{} - {}                                       ",
            i,
            vendor_name(device.vendor())
        );
        println!("└─────────────────────────────────────────────────────────┘");

        print_gpu_info(device.as_ref())?;
        println!();
    }

    println!("═══════════════════════════════════════════════════════════");
    println!("                  Monitoring Complete");
    println!("═══════════════════════════════════════════════════════════");

    Ok(())
}

fn vendor_name(vendor: Vendor) -> &'static str {
    match vendor {
        Vendor::Nvidia => "NVIDIA",
        Vendor::Amd => "AMD",
        Vendor::Intel => "Intel",
        Vendor::Apple => "Apple",
    }
}

fn print_gpu_info(device: &dyn Device) -> Result<(), Box<dyn std::error::Error>> {
    // Basic info
    println!("\n[MB] Device Information:");
    println!("  Vendor:  {}", vendor_name(device.vendor()));
    println!("  Index:   {}", device.index());
    println!(
        "  Name:    {}",
        device.name().unwrap_or_else(|_| "Unknown".to_string())
    );

    if let Ok(driver) = device.driver_version() {
        println!("  Driver:  {}", driver);
    }

    if let Ok(pci) = device.pci_info() {
        println!("  PCI:     {}", pci.bus_id);
    }

    // Temperature
    if let Ok(temp) = device.temperature() {
        print!("\n[TEMP]  Temperature: ");
        let primary = temp.primary();
        if let Some(t) = primary {
            print!("{:.1}°C", t);
            if t > 80.0 {
                print!(" ⚠️ ");
            }
            println!();
        } else {
            println!("N/A");
        }
    }

    // Power
    if let Ok(power) = device.power() {
        if power.current > 0.0 || power.limit > 0.0 {
            println!("\n[VOLT] Power:");
            if power.current > 0.0 {
                print!("  Draw:    {:.2}W", power.current);
                if power.limit > 0.0 {
                    let percent = (power.current / power.limit) * 100.0;
                    print!(" ({:.0}%)", percent);
                }
                println!();
            }
            if power.limit > 0.0 {
                println!("  Limit:   {:.2}W", power.limit);
            }
        }
    }

    // Clocks
    if let Ok(clocks) = device.clocks() {
        if clocks.graphics > 0 || clocks.memory > 0 {
            println!("\n🔄 Clocks:");
            if clocks.graphics > 0 {
                println!("  GPU:     {} MHz", clocks.graphics);
            }
            if clocks.memory > 0 {
                println!("  Memory:  {} MHz", clocks.memory);
            }
        }
    }

    // Utilization
    if let Ok(util) = device.utilization() {
        if util.gpu > 0.0 || util.memory > 0.0 {
            println!("\n📈 Utilization:");
            if util.gpu > 0.0 {
                println!("  GPU:     {:.1}%", util.gpu);
            }
            if util.memory > 0.0 {
                println!("  Memory:  {:.1}%", util.memory);
            }
        }
    }

    // Memory
    if let Ok(mem) = device.memory() {
        if mem.total > 0 {
            println!("\n💾 Memory:");
            let total_gb = mem.total as f64 / (1024.0 * 1024.0 * 1024.0);
            let used_gb = mem.used as f64 / (1024.0 * 1024.0 * 1024.0);
            let percent = if mem.total > 0 {
                (mem.used as f64 / mem.total as f64) * 100.0
            } else {
                0.0
            };
            println!("  Total:   {:.2} GB", total_gb);
            println!("  Used:    {:.2} GB ({:.0}%)", used_gb, percent);
        }
    }

    // Fan
    if let Ok(Some(fan)) = device.fan_speed() {
        println!("\n[FAN] Fan:");
        match fan {
            simonlib::gpu::traits::FanSpeed::Rpm(rpm) => {
                println!("  Speed:   {} RPM", rpm);
            }
            simonlib::gpu::traits::FanSpeed::Percent(percent) => {
                println!("  Speed:   {}%", percent);
            }
        }
    }

    Ok(())
}

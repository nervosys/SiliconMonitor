//! CPU Monitoring Example
//!
//! This example demonstrates comprehensive CPU monitoring using Silicon Monitor.
//! It shows per-core tracking, frequency monitoring, utilization, and temperature
//! across all supported platforms (Linux, Windows, macOS).

use simonlib::silicon::{CpuClusterType, SiliconMonitor};

#[cfg(target_os = "linux")]
use simonlib::silicon::linux::LinuxSiliconMonitor;

#[cfg(target_os = "windows")]
use simonlib::silicon::windows::WindowsSiliconMonitor;

#[cfg(all(feature = "apple", target_os = "macos"))]
use simonlib::silicon::apple::AppleSiliconMonitor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Silicon Monitor - CPU Monitoring Example ===\n");

    // Create platform-specific monitor
    #[cfg(target_os = "linux")]
    let monitor = LinuxSiliconMonitor::new()?;

    #[cfg(target_os = "windows")]
    let monitor = WindowsSiliconMonitor::new()?;

    #[cfg(all(feature = "apple", target_os = "macos"))]
    let monitor = AppleSiliconMonitor::new()?;

    #[cfg(not(any(
        target_os = "linux",
        target_os = "windows",
        all(feature = "apple", target_os = "macos")
    )))]
    {
        eprintln!("Platform not supported for this example");
        return Ok(());
    }

    // Get CPU information
    let (cores, clusters) = monitor.cpu_info()?;

    println!("CPU Cores: {}", cores.len());
    println!("CPU Clusters: {}\n", clusters.len());

    // Display cluster information
    println!("=== CPU Clusters ===");
    for cluster in &clusters {
        let cluster_name = match cluster.cluster_type {
            CpuClusterType::Performance => "Performance (P-cores)",
            CpuClusterType::Efficiency => "Efficiency (E-cores)",
            CpuClusterType::Standard => "Standard",
        };

        println!("\n{} Cluster:", cluster_name);
        println!("  Cores: {:?}", cluster.core_ids);
        println!("  Average Frequency: {} MHz", cluster.frequency_mhz);
        println!("  Average Utilization: {}%", cluster.utilization);

        if let Some(power) = cluster.power_watts {
            println!("  Power Consumption: {:.2} W", power);
        }
    }

    // Display per-core information
    println!("\n=== Per-Core Details ===");
    for core in &cores {
        let cluster_name = match core.cluster {
            CpuClusterType::Performance => "P",
            CpuClusterType::Efficiency => "E",
            CpuClusterType::Standard => "S",
        };

        print!("Core {:2} [{}]: ", core.id, cluster_name);
        print!("{:4} MHz, ", core.frequency_mhz);
        print!("{:3}% util", core.utilization);

        if let Some(temp) = core.temperature {
            print!(", {}°C", temp);
        }

        println!();
    }

    // Calculate and display summary statistics
    println!("\n=== Summary Statistics ===");

    let total_cores = cores.len();
    let avg_freq = cores.iter().map(|c| c.frequency_mhz as u64).sum::<u64>() / total_cores as u64;
    let avg_util = cores.iter().map(|c| c.utilization as u32).sum::<u32>() / total_cores as u32;

    println!("Total Cores: {}", total_cores);
    println!("Average Frequency: {} MHz", avg_freq);
    println!("Average Utilization: {}%", avg_util);

    // Temperature statistics (if available)
    let temps: Vec<i32> = cores.iter().filter_map(|c| c.temperature).collect();

    if !temps.is_empty() {
        let min_temp = temps.iter().min().unwrap();
        let max_temp = temps.iter().max().unwrap();
        let avg_temp = temps.iter().sum::<i32>() / temps.len() as i32;

        println!("\nTemperature Range: {}°C - {}°C", min_temp, max_temp);
        println!("Average Temperature: {}°C", avg_temp);
    }

    // I/O Controllers
    println!("\n=== I/O Controllers ===");
    match monitor.io_info() {
        Ok(controllers) if !controllers.is_empty() => {
            for ctrl in &controllers {
                println!(
                    "  {} ({}): {:.2} MB/s",
                    ctrl.name, ctrl.controller_type, ctrl.bandwidth_mbps
                );
            }
        }
        Ok(_) => println!("  No I/O controllers detected"),
        Err(e) => println!("  Error reading I/O info: {}", e),
    }

    // Network Silicon
    println!("\n=== Network Interfaces ===");
    match monitor.network_info() {
        Ok(networks) if !networks.is_empty() => {
            for net in &networks {
                println!(
                    "  {} ({} Mbps): RX {:.2} MB/s, TX {:.2} MB/s, {} pkt/s",
                    net.interface,
                    net.link_speed_mbps,
                    net.rx_bandwidth_mbps,
                    net.tx_bandwidth_mbps,
                    net.packet_rate
                );
            }
        }
        Ok(_) => println!("  No network interfaces detected"),
        Err(e) => println!("  Error reading network info: {}", e),
    }

    // Platform-specific information
    #[cfg(target_os = "linux")]
    {
        println!("\n=== Linux-Specific Information ===");
        println!("CPU Governor: Check individual cores for governor settings");
        println!("Thermal Zones: Using hwmon and thermal_zone sensors");
    }

    #[cfg(target_os = "windows")]
    {
        println!("\n=== Windows-Specific Information ===");
        println!("Note: Temperature monitoring requires administrator privileges");
        println!("Using Performance Counters and WMI for monitoring");
    }

    #[cfg(all(feature = "apple", target_os = "macos"))]
    {
        println!("\n=== macOS-Specific Information ===");
        println!("Using powermetrics for comprehensive monitoring");
        println!("Hybrid architecture detected (P-cores + E-cores)");
    }

    Ok(())
}

// Memory Management Example for Simon
//
// Demonstrates comprehensive memory and swap monitoring.

use simonlib::{format_bytes, memory_summary, MemoryMonitor, MemoryPressure, SwapType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧠 Memory & Swap Monitor");
    println!("{}", "═".repeat(60));

    // Get detailed memory information
    let monitor = MemoryMonitor::new()?;

    // === Memory Information ===
    println!("\n📊 Memory Overview");
    println!("────────────────────────────────────────────────────────────");

    let mem = &monitor.memory;
    let pressure = monitor.pressure();

    println!("   {} Memory Pressure: {:?}", pressure.emoji(), pressure);
    println!("   {}", pressure.description());
    println!();
    println!("   Total:      {:>12}", format_bytes(mem.total));
    println!(
        "   Used:       {:>12} ({:.1}%)",
        format_bytes(mem.used),
        mem.usage_percent()
    );
    println!("   Available:  {:>12}", format_bytes(mem.available));
    println!("   Free:       {:>12}", format_bytes(mem.free));

    if mem.cached > 0 || mem.buffers > 0 {
        println!();
        println!("   Cached:     {:>12}", format_bytes(mem.cached));
        println!("   Buffers:    {:>12}", format_bytes(mem.buffers));
    }

    if mem.active > 0 {
        println!();
        println!("   Active:     {:>12}", format_bytes(mem.active));
        println!("   Inactive:   {:>12}", format_bytes(mem.inactive));
    }

    // === Swap Information ===
    println!("\n💾 Swap Information");
    println!("────────────────────────────────────────────────────────────");

    let swap = &monitor.swap;

    if swap.has_swap() {
        let swap_pressure = monitor.swap_pressure();
        println!(
            "   {} Swap Pressure: {:?}",
            swap_pressure.emoji(),
            swap_pressure
        );
        println!();
        println!("   Total:      {:>12}", format_bytes(swap.total));
        println!(
            "   Used:       {:>12} ({:.1}%)",
            format_bytes(swap.used),
            swap.usage_percent()
        );
        println!("   Free:       {:>12}", format_bytes(swap.free));

        if !swap.devices.is_empty() {
            println!("\n   📁 Swap Devices:");
            for device in &swap.devices {
                let type_str = match device.swap_type {
                    SwapType::File => "📄",
                    SwapType::Partition => "💽",
                    SwapType::Zram => "🗜️",
                    SwapType::Unknown => "❓",
                };
                println!(
                    "      {} {} - {} ({:.1}% used, priority {})",
                    type_str,
                    device.path,
                    format_bytes(device.total_bytes),
                    device.usage_percent(),
                    device.priority
                );
            }
        }

        // ZRAM information
        if let Some(ref zram) = swap.zram {
            println!("\n   🗜️ ZRAM (Compressed RAM):");
            println!("      Device:      {}", zram.device);
            println!("      Disk Size:   {}", format_bytes(zram.disksize));
            println!("      Original:    {}", format_bytes(zram.original_bytes));
            println!("      Compressed:  {}", format_bytes(zram.compressed_bytes));
            println!("      Ratio:       {:.2}x", zram.compression_ratio);
            println!("      Algorithm:   {}", zram.algorithm);
            println!(
                "      Savings:     {} ({:.1}%)",
                format_bytes(zram.memory_savings()),
                zram.savings_percent()
            );
        }
    } else {
        println!("   ⚠️  No swap configured");
        println!("   💡 Consider enabling swap for better memory management");
    }

    // === VM Statistics ===
    let stats = &monitor.stats;
    if stats.page_faults_minor > 0 || stats.swap_in > 0 {
        println!("\n📈 VM Statistics");
        println!("────────────────────────────────────────────────────────────");
        println!("   Page faults (minor): {}", stats.page_faults_minor);
        println!("   Page faults (major): {}", stats.page_faults_major);
        println!("   Swap in:             {}", stats.swap_in);
        println!("   Swap out:            {}", stats.swap_out);
        if stats.oom_kills > 0 {
            println!("   ⚠️  OOM kills:        {}", stats.oom_kills);
        }
    }

    // === VM Settings ===
    if !monitor.vm_settings.is_empty() {
        println!("\n⚙️  VM Settings");
        println!("────────────────────────────────────────────────────────────");
        for (key, value) in &monitor.vm_settings {
            let description = match key.as_str() {
                "swappiness" => format!("Swappiness: {} (0=avoid swap, 100=aggressive)", value),
                "vfs_cache_pressure" => format!("VFS Cache Pressure: {}", value),
                "dirty_ratio" => format!("Dirty Ratio: {}%", value),
                "dirty_background_ratio" => format!("Dirty Background Ratio: {}%", value),
                "overcommit_memory" => {
                    let mode = match value.as_str() {
                        "0" => "heuristic",
                        "1" => "always",
                        "2" => "never",
                        _ => value,
                    };
                    format!("Overcommit: {}", mode)
                }
                _ => format!("{}: {}", key, value),
            };
            println!("   {}", description);
        }
    }

    // === Top Memory Consumers ===
    println!("\n🔝 Top Memory Consumers");
    println!("────────────────────────────────────────────────────────────");
    println!(
        "   {:>6}  {:>10}  {:>10}  {:>6}  {}",
        "PID", "RSS", "Virtual", "Mem%", "Name"
    );
    println!("   {}", "─".repeat(54));

    let top_procs = monitor.top_processes(10);
    for proc in top_procs {
        println!(
            "   {:>6}  {:>10}  {:>10}  {:>5.1}%  {}",
            proc.pid,
            format_bytes(proc.rss),
            format_bytes(proc.vms),
            proc.memory_percent,
            if proc.name.len() > 20 {
                &proc.name[..20]
            } else {
                &proc.name
            }
        );
    }

    // === Health Score ===
    println!("\n🏥 Memory Health");
    println!("────────────────────────────────────────────────────────────");

    let score = monitor.health_score();
    let health_bar = create_health_bar(score);
    let health_emoji = match score {
        90..=100 => "💚",
        70..=89 => "💛",
        50..=69 => "🧡",
        _ => "❤️",
    };

    println!("   {} Health Score: {}/100", health_emoji, score);
    println!("   {}", health_bar);

    // === Quick Summary ===
    println!("\n📋 Quick Summary");
    println!("────────────────────────────────────────────────────────────");

    let summary = memory_summary()?;
    println!(
        "   Memory: {:.1}% used ({} / {})",
        summary.memory_percent,
        format_bytes(summary.total_memory - summary.available_memory),
        format_bytes(summary.total_memory)
    );

    if summary.total_swap > 0 {
        println!(
            "   Swap:   {:.1}% used ({} / {})",
            summary.swap_percent,
            format_bytes(summary.used_swap),
            format_bytes(summary.total_swap)
        );
    }

    if summary.has_zram {
        if let Some(ratio) = summary.zram_ratio {
            println!("   ZRAM:   {:.2}x compression ratio", ratio);
        }
    }

    // === Tips ===
    println!("\n💡 Tips");
    println!("────────────────────────────────────────────────────────────");

    match pressure {
        MemoryPressure::Low => {
            println!("   ✅ Memory usage is healthy");
        }
        MemoryPressure::Medium => {
            println!("   ⚠️  Consider closing unused applications");
        }
        MemoryPressure::High => {
            println!("   🟠 High memory pressure detected");
            println!("   • Close memory-intensive applications");
            println!("   • Consider increasing swap or adding ZRAM");
        }
        MemoryPressure::Critical => {
            println!("   🔴 CRITICAL: System may become unstable!");
            println!("   • Close applications immediately");
            println!("   • Save your work");
            println!("   • Consider rebooting if system becomes unresponsive");
        }
    }

    #[cfg(target_os = "linux")]
    {
        println!();
        println!("   Linux commands:");
        println!("   • sudo sysctl vm.swappiness=10  # Reduce swap usage");
        println!("   • sudo sync; echo 3 | sudo tee /proc/sys/vm/drop_caches  # Clear caches");
    }

    #[cfg(windows)]
    {
        println!();
        println!("   Windows tips:");
        println!("   • Use Task Manager (Ctrl+Shift+Esc) to see details");
        println!("   • Adjust virtual memory in System Properties");
    }

    println!("\n📝 API Examples");
    println!("────────────────────────────────────────────────────────────");
    println!("   // Create monitor");
    println!("   let monitor = MemoryMonitor::new()?;");
    println!();
    println!("   // Get memory usage");
    println!("   let usage = monitor.memory.usage_percent();");
    println!("   let pressure = monitor.pressure();");
    println!();
    println!("   // Check swap");
    println!("   if monitor.swap.has_zram() {{");
    println!("       let ratio = monitor.swap.zram.unwrap().compression_ratio;");
    println!("   }}");
    println!();
    println!("   // Get top processes");
    println!("   let top = monitor.top_processes(10);");

    Ok(())
}

fn create_health_bar(score: u32) -> String {
    let filled = (score as usize / 5).min(20);
    let empty = 20 - filled;

    let color = match score {
        90..=100 => "🟩",
        70..=89 => "🟨",
        50..=69 => "🟧",
        _ => "🟥",
    };

    format!(
        "   [{}{}] {}%",
        color.repeat(filled),
        "⬜".repeat(empty),
        score
    )
}

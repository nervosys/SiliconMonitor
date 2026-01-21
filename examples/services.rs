//! System Service Monitoring Example
//!
//! Run with: cargo run --release --example services

use simonlib::services::{
    common_services, get_services_status, is_service_running, service_summary, ServiceMonitor,
    ServiceStatus, StartupType,
};

fn main() {
    println!("🔧 System Service Monitor");
    println!("============================================================\n");

    // Get service summary
    match service_summary() {
        Ok(summary) => {
            println!("📊 Service Summary");
            println!("├─ Total services: {}", summary.total);
            println!(
                "├─ Running: {} ({}%)",
                summary.running,
                if summary.total > 0 {
                    summary.running * 100 / summary.total
                } else {
                    0
                }
            );
            println!("├─ Stopped: {}", summary.stopped);
            println!("├─ Failed: {}", summary.failed);
            println!("└─ Enabled at boot: {}", summary.enabled);
            println!();
        }
        Err(e) => {
            println!("⚠️  Could not get service summary: {}", e);
            println!();
        }
    }

    // Check common services
    println!("🔍 Common Services Status");
    println!("------------------------------------------------------------");
    let common = common_services();
    if let Ok(statuses) = get_services_status(common.clone()) {
        for name in common.iter().take(12) {
            let status = statuses.get(*name).unwrap_or(&ServiceStatus::NotFound);
            let icon = match status {
                ServiceStatus::Running => "✅",
                ServiceStatus::Stopped => "⏹️",
                ServiceStatus::Failed => "❌",
                ServiceStatus::Starting => "🔄",
                ServiceStatus::Stopping => "⏸️",
                ServiceStatus::NotFound => "❓",
                ServiceStatus::Unknown => "❔",
            };
            println!("   {} {:30} {}", icon, name, status);
        }
    }
    println!();

    // Detailed service view
    println!("📋 All Services (first 20)");
    println!("------------------------------------------------------------");
    match ServiceMonitor::new() {
        Ok(monitor) => {
            let services = monitor.services();
            println!(
                "   {:30} {:12} {:12} {:8}",
                "NAME", "STATUS", "STARTUP", "PID"
            );
            println!(
                "   {:30} {:12} {:12} {:8}",
                "─".repeat(30),
                "─".repeat(12),
                "─".repeat(12),
                "─".repeat(8)
            );

            for service in services.iter().take(20) {
                let pid_str = service
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".to_string());

                let status_str = match service.status {
                    ServiceStatus::Running => "\x1b[32mrunning\x1b[0m",
                    ServiceStatus::Stopped => "\x1b[90mstopped\x1b[0m",
                    ServiceStatus::Failed => "\x1b[31mfailed\x1b[0m",
                    ServiceStatus::Starting => "\x1b[33mstarting\x1b[0m",
                    ServiceStatus::Stopping => "\x1b[33mstopping\x1b[0m",
                    _ => "unknown",
                };

                let startup_str = match service.startup_type {
                    StartupType::Automatic => "auto",
                    StartupType::Manual => "manual",
                    StartupType::Disabled => "disabled",
                    StartupType::OnDemand => "on-demand",
                    StartupType::Unknown => "unknown",
                };

                println!(
                    "   {:30} {:12} {:12} {:8}",
                    &service.name[..service.name.len().min(30)],
                    status_str,
                    startup_str,
                    pid_str
                );
            }

            if services.len() > 20 {
                println!("   ... and {} more services", services.len() - 20);
            }
            println!();

            // Running services
            let running = monitor.running_services();
            println!("🟢 Running Services: {}", running.len());

            // Failed services
            let failed = monitor.failed_services();
            if !failed.is_empty() {
                println!("\n❌ Failed Services:");
                for service in failed.iter().take(5) {
                    println!("   • {} - {:?}", service.name, service.error_message);
                }
            }
        }
        Err(e) => {
            println!("   ⚠️  Could not enumerate services: {}", e);
        }
    }
    println!();

    // Check specific service
    println!("🔎 Specific Service Checks");
    println!("------------------------------------------------------------");

    #[cfg(target_os = "windows")]
    {
        let services_to_check = ["wuauserv", "Spooler", "BITS", "WinDefend", "nonexistent"];
        for name in services_to_check {
            let running = is_service_running(name);
            let icon = if running { "✅" } else { "⏹️" };
            println!(
                "   {} {} is {}",
                icon,
                name,
                if running { "running" } else { "not running" }
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        let services_to_check = ["sshd", "docker", "nginx", "cron", "nonexistent"];
        for name in services_to_check {
            let running = is_service_running(name);
            let icon = if running { "✅" } else { "⏹️" };
            println!(
                "   {} {} is {}",
                icon,
                name,
                if running { "running" } else { "not running" }
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        let services_to_check = ["com.apple.dock", "com.apple.Finder", "nonexistent"];
        for name in services_to_check {
            let running = is_service_running(name);
            let icon = if running { "✅" } else { "⏹️" };
            println!(
                "   {} {} is {}",
                icon,
                name,
                if running { "running" } else { "not running" }
            );
        }
    }

    println!();

    // Usage tips
    println!("💡 Usage Tips");
    println!("------------------------------------------------------------");
    #[cfg(target_os = "windows")]
    {
        println!("   • Run as Administrator to control services");
        println!("   • Use 'sc query <name>' to get service details");
        println!("   • Use 'services.msc' for GUI service management");
    }

    #[cfg(target_os = "linux")]
    {
        println!("   • Use sudo to start/stop/enable/disable services");
        println!("   • 'systemctl status <name>' for detailed info");
        println!("   • 'journalctl -u <name>' for service logs");
    }

    #[cfg(target_os = "macos")]
    {
        println!("   • Use sudo for system-level services");
        println!("   • 'launchctl list' shows all services");
        println!("   • Plist files in /Library/LaunchDaemons/");
    }

    println!();
    println!("📝 API Examples");
    println!("------------------------------------------------------------");
    println!("   // Create monitor");
    println!("   let monitor = ServiceMonitor::new()?;");
    println!();
    println!("   // Check if service is running");
    println!("   if monitor.is_active(\"docker\") {{");
    println!("       println!(\"Docker is running!\");");
    println!("   }}");
    println!();
    println!("   // Control services (requires privileges)");
    println!("   monitor.start(\"nginx\")?;");
    println!("   monitor.stop(\"nginx\")?;");
    println!("   monitor.restart(\"nginx\")?;");
    println!("   monitor.enable(\"nginx\")?;  // Enable at boot");
}

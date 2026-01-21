//! Bandwidth Testing Example (iperf-style)
//!
//! Demonstrates network throughput measurement similar to iperf3.
//!
//! Run: cargo run --release --example bandwidth_test

use simonlib::{
    loopback_test, memory_bandwidth_test, quick_bandwidth_estimate, BandwidthConfig, DEFAULT_PORT,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════════════╗");
    println!("║          📶 Bandwidth Tester - Network Throughput Analysis         ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");

    // Memory bandwidth test (always works, no network needed)
    println!("║                                                                    ║");
    println!("║  🧠 Memory Bandwidth Test                                          ║");
    println!("║  ─────────────────────────────────────────────────────────────────║");

    let mem_result = memory_bandwidth_test(Duration::from_secs(2));
    println!(
        "║    Copy Bandwidth: {:.2} GB/s                                      ║",
        mem_result.bandwidth_gbytes_per_sec
    );
    println!(
        "║    Bytes Copied: {} MB                                          ║",
        mem_result.total_bytes_copied / (1024 * 1024)
    );
    println!(
        "║    Duration: {:.2}s                                                ║",
        mem_result.duration_secs
    );
    println!(
        "║    Iterations: {}                                                   ║",
        mem_result.iterations
    );

    // Loopback test (local network stack performance)
    println!("║                                                                    ║");
    println!("║  🔄 Loopback Test (Local Network Stack)                            ║");
    println!("║  ─────────────────────────────────────────────────────────────────║");

    match loopback_test(Duration::from_secs(2)) {
        Ok(result) => {
            println!(
                "║    Loopback Bandwidth: {:.2} Mbps                                  ║",
                result.bandwidth_mbps
            );
            println!(
                "║    Bytes Transferred: {} KB                                     ║",
                result.bytes_transferred / 1024
            );
            println!(
                "║    Duration: {:.3}s                                               ║",
                result.duration_secs
            );
        }
        Err(e) => {
            println!("║    ❌ Failed: {:<54} ║", e);
            println!("║    (This is normal if loopback test server not available)      ║");
        }
    }

    // Quick bandwidth estimate (if internet available)
    println!("║                                                                    ║");
    println!("║  🌐 Quick Internet Bandwidth Estimate                              ║");
    println!("║  ─────────────────────────────────────────────────────────────────║");

    println!("║    Testing connectivity to public servers...                       ║");
    match quick_bandwidth_estimate() {
        Some(mbps) => {
            println!(
                "║    ✅ Estimated Bandwidth: {:.2} Mbps                              ║",
                mbps
            );
        }
        None => {
            println!("║    ❌ Could not estimate bandwidth                               ║");
            println!("║    (Requires internet connectivity or iperf server)             ║");
        }
    }

    // Custom bandwidth test (if you have an iperf server)
    println!("║                                                                    ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");
    println!("║  📚 Custom Test Usage                                              ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");
    println!("║                                                                    ║");
    println!("║  To test against your own iperf3 server:                           ║");
    println!("║                                                                    ║");
    println!("║  1. Start iperf3 server: iperf3 -s                                 ║");
    println!("║                                                                    ║");
    println!("║  2. Use this code:                                                 ║");
    println!("║     ```                                                            ║");
    println!("║     use simonlib::{{bandwidth_test, BandwidthConfig}};                  ║");
    println!("║     use std::time::Duration;                                       ║");
    println!("║                                                                    ║");
    println!("║     let config = BandwidthConfig::default()                        ║");
    println!("║         .with_duration(Duration::from_secs(5));                    ║");
    println!("║                                                                    ║");
    println!(
        "║     let result = bandwidth_test(\"server_ip\", {}, &config)?;       ║",
        DEFAULT_PORT
    );
    println!("║     println!(\"Bandwidth: {{:.2}} Mbps\", result.bandwidth_mbps);      ║");
    println!("║     ```                                                            ║");
    println!("║                                                                    ║");
    println!("║  Configuration options:                                            ║");
    println!("║    - with_duration(Duration)  : Test duration                      ║");
    println!("║    - with_buffer_size(usize)  : Transfer buffer size               ║");
    println!("║    - with_parallel_streams(u8): Parallel connections               ║");
    println!("║    - upload_mode()            : Test upload instead of download    ║");
    println!("║    - with_timeout(Duration)   : Connection timeout                 ║");

    println!("╚════════════════════════════════════════════════════════════════════╝");

    // Demo config builder
    println!();
    println!("BandwidthConfig example:");
    let _config = BandwidthConfig::default()
        .with_duration(Duration::from_secs(10))
        .with_buffer_size(256 * 1024)
        .with_parallel_streams(4)
        .with_timeout(Duration::from_secs(5));

    println!("  Duration: 10s");
    println!("  Buffer: 256KB");
    println!("  Parallel streams: 4");
    println!("  Timeout: 5s");

    Ok(())
}

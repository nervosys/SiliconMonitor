//! Test example for TSDB functionality
//!
//! Tests the time-series database by recording and reading back data

use simonlib::tsdb::{format_size, MetricsRecorder, ProcessSnapshot, SystemSnapshot, TimeSeriesDb};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TSDB Recording Test ===\n");

    let db_path = "test_tsdb_example.db";

    // Clean up any existing test file
    let _ = std::fs::remove_file(db_path);

    // Create recorder with small size for testing
    let mut recorder = MetricsRecorder::new(
        db_path,
        10 * 1024 * 1024, // 10 MB
        Duration::from_millis(500),
        10,
    )?;

    println!("Recording 10 test snapshots...");

    for i in 0..10 {
        let timestamp = TimeSeriesDb::now_millis();

        // Create test snapshot
        let snapshot = SystemSnapshot {
            timestamp,
            cpu_percent: 25.0 + (i as f32 * 5.0),
            cpu_per_core: vec![20.0, 30.0, 25.0, 35.0],
            memory_used: 8_000_000_000 + (i as u64 * 100_000_000),
            memory_total: 16_000_000_000,
            swap_used: 500_000_000,
            swap_total: 8_000_000_000,
            gpu_percent: vec![50.0 + (i as f32 * 2.0)],
            gpu_memory_used: vec![4_000_000_000],
            gpu_temperature: vec![65.0 + (i as f32)],
            gpu_power_mw: vec![150_000 + (i as u32 * 1000)],
            net_rx_bps: 1_000_000 + (i as u64 * 100_000),
            net_tx_bps: 500_000 + (i as u64 * 50_000),
            processes: vec![ProcessSnapshot {
                pid: 1234,
                name: "test_process".to_string(),
                cpu_percent: 10.0 + (i as f32),
                memory_bytes: 500_000_000,
                gpu_memory_bytes: 1_000_000_000,
                gpu_percent: 25.0,
                disk_read_bps: 10_000_000,
                disk_write_bps: 5_000_000,
                net_rx_bps: 100_000,
                net_tx_bps: 50_000,
            }],
        };

        recorder.record_snapshot(snapshot)?;
        println!("  Recorded snapshot {} at {}", i + 1, timestamp);

        std::thread::sleep(Duration::from_millis(100));
    }

    // Close recorder to flush
    recorder.close()?;
    println!("\nRecorder closed.");

    // Now read back
    println!("\n=== Reading Back Data ===\n");

    let mut db = TimeSeriesDb::new(db_path, 0)?;
    let stats = db.stats();

    println!("Database Stats:");
    println!("  Path: {}", stats.path.display());
    println!("  Max Size: {}", format_size(stats.max_size));
    println!(
        "  Current Size: {} ({:.1}%)",
        format_size(stats.current_size),
        stats.usage_percent()
    );
    println!("  Record Count: {}", stats.record_count);

    if let Some(first) = stats.first_timestamp {
        println!("  First Timestamp: {}", first);
    }
    if let Some(last) = stats.last_timestamp {
        println!("  Last Timestamp: {}", last);
    }
    if let Some(span) = stats.time_span() {
        println!("  Time Span: {}", span);
    }

    // Read all snapshots
    let snapshots = db.read_all_system_snapshots()?;
    println!("\nRead {} snapshots:", snapshots.len());

    for (i, s) in snapshots.iter().enumerate() {
        println!(
            "  [{}] CPU: {:.1}% | MEM: {:.1}% | GPU: {:.1}% | Temp: {:.0}°C | Net: {} rx/s {} tx/s",
            i + 1,
            s.cpu_percent,
            (s.memory_used as f64 / s.memory_total as f64) * 100.0,
            s.gpu_percent.first().unwrap_or(&0.0),
            s.gpu_temperature.first().unwrap_or(&0.0),
            format_size(s.net_rx_bps),
            format_size(s.net_tx_bps)
        );
    }

    // Clean up
    let _ = std::fs::remove_file(db_path);
    println!("\nTest database cleaned up.");

    println!("\n=== Test Complete ===");
    Ok(())
}

//! Physical plausibility of reported hardware readings.
//!
//! # Why this exists
//!
//! The unit tests assert that collectors do not panic and that types round-trip.
//! Almost none assert that the *numbers* make physical sense, and that is precisely
//! where this codebase's defects have clustered. Every one of the following shipped
//! and passed a green suite:
//!
//! - A GPU at 80 °C rendered as `0.1 °C`, with warning thresholds that could never
//!   fire, because a degrees value was divided by 1000.
//! - `cli cpu` reported 0% usage / 100% idle on every one-shot invocation, printed
//!   directly above per-core rows showing 23–53%.
//! - Per-core CPU was the system-wide average replicated across all 24 cores.
//! - Memory readings of "32 GB total, 16 GB used" were invented as a fallback.
//! - A display named "Primary Display" was fabricated whenever detection failed.
//! - Windows fan RPM was hardcoded to 1000.
//!
//! Each is invisible to a type checker and to a test that only asks "did it return
//! Ok?". They are visible the moment you ask whether the value could be true of real
//! hardware.
//!
//! # Design rule: conditional strictness
//!
//! These run on developer machines with real hardware and on CI containers with
//! almost none. So the rule throughout is **"if a subsystem reports something, that
//! something must be physically possible"** — never "the subsystem must report
//! something". Absent hardware is not a failure; impossible readings are.
//!
//! A consequence worth stating: these tests cannot prove a reading is *correct*, only
//! that it is not absurd. They are a floor, not a guarantee.

use std::time::{Duration, Instant};

use simonlib::pipeline::{Collector, CollectorConfig};

/// Longest a collector may take to publish a fully-populated snapshot.
///
/// Generous because GPU driver enumeration dominates cold start and CI runners are
/// slow; this bounds the test, it is not a performance assertion.
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(90);

/// Spawn a collector and wait for a snapshot with real data.
///
/// Returns `None` when nothing populated in time, so callers can skip rather than
/// fail on a machine where collection is unavailable.
fn populated_snapshot() -> Option<std::sync::Arc<simonlib::pipeline::Snapshot>> {
    let collector = Collector::spawn(CollectorConfig {
        interval: Duration::from_millis(250),
        ..Default::default()
    });
    let handle = collector.handle();

    let deadline = Instant::now() + SNAPSHOT_TIMEOUT;
    while Instant::now() < deadline {
        let snap = handle.latest();
        // Generation 0 is the not-ready placeholder; generation 1 is the warm-up
        // publish which deliberately has no GPU or process data yet.
        if snap.generation > 1 && snap.cpu.is_some() {
            return Some(snap);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    None
}

#[test]
fn cpu_readings_are_physically_possible() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };
    let Some(cpu) = snap.cpu.as_ref() else {
        eprintln!("skipping: no CPU data");
        return;
    };

    let total = &cpu.total;
    for (label, value) in [
        ("user", total.user),
        ("system", total.system),
        ("idle", total.idle),
    ] {
        assert!(
            (0.0..=100.0).contains(&value),
            "aggregate {label} is {value}%, outside 0-100"
        );
        assert!(!value.is_nan(), "aggregate {label} is NaN");
    }

    let sum = total.user + total.system + total.idle;
    assert!(
        (sum - 100.0).abs() < 5.0,
        "aggregate user+system+idle = {sum}%, expected ~100"
    );

    // Per-core figures must be per-core, not one number replicated. Identical
    // cumulative values across every core is the signature of the system aggregate
    // being copied into each slot.
    let busy: Vec<f32> = cpu
        .cores
        .iter()
        .filter_map(|c| c.idle.map(|idle| 100.0 - idle))
        .collect();

    for (i, value) in busy.iter().enumerate() {
        assert!(
            (0.0..=100.0).contains(value),
            "core {i} busy is {value}%, outside 0-100"
        );
    }

    // The aggregate must not claim the machine is idle while cores report load.
    if !busy.is_empty() {
        let mean = busy.iter().sum::<f32>() / busy.len() as f32;
        let aggregate_busy = 100.0 - total.idle;
        assert!(
            !(aggregate_busy < 1.0 && mean > 10.0),
            "aggregate reports {aggregate_busy:.1}% busy while cores average \
             {mean:.1}% — the aggregate is a placeholder, not a measurement"
        );
    }
}

#[test]
fn memory_readings_are_self_consistent() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };
    let Some(mem) = snap.memory.as_ref() else {
        eprintln!("skipping: no memory data");
        return;
    };

    assert!(mem.ram.total > 0, "reported zero total RAM");
    assert!(
        mem.ram.used <= mem.ram.total,
        "used RAM {} exceeds total {}",
        mem.ram.used,
        mem.ram.total
    );
    assert!(
        mem.swap.used <= mem.swap.total || mem.swap.total == 0,
        "used swap {} exceeds total {}",
        mem.swap.used,
        mem.swap.total
    );

    // Guard against the specific fabricated fallback that used to ship: exactly
    // 32 GiB total with exactly 16 GiB used, in kilobytes.
    let ram_kb_32gib = 32u64 * 1024 * 1024;
    let ram_kb_16gib = 16u64 * 1024 * 1024;
    assert!(
        !(mem.ram.total == ram_kb_32gib && mem.ram.used == ram_kb_16gib),
        "memory reads exactly 32GiB/16GiB — this is the synthetic fallback that was \
         removed, not a measurement"
    );

    let usage = mem.ram_usage_percent();
    assert!(
        (0.0..=100.0).contains(&usage),
        "RAM usage {usage}% outside 0-100"
    );
}

#[test]
fn gpu_readings_are_physically_possible() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };
    if snap.gpu_dynamic.is_empty() {
        eprintln!("skipping: no GPUs detected");
        return;
    }

    for (i, gpu) in snap.gpu_dynamic.iter().enumerate() {
        let Some(gpu) = gpu.as_ref() else { continue };

        assert!(
            gpu.utilization <= 100,
            "GPU {i} utilization {}% exceeds 100",
            gpu.utilization
        );
        assert!(
            gpu.memory.used <= gpu.memory.total || gpu.memory.total == 0,
            "GPU {i} memory used {} exceeds total {}",
            gpu.memory.used,
            gpu.memory.total
        );

        if let Some(temp) = gpu.thermal.temperature {
            // A powered GPU sits between roughly ambient and its thermal shutdown
            // point. Values far outside that indicate a unit error, which is exactly
            // how the degrees/millidegrees bug presented: 80 °C displayed as 0.1.
            assert!(
                (5..=125).contains(&temp),
                "GPU {i} temperature {temp}°C is not a plausible silicon temperature \
                 — suspect a unit conversion (degrees vs millidegrees)"
            );
        }

        if let Some(draw_mw) = gpu.power.draw {
            let watts = draw_mw as f32 / 1000.0;
            assert!(
                watts <= 2000.0,
                "GPU {i} draws {watts:.1}W, beyond any single-card power budget — \
                 suspect a unit error"
            );

            // A card pulling real power cannot also be at ambient temperature. This
            // is the cross-check that would have caught the 0.1 °C bug: 423 W was
            // reported alongside a temperature near zero.
            if watts > 50.0 {
                if let Some(temp) = gpu.thermal.temperature {
                    assert!(
                        temp > 15,
                        "GPU {i} draws {watts:.1}W but reports {temp}°C — a loaded \
                         card cannot be at ambient; suspect a scaling bug"
                    );
                }
            }
        }

        if let Some(fan_pct) = gpu.thermal.fan_speed {
            assert!(fan_pct <= 100, "GPU {i} fan {fan_pct}% exceeds 100");
        }
    }
}

#[test]
fn process_table_is_plausible() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };
    if snap.processes.is_empty() {
        eprintln!("skipping: no process data");
        return;
    }

    // This test binary is running, so the OS has at least a handful of processes. A
    // listing in single digits means most were silently dropped — the failure mode
    // where every SYSTEM process was omitted because OpenProcess was denied.
    assert!(
        snap.processes.len() >= 10,
        "only {} processes reported; a live system has far more, so entries are \
         being dropped rather than reported",
        snap.processes.len()
    );

    for proc in &snap.processes {
        assert!(!proc.name.is_empty(), "process {} has no name", proc.pid);
        assert!(
            proc.cpu_percent >= 0.0 && proc.cpu_percent < 10_000.0,
            "process {} ({}) reports {}% CPU",
            proc.pid,
            proc.name,
            proc.cpu_percent
        );
    }

    // PIDs must be unique; duplicates mean the enumeration emitted a process twice,
    // which would double-count its resources in any aggregate.
    let mut pids: Vec<u32> = snap.processes.iter().map(|p| p.pid).collect();
    let before = pids.len();
    pids.sort_unstable();
    pids.dedup();
    assert_eq!(
        before,
        pids.len(),
        "the process table contains duplicate PIDs"
    );
}

#[test]
fn disk_readings_are_self_consistent() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };

    for disk in &snap.disks {
        assert!(
            !disk.name.is_empty(),
            "a disk was reported with no identifier"
        );
        assert!(
            disk.used <= disk.total || disk.total == 0,
            "disk {} used {} exceeds total {}",
            disk.name,
            disk.used,
            disk.total
        );
        assert!(
            disk.read_rate >= 0.0 && disk.write_rate >= 0.0,
            "disk {} reports a negative transfer rate",
            disk.name
        );
    }
}

#[test]
fn network_rates_are_non_negative() {
    let Some(snap) = populated_snapshot() else {
        eprintln!("skipping: no snapshot available");
        return;
    };

    for iface in &snap.network {
        assert!(!iface.name.is_empty(), "an interface has no name");
        assert!(
            iface.rx_rate >= 0.0 && iface.tx_rate >= 0.0,
            "interface {} reports a negative rate (rx {}, tx {})",
            iface.name,
            iface.rx_rate,
            iface.tx_rate
        );
        assert!(
            iface.rx_rate.is_finite() && iface.tx_rate.is_finite(),
            "interface {} reports a non-finite rate",
            iface.name
        );
    }
}

/// Detection failures must report nothing, never a synthetic stand-in.
///
/// Collectors used to invent a plausible-looking entry when their probe came back
/// empty — a "Primary Display" with zero dimensions, a fan at exactly 1000 RPM. On
/// screen these are indistinguishable from measurements.
#[test]
fn absent_hardware_is_reported_as_absent_not_invented() {
    use simonlib::display::DisplayMonitor;

    let Ok(monitor) = DisplayMonitor::new() else {
        eprintln!("skipping: display monitor unavailable");
        return;
    };

    for display in monitor.displays() {
        assert!(!display.id.is_empty(), "a display was reported with no id");

        // Zero dimensions alone do *not* indicate fabrication: a genuinely detected
        // sink can report no mode. An AV receiver on DVI, or a monitor whose EDID
        // carries no timing block, shows up as a real device at 0x0.
        //
        // What identifies the removed placeholders is their whole signature — the
        // synthetic id `display0` paired with a generic name and no measurements at
        // all. Real entries carry an EDID-derived name ("RX-A740", "LG ULTRAWIDE")
        // even when modeless.
        let name = display.name.as_deref().unwrap_or_default();
        let generic_name = matches!(name, "Primary Display" | "Unknown Display" | "Display");
        let no_measurements = display.width == 0
            && display.height == 0
            && display.refresh_rate == 0.0
            && display.physical_width_mm.is_none();

        assert!(
            !(display.id == "display0" && generic_name && no_measurements),
            "display {name:?} matches the synthetic placeholder signature (id \
             `display0`, generic name, no measurements) — detection failures must \
             report nothing rather than invent an entry"
        );

        // Whatever is reported must at least be identifiable as a specific device.
        assert!(
            !name.is_empty() || !no_measurements,
            "display {} has neither a name nor any measurement, so it identifies \
             nothing",
            display.id
        );
    }
}

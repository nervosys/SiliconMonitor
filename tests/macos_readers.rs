// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! The macOS CPU and memory readers, exercised on macOS.
//!
//! These were written by cross-compilation from a Windows machine and had never
//! executed anywhere when they were committed. That is the same position the Linux
//! SMART paths were in for 3.0.0, and the answer there turned out to be that CI
//! *is* the missing hardware: the test matrix runs on `macos-latest`, so a test
//! gated to macOS runs the code on a real Mac on every push.
//!
//! So these assert what a reading must satisfy to be a reading — a CPU split that
//! accounts for the whole processor, a memory total that matches installed RAM,
//! used memory that fits inside it. A parser that silently misreads `vm_stat` or
//! `top` fails here rather than shipping numbers nobody checked.
//!
//! On every other platform this file compiles to nothing.

#![cfg(target_os = "macos")]

use simonlib::Simon;

/// `Simon::new` calls `detect_platform_info` in its constructor, so this failing
/// is what previously made every other reader unreachable on macOS regardless of
/// whether it worked.
#[test]
fn simon_can_be_constructed_on_macos() {
    let simon = Simon::new().expect("Simon::new must succeed on macOS");
    let board = simon.board_info();

    assert_eq!(board.platform.system, "Darwin");
    assert!(
        !board.platform.release.is_empty(),
        "kern.osrelease should be readable"
    );
    assert!(
        !board.hardware.model.is_empty(),
        "hw.model should be readable"
    );
}

#[test]
fn the_cpu_split_accounts_for_the_whole_processor() {
    let simon = Simon::new().expect("construct");
    let cpu = simon
        .cpu()
        .expect("CPU statistics must be readable on macOS");

    let total = cpu.total.user + cpu.total.nice + cpu.total.system + cpu.total.idle;
    assert!(
        (95.0..=105.0).contains(&total),
        "user {} + nice {} + system {} + idle {} = {total}, which does not account \
         for the processor — the `top` line was probably misparsed",
        cpu.total.user,
        cpu.total.nice,
        cpu.total.system,
        cpu.total.idle
    );

    for (name, value) in [
        ("user", cpu.total.user),
        ("system", cpu.total.system),
        ("idle", cpu.total.idle),
    ] {
        assert!(
            (0.0..=100.0).contains(&value),
            "{name} is {value}%, which is not a percentage"
        );
    }
}

#[test]
fn every_logical_core_is_enumerated() {
    let simon = Simon::new().expect("construct");
    let cpu = simon.cpu().expect("CPU statistics");

    assert!(
        cpu.core_count() > 0,
        "hw.logicalcpu should report at least one core"
    );
    assert_eq!(
        cpu.online_count(),
        cpu.core_count(),
        "macOS does not offline cores"
    );
}

/// Per-core utilisation is deliberately absent: `top` reports one aggregate, and
/// copying it into each core would present an average as a measurement. This pins
/// that decision so it cannot be quietly reversed.
#[test]
fn per_core_utilisation_is_absent_rather_than_the_average_repeated() {
    let simon = Simon::new().expect("construct");
    let cpu = simon.cpu().expect("CPU statistics");

    for core in &cpu.cores {
        assert!(
            core.user.is_none() && core.system.is_none() && core.idle.is_none(),
            "core {} carries per-core utilisation, which no macOS tool reports",
            core.id
        );
    }
}

#[test]
fn memory_totals_are_internally_consistent() {
    let simon = Simon::new().expect("construct");
    let memory = simon.memory().expect("memory statistics must be readable");

    assert!(
        memory.ram.total > 0,
        "hw.memsize should report installed RAM"
    );

    // No Mac ships with under 1 GiB, and none has 1 PiB. A page size misread as
    // 4096 on Apple Silicon would land far below this floor.
    const ONE_GIB: u64 = 1024 * 1024 * 1024;
    assert!(
        memory.ram.total >= ONE_GIB,
        "total RAM reported as {} bytes, below any real Mac — suspect the page size",
        memory.ram.total
    );
    assert!(
        memory.ram.total < 1024 * ONE_GIB,
        "total RAM reported as {} bytes",
        memory.ram.total
    );

    assert!(
        memory.ram.used > 0,
        "a running system has used memory; 0 means the page counts were not read"
    );
    assert!(
        memory.ram.used <= memory.ram.total,
        "used {} exceeds total {}",
        memory.ram.used,
        memory.ram.total
    );

    let percent = memory.ram_usage_percent();
    assert!(
        (0.0..=100.0).contains(&percent),
        "usage of {percent}% is not a percentage"
    );
}

#[test]
fn swap_is_reported_without_exceeding_its_own_total() {
    let simon = Simon::new().expect("construct");
    let memory = simon.memory().expect("memory statistics");

    // CI runners may have swap disabled, which is a real zero rather than a
    // failure — so this checks consistency, not presence.
    assert!(
        memory.swap.used <= memory.swap.total,
        "swap used {} exceeds total {}",
        memory.swap.used,
        memory.swap.total
    );
}

#[test]
fn uptime_is_positive_and_not_a_wrapped_duration() {
    let simon = Simon::new().expect("construct");
    let uptime = simon.uptime().expect("uptime must be readable");

    assert!(
        uptime.as_secs() > 0,
        "a machine running tests has been up for more than a second"
    );
    // A subtraction done the wrong way round would wrap to ~584 billion years.
    assert!(
        uptime.as_secs() < 60 * 60 * 24 * 365 * 20,
        "uptime of {} seconds suggests kern.boottime was misread",
        uptime.as_secs()
    );
}

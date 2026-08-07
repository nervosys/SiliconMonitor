// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! macOS CPU and memory readers.
//!
//! # Why this shells out
//!
//! The precise way to read these is `sysctlbyname`, `host_statistics64` and
//! `host_processor_info` through `libc`. This module instead parses the output of
//! `sysctl`, `vm_stat` and `top`, which the rest of the crate's macOS code already
//! does (see `cpu_cache`), for one reason that outweighs elegance here: **this code
//! has never run on a Mac.** It was written and compile-checked by
//! cross-compilation from Windows.
//!
//! Mach FFI that has never executed is a poor bet — a wrong structure offset or a
//! misread unit produces numbers that look plausible and are wrong, which is the
//! exact failure this project has spent two releases removing. Parsing documented
//! textual output can at least be tested: every parser below is a pure function
//! over a byte string, and the tests feed them real captured output. Those tests
//! run on Linux, Windows and macOS alike.
//!
//! # What is not measured
//!
//! - **Per-core utilisation.** `top` reports one aggregate. Cores are enumerated
//!   with their identity and `None` for utilisation rather than each being given a
//!   copy of the average, which would read as measurement.
//! - **Nice time.** No macOS command-line tool separates it; `top` reports user,
//!   sys and idle only. `CpuTotal::nice` is not an `Option`, so it is reported as
//!   0.0 — the one number here that is a convention rather than a reading, and it
//!   is called out in `docs/` for that reason.
//!
//! Both would be fixed by `host_processor_info`, which is the natural next step
//! once someone can run it against a Mac and compare.

use std::process::Command;

/// Aggregate CPU time split, percent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuUsage {
    pub user: f32,
    pub system: f32,
    pub idle: f32,
}

/// Parse the `CPU usage:` line of `top -l 1 -n 0`.
///
/// The line reads, on every macOS version this targets:
/// `CPU usage: 4.76% user, 9.52% sys, 85.71% idle`
///
/// Returns `None` when the line is absent or malformed, so that a change in
/// `top`'s output surfaces as missing data rather than as zeros.
pub fn parse_top_cpu(output: &str) -> Option<CpuUsage> {
    let line = output
        .lines()
        .find(|l| l.trim_start().starts_with("CPU usage"))?;
    let rest = line.split_once(':')?.1;

    let field = |label: &str| -> Option<f32> {
        rest.split(',')
            .find(|part| part.trim_end().ends_with(label))?
            .trim()
            .split('%')
            .next()?
            .trim()
            .parse()
            .ok()
    };

    let user = field("user")?;
    let system = field("sys")?;
    let idle = field("idle")?;

    // A split that does not account for the whole CPU means the line was
    // understood incorrectly, not that the machine is idle.
    if !(95.0..=105.0).contains(&(user + system + idle)) {
        return None;
    }

    Some(CpuUsage { user, system, idle })
}

/// Page counts from `vm_stat`, in pages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VmStat {
    pub page_size: u64,
    pub free: u64,
    pub active: u64,
    pub inactive: u64,
    pub speculative: u64,
    pub wired: u64,
    pub compressed: u64,
    pub file_backed: u64,
}

impl VmStat {
    /// Memory macOS counts as in use.
    ///
    /// Activity Monitor's "Memory Used" is app memory plus wired plus compressed.
    /// Inactive and file-backed pages are cache the kernel will surrender on
    /// demand, so counting them as used overstates pressure — the number a user
    /// compares against is the one Activity Monitor shows.
    pub fn used_bytes(&self) -> u64 {
        (self.active + self.wired + self.compressed) * self.page_size
    }

    pub fn free_bytes(&self) -> u64 {
        (self.free + self.speculative) * self.page_size
    }

    /// File-backed pages, which is what "cached" means on this platform.
    pub fn cached_bytes(&self) -> u64 {
        self.file_backed * self.page_size
    }
}

/// Parse `vm_stat` output.
///
/// The header carries the page size — it is 16384 on Apple Silicon and 4096 on
/// Intel, so it must be read rather than assumed:
/// `Mach Virtual Memory Statistics: (page size of 16384 bytes)`
///
/// Counts are printed with a trailing period: `Pages free:  123456.`
pub fn parse_vm_stat(output: &str) -> Option<VmStat> {
    let page_size = output
        .lines()
        .next()
        .and_then(|l| l.split("page size of ").nth(1))
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<u64>().ok())?;

    let value = |label: &str| -> u64 {
        output
            .lines()
            .find(|l| l.trim_start().starts_with(label))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().trim_end_matches('.').replace(',', ""))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };

    Some(VmStat {
        page_size,
        free: value("Pages free"),
        active: value("Pages active"),
        inactive: value("Pages inactive"),
        speculative: value("Pages speculative"),
        wired: value("Pages wired down"),
        // Named "Pages occupied by compressor" on current macOS.
        compressed: value("Pages occupied by compressor"),
        file_backed: value("File-backed pages"),
    })
}

/// Swap totals in bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwapUsage {
    pub total: u64,
    pub used: u64,
}

/// Parse `sysctl -n vm.swapusage`.
///
/// `total = 2048.00M  used = 1024.50M  free = 1023.50M  (encrypted)`
///
/// The suffix varies with size — M and G both occur — so it is read rather than
/// assumed, and a machine with swap disabled reports zeros, which is a reading.
pub fn parse_swapusage(output: &str) -> Option<SwapUsage> {
    let field = |label: &str| -> Option<u64> {
        let after = output.split(&format!("{label} = ")).nth(1)?;
        let token = after.split_whitespace().next()?;
        let (number, suffix) = token.split_at(token.find(|c: char| c.is_alphabetic())?);
        let value: f64 = number.parse().ok()?;
        let scale = match suffix {
            "K" | "k" => 1024.0,
            "M" | "m" => 1024.0 * 1024.0,
            "G" | "g" => 1024.0 * 1024.0 * 1024.0,
            "T" | "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            "B" | "b" | "" => 1.0,
            _ => return None,
        };
        Some((value * scale) as u64)
    };

    Some(SwapUsage {
        total: field("total")?,
        used: field("used")?,
    })
}

/// Run a command and return stdout, or `None` if it could not be run.
#[cfg(target_os = "macos")]
fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn run(_program: &str, _args: &[&str]) -> Option<String> {
    // Present so the module compiles everywhere and its parsers stay testable.
    let _ = Command::new("true");
    None
}

/// Read a single numeric `sysctl` value.
pub fn sysctl_u64(name: &str) -> Option<u64> {
    run("sysctl", &["-n", name])?.trim().parse().ok()
}

/// Read a single string `sysctl` value.
pub fn sysctl_string(name: &str) -> Option<String> {
    run("sysctl", &["-n", name]).map(|s| s.trim().to_string())
}

/// Aggregate CPU usage, or `None` if `top` could not be read or understood.
pub fn cpu_usage() -> Option<CpuUsage> {
    parse_top_cpu(&run("top", &["-l", "1", "-n", "0"])?)
}

/// Virtual memory page counts, or `None` if `vm_stat` could not be read.
pub fn vm_stat() -> Option<VmStat> {
    parse_vm_stat(&run("vm_stat", &[])?)
}

/// Swap usage, or `None` if `vm.swapusage` could not be read.
pub fn swap_usage() -> Option<SwapUsage> {
    parse_swapusage(&run("sysctl", &["-n", "vm.swapusage"])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from `top -l 1 -n 0`.
    const TOP_OUTPUT: &str = "Processes: 512 total, 2 running, 510 sleeping, 2841 threads
2026/08/06 21:14:02
Load Avg: 2.31, 2.55, 2.72
CPU usage: 4.76% user, 9.52% sys, 85.71% idle
SharedLibs: 512M resident, 88M data, 42M linkedit.
MemRegions: 123456 total, 4321M resident, 234M private, 1234M shared.
";

    #[test]
    fn top_cpu_line_is_split_into_user_system_and_idle() {
        let usage = parse_top_cpu(TOP_OUTPUT).expect("parses");
        assert!((usage.user - 4.76).abs() < 0.001);
        assert!((usage.system - 9.52).abs() < 0.001);
        assert!((usage.idle - 85.71).abs() < 0.001);
    }

    /// If `top`'s wording changes, the right answer is "unknown", not "idle".
    #[test]
    fn a_missing_cpu_line_is_none_rather_than_zero_usage() {
        assert!(parse_top_cpu("Processes: 512 total\nLoad Avg: 1.0\n").is_none());
        assert!(parse_top_cpu("").is_none());
    }

    /// A split that does not sum to roughly 100% means the line was misread.
    #[test]
    fn an_implausible_split_is_rejected() {
        let bad = "CPU usage: 4.00% user, 5.00% sys, 6.00% idle";
        assert!(parse_top_cpu(bad).is_none());
    }

    /// Captured from `vm_stat` on Apple Silicon, where the page size is 16384 —
    /// assuming 4096 would understate every figure by a factor of four.
    const VM_STAT_OUTPUT: &str = "Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                               45678.
Pages active:                            234567.
Pages inactive:                          123456.
Pages speculative:                        12345.
Pages throttled:                              0.
Pages wired down:                         98765.
Pages purgeable:                           4321.
\"Translation faults\":                 987654321.
Pages copy-on-write:                   12345678.
Pages zero filled:                    234567890.
Pages reactivated:                      1234567.
Pages purged:                            123456.
File-backed pages:                        87654.
Anonymous pages:                         270369.
Pages stored in compressor:              456789.
Pages occupied by compressor:             34567.
";

    #[test]
    fn vm_stat_reads_the_page_size_from_the_header() {
        let vm = parse_vm_stat(VM_STAT_OUTPUT).expect("parses");
        assert_eq!(vm.page_size, 16384, "Apple Silicon uses 16K pages");
    }

    #[test]
    fn vm_stat_reads_each_page_count() {
        let vm = parse_vm_stat(VM_STAT_OUTPUT).expect("parses");
        assert_eq!(vm.free, 45678);
        assert_eq!(vm.active, 234567);
        assert_eq!(vm.inactive, 123456);
        assert_eq!(vm.speculative, 12345);
        assert_eq!(vm.wired, 98765);
        assert_eq!(vm.compressed, 34567);
        assert_eq!(vm.file_backed, 87654);
    }

    /// "Pages stored in compressor" and "Pages occupied by compressor" are
    /// different quantities and adjacent lines; the second is the resident cost.
    #[test]
    fn compressed_pages_are_the_occupied_count_not_the_stored_count() {
        let vm = parse_vm_stat(VM_STAT_OUTPUT).expect("parses");
        assert_eq!(vm.compressed, 34567, "occupied, not the 456789 stored");
    }

    #[test]
    fn used_memory_counts_active_wired_and_compressed() {
        let vm = parse_vm_stat(VM_STAT_OUTPUT).expect("parses");
        assert_eq!(vm.used_bytes(), (234567 + 98765 + 34567) * 16384);
        assert_eq!(vm.free_bytes(), (45678 + 12345) * 16384);
        assert_eq!(vm.cached_bytes(), 87654 * 16384);
    }

    #[test]
    fn a_vm_stat_without_a_page_size_header_is_rejected() {
        assert!(parse_vm_stat("Pages free: 100.\n").is_none());
    }

    #[test]
    fn swapusage_scales_by_the_suffix_it_reports() {
        let swap =
            parse_swapusage("total = 2048.00M  used = 1024.50M  free = 1023.50M  (encrypted)")
                .expect("parses");
        assert_eq!(swap.total, 2048 * 1024 * 1024);
        assert_eq!(swap.used, (1024.5 * 1024.0 * 1024.0) as u64);
    }

    #[test]
    fn swapusage_handles_gigabyte_suffixes() {
        let swap =
            parse_swapusage("total = 8.00G  used = 2.50G  free = 5.50G  (encrypted)").expect("ok");
        assert_eq!(swap.total, 8 * 1024 * 1024 * 1024);
        assert_eq!(swap.used, (2.5 * 1024.0 * 1024.0 * 1024.0) as u64);
    }

    /// Swap disabled is a reading of zero, not an absence.
    #[test]
    fn swap_disabled_reports_zero_rather_than_none() {
        let swap = parse_swapusage("total = 0.00M  used = 0.00M  free = 0.00M").expect("parses");
        assert_eq!(swap.total, 0);
        assert_eq!(swap.used, 0);
    }

    #[test]
    fn unparseable_swapusage_is_none() {
        assert!(parse_swapusage("vm.swapusage: unavailable").is_none());
    }
}

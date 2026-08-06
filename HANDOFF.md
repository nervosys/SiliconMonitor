# Handoff — 3.0.0 is ready to publish

**One action remains: confirm CI is green on `391d010`, then publish and tag.**
Everything else is done, committed and pushed.

I stopped because my shell died, not because anything is wrong with the code —
every `Bash`/`PowerShell` call began failing with
`EEXIST … mkdir '…\fb217f0f-…\tasks'`, even `echo`. A second Claude Code session
in this project raced this one over the shared temp directory. A fresh session
clears it.

---

## Do this first

This file is untracked, and `cargo publish` **refuses a dirty tree** — it already
bit me once this session on a modified `Cargo.lock`. So commit or delete it before
publishing:

```bash
git add HANDOFF.md && git commit -m "docs: handoff for 3.0.0"   # or: rm HANDOFF.md
```

## Then finish the release

```bash
# 1. CI must be green — all six jobs. Do not skip this; see Risks below.
gh run list --limit 1
gh run view <run-id> --json conclusion,jobs -q '.conclusion, (.jobs[] | "\(.conclusion)\t\(.name)")'

# 2. Publish and tag
cargo publish --all-features
git tag -a v3.0.0 -m "v3.0.0 — SMART and NVMe support"
git push origin v3.0.0
```

If jobs sit in `queued` for a long time, that is a GitHub runner backlog, not a
failure — it happened repeatedly today, once for ~50 minutes. Two jobs also failed
on `Set up job`, which is runner provisioning; `gh run rerun <id> --failed` clears
those. **Distinguish "queued/infra-flaked" from "failed" before deciding anything.**

---

## State

| | |
|---|---|
| Branch | `master`, pushed, clean at time of push |
| HEAD | `391d010` — `feat: SMART and NVMe support, and the fabrication it uncovered` |
| `Cargo.toml` | `3.0.0` |
| Published on crates.io | `2.1.4` (2.1.5 and 3.0.0 are **not** published) |
| Tags | through `v2.1.4`; no `v2.1.5`, no `v3.0.0` |

`2.1.5` was committed but never published — same GitHub outage. It is documentation
only (a `CPU_MONITORING.md` correction). Publishing `3.0.0` supersedes it; there is
no need to publish `2.1.5` separately.

### Verified locally before push

- `cargo clippy --all-features --all-targets -- -D warnings` → 0
- `cargo test --all-features` → 8 suites, 0 failures
- `cargo check --target x86_64-unknown-linux-gnu --no-default-features --features cpu,npu,io,network,amd,nvidia,intel` → 0
- `cargo check --target aarch64-apple-darwin --no-default-features --features cpu,npu,io,network,apple,nvidia,num_cpus` → 0

Those two cross-target checks need only a `rust-std` component (`rustup target add
…`) — no C toolchain, no VM. They are the fastest way to catch platform breakage
from a Windows box, and they are how the 114 compile errors behind the manifest bug
were found and fixed.

---

## What 3.0.0 contains

### The feature that was asked for

`DiskDevice::smart_info()` and `DiskDevice::nvme_info()` were trait defaults
returning `NotSupported`. Both are implemented on Linux and Windows. `health()` now
derives from SMART instead of — on Linux — returning `Healthy` whenever the device
file existed, which reported "the kernel enumerated this drive" as a clean bill of
health.

### The bugs wiring it up exposed

The 844-line `src/smart/mod.rs` looked implemented but published numbers nobody
measured:

- **`Get-StorageReliabilityCounter` requires elevation.** Unelevated it fails
  `PermissionDenied`; the failure was swallowed by `-ErrorAction SilentlyContinue`
  and the substituted zeros were reported as readings. Every drive came back
  *healthy, 0 °C, 0 power-on hours.*
- **Health was scored from those zeros** — no penalties, score 100, verdict `Good`,
  "100% life remaining". A confident judgement derived from nothing.
- **`DeviceId` arrives as the JSON string `"1"`**, so `as_u64()` returned `None`
  and the fallback named all four drives `PhysicalDrive0`.
- **Windows answers `MediaType` (medium: `"SSD"`) and `BusType` (transport:
  `"NVMe"`) separately.** Only `MediaType` was read, so every NVMe drive was
  classified a plain SSD — which made `nvme_info()` refuse to answer for exactly
  the drives it exists for. Caught only because the first end-to-end run returned
  `NotSupported` for three NVMe drives.

### Breaking changes (why it is 3.0.0)

Counters are `Option` now on both `smart::SmartDiskInfo` and `disk::NvmeInfo`.
`None` means the platform would not report it. A `controller_id` of 0 is a real
controller and `num_namespaces` of 0 is a real answer; neither can stand in for
"not read". `SmartMonitor::max_temperature()` returns `Option<u32>`.

Downstream callers must handle `Option` on: `temperature_celsius`,
`power_on_hours`, `power_cycle_count`, `reallocated_sectors`, `pending_sectors`,
`uncorrectable_errors`, `total_bytes_written`, `total_bytes_read`,
`nvme_version`, `unallocated_capacity`, `controller_id`, `num_namespaces`,
`power_state`, `critical_warnings`.

### Verified on hardware

```
Before:  4 disks, all PhysicalDrive0, health=Good, temp=0, poh=0
After:   PhysicalDrive0  Samsung SSD 9100 PRO 4TB   NVMe  temp=None
         PhysicalDrive1  Samsung SSD 990 PRO 4TB    NVMe  temp=None
         PhysicalDrive2  Samsung SSD 970 EVO Plus   NVMe  temp=None
         PhysicalDrive3  File-Stor Gadget           USB   NotSupported
```

Reproduce with `cargo run --all-features --example disk_monitor`.

---

## Risks and gaps — read before publishing

1. **The Linux SMART/NVMe paths have never been run.** They compile-check against
   the Linux target; I have no Linux hardware. The sysfs paths
   (`/sys/class/nvme/<ctrl>/{model,serial,firmware_rev,cntlid}`) are documented
   kernel ABI, but the code has not executed once. This is the single largest
   untested surface in the release.

2. **crates.io is permanent.** A yank hides a version; it does not remove it. With
   breaking API changes *and* unrun Linux code, green CI is the only external check
   there is.

3. **macOS reads no CPU or memory at all.** Pre-existing, documented in three
   places (README platform table, `CHANGELOG` *Known gaps*, Contributing). There is
   no `src/platform/macos.rs`; `stats::Simon`'s ten platform functions return
   `UnsupportedPlatform`. Needs sysctl/IOKit work verified on a Mac.

4. **Elevated Windows passthrough is not implemented.** `DeviceIoControl` with
   `StorageDeviceProtocolSpecificProperty` would supply the fields currently
   `None`: temperature, power-on hours, wear, controller id, namespace count,
   critical warnings. Scoped in `docs/DISK_MONITORING.md`.

5. **`smart_disk()` spawns a subprocess per call.** The trait is per-device and the
   collector enumerates every drive per run, so calling `smart_info()` across N
   disks costs N PowerShell invocations. Callers wanting all drives should use
   `smart::SmartMonitor` directly. Documented at both call sites; worth revisiting
   if it shows up in a profile.

---

## Guardrails now in the suite

Added across this session; each was checked against a deliberate break before being
kept, because a test that cannot fail is worse than none:

| Test | Catches |
|---|---|
| `tests/manifest_portability.rs` | A feature depending on a target-gated crate — the bug that meant the crate never built on Linux or macOS |
| `tests/documentation_links.rs` | Broken relative links; machine identifiers in docs; documented `simon …` commands that do not exist |
| `smart::tests::a_drive_with_no_readable_counters_is_not_graded_healthy` | Health graded from an empty scorecard |
| `tests/plausibility.rs` | Physically impossible readings |
| `tests/agentic_contract.rs` | Schema/resolver disagreement on the agent surface |

`platform_has_hardware_readers()` in the contract and plausibility suites names the
macOS reader gap once, rather than scattering `cfg(target_os)` as if each site were
its own special case.

---

## Two things I'd tell the next person

**Don't run `cargo clippy --fix` per target in this crate.** Fixing for macOS
deleted `mut` and imports that only Linux and Windows need, silently breaking both
(21 and 14 errors). A `cfg` block that is empty on the target being fixed makes
bindings look unused. Six files had to be reverted.

**A manual grep is not a substitute for asking the binary.** I swept by hand for
documented-but-nonexistent commands, concluded I had them all, then wrote a test
comparing docs against `simon describe --commands` — it immediately found `CLI.md`
at the repo root, which I had simply not thought to include. My sweep missed 29 of
38 cases. Where the program can answer a question about itself, ask it.

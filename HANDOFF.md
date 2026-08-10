# Status

Current as of 3.3.0. This file is excluded from the published crate
(`Cargo.toml`'s `exclude` list) and is for whoever picks the work up next.

## Released

| Version | State |
|---|---|
| 3.0.0 | Published, tagged `v3.0.0`. SMART and NVMe support. |
| 3.0.1 | Feature-combination fix — see CHANGELOG. |
| 3.1.0 | Windows NVMe passthrough (unelevated) and macOS CPU/memory. Published, tagged. |
| 3.2.0 | macOS per-core CPU, NVMe current power state, `--all-targets` in the feature job. Published, tagged. |
| 3.3.0 | Windows ATA SMART (unelevated). Committed, not yet tagged. |
| 2.1.5 | Committed, never published. Documentation only; superseded by 3.0.0. |

## Verification that is worth repeating

```bash
cargo fmt --all -- --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo check --target x86_64-unknown-linux-gnu --no-default-features --features cpu,npu,io,network,amd,nvidia,intel
cargo check --target aarch64-apple-darwin --no-default-features --features cpu,npu,io,network,apple,nvidia,num_cpus
```

The two cross-target checks need only `rustup target add …` — no C toolchain, no
VM. They are the fastest way to catch platform breakage from a Windows box, and
they are how the 114 compile errors behind the 3.0.0 manifest bug were found.

CI additionally checks every feature in isolation (job **Feature combinations**).
That job exists because `--all-features` cannot catch a feature that only builds
because another supplies what it is missing — which is exactly how the `cli`
feature stayed broken through eight published versions.

## Open work

1. **The Windows ATA SMART path has never met a SATA drive.** 3.3.0 reads the
   attribute table unelevated through `IOCTL_STORAGE_PREDICT_FAILURE`, and the
   parse in `src/disk/ata_smart.rs` is tested only against buffers this project
   built. This machine has three NVMe drives and a USB gadget; on all four the
   path returns `NotSupported`, which exercises the decline and nothing else.

   **If you have a SATA SSD or HDD, that is the thing to use it for.** One run of
   `cargo run --example disk_monitor` against `smartctl -A` settles it. Watch
   power-on hours in particular: a minority of drives report that attribute in
   minutes, and nothing in the structure says which.

   Note for anyone re-reading the earlier plan: `IOCTL_ATA_PASS_THROUGH` was the
   scoped approach and it is a dead end. Its `CTL_CODE` carries `FILE_READ_ACCESS
   | FILE_WRITE_ACCESS`, so the access check happens before the driver is reached
   and a read/write handle on `\\.\PhysicalDriveN` needs Administrator — measured,
   `ERROR_ACCESS_DENIED` on all four drives. Whether an ioctl needs elevation is a
   property of its `CTL_CODE`, and is worth reading off the definition before
   planning around it.

2. **macOS GPU, power and temperature are still unimplemented.** CPU (per-core,
   with nice time), memory, swap, uptime and board info work — `Simon::cpu()`,
   `memory()`, `uptime()`. `Simon::snapshot()` still fails, because it requires
   every reader. Power and temperature need `powermetrics`, which requires root,
   so they may not be reachable unelevated at all; that is worth establishing
   before writing anything.

   **If you have a Mac, that is the thing to use it for.** These readers were
   written by cross-compilation and are verified only by `tests/macos_readers.rs`
   on `macos-latest`, which checks that readings are *plausible* — not that they
   are *correct*. Comparing `Simon::cpu()` and `memory()` against Activity Monitor
   once would settle whether the `vm_stat` accounting matches what a user sees.

3. **The Linux SMART/NVMe paths have executed exactly once**, in CI on
   `33ee241` — 733 tests, 0 failures. No one has run them against real Linux
   hardware. The sysfs paths (`/sys/class/nvme/<ctrl>/{model,serial,firmware_rev,cntlid}`)
   are documented kernel ABI, but tests are not a substitute for a drive.

4. **`smart_disk()` spawns a subprocess per call.** The trait is per-device and
   the collector enumerates every drive per run, so `smart_info()` across N disks
   costs N PowerShell invocations. Use `smart::SmartMonitor` directly for all
   drives. Documented at both call sites.

   Narrower since 3.3.0: NVMe and SATA drives are answered by their passthrough
   before the fallback is reached, so this now costs only for devices that decline
   both — USB bridges, chiefly. It is not fixed, only less often paid.

## Guardrails, and why each exists

| Test | Catches |
|---|---|
| `tests/manifest_portability.rs` | A feature depending on a target-gated crate — the bug that meant the crate never built on Linux or macOS |
| CI job *Feature combinations* | A feature that does not build in isolation — found `cargo install` broken on Linux on its first run |
| `tests/macos_readers.rs` | macOS readings that are not plausible readings; runs on `macos-latest`, which is the only Mac this project has |
| `tests/documentation_links.rs` | Broken relative links; machine identifiers in docs; documented `simon …` commands that do not exist |
| `smart::tests::a_drive_with_no_readable_counters_is_not_graded_healthy` | Health graded from an empty scorecard |
| `smart::tests::a_self_reported_failure_survives_inference` | A drive predicting its own failure being scored back to `Good` from clean-looking counters |
| `disk::ata_smart::tests` | ATA structure misparses: bad checksum accepted, zero-filled buffer read as a clean drive, temperature taken from the full 48-bit raw, table truncated at the first gap |
| `tests/plausibility.rs` | Physically impossible readings |
| `tests/agentic_contract.rs` | Schema/resolver disagreement on the agent surface |

Each was checked against a deliberate break before being kept, because a test
that cannot fail is worse than none.

## Two things worth knowing

**Don't run `cargo clippy --fix` per target in this crate.** Fixing for macOS
deleted `mut` and imports that only Linux and Windows need, silently breaking
both (21 and 14 errors). A `cfg` block that is empty on the target being fixed
makes bindings look unused. Six files had to be reverted.

**Whether a Windows ioctl needs elevation is written in its `CTL_CODE`.** The
access bits in the definition, and the mask the handle was opened with, decide it
— not how privileged the operation sounds. Two capabilities were scoped as
"needs Administrator" on the second reading and turned out not to (NVMe in 3.1.0,
ATA in 3.3.0), and one was scoped as reachable and is not
(`IOCTL_ATA_PASS_THROUGH`). Reading the `CTL_CODE` first would have settled all
three in a minute each.

**A manual grep is not a substitute for asking the binary.** A hand sweep for
documented-but-nonexistent commands missed 29 of 38 cases; a test comparing docs
against `simon describe --commands` found them all, including a `CLI.md` at the
repo root nobody had thought to include. The same lesson produced the feature
sweep above: the `cli` breakage was found by running every combination, not by
reading the manifest.

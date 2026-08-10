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

4. ~~**`smart_disk()` spawns a subprocess per call.**~~ Fixed in 3.3.0 by
   `SmartMonitor::cached_disks()`, which shares one sweep process-wide for 2 s.
   A sweep is 1.23 s on this machine, and a four-drive pass could take twelve of
   them. Two things narrowed the problem before it was fixed: NVMe and SATA drives
   are now answered by their passthrough and never reach the collector at all, so
   what remains to benefit is USB storage — and every Linux machine, where a
   sweep spawns `smartctl` once per drive and the old shape was quadratic.

## The plan for what is left

Everything remaining is blocked on hardware this project does not have. That is
the single fact to take from this section: there is no more code to write that
would be honest to write. Each item below is a *verification* task with a
concrete recipe, ordered by what it unblocks.

**A. Verify the ATA path, then tag 3.3.0.** *Needs: any SATA SSD or HDD, on
Windows. Half an hour.*

1. Attach the drive — internal, or a USB-SATA enclosure whose bridge tunnels
   SMART (JMicron and ASMedia generally do; the cheapest ones do not).
2. `cargo run --example disk_monitor`, and `smartctl -A /dev/sdX` for the same
   drive.
3. Compare, in this order of importance: reallocated (5), pending (197) and
   uncorrectable (198) sector counts, since those are what `health()` grades on
   and what no other Windows path can reach; then temperature; then power-on
   hours.
4. If nothing appears at all, the parse refused the structure. The likely cause
   is the checksum — `AtaSmartData::parse` declines a structure whose final byte
   does not make all 512 sum to zero. Dump `VendorSpecific` and check by hand
   before loosening it, and if it must be loosened, note that smartmontools
   warns-and-continues here and this deliberately does not.
5. Watch power-on hours specifically. A minority of drives report that attribute
   in minutes rather than hours and nothing in the structure says which; if the
   number is 60× what it should be, that is what happened, and it is a per-vendor
   quirk table, not a bug in the parse.

Done when a real drive's attributes match `smartctl -A`. Then `git tag v3.3.0`.
Until then 3.3.0 is committed but deliberately untagged.

**B. Establish what macOS power and temperature can reach unelevated.** *Needs: a
Mac. An hour, before any code.*

This is research, not implementation, and doing it first is the point.
`powermetrics` requires root, which would make the obvious implementation
unusable in the same way `IOCTL_ATA_PASS_THROUGH` would have been — and the ATA
work is the argument for checking first. Run, as a normal user:

- `powermetrics -n1 --samplers cpu_power` — expected to fail; confirm it does.
- `ioreg -rc AppleSmartBattery` and `ioreg -rc IOPMPowerSource` for power.
- `ioreg -rc AppleSMC` and the `SMC` keys for temperature.
- On Apple silicon, `IOHIDEventSystemClient` temperature sensors, which are
  readable unelevated where SMC keys are not.

Write down which of these answer without root *before* writing a reader. If none
do, the honest outcome is a documented `NotSupported`, not an implementation that
only works under `sudo`.

**C. Validate the macOS readers that already exist.** *Needs: a Mac. Twenty
minutes.* Compare `Simon::cpu()` and `Simon::memory()` against Activity Monitor.
`tests/macos_readers.rs` checks readings are *plausible*, not *correct*; the
specific thing in doubt is whether the `vm_stat` accounting matches what a user
sees under "Memory Used", which is a different figure from free-plus-inactive.

**D. Run the Linux SMART/NVMe paths on real hardware.** *Needs: a Linux box with
an NVMe drive. Twenty minutes.* `cargo run --example disk_monitor` against
`nvme list` and `smartctl -A`. The sysfs paths are documented kernel ABI and the
code has passed CI, but CI has no drive. The quadratic collector shape fixed in
3.3.0 was worst on Linux, so this is also the first real check of that.

**Deliberately not planned:** SMART failure thresholds on Windows. They come from
SMART READ THRESHOLDS, which has no `IOCTL_STORAGE_*` equivalent, so the only
route is an elevated pass-through — giving back exactly what 3.3.0 bought. The
drive's own `PredictFailure` verdict is the same judgement the thresholds would
have produced, and is already reported.

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

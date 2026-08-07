# Status

Current as of 3.0.1. This file is excluded from the published crate
(`Cargo.toml`'s `exclude` list) and is for whoever picks the work up next.

## Released

| Version | State |
|---|---|
| 3.0.0 | Published, tagged `v3.0.0`. SMART and NVMe support. |
| 3.0.1 | Feature-combination fix — see CHANGELOG. |
| 3.1.0 | Windows NVMe passthrough (unelevated) and macOS CPU/memory. Published, tagged. |
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

1. **Current NVMe power state is still `None`.** The available power state table
   comes from Identify Controller and is populated; the current state needs Get
   Features (FID 0x02), a separate admin command. SATA SMART attributes still go
   through `Get-StorageReliabilityCounter` and so still need elevation — ATA
   pass-through would remove that. Both scoped in `docs/DISK_MONITORING.md`.

2. **macOS GPU, power and temperature are still unimplemented.** CPU, memory,
   swap, uptime and board info landed in 3.1.0 (`src/platform/macos.rs`), so
   `Simon::cpu()`, `memory()` and `uptime()` work there. `Simon::snapshot()` still
   fails, because it requires every reader. Per-core utilisation and nice time are
   also absent — `top` gives one aggregate and separates no nice time; both need
   `host_processor_info`.

   **If you have a Mac, that is the thing to use it for.** The 3.1.0 readers were
   written by cross-compilation and are verified only by
   `tests/macos_readers.rs` on `macos-latest`, which checks that readings are
   plausible — not that they are *correct*. Comparing `Simon::cpu()` and
   `memory()` against Activity Monitor once would settle whether the `vm_stat`
   accounting matches what a user sees.

3. **The Linux SMART/NVMe paths have executed exactly once**, in CI on
   `33ee241` — 733 tests, 0 failures. No one has run them against real Linux
   hardware. The sysfs paths (`/sys/class/nvme/<ctrl>/{model,serial,firmware_rev,cntlid}`)
   are documented kernel ABI, but tests are not a substitute for a drive.

4. **`smart_disk()` spawns a subprocess per call.** The trait is per-device and
   the collector enumerates every drive per run, so `smart_info()` across N disks
   costs N PowerShell invocations. Use `smart::SmartMonitor` directly for all
   drives. Documented at both call sites.

## Guardrails, and why each exists

| Test | Catches |
|---|---|
| `tests/manifest_portability.rs` | A feature depending on a target-gated crate — the bug that meant the crate never built on Linux or macOS |
| CI job *Feature combinations* | A feature that does not build in isolation — found `cargo install` broken on Linux on its first run |
| `tests/macos_readers.rs` | macOS readings that are not plausible readings; runs on `macos-latest`, which is the only Mac this project has |
| `tests/documentation_links.rs` | Broken relative links; machine identifiers in docs; documented `simon …` commands that do not exist |
| `smart::tests::a_drive_with_no_readable_counters_is_not_graded_healthy` | Health graded from an empty scorecard |
| `tests/plausibility.rs` | Physically impossible readings |
| `tests/agentic_contract.rs` | Schema/resolver disagreement on the agent surface |

Each was checked against a deliberate break before being kept, because a test
that cannot fail is worse than none.

## Two things worth knowing

**Don't run `cargo clippy --fix` per target in this crate.** Fixing for macOS
deleted `mut` and imports that only Linux and Windows need, silently breaking
both (21 and 14 errors). A `cfg` block that is empty on the target being fixed
makes bindings look unused. Six files had to be reverted.

**A manual grep is not a substitute for asking the binary.** A hand sweep for
documented-but-nonexistent commands missed 29 of 38 cases; a test comparing docs
against `simon describe --commands` found them all, including a `CLI.md` at the
repo root nobody had thought to include. The same lesson produced the feature
sweep above: the `cli` breakage was found by running every combination, not by
reading the manifest.

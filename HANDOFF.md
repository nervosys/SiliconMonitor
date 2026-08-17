# Status

Current as of 5.0.0. This file is excluded from the published crate
(`Cargo.toml`'s `exclude` list) and is for whoever picks the work up next.

## Released

| Version | State |
|---|---|
| 3.0.0 | Published, tagged `v3.0.0`. SMART and NVMe support. |
| 3.0.1 | Feature-combination fix — see CHANGELOG. |
| 3.1.0 | Windows NVMe passthrough (unelevated) and macOS CPU/memory. Published, tagged. |
| 3.2.0 | macOS per-core CPU, NVMe current power state, `--all-targets` in the feature job. Published, tagged. |
| 3.3.0 | Windows ATA SMART (unelevated), shared SMART collector, ontology to 94 entities. Published, tagged `v3.3.0`. The ATA parse is unverified against real SATA hardware — see open work 1. |
| 3.4.0 | DIMM slot topology and USB in the ontology (113 entities). Published, tagged `v3.4.0`. |
| 3.5.0 | Ontology-driven conformance tests, Windows PCI reader fixed, PCI domain (123 entities). Published, tagged `v3.5.0`. |
| 3.6.0 | PCIe link state on Windows via cfgmgr32; the PCI domain fully resolves. Published, tagged `v3.6.0`. |
| 3.7.0 | Virtualization, NUMA and ECC in the ontology (134 entities); two misleading virtualization readings withdrawn. Published, tagged `v3.7.0`. |
| 3.8.0 | `simon tune`: use-case detection and profile recommendations, with an automatic server. Recommend-only by default. Published, tagged `v3.8.0`. |
| 3.9.0 | Headless GUI reads four previously unreadable tabs; racy coverage test fixed. Published, tagged `v3.9.0`. |
| 3.10.0 | Hyper-V root partition distinguished from a guest via CPUID leaf 0x40000003; bare metal no longer reported as a VM. Published, tagged `v3.10.0`. |
| 4.0.0 | GUI rebuilt on Dewey; the ~10k-line egui implementation and eframe deleted. MSRV 1.85. Published, tagged `v4.0.0`. **Withdrawn in 5.0.0.** |
| 4.0.1 | GUI moved to Dewey's `agpu` backend over a wgpu Vulkan spec violation. Published, tagged `v4.0.1`. Withdrawn. |
| 4.0.2 | Dewey GUI made legible and its tabs clickable. Published, tagged `v4.0.2`. Withdrawn. |
| 4.0.3 | Dewey GUI restyled onto the recovered `CyberColors` palette. Published, tagged `v4.0.3`. Withdrawn. |
| 4.0.4 | Dewey Overview rebuilt from the egui widget vocabulary. Published, tagged `v4.0.4`. Withdrawn. |
| 5.0.0 | **The Dewey port is withdrawn and the egui GUI restored.** Claimed MSRV back to 1.70 — which was not true, see 5.2.0. Published, tagged `v5.0.0`. See open work 9. |
| 5.1.0 | Applied settings are reversible: `read_current`, `ApplyOutcome.previous`, `revert_setting`, `revert_cycle`. Published, tagged `v5.1.0`. See open work 13. |
| 5.2.0 | `simon ai models` reads an IronVault vault, behind an optional `vault` feature. **MSRV corrected to 1.88**, having been wrong since 5.0.0. |
| 2.1.5 | Committed, never published. Documentation only; superseded by 3.0.0. |

## Verification that is worth repeating

```bash
cargo fmt --all -- --check
cargo clippy --all-features --all-targets -- -D warnings
cargo test --all-features
cargo test                       # default features: the `vault` tests must drop out
cargo +1.88 check --all-targets  # the declared MSRV, actually built
cargo check --target x86_64-unknown-linux-gnu --no-default-features --features cpu,npu,io,network,amd,nvidia,intel
cargo check --target aarch64-apple-darwin --no-default-features --features cpu,npu,io,network,apple,nvidia,num_cpus
```

**Build the MSRV, do not declare it.** `rust-version` said 1.70 from 3.x through
5.1.0 and had been false since 5.0.0: restoring the egui GUI brought in
`image 0.25.10` and `home 0.5.12`, which declare 1.88 themselves, and `image`
reaches `pxfm`, an edition-2024 crate that pre-1.85 cargo cannot even parse. Two
releases shipped a floor nothing had ever compiled against, because nothing did.
`cargo +<msrv> check` is one command and it is the only thing that makes the
field mean anything.

The corollary bit in the other direction too: raising the declared floor to 1.88
un-gated ten clippy lints that `incompatible_msrv` had been silently suppressing
at 1.70. A wrong `rust-version` does not merely mislead readers — it changes
which lints run.

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

5. **The ontology names 134 entities; the library has ~88 subsystem modules.**
   3.3.0 added 34 (disk SMART/health/NVMe, CPU cache, firmware, TPM), 3.4.0 added
   19 (DIMM slot topology, USB), 3.5.0 added PCI, 3.7.0 added virtualization, NUMA
   and ECC.

   What is left is mostly readers that answer nothing on a desktop, and so cannot
   be verified here: RAPL energy counters (Linux MSR interface), environmental
   sensors (accelerometer, ambient light — laptop and tablet hardware), cgroup and
   container accounting, and the datacenter and fleet modules. Each is a phase of
   the same shape as the ones above; none should be declared until someone can
   watch its resolver answer on hardware that has the thing.

   **`tests/ontology_conformance.rs` now checks the things that used to need an
   eye.** Five entities across 3.3.0 and 3.4.0 shipped an enum's `Unknown` through
   as a *measured* value, each caught in review; that class is now caught by
   construction, along with non-nullable nulls, unit/JSON-type mismatches and
   absences with no stated reason. Run it before adding a domain and after — it
   found three defects on its first run, one of them an entity declared since the
   ontology was written that no resolver had ever touched.

   An id that resolves to a confident guess is worse for an agent than one that
   does not exist, so add a domain only when its resolver can say why each absence
   is absent.

6. ~~**`VirtMonitor::is_virtual_machine()` returns true on a Hyper-V root
   partition.**~~ Fixed in 3.10.0 via Hyper-V CPUID leaf 0x40000003: the
   partition privilege mask holds `CreatePartitions` and `CpuManagement` only on
   a root partition. `hypervisor_indicates_vm()` is now the single path, so
   `detect_platform()` and the ontology entity were corrected at the same time,
   and the 3.7.0 ontology workaround is withdrawn.

   **The guest side is still unverified against a real Hyper-V VM.** The root
   side is measured on this desktop (`ebx=0x002bb9ff`). If you have a Hyper-V
   guest, one run of `simon get system.virtualization.platform` settles it —
   expect `virtual_machine`. CI's Windows runners are Azure Hyper-V guests and
   exercise the arm, but no assertion pins the value there because the same test
   must pass on this bare-metal desktop.

7. ~~**The Windows PCI reader blocks the PCI ontology domain.**~~ Fixed in 3.5.0.
   The reader reported a device-instance path as the address and never reported a
   bound driver, so 3.4.0 withheld the domain rather than ship unstable ids and a
   false "no driver is bound". Both now come from
   `HKLM\SYSTEM\CurrentControlSet\Enum`, which is readable unelevated:
   `LocationInformation`'s parenthesised tail carries the bus/device/function
   triple, and `Service` carries the driver. Addresses are conventional BDF
   (`0000:7a:00.0`) and the domain ships.

   PCIe link width and speed followed in 3.6.0, from the device node property
   store via `CM_Get_DevNode_PropertyW`. Two notes for anyone extending this:
   `Get-PnpDeviceProperty` reads the same values and costs 0.4 s per device (25 s
   here, against 0.62 s for the whole monitor via `cfgmgr32`), and the registry
   property store under `Enum\...\Properties` is ACL-blocked unelevated, so
   neither is a shortcut. The remaining `DEVPKEY_PciDevice_*` properties — payload
   sizes, AER capability, ARI and ATS support, SR-IOV — are readable by the same
   two calls with a different pid, if anyone wants them.

8. **`simon tune`'s policy table covers five settings, and its game detection is
   a name table.** Both are deliberate first cuts, and both are where the feature
   grows.

   The policy table in `tuning::policy_for` is keyed on setting id, not on a
   category, because two vendors' "performance mode" are not interchangeable —
   pretending otherwise is how a tuner writes one vendor's value into another's
   register. Adding a setting means adding an apply handler first: a policy for
   something unwritable produces advice nobody can take, which is why the planner
   records it under `skipped` rather than recommending it.

   Game detection matches launcher and engine process names, which will miss most
   games. It is used only as corroborating evidence and carries lower confidence
   than an identified AI framework for that reason. The right answer is to ask
   the graphics driver what is presenting full-screen; that is not implemented.

   **The rule to preserve:** a proposed value comes from what the driver declared
   — a `choices` entry or the reported `default` — never from this crate and never
   from a model. `tuning::tests::a_recommendation_never_proposes_a_value_the_driver_did_not_offer`
   is the test that keeps it true. A model may classify; it may not pick numbers.

9. **The Dewey port was tried across 4.0.0–4.0.4 and withdrawn in 5.0.0.**
   `src/gui/` is the egui application again — `app.rs`, `widgets.rs`, `theme.rs`,
   `profile_tab.rs`, `headless.rs`, `mod.rs`, restored from `927ffaa^`. The
   `deweygui` dependency, the `dewey-gui` feature and `simonlib::gui_dewey` are
   gone rather than left as dead aliases.

   **Read this before proposing the port again.** The argument for it was sound:
   the 3.9.0 spinner bug came from a headless path that diverged from the
   interactive one, and Dewey's `Command::Task` removes that class of bug. What
   the argument missed is what the egui GUI actually does. Side by side, the
   original shows nine metric cards including live GPU temperatures, six
   sparkline charts with axes and min/max, a threshold legend, an uptime and task
   status line, an AI ask bar and JSON/CSV export — and on the development
   machine it detects three NVIDIA GPUs where the port reported
   `Accelerators: unavailable — Failed to initialize COM`.

   So the port was not merely unfinished to look at. **It was reading less
   hardware, and the tests said nothing about either fact.** They asserted that
   every tab emitted named ontology nodes, which the port did faithfully while
   displaying a fraction of what it replaced. A contract that checks the shape of
   the output and never its substance will certify a regression as complete. If
   this is attempted again, the acceptance criterion is a screenshot of both
   GUIs side by side and a diff of what each reports on the same machine — not a
   node count.

   Four releases went into making the replacement presentable and it never got
   close. Budget accordingly.

10. **`CpuStats::new()` and `MemoryStats::new()` are zero-constructors with
   constructor-shaped names.** Neither reads anything: `MemoryStats::new()`
   returns all zeros, `CpuStats::new()` returns no cores and 100% idle. The real
   values come from the per-platform `read_cpu_stats` / `read_memory_stats`.

   Both GUI call sites are fixed (4.0.0), and the Dewey CPU tab asserts against
   the zero-constructor's exact signature so a regression fails loudly. **The
   Linux arms are verified by inspection, not compilation** — the documented Linux
   cross-check cannot include `gui`, which drags in reqwest and so
   ring/openssl-sys, needing a C toolchain this box lacks. CI compiles Linux.

   macOS still reaches both zero-constructors: there is no
   `platform::macos::read_cpu_stats` or `read_memory_stats` to call (open work 2).

   **The real fix is a rename, and it is breaking.** `SystemStats::new()` shows
   the right shape — it dispatches to the platform reader. The other two cannot
   follow it: `platform::linux::memory::read_memory_stats` calls
   `MemoryStats::new()?` as its starting struct, so making `new()` read would
   recurse forever. They are builder bases wearing a constructor's name, and
   `empty()` or `zeroed()` would have prevented both defects. 4.0.0 was the moment
   to do it and it was not taken — the GUI migration was already the breaking
   change and stacking a second one would have muddied it. Next major version.

   This has now caused two defects found a day apart. **Assume any other
   `T::new()` in this crate may be a zero-constructor until checked.**

11. **Verify with `--lib --tests` when the disk is tight.** `cargo test
   --all-features` links every example. That is affordable again now the duplicate
   egui is gone, but if it ever fails with `link.exe` 1318, the split is
   `cargo test --all-features --lib --tests` for execution plus
   `cargo clippy --all-features --all-targets` for type-checking the examples.
   Note `--lib --tests` skips doc-tests; run those before a release.


12. **Two Dewey bugs found during the port, recorded because they are real
   and unfixed — but no longer reachable from this crate.** Neither affects simon
   now that `deweygui` is gone. Both are for whoever works on Dewey itself, or
   for anyone who reconsiders open work 9.

   *wgpu Vulkan semaphore reuse.* Dewey's default eframe path reuses one binary
   semaphore across swapchain images, which the validation layer rejects as
   `VUID-vkQueueSubmit-pSignalSemaphores-00067` on every frame. Undefined
   behaviour by the spec rather than noise, though it renders. The fault is
   wgpu's, below both Dewey and eframe. Dewey's `agpu` backend does not hit it.

   *Intermittent blank render.* Resizing an `agpu` window — including a
   `SetWindowPos` to the size it already was — sometimes leaves it entirely blank
   with the process alive and nothing logged. It reproduces in Dewey's own
   `counter_agpu` example. Instrumenting `agpu_backend.rs` showed winit
   delivering a spurious `Resized(<the configured size>)` immediately after the
   true size, shrinking the surface while the window stays large. Three fixes
   failed and were reverted rather than left in place: `ControlFlow::WaitUntil`,
   requesting a redraw on resize, and reconciling against `window.inner_size()`
   each frame — the last because winit's own `inner_size()` reports the stale
   value too. That is the detail any further attempt should start from. Repro:
   three `eprintln!` calls in `resize`, the surface-acquire error arm and the
   frame-area computation in `render`, driven by `ShowWindow(hwnd, 3)`.

13. **Applied settings are reversible; the tuning loop is not yet closed.**
   `ApplyHandler::read_current()` reads a setting before it is written,
   `ApplyOutcome.previous` carries what was overwritten, `revert_setting()` puts
   it back through the same confirmed and audit-logged path, and
   `tuning::serve::revert_cycle()` undoes everything one cycle applied, in
   reverse.

   Before this, `simon tune --apply` could change a machine and had no way to
   change it back: the trait could only write, and the outcome recorded only what
   was requested. That is worth remembering as a shape of bug — the write path
   was complete and correct on its own terms, and useless for anything that
   needed to undo it.

   **The rule to preserve:** when no prior value was recorded, `revert_setting`
   refuses rather than writing a default. Putting a machine into a state it was
   never in is a worse failure than leaving it where the caller put it. A handler
   without `read_current` makes its setting explicitly one-way; the default
   returns `None` so existing handlers still compile, which means *adding* a
   handler without it silently gives up reversibility. Implement it whenever the
   source can be read back.

   Verified on Windows: `read_current` returns the same GUID that
   `simon profile explain active_scheme_guid` reports through unrelated code in
   `profile::cpu`. **The Linux sysfs readers are written by inspection and have
   never run** — there was no Linux machine in the session that added them, and
   CI compiles the path without exercising a sysfs write.

   **What is left to close the loop:** measure a metric before applying,
   re-measure after a settle period, and revert automatically on regression.
   The hard part is not the mechanism, it is deciding what proves a given setting
   helped — and the same rule that governs values should govern this. A loop that
   invents a success criterion is the optimiser equivalent of a model picking a
   power limit. If a setting's effect cannot be measured, the honest verdict is
   "unverifiable", not "improved".

## The plan for what is left

Items A–D are blocked on hardware this project does not have, and are
*verification* tasks with concrete recipes. E and F are ordinary software work
that anyone can pick up on any machine.

(This section previously opened by claiming everything left was hardware-blocked.
That was true when it was written and stopped being true one release later, which
is worth noticing: the sentence survived a commit that invalidated it.)

**A. Verify the ATA path against a real SATA drive.** *Needs: any SATA SSD or
HDD, on Windows. Half an hour.* **This is now retrospective.** The
recommendation was to hold the release until this check had been made; the
decision was to publish anyway, so 3.3.0 is on crates.io with the ATA parse
unverified. That raises the stakes rather than removing them: a defect found here
is a yank or a 3.3.1, not an unreleased fix.

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

Done when a real drive's attributes match `smartctl -A`. If they do not, the fix
ships as 3.3.1; if the readings are actively wrong rather than merely absent,
yank 3.3.0, because a wrong SMART attribute is the class of error that gets acted
on.

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

**E.** ~~Read PCIe link state on Windows.~~ Done in 3.6.0; see open work 7 for the
two routes that do not work and why.

**F. Continue the ontology sweep.** *Needs: nothing. Ongoing.* NUMA, RAPL,
sensors, virtualization and EDAC are the remaining clusters with readers behind
them. Add a domain per change, verify each field resolves to a true provenance on
the machine at hand, and check the `Unknown` variant first — see open work 5 for
why that is the standing instruction.

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
| `tests/ontology_conformance.rs` | Anything a new entity can get wrong: unreachable ids, absences with no reason, non-nullable nulls, values whose JSON type contradicts their unit, `Unknown` dressed as a measurement. Derives its cases from the ontology, so a domain added later is covered without editing it |

Each was checked against a deliberate break before being kept, because a test
that cannot fail is worse than none.

**A green suite is evidence about the surface it covers, and nothing else.**
The Dewey port passed 839 tests through four releases while its window opened at
800x600 titled "Dewey App", clipped every value off the right edge, responded to
no clicks at all, and reported no GPUs on a machine with three. The tests
asserted that each tab emitted named ontology nodes; the port did that
faithfully. Nothing failed because nothing was asked. The bugs were found by
screenshotting the window and looking at it — and each round of looking found
another one. If a change affects something a person sees, look at it before
reporting it done.

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

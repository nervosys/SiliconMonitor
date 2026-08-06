# Changelog

All notable changes to Silicon Monitor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.0.0] - 2026-08-06

SMART and NVMe support, and the removal of a fabrication the work uncovered.

### Added

- **`DiskDevice::smart_info()`** on Linux and Windows. Returns the drive's health
  verdict and whatever attributes the platform exposes. Verified on Windows against
  four physical drives.
- **`DiskDevice::nvme_info()`** on Linux and Windows. Identity — model, serial,
  firmware, capacity — needs no privileges. Linux additionally reads the controller
  id and namespace count from `/sys/class/nvme`, which Windows does not expose
  unelevated. Returns `NotSupported` for non-NVMe devices.
- `DiskDevice::health()` is now derived from SMART on both platforms. The Linux
  implementation previously returned `Healthy` whenever the device file existed,
  reporting "the kernel enumerated this drive" as a clean bill of health.

### Fixed

- **SMART reported fabricated readings for every drive.**
  `Get-StorageReliabilityCounter` requires elevation; unelevated it fails with
  `PermissionDenied`. That failure was swallowed by `-ErrorAction SilentlyContinue`
  and replaced with `0`, so simon reported every drive as healthy with a
  temperature of 0 °C and zero power-on hours. An access error was being presented
  as a measurement.
- **Every drive was named `PhysicalDrive0`.** `DeviceId` arrives from PowerShell as
  a JSON *string* (`"1"`), and `as_u64()` returned `None` for all of them, so the
  `.unwrap_or(0)` fallback collapsed four distinct drives into one name.
- **Health was graded from an empty scorecard.** With counters defaulting to zero,
  a drive whose data could not be read took no penalties, scored 100, and was
  declared `Good` with "100% life remaining" — a confident verdict derived from
  nothing. Scoring now requires evidence; with none, the platform's own verdict
  stands and no life estimate is published.
- **Every NVMe drive was classified as a plain SSD.** Windows answers `MediaType`
  (the medium, `"SSD"`) and `BusType` (the transport, `"NVMe"`) separately, and only
  `MediaType` was read — which then made `nvme_info()` refuse to answer for the
  drives it was written for.

### Changed — breaking

- `smart::SmartDiskInfo`'s counters are now `Option`: `temperature_celsius`,
  `power_on_hours`, `power_cycle_count`, `reallocated_sectors`, `pending_sectors`,
  `uncorrectable_errors`, `total_bytes_written`, `total_bytes_read`. `None` means
  the platform would not report it; it does not mean zero.
- `disk::NvmeInfo`'s `nvme_version`, `unallocated_capacity`, `controller_id`,
  `num_namespaces`, `power_state` and `critical_warnings` are now `Option` for the
  same reason. A `controller_id` of 0 is a real controller and a `num_namespaces`
  of 0 is a real answer; neither can stand in for "not read".
- `SmartMonitor::max_temperature()` returns `Option<u32>` rather than `0` when no
  drive reported a temperature.

## [2.1.5] - 2026-08-06

### Fixed

- **`docs/CPU_MONITORING.md` described the Windows implementation as a skeleton.**
  It called `src/silicon/windows.rs` "basic structure and skeleton" with a
  "placeholder for WMI and Performance Counter integration", and marked every
  Windows cell 🚧. The file is 644 lines across 25 functions with no TODO markers,
  and `simon cli cpu` returns twenty-four distinct per-core figures on a Ryzen 9
  9900X — not one average replicated, which is a defect this repository has shipped
  before. Per-core utilization corrected to ✅ on that evidence; temperature
  corrected to ❌, since `simon cli temperature` finds no sensors and the board
  ones need a signed kernel driver simon does not ship.
- **The same table marked macOS "✅ Complete" for a column nothing has ever
  exercised.** The crate could not build on macOS until 2.1.2. Those marks are now
  labelled unverified rather than corrected, because verifying them needs a Mac.
- The table also did not say which subsystem it describes. `src/silicon/` is the
  enhanced per-core layer; `CpuStats` / `stats::Simon` is the general path and has
  no macOS implementation at all. A ✅ in one table did not imply the other worked,
  and nothing said so.

### Verified, not changed

- Every documented flag (`--format`, `--watch`, `--frame`, `--script`,
  `--writable`, `--search`) exists on the command it is shown with.
- Every documented entity id resolves: `gpu.0.power.limit` and
  `gpu.0.thermal.temperature` return measured values, and the template form
  `gpu.{n}.name` is rejected with the guidance the contract promises.
- `docs/DISK_MONITORING.md`'s "trait defined, not yet implemented" claims for SMART
  and NVMe are accurate — both are trait defaults returning `NotSupported` with no
  platform override.

## [2.1.4] - 2026-08-06

### Fixed

- **The documented commands did not exist.** The README's main usage block showed
  `simon cpu`, `simon gpu`, `simon memory`, `simon processes`, `simon audio`,
  `simon bluetooth`, `simon displays` and `simon usb`. All of these live under
  `simon cli`, and `displays` is `display`. Anyone following the quick-start hit
  `error: unrecognized subcommand` on their first command — while the watch-mode
  examples ten lines below used the correct form. `CLI.md` had twenty-nine such
  lines and `docs/UTILITIES.md` one more.

### Added

- A test comparing every `simon …` invocation in the documentation against
  `simon describe --commands`, the machine-readable catalog the binary generates
  from its own argument parser. Documentation is not compiled, but the binary can
  be asked what it accepts, so the two can be checked against each other. It found
  `CLI.md`, which a manual sweep of the same question had missed entirely.

## [2.1.3] - 2026-08-05

### Fixed

- **The README's platform table claimed macOS CPU and memory support that does not
  exist.** Both were marked fully supported. Neither has a reader — the claim
  survived because the crate could not build on macOS at all, so nothing ever
  contradicted it. Corrected, with the gap and the reason stated beneath the table,
  and macOS disk's I/O-counter limitation noted alongside.
- **The Contributing section listed the GUI as "planned but not yet
  implemented"** while three other sections of the same README documented how to
  run it.
- The 2.1.2 changelog entry described only the manifest bug and the CI gaps, having
  been written before the 114 compile errors, the lint pass, and the defects that
  surfaced behind them. It now records what actually shipped.

This is a documentation-only release. 2.1.2's published page carries the incorrect
table, which is the reason for shipping a patch rather than waiting.

## [2.1.2] - 2026-08-05

### Fixed

- **The crate did not build on Linux or macOS — and never had.** A missing
  `[dependencies]` header meant the CLI, TUI, GUI, remote-AI-backend, and logging
  dependencies, plus the unconditional `chrono` and `async-trait`, were all parsed
  as a continuation of `[target.'cfg(windows)'.dependencies]`. On any other
  platform the `cli`, `gui`, and `remote-backends` features enabled code whose
  crates were absent from the dependency graph, and the build failed outright with
  `cannot find crate 'crossterm'` and a dozen more like it.

  This dates to the **initial commit** (2026-01-21) and shipped in **every one of
  the eight versions published to crates.io**, while the README advertised a native
  desktop application for Windows, Linux, and macOS. It survived because
  development happened on Windows, where the manifest is correct, and because a
  section header is invisible when you are reading the dependency beneath it —
  every individual line was right.

  `cargo tree --target x86_64-unknown-linux-gnu --features full` shows it in one
  command, and now a test asks the manifest the same question on every run.

- **The GUI could not build on Linux either.** `eframe` is declared with
  `default-features = false`, which drops its windowing backends along with the
  renderer defaults. Windows and macOS each have one backend and winit finds it
  without a feature; Linux has two, winit selects neither, and the build stops at
  `compile_error!`. `x11` and `wayland` are both named now.
- **`nvidia` did not resolve on macOS.** `nvml-wrapper` was declared only in the
  Linux and Windows target sections, but a feature is not target-conditional, so
  enabling it elsewhere referenced a crate absent from the graph. It is
  cross-platform now — nvml-wrapper is a runtime loader for libnvidia-ml, so it
  compiles anywhere and finds no library where there is no driver.

- **114 compile errors in code that had never been built** — 73 on Linux, 41 on
  macOS, across roughly forty files. The manifest bug meant cargo never reached
  simon's own source on those platforms, so none of it had ever been checked. Most
  were mechanical (a `SimonError::IoError` variant that does not exist, at 23
  sites; structs and enums whose fields and variants had been renamed under code
  that never recompiled). Several were not:

  - `motherboard/linux.rs` tested `contains("DP")` before `contains("eDP")`. Since
    `"eDP-1"` contains `"DP"`, **every internal laptop panel was reported as an
    external DisplayPort** and the eDP arm was unreachable.
  - `disk/linux.rs` indexed `parts[1]` of a `dev` file split on `':'` — a panic on
    any malformed file, sitting behind a `.unwrap_or(0)` that made it look handled.
    The major/minor pair it parsed was never read by anything.
  - `disk/macos.rs` and `gpu/intel.rs` reported measurements they never made: one
    combined `iostat` throughput figure halved into read and write, and a
    `gpu_memory: 0` meaning "shares system memory". Zero is not unknown.
  - `services.rs` borrowed `self` mutably and immutably at once; `numa/mod.rs`
    referenced a binding one line after it left scope; `process_monitor.rs` divided
    `f64` by `u64`; `hwmon/smart.rs` called `glob::glob` with no `glob` dependency.

- **`thermal.<none>` contradicted the readings beside it.** The resolver always
  enumerates cpu, gpu and motherboard and writes an `unavailable` row for each, then
  *also* emitted a summary row saying the domain enumerated nothing. Only reachable
  where no sensor reads at all, which is why the first macOS run found it and no
  Windows run could. The read-failure path still emits it, correctly — that path
  returns before pushing anything.

- **The plausibility suite encoded Windows' CPU accounting model.** It demanded
  user+system+idle sum to ~100. Linux divides by every field in `/proc/stat`, so
  nice, iowait, irq, softirq and steal are real time belonging to none of the
  three; a virtualized runner sits at 94%. The invariant that holds everywhere is
  that three shares of one total cannot exceed it.

- **The process-state set omitted `'I'`** — idle kernel thread, reported by Linux
  since 3.13 and carried by every `kworker/R-*`. The reader also fell back to `'?'`
  for an unreadable state where the rest of simon uses `'U'`.

- **102 clippy findings in never-linted Linux code**, which CI rejects under
  `-D warnings`.

### Added

- `tests/manifest_portability.rs`: every crate reachable from a feature must be
  declared in the plain `[dependencies]` table unless it is one of the three
  (`drm`, `drm-ffi`, `plist`) that are target-gated by design with call sites
  gated to match; and crates used without a `cfg` guard must not be target-gated.
  Verified against the pre-fix manifest, where it names each affected crate.

### Known gaps

- **simon does not measure CPU or memory on macOS.** There are readers for Linux
  and Windows and none for macOS: `CpuStats::new` and `MemoryStats::new` return
  empty, and `stats::Simon`'s ten platform functions report `UnsupportedPlatform`
  naming `SiliconMonitor`, which does have working macOS paths. The crate builds,
  lints, and passes its suite there — it does not yet read hardware. Tests that
  assert something *about a reading* are gated on `platform_has_hardware_readers()`
  so the gap is named once rather than hidden behind scattered `cfg`s. Implementing
  this means sysctl and IOKit work that has to be verified on real hardware.

### Fixed — CI

The manifest bug above was invisible to development but not to CI, which had been
red on every push. These are the reasons it stayed red without anyone learning
anything from it.

- **CI never compiled the `jetson-utils` feature.** Every job ran
  `--features full`, and `full` deliberately omits it — so
  `src/utils/{swap,clocks,power_mode,security}.rs` was never checked, linted, or
  tested by any CI run. That is shipped code a user can enable, and it is the most
  safety-sensitive in the repository; the feature's own comment points at
  SECURITY.md. It had accumulated 25 clippy warnings, invisible. `jetson-utils`
  pulls in no dependencies — it only gates `cfg` — so building everything costs
  nothing. All jobs now use `--all-features`.
- **Clippy ran without `--all-targets`,** so examples and integration tests were
  never linted. That is how `examples/agent_backends.rs` came to abort on its own
  second step, unnoticed, for months.
- **Three doc tests never ran in CI:** `--features full` runs 70, `--all-features`
  runs 73, and the three it skipped did not compile.
- The documentation tests added in 2.1.1 panicked outside a git checkout, which
  would have broken `cargo test` for anyone building from the packaged tarball or a
  vendored copy. They now skip when there is no checkout to enumerate, while still
  refusing to pass vacuously inside one.

## [2.1.1] - 2026-08-05

### Fixed

- **The README rendered six broken images**, on GitHub and on the crates.io page
  for 2.1.0. Two "Screenshots" sections linked PNGs under `docs/images/` that were
  never added — only the directory's own README describing how to capture them was
  ever committed. A broken image fails silently: the markdown is valid and the
  build is green, so nothing caught it. The TUI section now embeds a real frame
  captured through the same headless renderer the tests drive, so it cannot drift
  from what the TUI actually draws; the GUI section says plainly that screenshots
  are outstanding and how to see the interface meanwhile.

### Added

- A test asserting every relative markdown link and image resolves, and a second
  asserting documentation carries no machine identifiers. The embedded TUI frame is
  real tool output, which includes the host's name — sanitizing it was a manual
  step, and manual steps are the ones skipped next time the capture is refreshed.

## [2.1.0] - 2026-08-04

Model names in this repository were frozen lists that had aged out of date. The
fix is not new names — those would age too — but reaching for the provider's own
listing wherever one exists, and saying plainly that anything still hardcoded is a
guess.

### Fixed

- **The GUI's model dropdown never actually asked hosted providers what they
  serve.** `fetch_models_async` read the API key only from the tab's text field.
  Both hosted listing endpoints require authentication, so a user with
  `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` already exported — the normal case — sent
  an unauthenticated request, got a 401, received an empty list, and silently fell
  back to hardcoded names. The live-refresh path existed but could not be reached.
  The key is now resolved from the field *or* the environment, and setting a key in
  the tab drops the cached unauthenticated result so the provider is asked again.

### Changed

- **`simon ai manifest` no longer advertises a fixed `supported_models` list.**
  Seven export formats each carried a frozen array of model ids. Nothing consumed
  them, every one had gone stale (the OpenAI entry still listed `gpt-4o`, the
  Anthropic entry `claude-3-opus-20240229`), and the claim was wrong in principle:
  the manifest describes *tools*, so any model that calls tools in that provider's
  format can use it. Replaced with `model_discovery`, naming the provider's listing
  endpoint — which answers the question for the caller's own account and cannot
  rot. **Consumers reading `supported_models` must switch to `model_discovery`.**
- **Refreshed the model names that remain**, in the GUI's fallback list, the
  auto-detect defaults (now the named constants `DEFAULT_OPENAI_MODEL`,
  `DEFAULT_ANTHROPIC_MODEL`, `DEFAULT_GITHUB_MODEL`), the examples, and the docs.
  The GUI labels these "Not listed by provider" because that is what they are.
- **GitHub Models is no longer offered — GitHub retired it on 2026-07-30.** The
  playground, catalogue, inference API, and BYOK access are all shut down. Backend
  discovery previously treated the mere presence of `GITHUB_TOKEN` as an available
  backend; that variable is set on a great many machines for unrelated reasons (the
  `gh` CLI, CI), so those users were handed a provider guaranteed to fail, presented
  as ready to use. Discovery no longer offers it, the GUI labels it retired, and
  `BackendType::RemoteGitHub` and `DEFAULT_GITHUB_MODEL` are `#[deprecated]`. Both
  are retained so saved configurations still deserialize rather than failing to
  load.
- **Backend capability metadata was model-pinned and wrong.** `RemoteAnthropic`
  reported a 200K context and `$3.00/1M` ("Claude 3.5 Sonnet"); `RemoteOpenAI`
  reported 128K and `$5.00` ("GPT-4o pricing"). Context is now 1M for both, and the
  per-token cost is `None` — a single figure cannot express a hosted rate, which
  splits into input and output prices and varies per model. `None` is documented as
  "not known", not "free", and the discovery example now distinguishes the two
  rather than printing "Free" for a paid API.

- **The `agent_backends` example ran only two of its seven sections.** It opened by
  building `AgentConfig::new(ModelSize::Medium)` and calling it "the rule-based
  built-in backend, always available". No such backend exists — `Agent::new`
  requires one to be configured — so the example aborted on its own second step and
  everything after it was unreachable. It now uses the discovered backend, and
  reports a missing or failing backend instead of dying on it.

### Added

- Tests pinning the invariants behind all of the above: a provider shipping
  hardcoded names must have somewhere to find a credential (or those names can
  never be replaced), a local provider must never guess a model name, and no export
  format may ship a frozen model catalogue again.

## [2.0.1] - 2026-08-04

The first 2.x published to crates.io. 2.0.0 was tagged but never published: these
two fixes landed after the tag, and shipping them as 2.0.0 would have made the tag
point at code that was not what was released.

### Fixed

- **`--offline` announced a guarantee it did not enforce.** The flag is documented
  as disabling all network features; it set two environment variables and stopped.
  `consent::is_offline_mode` had no callers anywhere. A hosted backend received
  this machine's hardware inventory — including GPU UUIDs and PCI bus ids, which
  are stable device identifiers — while the tool reported that it would not.
  `Agent::ask` now refuses any backend whose `runs_on_host` is false when offline
  mode is set. The predicate is egress rather than "makes a network call", so a
  model served over loopback stays usable.
- **Two README links did not resolve.** `docs/AI_AGENT.md` was dropped by an
  earlier correction whose check only looked at the repository root, so any target
  in a subdirectory read as missing. The link checker now resolves each target
  relative to the file containing it and walks every tracked markdown file; three
  further broken links in CLI.md were found and fixed the same way.

## [2.0.0] - 2026-08-04

Tagged, not published. See 2.0.1.

A major version because five public signatures changed, not because the release is
larger than most. Under SemVer that is a major bump regardless of how unlikely the
breakage is, and this project states it adheres to SemVer — calling it 1.6.0 would
be a claim contradicted by the artifact.

### Breaking

`GpuCollection` no longer hands out references to the `Box` holding a GPU. Five
methods return the trait object directly:

| Before | After |
|---|---|
| `get(i) -> Option<&Box<dyn Gpu>>` | `get(i) -> Option<&dyn Gpu>` |
| `get_mut(i) -> Option<&mut Box<dyn Gpu>>` | `get_mut(i) -> Option<&mut (dyn Gpu + 'static)>` |
| `nvidia_gpus() -> Vec<&Box<dyn Gpu>>` | `nvidia_gpus() -> Vec<&dyn Gpu>` |
| `amd_gpus() -> Vec<&Box<dyn Gpu>>` | `amd_gpus() -> Vec<&dyn Gpu>` |
| `intel_gpus() -> Vec<&Box<dyn Gpu>>` | `intel_gpus() -> Vec<&dyn Gpu>` |

Callers that stored the result as `&Box<dyn Gpu>` need the type updated; call sites
that immediately invoke a trait method need no change, since `&Box<T>` already
auto-dereferenced. Migration is `.as_ref()` where a `Box` is genuinely wanted.

### Added — Machine-readable ontology over every reading

Every value simon reports now has a stable dotted id, a unit, and a **provenance**
saying where it came from. The three interfaces share one vocabulary rather than
three.

- `simon describe` — the schema: ids, units, provenance, descriptions. Touches no
  hardware, so it is identical on every machine and can be fetched and cached
  ahead of time.
- `simon describe --commands` — the command surface, walked out of the argument
  parser so it cannot drift from what the binary accepts.
- `simon describe --writable` — settings backed by a registered apply handler.
  Generated from the handler registry, so the schema cannot advertise a write the
  binary will reject.
- `simon get <id>` — one reading. Exits 1 for an unknown id, 2 for a known id
  with no value here, so a caller can tell "no such thing" from "nothing to
  report".
- `simon snapshot [--validate]` — every resolvable entity, across all ten domains.
  The count varies with the hardware present and the process table, so it is not
  quoted here.

`provenance` is the point: `measured` was sampled now, `specification` is a
published constant true of the hardware but not observed here, `derived` names its
inputs, and `unavailable` carries the reason it could not be read. Only `measured`
may be treated as a live observation. Nothing substitutes zero or a plausible
constant for a value it could not obtain, and a reading outside its declared range
is withheld rather than clamped.

### Added — Headless inspection of the TUI and GUI

Both interactive surfaces can now be read and driven without a terminal or a
display.

- `simon tui --frame [--tab NAME]` and `simon gui --frame [--tab NAME]`
- `simon tui --script` — `goto`, `key`, `refresh`, `capture`, `assert`, `refute`.
  Key steps go through the same handler the interactive loop calls.
- `simon gui --script` — the same minus `key`, since GUI tabs are addressable by
  name.

### Fixed

- The Profiles tab appeared dead. It was rendering all 19 groups; `RichText::strong()`
  resolved to the panel colour, so every `strong()` label in the GUI was invisible. The
  light theme had the same bug in white. The AI tab's reported failure has the same
  cause.
- Collapsing-header triangles (U+25BE/U+25B8) rendered as tofu — Geometric Shapes
  are not covered by the bundled emoji font — and duplicated the arrow the widget
  already draws.
- Numerous readings were constants presented as measurements: a boot time of exactly
  45 seconds whenever the real one could not be read, `min_freq = max_freq / 4`,
  Secure Boot inferred from a registry key's existence rather than its value, Apple
  GPU clock and power ceilings from a core-count table, macOS disk throughput halved
  into a fabricated read/write split, and NIC link speed divided by 1000 for any
  unrecognised unit.
- The port scanner reported `Filtered` — a specific claim that a firewall dropped
  the packet — for host-unreachable and permission-denied errors that never reached
  the host.

### Changed

- `lto = "thin"` in the release profile. Fat LTO triggers a compiler ICE on rustc
  1.97.1 for this crate, in lint-level sorting under `-C lto -C codegen-units=1`.
  Binary grows 3.6%. Restore `lto = true` once the toolchain is fixed.
- `ai_api::HardwareOntology` is superseded by `crate::ontology`. It carries no
  provenance, cannot resolve an id to a value, and no command exposes it.

## [1.5.0] - 2026-07-29

### Added — Lock-free snapshot pipeline

Hardware collection moved off the render thread entirely. A dedicated collector
thread owns every hardware handle and publishes immutable snapshots into an
`ArcSwap` slot; UI threads do a lock-free atomic load and never touch a driver.

- Independent collectors run **concurrently** within a tick, so tick cost is the
  slowest single collector rather than the sum of all of them.
- A **warm-up snapshot** is published from the collectors needing no driver setup
  (CPU, memory, system stats, disks), so first data no longer waits on GPU
  enumeration.
- `Snapshot` carries per-collector timings, making collection cost observable
  rather than guessed at.

Measured on a 3-GPU Windows host:

| | Before | After |
|---|---|---|
| Collection tick | 1184 ms serial | 338 ms concurrent |
| Process collector | 685 ms | 186 ms |
| GPU collector | 775 ms | 337 ms |
| Cold start (first data) | 8–12 s | ~192 ms |
| Frame data access | blocking driver call | 19 ns |

TUI, GUI and `simon record` all read from the pipeline. TUI and GUI now repaint only
when a new snapshot arrives instead of on a fixed tick.

### Added — IronWorks as the built-in inference engine

[IronWorks](https://github.com/nervosys/ironworks) is now the default backend and the
only engine simon ships against, reached over its OpenAI-compatible server. Every
other backend is an external provider (`BackendType::is_builtin_engine`).

### Added — CLI AI providers

`ollama`, `claude`, `codex` and `gemini` can be driven as subprocesses, detected on
`PATH`. No API key needed, since the tool is already authenticated.

`BackendType::runs_on_host` distinguishes local *inference* from a local *process*:
only `ollama` runs the model on your machine; the others relay prompts to their
vendor. Backend selection prefers on-host inference.

### Fixed — data that was fabricated or silently incomplete

- **Windows per-core CPU was the system average replicated.** `read_cpu_stats` used
  `GetSystemTimes` (system-wide) and assigned the identical value to every core, so
  a 24-core machine drew 24 identical bars that looked measured. Now uses real
  per-processor data via `NtQuerySystemInformation`.
- **Unelevated process listings omitted every SYSTEM process.** Processes whose
  `OpenProcess` failed were dropped entirely, and the plausible count hid it. They
  now emit a reduced row from the Toolhelp snapshot (315 → 439 processes observed).
- **The TUI invented memory readings** — a hardcoded 32 GB/16 GB fallback rendered
  identically to measured values.
- **Windows fan RPM was hardcoded to 1000** whenever `ActiveCooling` was true.
- **The GUI network chart plotted `(cumulative_bytes / 1MB) % 10000`** — a sawtooth
  of a running total, labelled as throughput. Now plots actual MB/s.
- **`simon record` sampled at the wrong rate.** It slept the full interval on top of
  ~1.2 s collection, so `--interval 1` recorded every ~2.2 s and every derived rate
  was wrong by that ratio.
- **macOS audio never classified any device as Input** (`has_output || true` made the
  flag dead).
- **vLLM requests all 404'd** — its default endpoint lacked `/v1` while discovery
  probed a different path, so it reported available but never worked.
- **GPU snapshots short-circuited on the first error**, blanking all devices when one
  failed. Now index-preserving, so a failing device keeps its slot.
- **llama.cpp was never discovered** despite being implemented; availability was
  hardcoded to `false`.

### Fixed — documentation that contradicted the code

- All 53 doc tests were failing (`use simon::` vs the actual crate name `simonlib`),
  plus six examples that had drifted from their APIs. All 70 now pass.
- The agent module documented four "Design Principles" that were each false: it
  claimed to run in a separate thread (no thread is ever spawned), to offer
  100M/500M/1B models (none are loaded), to keep all processing local (a backend is
  required, and hosted ones transmit telemetry), and to be consent-aware (no code
  path touches the consent module).
- `ask_with_timeout` ignores its timeout argument; this is now documented rather
  than implied otherwise.
- The README and `docs/AI_AGENT.md` described a rule-based offline fallback engine
  that does not exist, and linked to a missing `LOCAL_AI_BACKENDS.md`.

### Changed

- `GpuCollection::snapshot_all` queries devices concurrently; added
  `snapshot_all_partial` for index-preserving partial results.
- New `platform::windows::logical_drives` replaces a 26-letter `fs::metadata` probe
  that blocked on disconnected network drives.
- Process enumeration memoizes per-PID usernames on `(pid, creation_time)` and
  requests only `PROCESS_QUERY_LIMITED_INFORMATION`.
- Repository formatted with `cargo fmt --all`; `cargo fmt --check` had been failing.

## [1.4.0] - 2026-05-14

### Added — Hardware Profile Inspector (NVPI / XTU / Ryzen Master / nvme-cli)

A unified read-write inspector for vendor driver settings, application profiles,
and tunable hardware parameters across five subsystems (GPU, CPU, NVMe, Display,
Memory). Inspired by NVIDIA Profile Inspector, Intel XTU, AMD Ryzen Master, and
`nvme-cli`.

- **Five subsystem providers** with platform-specific surfaces:
  - **GPU**: NVML driver state (clocks, power limit, ECC, persistence), Linux
    `/sys/module/nvidia/parameters`, AMD `/sys/class/drm/card*/device/` knobs
    (DPM state, perf level, OD tables, hwmon power cap), Intel `gt_*_freq_mhz`,
    Windows display-class registry walk (~390 vendor driver overrides), full
    NVIDIA DRS binary database scan (~12k per-application profiles from
    `nvdrsdb*.bin`).
  - **CPU**: Linux cpufreq policy + governor + EPP, `intel_pstate` toggles, MSR
    `0x610` PL1/PL2 decode, Windows active power scheme + scheme enumeration via
    `PowerEnumerate`, RAPL energy domains.
  - **NVMe**: Linux `/sys/class/nvme/*` controller + namespace queue policy,
    typed Get-Features admin command (`NVME_IOCTL_ADMIN_CMD`) decoding Power
    Management, Volatile Write Cache, Number of Queues, Async Event Config,
    APST, Software Progress Marker.
  - **Display**: existing `DisplayMonitor` per-display state plus full EDID
    block decoder (manufacturer PNP code, product/serial, monitor name,
    preferred timing, DPMS flags, gamma).
  - **Memory**: SMBIOS Type 17 per-DIMM (rated vs configured speed flags
    XMP/EXPO-off conditions), plus XMP 2.0/3.0 + AMD EXPO SPD blob decoder on
    Linux (`/sys/bus/i2c/devices/*/eeprom`).
- **Apply (write) layer** with audit log:
  - `ApplyHandler` trait, `ApplyOutcome { status, message, timestamp, ... }`
  - JSON-lines audit log at `%LOCALAPPDATA%\simon\profile_audit.log` (Windows) /
    `$XDG_STATE_HOME/simon/profile_audit.log` (Linux), every attempt logged
    including refused/not-writable
  - Confirmation required (`--confirm` CLI flag, or `confirm=true` + env
    `SIMON_ALLOW_AGENT_WRITES=1` double-gate for MCP)
  - Concrete handlers: NVIDIA persistence mode (Linux NVML), Linux cpufreq
    governor, Linux AMD `power_dpm_force_performance_level`, Linux Intel
    `gt_max_freq_mhz`, Windows `PowerSetActiveScheme`
- **Analysis surface**:
  - `simon profile diff <baseline.json>` — drift report between snapshots
  - `simon profile deviations` — settings changed from declared default,
    sorted by risk
  - `simon profile explain <id>` — full setting metadata with related-setting
    lookup
  - `simon profile active [--matched]` — running PIDs joined against NVIDIA
    DRS database
  - `simon profile search <query>` — substring match across all settings
  - `simon profile watch [--interval]` — continuous change detection
  - `simon profile bench` — per-provider snapshot timing
  - `simon profile schemes` — enumerate Windows power schemes with friendly
    names
- **Caching**: `CachedProfileInspector` with per-subsystem TTLs (NVMe 30s, GPU
  15s, Memory 60s, Display 5s, CPU 2s); process-global hit/miss counters.
- **Interfaces**:
  - 16 CLI subcommands under `simon profile`
  - 12 MCP tools (`list_profile_subsystems`, `get_profile_settings`,
    `search_profile_settings`, `get_active_app_profiles`, `get_profile_deviations`,
    `explain_profile_setting`, `list_writable_profile_settings`,
    `apply_profile_setting`, `benchmark_profile_providers`, plus existing ones)
  - GUI: dedicated "🛠 Profiles" tab with deviation panel, audit log tail,
    risk-color-coded settings tree, text filter, subsystem chips, cache stats
  - TUI: "Profiles" tab with subsystem strip, `[d]` toggle for deviations
    overlay
  - Prometheus exporter: `profile_groups_total{subsystem}`,
    `profile_settings_total{subsystem}`, `profile_deviations_count{risk}`,
    `profile_writable_handlers_total`, `profile_cache_hits/misses_total`
- **Examples**: `examples/profile_inspector.rs`, `examples/active_profiles.rs`
- **Tests**: 55 unit tests for the profile module (525 total in the library)
- **Verified live**: 23,500 settings across 19 device groups on a Windows
  development host with RTX 3090 Ti + Micron DDR5 + Samsung NVMe.

### Fixed

- `display::DisplayInfo::aspect_ratio()` no longer panics with divide-by-zero
  on inactive virtual displays reporting 0x0 dimensions.

## [0.3.0] - 2026-02-02

### Added
- **Audio monitoring** - Device enumeration, volume control, mute status
- **Bluetooth monitoring** - Adapter and device tracking, connection states, battery levels
- **Display monitoring** - Monitor info, resolution, refresh rate, HDR support
- **USB monitoring** - Device enumeration, speed detection, class identification

### Platform Support
- Linux: Full implementations using sysfs, PulseAudio, BlueZ, xrandr
- Windows/macOS: Stub implementations ready for platform APIs

## [0.2.0] - 2026-02-02

### Added
- **Latest AI model support** - GPT-4o, GPT-4.5, o1, o3, Claude 4 Opus/Sonnet, Gemini 2.0, Grok 3, Llama 4, Mistral Large, DeepSeek-R1/V3
- **New CLI subcommand structure** - `simon ai query/manifest/server` and `amon query/manifest/server`
- **MCP server** - Claude Desktop integration via `simon ai server` or `amon server`
- **Multi-format manifest export** - openai, anthropic, gemini, grok, llama, mistral, deepseek, mcp, jsonld formats
- **AI agent export formats** - Export tool definitions for all major AI providers

### Changed  
- CLI restructured with nested subcommands for better organization
- `amon` now mirrors `simon ai` subcommand structure

### Fixed
- CI badge links in README (ci.yml → build-and-push.yml)
- Crates.io badge (simon → silicon-monitor)
- Compiler warnings for unused fields

## [0.1.0] - 2026-01-30

### Added

#### Core Features
- **Cross-platform hardware monitoring** - CPU, GPU, memory, disk, network monitoring
- **Multi-vendor GPU support** - NVIDIA (NVML), AMD (ROCm/sysfs), Intel (Level Zero)
- **Process monitoring** - System processes with GPU attribution
- **Network monitoring** - Interface stats, bandwidth tracking

#### GUI Application
- **Modern egui-based GUI** with dark/light theme support
- **Real-time dashboards** - CPU, GPU, memory, disk, network visualization
- **AI chatbot integration** - Natural language system queries
- **Data export** - JSON/CSV export functionality
- **Alert system** - Configurable threshold alerts
- **Historical data** - Time-series metric storage

#### AI Integration
- **AI Data API** - 35+ tools across 8 categories for AI system visibility
- **Observability API** - MCP-like permission system for external AI access
- **Agent engine** - Context-aware query processing
- **Tool call visualization** - See what tools the AI uses

#### Observability Module
- **System context materialization** - Structured state for AI reasoning
- **Event system** - Threshold alerts, state change detection
- **Metric collection** - Time-series with aggregation (min/max/avg/percentiles)
- **Permission system** - Capability-based access control (MCP-inspired)
- **HTTP/WebSocket server** - REST API for external access
- **Real-time streaming** - WebSocket metric/event streaming

#### Platform Support
- **Linux** - Full support (procfs, sysfs, device paths)
- **Windows** - Core monitoring (Win32 API)
- **macOS** - Basic support (IOKit)

### Security
- Added `.gitignore` patterns for sensitive files
- Capability-based permission system for API access
- Rate limiting for external API requests
- Sandbox detection for telemetry consent

---

[0.1.0]: https://github.com/nervosys/SiliconMonitor/releases/tag/v0.1.0

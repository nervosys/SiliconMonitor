# Changelog

All notable changes to Silicon Monitor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2026-08-04

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

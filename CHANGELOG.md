# Changelog

All notable changes to Silicon Monitor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

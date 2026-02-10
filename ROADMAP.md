# Silicon Monitor (simon) — Development Roadmap

## Overview

Silicon Monitor is the world's first agentic system monitoring utility and API. Built in Rust, it provides unified cross-platform APIs for CPUs, GPUs (NVIDIA/AMD/Intel/Apple), NPUs, memory, disks, motherboards, processes, network interfaces, peripherals (audio, Bluetooth, display, USB), and more — with native AI agent integration.

- **Crate**: [`silicon-monitor`](https://crates.io/crates/silicon-monitor) v0.4.0
- **License**: AGPL-3.0-or-later (commercial dual-license available)
- **MSRV**: Rust 1.70+

---

## ✅ Completed Features

### Core Monitoring
- [x] **GPU Monitoring** — NVIDIA (NVML), AMD (sysfs/WMI), Intel (i915/xe/WMI), Apple Silicon (powermetrics)
- [x] **CPU Monitoring** — Per-core utilization, frequency, governors, hybrid P/E architecture, cpufreq scaling
- [x] **Memory Monitoring** — RAM/swap, ZRAM, NUMA, huge pages, pressure levels, watermarks
- [x] **Disk Monitoring** — NVMe/SATA/USB detection, SMART health, I/O stats, cross-platform (Linux/Windows/macOS)
- [x] **Network Monitoring** — Interface statistics, bandwidth rates, connection tracking (TCP/UDP with PID mapping)
- [x] **Network Tools** — nmap-style port scanning, ping, traceroute, DNS lookup, banner grabbing, packet capture
- [x] **Process Monitoring** — CPU/memory usage, GPU attribution, delta-based per-process CPU%
- [x] **Motherboard Sensors** — Temperature, voltage, fan readings via hwmon/WMI
- [x] **NPU/Neural Engine** — ANE (Apple), Intel NPU, AMD AI Engine monitoring (via `npu` feature)
- [x] **Audio Monitoring** — Device enumeration, volume levels, mute states (Linux/Windows/macOS)
- [x] **Bluetooth Monitoring** — Adapter/device enumeration, battery levels, connection states
- [x] **Display Monitoring** — Resolutions, refresh rates, HDR, scaling, connection types
- [x] **USB Monitoring** — Device enumeration, device classes, speeds (up to USB4), topology
- [x] **Battery/Power Supply** — Charge state, health, wear level, cycle count, USB-PD/UPS support
- [x] **Fan Control** — PWM control, fan profiles (Silent/Quiet/Cool/Performance), thermal zone integration
- [x] **Boot Configuration** — UEFI/Legacy/SecureBoot detection, startup items, kernel modules
- [x] **System Services** — Cross-platform service monitoring (systemd on Linux, WMI on Windows)
- [x] **Health Scoring** — 0–100 system health score with per-subsystem status levels

### GPU Backend Architecture
- [x] Trait-based `Device` abstraction (`src/gpu/traits.rs`)
- [x] NVIDIA backend via NVML (`src/gpu/nvidia_new.rs`)
- [x] AMD backend via sysfs/WMI (`src/gpu/amd_rocm.rs`)
- [x] Intel backend via i915/xe/WMI (`src/gpu/intel_levelzero.rs`)
- [x] Apple Silicon backend via powermetrics (`src/gpu/apple.rs`)
- [x] Windows GPU helpers — DXGI adapter enumeration, WMI perf counters, per-engine metrics
- [x] Unified `GpuCollection` with `auto_detect()` across all vendors
- [x] GPU process attribution (PIDs using GPU memory)

### AI Agent Integration
- [x] **AI Data API** — 35+ monitoring tools for AI agent integration
- [x] **Agent Framework** — Local + remote backends, Ollama integration, ModelSize selection (100M–1B)
- [x] **Auto-query system** — Natural language to tool selection
- [x] **Multi-format export** — OpenAI functions, Anthropic tools, MCP server, prompt format
- [x] **MCP Server** — Model Context Protocol for Claude/LLM integration
- [x] **Hardware Ontology** — Structured hardware description for AI discoverability
- [x] **AI Workload Detection** — Framework auto-detect (PyTorch/TF/JAX), training metrics, inference latency
- [x] **GUI Chatbot** — Real-time system data in agent responses, tool call visualization
- [x] **Historical data queries** — 30-minute history, AI agent integration
- [x] **Response caching** — LRU cache for agent responses

### User Interfaces
- [x] **GUI (egui/eframe)** — Full native desktop application
  - Real-time charts with 60-second history
  - Tabbed interface (Overview, CPU, Accelerators, Memory, Storage, Network, Connections, AI Chat)
  - Cyber theme with neon colors, dark/light toggle
  - Alert/notification system (threshold-based)
  - Data export (JSON/CSV)
  - Emoji font support (Noto Emoji)
- [x] **TUI (ratatui)** — Terminal dashboard with selectable color themes, Peripherals tab, process detail view
- [x] **CLI** — `simon` (component monitoring) + `amon` (AI agent interface), `--watch` mode

### Infrastructure
- [x] **Time-series DB** — File-based TSDB with binary format, rotation, process snapshots
- [x] **Observability API** — Metrics, events, streaming, API keys, capabilities, rate limiting
- [x] **Sandbox Detection** — VM (VMware/VBox/QEMU/Hyper-V/KVM), containers (Docker/LXC), Wine, debugger
- [x] **Consent Management** — GDPR/CCPA-compliant with `--no-telemetry`/`--offline` flags, audit trail
- [x] **Configuration** — TOML-based config with persistence (interval, color scheme, GPU selection)
- [x] **Bandwidth Testing** — iperf-style TCP client with parallel streams

### Code Quality
- [x] Zero compiler warnings
- [x] Serde serialization for all metric types
- [x] Feature flags for platform/vendor-specific code (`nvidia`, `amd`, `intel`, `apple`, `cpu`, `npu`, `io`, `network`, `cli`, `gui`)
- [x] Comprehensive error handling with `thiserror`
- [x] Criterion benchmarks (CPU stats, GPU queries, process enumeration)
- [x] 250+ tests
- [x] Release profile optimized (`lto = true`, `codegen-units = 1`, `strip = true`)
- [x] Published to [crates.io](https://crates.io/crates/silicon-monitor)

---

## 📋 Planned Features

### Near-term (v0.5.0)

#### GPU Control & Tuning
- [ ] GPU clock control (safe wrappers with proper permissions)
- [ ] Power limit adjustment
- [ ] Fan curve control (NVIDIA/AMD)
- [ ] Multi-GPU workload balancing info

#### Process Monitoring
- [ ] Process tree visualization
- [ ] Container/cgroup awareness
- [ ] Process resource limiting suggestions

#### AI Agent
- [ ] Streaming responses in GUI chatbot
- [ ] Anomaly detection prompts
- [ ] System optimization recommendations

#### GUI
- [ ] Custom dashboard layouts
- [ ] System tray mode (setting added, tray-icon integration pending)

### Medium-term (v0.6.0)

#### Platform Expansion
- [ ] FreeBSD support
- [ ] WSL2 GPU passthrough detection
- [ ] Intel discrete GPU support (Arc series)

#### Monitoring Enhancements
- [ ] PCIe bandwidth monitoring
- [ ] Thunderbolt device monitoring
- [ ] EDID parsing for display details

### Long-term (v1.0.0)

#### Enterprise Features
- [ ] Remote monitoring (network daemon)
- [ ] Prometheus metrics endpoint
- [ ] Grafana dashboard templates
- [ ] Multi-host aggregation
- [ ] REST API server mode

#### Advanced AI
- [ ] Local LLM integration (llama.cpp via `local-llamacpp` feature)
- [ ] Predictive maintenance alerts
- [ ] Automated performance tuning
- [ ] Natural language system control ("reduce fan noise", "limit GPU power")

---

## 🐛 Known Issues

### High Priority
- [ ] Windows: Some hwmon sensors fall back to WMI (slower than direct sysfs on Linux)

### Medium Priority
- [ ] AMD GPU: Some metrics unavailable without root/admin privileges
- [ ] Intel GPU: Limited to i915/xe drivers (no discrete Arc GPU support yet)
- [ ] Network: Virtual interfaces may show incorrect rates

### Low Priority
- [ ] Some reserved code paths marked `#[allow(dead_code)]`

---

## 📊 Platform Support Matrix

| Feature        | Linux | Windows | macOS |
| -------------- | ----- | ------- | ----- |
| NVIDIA GPU     | ✅     | ✅       | ❌     |
| AMD GPU        | ✅     | ✅       | ❌     |
| Intel GPU      | ✅     | ✅       | ❌     |
| Apple Silicon  | ❌     | ❌       | ✅     |
| CPU Monitoring | ✅     | ✅       | ✅     |
| Memory         | ✅     | ✅       | ✅     |
| Disk           | ✅     | ✅       | ✅     |
| Network        | ✅     | ✅       | ✅     |
| Processes      | ✅     | ✅       | ✅     |
| Motherboard    | ✅     | ✅       | ✅     |
| Audio          | ✅     | ✅       | ✅     |
| Bluetooth      | ✅     | ✅       | ✅     |
| Display        | ✅     | ✅       | ✅     |
| USB            | ✅     | ✅       | ✅     |
| GUI            | ✅     | ✅       | ✅     |
| TUI            | ✅     | ✅       | ✅     |
| AI Agent       | ✅     | ✅       | ✅     |

Legend: ✅ Supported | ❌ Not applicable

---

## 📅 Release History

### v0.4.0 (February 2026)
- Switched to AGPL-3.0-or-later with commercial dual-license and CLA
- Packaging readiness for crates.io (exclude lists, metadata, docs)
- Performance profiling with Criterion benchmarks
- NPU monitoring wired into TUI and platform backends
- Windows GPU backends enhanced with DXGI, per-engine metrics, OHM/LHM temps

### v0.3.0 (January 2026)
- Peripheral monitoring: audio, Bluetooth, display, USB
- CLI watch mode for peripheral commands
- Hardware control APIs for audio and Bluetooth
- Peripherals tab in TUI

### v0.2.0 (January 2026)
- AI agent discoverability (MCP, OpenAI, Claude, Gemini)
- CLI restructured with subcommands
- Cross-platform improvements

### v0.1.0 (January 2026)
- Initial public release
- Multi-vendor GPU monitoring
- AI Data API with 35+ tools
- GUI and TUI interfaces
- Process monitoring with GPU attribution

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. All contributions require signing the [CLA](CLA.md).

---

## 📝 Notes

- Security-sensitive utilities in `src/utils/` require audit before production use
- GPU control features require elevated privileges
- Some metrics are vendor/platform-specific

---

*Last updated: January 24, 2026*

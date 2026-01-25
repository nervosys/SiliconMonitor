# Silicon Monitor (simon) - Development Roadmap

## Overview

Silicon Monitor is a comprehensive cross-platform hardware monitoring library for Rust, providing unified APIs for CPUs, GPUs (NVIDIA/AMD/Intel), memory, disks, motherboards, processes, and network interfaces.

---

## ✅ Completed Features

### Core Monitoring
- [x] **GPU Monitoring** - Multi-vendor support (NVIDIA via NVML, AMD via sysfs, Intel via i915/xe)
- [x] **CPU Monitoring** - Per-core utilization, frequency, governors (Linux)
- [x] **Memory Monitoring** - RAM/swap usage, detailed breakdown
- [x] **Disk Monitoring** - NVMe/SATA/USB detection, SMART health, I/O stats
- [x] **Network Monitoring** - Interface statistics, bandwidth rates
- [x] **Process Monitoring** - CPU/memory usage with GPU attribution
- [x] **Motherboard Sensors** - Temperature, voltage, fan readings via hwmon

### GPU Backend Architecture
- [x] Trait-based `Device` abstraction (`src/gpu/traits.rs`)
- [x] NVIDIA backend via NVML (`src/gpu/nvidia_new.rs`)
- [x] AMD backend via sysfs (`src/gpu/amd_rocm.rs`)
- [x] Intel backend via i915/xe drivers (`src/gpu/intel_levelzero.rs`)
- [x] Unified `GpuCollection` for auto-detection
- [x] GPU process attribution (PIDs using GPU memory)

### AI Integration
- [x] **AI Data API** - 35+ monitoring tools for AI agent integration
  - System summary, GPU/CPU/memory/disk/network status
  - Process queries (top CPU, top memory, top GPU, search)
  - Hardware sensors (temperatures, fans, voltages)
- [x] **Auto-query system** - Detects relevant tools from natural language
- [x] **Multi-format export** - OpenAI functions, Anthropic tools, prompt format
- [x] **GUI Chatbot Integration** - Real-time system data in agent responses

### User Interfaces
- [x] **GUI (egui/eframe)** - Modern hardware monitoring dashboard
  - Real-time graphs with 60-second history
  - Tabbed interface (Overview, CPU, Accelerators, Memory, Storage, Network, AI Chat)
  - Cyber theme with neon colors
  - Emoji font support (Noto Emoji)
- [x] **TUI (ratatui)** - Terminal-based monitoring
- [x] **CLI** - Command-line tools (`simon`, `amon`)

### Code Quality
- [x] Zero compiler warnings (all suppressed with `#[allow(dead_code)]` where appropriate)
- [x] Serde serialization for all metric types
- [x] Feature flags for platform/vendor-specific code
- [x] Comprehensive error handling with `thiserror`

---

## 🚧 In Progress

### Windows Platform Support
- [ ] Complete CPU monitoring (currently partial)
- [ ] Complete memory monitoring (currently partial)
- [ ] Disk I/O statistics
- [ ] Motherboard sensor support via WMI/OpenHardwareMonitor

### macOS Platform Support
- [ ] Apple Silicon GPU monitoring (M1/M2/M3)
- [ ] IOKit integration for sensors
- [ ] powermetrics integration

---

## 📋 Planned Features

### Near-term (v0.2.0)

#### GPU Enhancements
- [ ] GPU clock control (safe wrappers with proper permissions)
- [ ] Power limit adjustment
- [ ] Fan curve control (NVIDIA/AMD)
- [ ] Multi-GPU workload balancing info

#### Process Monitoring Enhancements
- [ ] Per-process GPU utilization (not just memory)
- [ ] Process tree visualization
- [ ] Container/cgroup awareness
- [ ] Process resource limiting suggestions

#### AI Agent Improvements
- [ ] Streaming responses in GUI chatbot
- [x] Tool call visualization (show which tools were called)
- [x] Historical data queries ("What was GPU temp 5 minutes ago?")
- [ ] Anomaly detection prompts
- [ ] System optimization recommendations

### Medium-term (v0.3.0)

#### Platform Expansion
- [ ] FreeBSD support
- [ ] ARM Linux support (Raspberry Pi, Jetson)
- [ ] WSL2 GPU passthrough detection

#### New Monitoring Capabilities
- [ ] PCIe bandwidth monitoring
- [ ] USB device enumeration
- [ ] Bluetooth adapter info
- [ ] Audio device monitoring
- [ ] Display/monitor information

#### GUI Enhancements
- [ ] Custom dashboard layouts
- [x] Alert/notification system (threshold-based with UI panel)
- [x] Data export (CSV, JSON)
- [ ] System tray mode (setting added, tray-icon integration pending)
- [x] Dark/light theme toggle

### Long-term (v1.0.0)

#### Enterprise Features
- [ ] Remote monitoring (network daemon)
- [ ] Prometheus metrics endpoint
- [ ] Grafana dashboard templates
- [ ] Multi-host aggregation
- [ ] REST API server mode

#### Advanced AI Features
- [ ] Local LLM integration (llama.cpp)
- [ ] Predictive maintenance alerts
- [ ] Automated performance tuning
- [ ] Natural language system control

---

## 🐛 Known Issues

### Critical
- None currently

### High Priority
- [x] ~~TUI gauge panic when percentage exceeds 100%~~ (Fixed: added clamping in ui.rs)
- [ ] Windows: Limited sensor support compared to Linux

### Medium Priority
- [ ] AMD GPU: Some metrics unavailable without root/admin
- [ ] Intel GPU: Limited to i915/xe drivers (no discrete GPU support yet)
- [ ] Network: Virtual interfaces may show incorrect rates

### Low Priority
- [x] ~~README.md has markdown linting warnings~~ (Fixed: all issues resolved)
- [ ] Some reserved code paths marked `#[allow(dead_code)]`

---

## 📊 Platform Support Matrix

| Feature        | Linux | Windows | macOS |
| -------------- | ----- | ------- | ----- |
| NVIDIA GPU     | ✅     | ✅       | ❌     |
| AMD GPU        | ✅     | 🚧       | ❌     |
| Intel GPU      | ✅     | 🚧       | ❌     |
| Apple Silicon  | ❌     | ❌       | 🚧     |
| CPU Monitoring | ✅     | 🚧       | 🚧     |
| Memory         | ✅     | 🚧       | 🚧     |
| Disk           | ✅     | ✅       | 🚧     |
| Network        | ✅     | ✅       | 🚧     |
| Processes      | ✅     | ✅       | 🚧     |
| Motherboard    | ✅     | 🚧       | 🚧     |
| GUI            | ✅     | ✅       | ✅     |
| TUI            | ✅     | ✅       | ✅     |
| AI Agent       | ✅     | ✅       | ✅     |

Legend: ✅ Full support | 🚧 Partial/WIP | ❌ Not supported

---

## 📅 Release History

### v0.1.0 (Current - January 2026)
- Initial public release
- Multi-vendor GPU monitoring
- AI Data API with 35+ tools
- GUI and TUI interfaces
- Linux full support, Windows/macOS partial

### Recent Updates (January 24, 2026)
- ✅ Added data export functionality (JSON/CSV) in GUI status bar
- ✅ Fixed TUI gauge panic when percentage exceeds 100%
- ✅ Added tool call visualization in GUI chatbot
- ✅ Fixed all markdown linting issues in README.md
- ✅ Added ROADMAP.md for development tracking
- ✅ Added dark/light theme toggle (Light theme with full color palette)
- ✅ Added alert/notification system (CPU/memory/GPU thresholds with UI panel)
- ✅ Added historical data queries (30-min history, AI agent integration)
- ✅ Added system tray mode setting (UI ready, tray-icon pending)

---

## 🤝 Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Code style and conventions
- Testing requirements
- Pull request process
- Feature flag usage

---

## 📝 Notes

- Security-sensitive utilities in `src/utils/` require audit before production use
- GPU control features require elevated privileges
- Some metrics are vendor/platform-specific

---

*Last updated: January 24, 2026*

# Changelog

All notable changes to Silicon Monitor will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

//! CLI tool for Silicon Monitor (simon)

#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
#[cfg(feature = "cli")]
use colored::Colorize;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "simon")]
#[command(about = "Silicon Monitor: Comprehensive hardware monitoring for CPUs, GPUs, NPUs, memory, I/O, and network silicon", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Disable all remote data collection for this session (no telemetry, analytics, or crash reports)
    #[arg(long, global = true)]
    no_telemetry: bool,

    /// Run completely offline - disable all network features including remote data collection
    #[arg(long, global = true)]
    offline: bool,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Commands {
    /// Launch Graphical User Interface (GUI) - desktop application
    #[cfg(feature = "gui")]
    Gui,
    /// Launch Terminal User Interface (TUI) - interactive dashboard
    Tui,
    /// Command-line interface for monitoring hardware
    Cli {
        #[command(subcommand)]
        action: CliSubcommand,

        /// Update interval in seconds
        #[arg(short, long, default_value = "1.0", global = true)]
        interval: f64,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text", global = true)]
        format: String,

        /// Watch mode: continuously refresh output (press 'q' to quit)
        #[arg(short, long, global = true)]
        watch: bool,
    },
    /// AI agent features: query system, export manifests, start MCP server
    Ai {
        #[command(subcommand)]
        action: AiSubcommand,
    },
    /// Record system metrics to a time-series database for analysis
    Record {
        #[command(subcommand)]
        action: RecordSubcommand,
    },
    /// Manage privacy settings and data collection consent
    Privacy {
        #[command(subcommand)]
        action: PrivacySubcommand,
    },
    /// Inspect hardware profiles and tunable driver settings (NVPI/XTU/Ryzen Master/nvme-cli style)
    Profile {
        #[command(subcommand)]
        action: ProfileSubcommand,
    },
    /// Run as a headless daemon configured from a TOML file
    ///
    /// Like `serve`, but reads host/port/interval from config and writes a PID
    /// file. Refuses to bind a routable address without an api_key.
    Daemon {
        /// Path to the TOML configuration file
        #[arg(short, long)]
        config: Option<String>,
        /// Print a sample configuration file and exit
        #[arg(long)]
        sample_config: bool,
    },
    /// Serve the REST API and Prometheus metrics over HTTP
    ///
    /// Prometheus metrics are at /metrics/prometheus (not /metrics — that route
    /// returns JSON). The bundled Grafana dashboards in grafana/ consume this.
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value_t = 9100)]
        port: u16,
        /// Address to bind. Defaults to loopback; set 0.0.0.0 to expose on the
        /// network, which also exposes full hardware telemetry to it.
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
        /// Log each request to stderr
        #[arg(long)]
        log_requests: bool,
    },
}

/// Profile subcommands for hardware setting inspection
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum ProfileSubcommand {
    /// List subsystems with profile data
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show all settings for a subsystem (gpu, cpu, nvme, display, memory)
    Show {
        /// Subsystem name
        subsystem: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Search settings across all subsystems
    Search {
        /// Case-insensitive search term
        query: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Dump the complete profile snapshot
    Dump {
        /// Output as JSON (default: text)
        #[arg(long)]
        json: bool,
        /// Write to file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Write a setting value (requires --confirm). Only ids with a registered handler are writable.
    Set {
        /// Exact setting id (e.g. "persistence_mode", "scaling_governor")
        setting_id: String,
        /// New value. Bool: true/false/on/off; numbers parsed as int/float; otherwise treated as text.
        value: String,
        /// Required to actually perform the write.
        #[arg(long)]
        confirm: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List available Windows power schemes (GUID + friendly name)
    Schemes {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List all writable setting ids on this build
    Writable {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Benchmark each provider — wall-clock time and throughput
    Bench {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Watch for changes: snapshot at --interval and print added/removed/changed entries
    Watch {
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: f64,
        /// Maximum number of ticks before exit (0 = run until Ctrl-C)
        #[arg(short, long, default_value = "0")]
        ticks: u32,
        /// Restrict watching to one subsystem (gpu, cpu, nvme, display, memory)
        #[arg(short, long)]
        subsystem: Option<String>,
    },
    /// Tail the apply audit log
    Audit {
        /// Number of trailing lines to show
        #[arg(short, long, default_value = "20")]
        lines: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List settings whose current value differs from their declared default
    Deviations {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show full metadata for a single setting by id
    Explain {
        /// Exact setting id (e.g. "power_limit_mw", "pl1_w", "feat.write_cache.enabled")
        setting_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List running processes and whether each has a known driver profile
    Active {
        /// Show only processes with a known profile
        #[arg(short, long)]
        matched: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Compare a saved JSON snapshot against the current state
    Diff {
        /// Path to baseline snapshot JSON (created via `simon profile dump --json -o file.json`)
        baseline: PathBuf,
        /// Compare against another saved snapshot instead of current state
        #[arg(short, long)]
        against: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

/// AI subcommands for agent integration
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum AiSubcommand {
    /// Ask AI agent about system state (interactive if no query provided)
    Query {
        /// Question to ask the AI agent
        question: Option<String>,
    },
    /// Export tool manifests for AI agents (OpenAI, Claude, Gemini, etc.)
    Manifest {
        /// Output format: openai, anthropic, gemini, grok, llama, mistral, deepseek, jsonld, mcp, or json
        #[arg(short, long, default_value = "json")]
        format: String,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Start MCP (Model Context Protocol) server for Claude Desktop integration
    Server,
}

/// CLI subcommands for hardware monitoring
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum CliSubcommand {
    /// Show board information
    Board,
    /// Monitor GPU statistics
    Gpu,
    /// Monitor CPU statistics
    Cpu,
    /// Monitor memory statistics
    Memory,
    /// Monitor power statistics
    Power,
    /// Monitor temperature statistics
    Temperature,
    /// Monitor processes with smart categorization
    Processes,
    /// Monitor Tegra/Jetson engine clocks (APE, DLA, NVENC). Linux only —
    /// reads /sys/kernel/debug/clk, which does not exist on other platforms.
    /// For desktop GPU encoder/decoder utilization, use `gpu`.
    Engines,
    /// Monitor audio devices
    Audio,
    /// Monitor Bluetooth devices
    Bluetooth,
    /// Monitor displays
    Display,
    /// List USB devices
    Usb,
    /// Show all statistics
    All,
    /// Interactive real-time monitoring mode
    Monitor,
    /// Ask AI agent about system state
    Ai {
        /// Question to ask the AI agent (if not provided, enters interactive mode)
        query: Option<String>,
    },
    /// NVIDIA Jetson-specific utilities (requires Linux + Jetson hardware)
    Jetson {
        #[command(subcommand)]
        action: JetsonSubcommand,
    },
}

/// Jetson-specific subcommands (Linux + NVIDIA Jetson hardware only)
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum JetsonSubcommand {
    /// Jetson Clocks - Maximize performance by locking clocks at max frequency
    Clocks {
        #[command(subcommand)]
        action: JetsonClocksAction,
    },
    /// NVPModel - Power mode management (MAXN, 15W, 30W, etc.)
    Powermode {
        #[command(subcommand)]
        action: NvpmodelAction,
    },
    /// Swap file management (create, enable, disable, remove)
    Swap {
        #[command(subcommand)]
        action: SwapAction,
    },
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum JetsonClocksAction {
    /// Enable jetson_clocks (maximize performance)
    Enable,
    /// Disable jetson_clocks (restore settings)
    Disable,
    /// Show jetson_clocks status
    Status,
    /// Store current configuration
    Store,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum NvpmodelAction {
    /// Show current power mode
    Show,
    /// List all available power modes
    List,
    /// Set power mode by ID
    Set {
        /// Mode ID
        mode_id: u32,
        /// Force mode change
        #[arg(short, long)]
        force: bool,
    },
    /// Set power mode by name
    SetName {
        /// Mode name
        name: String,
        /// Force mode change
        #[arg(short, long)]
        force: bool,
    },
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum SwapAction {
    /// Show current swap status
    Status,
    /// Create a new swap file
    Create {
        /// Swap file path
        #[arg(short, long, default_value = "/swapfile")]
        path: PathBuf,
        /// Size in GB
        #[arg(short, long, default_value = "8")]
        size: u32,
        /// Enable on boot
        #[arg(short, long)]
        auto: bool,
    },
    /// Enable swap file
    Enable {
        /// Swap file path
        path: PathBuf,
    },
    /// Disable swap file
    Disable {
        /// Swap file path
        path: PathBuf,
    },
    /// Remove swap file
    Remove {
        /// Swap file path
        path: PathBuf,
    },
}

/// Record subcommands for time-series database operations
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum RecordSubcommand {
    /// Start recording system metrics to the database
    Start {
        /// Database file path
        #[arg(short, long, default_value = "simon_metrics.db")]
        database: PathBuf,

        /// Maximum database size (e.g., 100MB, 1GB)
        #[arg(long, default_value = "100MB")]
        max_size: String,

        /// Recording interval in seconds
        #[arg(short, long, default_value = "1.0")]
        interval: f64,

        /// Maximum number of processes to record per snapshot
        #[arg(long, default_value = "50")]
        max_processes: usize,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Show database statistics and information
    Info {
        /// Database file path
        #[arg(short, long, default_value = "simon_metrics.db")]
        database: PathBuf,
    },
    /// Query recorded metrics
    Query {
        /// Database file path
        #[arg(short, long, default_value = "simon_metrics.db")]
        database: PathBuf,

        /// Start time (minutes ago, or Unix timestamp)
        #[arg(long)]
        start: Option<String>,

        /// End time (minutes ago, or Unix timestamp)
        #[arg(long)]
        end: Option<String>,

        /// Output format (json or text)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Export recorded data to JSON or CSV
    Export {
        /// Database file path
        #[arg(short, long, default_value = "simon_metrics.db")]
        database: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Export format (json or csv)
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Delete the database file
    Clear {
        /// Database file path
        #[arg(short, long, default_value = "simon_metrics.db")]
        database: PathBuf,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

/// Privacy subcommands for managing data collection consent
#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum PrivacySubcommand {
    /// Show current consent and data collection status
    Status,
    /// Opt out of all remote data collection (revoke all consents)
    OptOut,
    /// Review and change consent settings interactively
    Review,
    /// Opt in to all data collection categories
    OptIn,
    /// Show what data would be collected for each consent category
    Info,
}

#[cfg(feature = "cli")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    env_logger::init();

    // Handle global privacy flags early
    if cli.no_telemetry || cli.offline {
        // Set environment variable to signal all components to disable telemetry
        std::env::set_var("SIMON_NO_TELEMETRY", "1");
        if cli.offline {
            std::env::set_var("SIMON_OFFLINE", "1");
        }
        if std::env::var("SIMON_VERBOSE").is_ok() {
            eprintln!(
                "{} {}",
                "[INFO]".cyan(),
                if cli.offline {
                    "Running in offline mode - all network features and data collection disabled"
                } else {
                    "Telemetry disabled for this session"
                }
            );
        }
    }

    match &cli.command {
        // GUI command - Graphical User Interface (default if no command given)
        #[cfg(feature = "gui")]
        Some(Commands::Gui) | None => {
            simonlib::gui::run().map_err(|e| format!("GUI error: {}", e))?;
        }

        // TUI command - Terminal User Interface
        Some(Commands::Tui) => {
            simonlib::tui::run()?;
        }

        // CLI commands - use shared MonitoringBackend
        Some(Commands::Cli {
            action,
            interval,
            format,
            watch,
        }) => {
            handle_cli_command(action, *interval, format, *watch)?;
        }

        // AI subcommands - query, manifest, server
        Some(Commands::Ai { action }) => match action {
            AiSubcommand::Query { question } => {
                handle_ai_command(question.as_deref())?;
            }
            AiSubcommand::Manifest { format, output } => {
                handle_ai_manifest(&format, output.as_ref())?;
            }
            AiSubcommand::Server => {
                handle_mcp_server()?;
            }
        },

        // Record command - time-series database operations
        Some(Commands::Record { action }) => {
            handle_record_command(action)?;
        }

        // Privacy command - manage data collection consent
        Some(Commands::Privacy { action }) => {
            handle_privacy_command(action)?;
        }

        // Profile command - hardware profile inspector
        Some(Commands::Profile { action }) => {
            handle_profile_command(action)?;
        }
        Some(Commands::Daemon {
            config,
            sample_config,
        }) => {
            handle_daemon_command(config.as_deref(), *sample_config)?;
        }
        Some(Commands::Serve {
            port,
            bind,
            log_requests,
        }) => {
            handle_serve_command(*port, bind, *log_requests)?;
        }

        // Default: launch GUI if available, otherwise TUI
        #[cfg(not(feature = "gui"))]
        None => {
            simonlib::tui::run()?;
        }
    }

    Ok(())
}

/// Handle top-level AI command (shortcut for 'cli ai')
#[cfg(feature = "cli")]
fn handle_ai_command(query: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Delegate to the same AI handler used by 'cli ai'
    handle_ai_query(query)
}

/// Handle MCP server command
#[cfg(feature = "cli")]
fn handle_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::ai_api::McpServer;

    eprintln!("[*] Starting MCP server on stdio...");
    eprintln!(
        "[*] Protocol version: {}",
        simonlib::ai_api::MCP_PROTOCOL_VERSION
    );

    let mut server = McpServer::new()?;
    server.run_stdio()?;
    Ok(())
}

/// Handle AI manifest export command
#[cfg(feature = "cli")]
fn handle_ai_manifest(
    format: &str,
    output: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::ai_api::{AgentManifest, ExportFormat};

    let manifest = AgentManifest::new();

    let export_format = match format.to_lowercase().as_str() {
        "openai" | "chatgpt" | "gpt" | "gpt4" | "o1" | "o3" => ExportFormat::OpenAI,
        "anthropic" | "claude" => ExportFormat::Anthropic,
        "gemini" | "google" => ExportFormat::Gemini,
        "grok" | "xai" => ExportFormat::Grok,
        "llama" | "meta" => ExportFormat::Llama,
        "mistral" | "mixtral" => ExportFormat::Mistral,
        "deepseek" | "r1" => ExportFormat::DeepSeek,
        "jsonld" | "json-ld" | "schema" => ExportFormat::JsonLd,
        "mcp" => ExportFormat::Mcp,
        "json" | "simple" => ExportFormat::SimpleJson,
        _ => {
            eprintln!("Unknown format '{}'. Supported: openai, anthropic, gemini, grok, llama, mistral, deepseek, jsonld, mcp, json", format);
            return Err("Invalid format".into());
        }
    };

    let exported = manifest.export(export_format);
    let json_output = serde_json::to_string_pretty(&exported)?;

    if let Some(path) = output {
        std::fs::write(path, &json_output)?;
        eprintln!("[+] Manifest written to: {}", path.display());
    } else {
        println!("{}", json_output);
    }

    Ok(())
}

/// Handle CLI subcommands using the shared MonitoringBackend
#[cfg(feature = "cli")]
fn handle_cli_command(
    action: &CliSubcommand,
    interval: f64,
    format: &str,
    watch: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::backend::{BackendConfig, MonitoringBackend};
    use simonlib::Simon;

    // Create backend config with the specified interval
    let config = BackendConfig {
        update_interval: Duration::from_secs_f64(interval),
        ..Default::default()
    };

    // Helper closure to run in watch mode
    let run_watch_mode = |display_fn: &dyn Fn() -> Result<(), Box<dyn std::error::Error>>| -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::{
            event::{self, Event, KeyCode},
            terminal::{disable_raw_mode, enable_raw_mode},
        };

        println!("Watch mode - Press 'q' to quit, refreshing every {:.1}s", interval);
        enable_raw_mode()?;

        loop {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");

            // Display the data
            if let Err(e) = display_fn() {
                disable_raw_mode()?;
                return Err(e);
            }

            println!("\n[Press 'q' to quit]");

            // Check for quit key with timeout
            if event::poll(Duration::from_secs_f64(interval))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') || key.code == KeyCode::Char('Q') {
                        break;
                    }
                }
            }
        }

        disable_raw_mode()?;
        Ok(())
    };
    match action {
        CliSubcommand::Board => {
            let stats = Simon::with_interval(interval)?;
            let board = stats.board_info();
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(board)?);
            } else {
                print_board_info(board);
            }
        }
        CliSubcommand::Gpu => {
            // Use MonitoringBackend for GPU data
            let backend = MonitoringBackend::with_config(config)?;
            if format == "json" {
                let gpu_info: Vec<_> = backend
                    .gpu_static_info()
                    .iter()
                    .zip(backend.gpu_dynamic_info().iter())
                    .map(|(s, d)| {
                        serde_json::json!({
                            "name": s.name,
                            "vendor": s.vendor.to_string(),
                            "utilization": d.utilization,
                            "memory_used": d.memory.used,
                            "memory_total": d.memory.total,
                            "temperature": d.thermal.temperature,
                            "power_draw": d.power.draw,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&gpu_info)?);
            } else {
                print_gpu_info_backend(&backend);
            }
        }
        CliSubcommand::Cpu => {
            let backend = MonitoringBackend::with_config(config)?;
            if format == "json" {
                if let Some(cpu) = backend.cpu_stats() {
                    println!("{}", serde_json::to_string_pretty(cpu)?);
                }
            } else {
                print_cpu_info_backend(&backend);
            }
        }
        CliSubcommand::Memory => {
            let backend = MonitoringBackend::with_config(config)?;
            if format == "json" {
                if let Some(mem) = backend.memory_stats() {
                    println!("{}", serde_json::to_string_pretty(mem)?);
                }
            } else {
                print_memory_info_backend(&backend);
            }
        }
        CliSubcommand::Power => {
            let mut stats = Simon::with_interval(interval)?;
            let snapshot = stats.snapshot()?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&snapshot.power)?);
            } else {
                print_power_info(&snapshot.power);
            }
        }
        CliSubcommand::Temperature => {
            let mut stats = Simon::with_interval(interval)?;
            let snapshot = stats.snapshot()?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&snapshot.temperature)?);
            } else {
                print_temperature_info(&snapshot.temperature);
            }
        }
        CliSubcommand::Processes => {
            let backend = MonitoringBackend::with_config(config)?;
            if format == "json" {
                let procs = backend.processes();
                println!("{}", serde_json::to_string_pretty(&procs)?);
            } else {
                print_process_info_backend(&backend)?;
            }
        }
        CliSubcommand::Engines => {
            let mut stats = Simon::with_interval(interval)?;
            let snapshot = stats.snapshot()?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&snapshot.engines)?);
            } else {
                print_engine_info(&snapshot.engines);
            }
        }
        CliSubcommand::All => {
            let backend = MonitoringBackend::with_config(config)?;
            if format == "json" {
                let state = backend.get_full_system_state();
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                print_all_info(&backend)?;
            }
        }
        CliSubcommand::Monitor => {
            let stats = Simon::with_interval(interval)?;
            run_interactive_mode(stats)?;
        }
        CliSubcommand::Ai { query } => {
            handle_ai_query(query.as_deref())?;
        }
        CliSubcommand::Audio => {
            use simonlib::audio::AudioMonitor;

            let display_audio = || -> Result<(), Box<dyn std::error::Error>> {
                let monitor = AudioMonitor::new()?;
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(monitor.devices())?);
                } else {
                    println!("{}", "═══ Audio Devices ═══".cyan().bold());
                    println!(
                        "  Master Volume: {}%",
                        monitor.master_volume().unwrap_or(100)
                    );
                    println!("  Muted: {}", if monitor.is_muted() { "Yes" } else { "No" });
                    for device in monitor.devices() {
                        println!(
                            "  {} ({:?}) - {:?}",
                            device.name, device.device_type, device.state
                        );
                    }
                }
                Ok(())
            };

            if watch {
                run_watch_mode(&display_audio)?;
            } else {
                display_audio()?;
            }
        }
        CliSubcommand::Bluetooth => {
            use simonlib::bluetooth::BluetoothMonitor;

            let display_bluetooth = || -> Result<(), Box<dyn std::error::Error>> {
                let monitor = BluetoothMonitor::new()?;
                if format == "json" {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "available": monitor.is_available(),
                            "adapters": monitor.adapters(),
                            "devices": monitor.devices(),
                        }))?
                    );
                } else {
                    println!("{}", "═══ Bluetooth ═══".cyan().bold());
                    println!(
                        "  Available: {}",
                        if monitor.is_available() { "Yes" } else { "No" }
                    );
                    println!("  Adapters: {}", monitor.adapters().len());
                    for adapter in monitor.adapters() {
                        println!("    {} ({})", adapter.name, adapter.address);
                    }
                    println!("  Devices: {}", monitor.devices().len());
                    for device in monitor.devices() {
                        println!(
                            "    {} - {:?}",
                            device.name.as_deref().unwrap_or("Unknown"),
                            device.state
                        );
                    }
                }
                Ok(())
            };

            if watch {
                run_watch_mode(&display_bluetooth)?;
            } else {
                display_bluetooth()?;
            }
        }
        CliSubcommand::Display => {
            use simonlib::display::DisplayMonitor;

            let display_displays = || -> Result<(), Box<dyn std::error::Error>> {
                let monitor = DisplayMonitor::new()?;
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(monitor.displays())?);
                } else {
                    println!("{}", "═══ Displays ═══".cyan().bold());
                    println!("  Count: {}", monitor.count());
                    for display in monitor.displays() {
                        println!(
                            "  {} {}x{} @ {:.0}Hz {:?}",
                            display.name.as_deref().unwrap_or("Unknown"),
                            display.width,
                            display.height,
                            display.refresh_rate,
                            display.connection
                        );
                    }
                }
                Ok(())
            };

            if watch {
                run_watch_mode(&display_displays)?;
            } else {
                display_displays()?;
            }
        }
        CliSubcommand::Usb => {
            use simonlib::usb::UsbMonitor;

            let display_usb = || -> Result<(), Box<dyn std::error::Error>> {
                let monitor = UsbMonitor::new()?;
                if format == "json" {
                    println!("{}", serde_json::to_string_pretty(monitor.devices())?);
                } else {
                    println!("{}", "═══ USB Devices ═══".cyan().bold());
                    println!("  Count: {}", monitor.devices().len());
                    for device in monitor.devices() {
                        let name = device.product.as_deref().unwrap_or("Unknown");
                        println!(
                            "  [{:04x}:{:04x}] {} ({:?})",
                            device.vendor_id, device.product_id, name, device.speed
                        );
                    }
                }
                Ok(())
            };

            if watch {
                run_watch_mode(&display_usb)?;
            } else {
                display_usb()?;
            }
        }
        CliSubcommand::Jetson { action } => {
            handle_jetson_command(action)?;
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
fn print_board_info(board: &simonlib::core::platform_info::BoardInfo) {
    println!("{}", "═══ Board Information ═══".cyan().bold());
    println!(
        "  {} {}",
        "Model:".white().bold(),
        board.hardware.model.yellow()
    );
    println!(
        "  {} {} {}",
        "System:".white().bold(),
        board.platform.system.green(),
        board.platform.machine.green()
    );
    if let Some(dist) = &board.platform.distribution {
        println!("  {} {}", "Distribution:".white().bold(), dist.green());
    }
    println!(
        "  {} {}",
        "Kernel:".white().bold(),
        board.platform.release.green()
    );

    if let Some(l4t) = &board.hardware.l4t {
        println!("  {} {}", "L4T:".white().bold(), l4t.magenta());
    }
    if let Some(cuda) = &board.libraries.cuda {
        println!("  {} {}", "CUDA:".white().bold(), cuda.magenta());
    }
}

#[cfg(feature = "cli")]
fn print_gpu_info(gpus: &std::collections::HashMap<String, simonlib::core::gpu::GpuInfo>) {
    println!("{}", "═══ GPU Information ═══".cyan().bold());
    if gpus.is_empty() {
        println!("  {}", "No GPUs detected".yellow());
        return;
    }
    // `GpuStats` is a HashMap, whose iteration order changes between calls. In the
    // interactive monitor that made the device list jump around between frames.
    let mut names: Vec<&String> = gpus.keys().collect();
    names.sort();

    for name in names {
        let gpu = &gpus[name];
        println!(
            "\n  {} {} ({})",
            "▶".green(),
            name.white().bold(),
            format!("{:?}", gpu.gpu_type).magenta()
        );

        // Color-code load based on usage
        let load_str = format!("{:.1}%", gpu.status.load);
        let load_colored = if gpu.status.load > 90.0 {
            load_str.red().bold()
        } else if gpu.status.load > 70.0 {
            load_str.yellow()
        } else {
            load_str.green()
        };
        println!("    {} {}", "Load:".white(), load_colored);

        // Clocks are unavailable on several adapters (the Windows AMD path reads
        // none). Printing "0 MHz (0-0 MHz)" states a clock that was never read.
        if gpu.frequency.current > 0 || gpu.frequency.max > 0 {
            println!(
                "    {} {} {} ({}-{} MHz)",
                "Frequency:".white(),
                format!("{} MHz", gpu.frequency.current).cyan(),
                "range:".dimmed(),
                gpu.frequency.min,
                gpu.frequency.max
            );
        }
        // Only Linux DVFS drivers expose a governor.
        if !gpu.frequency.governor.is_empty() {
            println!(
                "    {} {}",
                "Governor:".white(),
                gpu.frequency.governor.blue()
            );
        }

        if let (Some(used), Some(total)) = (gpu.status.memory_used, gpu.status.memory_total) {
            println!(
                "    {} {} / {} MB",
                "Memory:".white(),
                (used / 1024 / 1024).to_string().cyan(),
                total / 1024 / 1024
            );
        }

        if let Some(temp) = gpu.status.temperature {
            let temp_str = format!("{:.1}°C", temp);
            let temp_colored = if temp > 85.0 {
                temp_str.red().bold()
            } else if temp > 70.0 {
                temp_str.yellow()
            } else {
                temp_str.green()
            };
            println!("    {} {}", "Temperature:".white(), temp_colored);
        }
        if let Some(power) = gpu.status.power_draw {
            println!(
                "    {} {}",
                "Power:".white(),
                format!("{:.1}W", power).yellow()
            );
        }
    }
}

#[cfg(feature = "cli")]
fn print_cpu_info(cpu: &simonlib::core::cpu::CpuStats) {
    println!("{}", "═══ CPU Information ═══".cyan().bold());
    println!(
        "  {} {} (Online: {})",
        "Cores:".white().bold(),
        cpu.core_count().to_string().green(),
        cpu.online_count().to_string().green()
    );

    let usage = 100.0 - cpu.total.idle;
    let usage_str = format!("{:.1}%", usage);
    let usage_colored = if usage > 90.0 {
        usage_str.red().bold()
    } else if usage > 70.0 {
        usage_str.yellow()
    } else {
        usage_str.green()
    };
    println!(
        "  {} {} ({}: {:.1}%, {}: {:.1}%, {}: {:.1}%)",
        "Usage:".white().bold(),
        usage_colored,
        "user".dimmed(),
        cpu.total.user,
        "system".dimmed(),
        cpu.total.system,
        "idle".dimmed(),
        cpu.total.idle
    );

    println!("\n  {}", "Per-Core Usage:".white().bold());

    // Display cores in a compact grid format
    let cores_per_row = 4;
    for chunk in cpu.cores.chunks(cores_per_row) {
        print!("    ");
        for core in chunk {
            if core.online {
                let usage = 100.0 - core.idle.unwrap_or(0.0);
                let bar = create_usage_bar(usage, 10);
                let usage_str = format!("{:>5.1}%", usage);
                let usage_colored = if usage > 90.0 {
                    usage_str.red()
                } else if usage > 70.0 {
                    usage_str.yellow()
                } else {
                    usage_str.green()
                };
                print!(
                    "{}{:>2}{} {} {} ",
                    "CPU".dimmed(),
                    core.id,
                    ":".dimmed(),
                    bar,
                    usage_colored
                );
            } else {
                print!(
                    "{}{:>2}{} {}     ",
                    "CPU".dimmed(),
                    core.id,
                    ":".dimmed(),
                    "OFFLINE".red()
                );
            }
        }
        println!();
    }
}

/// Create a colored usage bar
#[cfg(feature = "cli")]
fn create_usage_bar(usage: f32, width: usize) -> String {
    let filled = ((usage / 100.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);

    let bar_char = "█";
    let empty_char = "░";

    let filled_str = bar_char.repeat(filled);
    let empty_str = empty_char.repeat(empty);

    let colored_filled = if usage > 90.0 {
        filled_str.red()
    } else if usage > 70.0 {
        filled_str.yellow()
    } else {
        filled_str.green()
    };

    format!("{}{}", colored_filled, empty_str.dimmed())
}

#[cfg(feature = "cli")]
fn print_memory_info(memory: &simonlib::core::memory::MemoryStats) {
    println!("{}", "═══ Memory Information ═══".cyan().bold());

    let ram_usage = memory.ram_usage_percent();
    let ram_bar = create_usage_bar(ram_usage, 20);
    let ram_used_gb = memory.ram.used as f64 / 1024.0 / 1024.0;
    let ram_total_gb = memory.ram.total as f64 / 1024.0 / 1024.0;

    let ram_pct_str = format!("{:.1}%", ram_usage);
    let ram_pct_colored = if ram_usage > 90.0 {
        ram_pct_str.red().bold()
    } else if ram_usage > 75.0 {
        ram_pct_str.yellow()
    } else {
        ram_pct_str.green()
    };

    println!(
        "  {} {} {:.2} / {:.2} GB {}",
        "RAM:".white().bold(),
        ram_bar,
        ram_used_gb,
        ram_total_gb,
        ram_pct_colored
    );

    if memory.swap.total > 0 {
        let swap_usage = memory.swap_usage_percent();
        let swap_bar = create_usage_bar(swap_usage, 20);
        let swap_used_gb = memory.swap.used as f64 / 1024.0 / 1024.0;
        let swap_total_gb = memory.swap.total as f64 / 1024.0 / 1024.0;

        let swap_pct_str = format!("{:.1}%", swap_usage);
        let swap_pct_colored = if swap_usage > 90.0 {
            swap_pct_str.red().bold()
        } else if swap_usage > 75.0 {
            swap_pct_str.yellow()
        } else {
            swap_pct_str.green()
        };

        println!(
            "  {} {} {:.2} / {:.2} GB {}",
            "SWAP:".white().bold(),
            swap_bar,
            swap_used_gb,
            swap_total_gb,
            swap_pct_colored
        );
    }
}

#[cfg(feature = "cli")]
fn print_power_info(power: &simonlib::core::power::PowerStats) {
    println!("{}", "═══ Power Information ═══".cyan().bold());

    // With no rails the total is a fixed zero, and "Total Power: 0.00W" reads as a
    // measurement of an idle machine rather than the absence of any power sensor.
    if power.rails.is_empty() {
        println!("  {}", "No power rails exposed by this platform".yellow());
        return;
    }

    let total = power.total_watts();
    let total_str = format!("{:.2}W", total);
    let total_colored = if total > 100.0 {
        total_str.red().bold()
    } else if total > 50.0 {
        total_str.yellow()
    } else {
        total_str.green()
    };
    println!("  {} {}", "Total Power:".white().bold(), total_colored);

    if !power.rails.is_empty() {
        println!("\n  {}", "Power Rails:".white().bold());
        for (name, rail) in &power.rails {
            if rail.online {
                let power_w = rail.power as f64 / 1000.0;
                let power_str = format!("{:.2}W", power_w);
                let power_colored = if power_w > 30.0 {
                    power_str.yellow()
                } else {
                    power_str.green()
                };
                println!(
                    "    {} {} ({:.1}V, {:.1}mA)",
                    format!("{}:", name).white(),
                    power_colored,
                    rail.voltage as f64 / 1000.0,
                    rail.current as f64
                );
            }
        }
    }
}

#[cfg(feature = "cli")]
fn print_temperature_info(temp: &simonlib::core::temperature::TemperatureStats) {
    println!("{}", "═══ Temperature Information ═══".cyan().bold());

    if temp.sensors.is_empty() {
        println!("  {}", "No temperature sensors detected".yellow());
        return;
    }

    for (name, sensor) in &temp.sensors {
        if sensor.online {
            let temp_str = format!("{:.1}°C", sensor.temp);
            let temp_colored = if sensor.temp > 85.0 {
                temp_str.red().bold()
            } else if sensor.temp > 70.0 {
                temp_str.yellow()
            } else {
                temp_str.green()
            };

            print!("  {} {}", format!("{}:", name).white(), temp_colored);
            if let Some(max) = sensor.max {
                print!(" (max: {})", format!("{:.1}°C", max).red());
            }
            println!();
        }
    }
}

#[cfg(feature = "cli")]
fn run_interactive_mode(mut stats: simonlib::Simon) -> Result<(), Box<dyn std::error::Error>> {
    use crossterm::{
        event::{self, Event, KeyCode},
        terminal::{disable_raw_mode, enable_raw_mode},
    };
    use std::time::Duration;

    println!("Interactive monitoring mode - Press 'q' to quit");
    println!("Updating every {:.1}s\n", stats.interval().as_secs_f64());

    enable_raw_mode()?;

    loop {
        // Check for quit key
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }

        // Clear screen
        print!("\x1B[2J\x1B[1;1H");

        // Get snapshot
        let snapshot = stats.snapshot()?;

        // Print summary
        // Not NVIDIA-specific: this path reports CPU, memory, thermals and any
        // detected GPU regardless of vendor.
        println!("=== Simon - System Monitoring ===");
        println!("Uptime: {:?}\n", snapshot.uptime);

        // GPU
        print_gpu_info(&snapshot.gpus);
        println!();

        // CPU
        print_cpu_info(&snapshot.cpu);
        println!();

        // Memory
        print_memory_info(&snapshot.memory);
        println!();

        // Temperature
        if let Some(max_temp) = snapshot.temperature.max_temp() {
            println!("Max Temperature: {:.1}°C", max_temp);
        }

        // Power. Only report a total when power rails were actually read: with no
        // rails the total is a fixed zero, and "Total Power: 0.00W" reads as a
        // measurement of an idle machine rather than the absence of a sensor.
        if snapshot.power.rails.is_empty() {
            println!("Total Power: not measured (no power rails on this platform)");
        } else {
            println!("Total Power: {:.2}W", snapshot.power.total_watts());
        }

        println!("\nPress 'q' to quit");

        std::thread::sleep(stats.interval());
    }

    disable_raw_mode()?;
    Ok(())
}

// ============================================================================
// BACKEND-BASED PRINT FUNCTIONS (using shared MonitoringBackend)
// ============================================================================

/// Print GPU info using MonitoringBackend
#[cfg(feature = "cli")]
fn print_gpu_info_backend(backend: &simonlib::backend::MonitoringBackend) {
    println!("{}", "═══ GPU Information ═══".cyan().bold());

    let static_info = backend.gpu_static_info();
    let dynamic_info = backend.gpu_dynamic_info();

    if static_info.is_empty() {
        println!("  {}", "No GPUs detected".yellow());
        return;
    }

    for (_idx, (si, di)) in static_info.iter().zip(dynamic_info.iter()).enumerate() {
        println!(
            "\n  {} {} ({})",
            "▶".green(),
            si.name.white().bold(),
            si.vendor.to_string().magenta()
        );

        // Utilization
        let util = di.utilization as f32;
        let util_str = format!("{:.1}%", util);
        let util_colored = if util > 90.0 {
            util_str.red().bold()
        } else if util > 70.0 {
            util_str.yellow()
        } else {
            util_str.green()
        };
        let util_bar = create_usage_bar(util, 15);
        println!("    {} {} {}", "Load:".white(), util_bar, util_colored);

        // Memory
        let mem_used_mb = di.memory.used / 1024 / 1024;
        let mem_total_mb = di.memory.total / 1024 / 1024;
        let mem_pct = if di.memory.total > 0 {
            (di.memory.used as f32 / di.memory.total as f32) * 100.0
        } else {
            0.0
        };
        let mem_bar = create_usage_bar(mem_pct, 15);
        println!(
            "    {} {} {} / {} MB",
            "Memory:".white(),
            mem_bar,
            mem_used_mb.to_string().cyan(),
            mem_total_mb
        );

        // Temperature.
        //
        // `GpuThermal::temperature` is degrees Celsius, not millidegrees. This
        // previously divided by 1000 and compared against 85000/70000, so an
        // RTX 3090 Ti at 80C rendered as "0.1°C" in green and the thermal warning
        // thresholds could never fire at any temperature.
        //
        // The neighbouring power field *is* milliwatts, which is what made the
        // mistake plausible.
        if let Some(temp) = di.thermal.temperature {
            let temp_str = format!("{:.1}°C", temp as f32);
            let temp_colored = if temp > 85 {
                temp_str.red().bold()
            } else if temp > 70 {
                temp_str.yellow()
            } else {
                temp_str.green()
            };
            println!("    {} {}", "Temperature:".white(), temp_colored);
        }

        // Power
        if let Some(power) = di.power.draw {
            let power_w = power as f32 / 1000.0;
            println!(
                "    {} {}",
                "Power:".white(),
                format!("{:.1}W", power_w).yellow()
            );
        }

        // Clocks
        if let Some(graphics) = di.clocks.graphics {
            if graphics > 0 {
                let memory = di.clocks.memory.unwrap_or(0);
                println!(
                    "    {} {} MHz (mem: {} MHz)",
                    "Clocks:".white(),
                    graphics.to_string().cyan(),
                    memory
                );
            }
        }
    }
}

/// Print CPU info using MonitoringBackend
#[cfg(feature = "cli")]
fn print_cpu_info_backend(backend: &simonlib::backend::MonitoringBackend) {
    println!("{}", "═══ CPU Information ═══".cyan().bold());

    if let Some(cpu) = backend.cpu_stats() {
        println!(
            "  {} {} (Online: {})",
            "Cores:".white().bold(),
            cpu.core_count().to_string().green(),
            cpu.online_count().to_string().green()
        );

        let usage = 100.0 - cpu.total.idle;
        let usage_str = format!("{:.1}%", usage);
        let usage_colored = if usage > 90.0 {
            usage_str.red().bold()
        } else if usage > 70.0 {
            usage_str.yellow()
        } else {
            usage_str.green()
        };
        println!(
            "  {} {} ({}: {:.1}%, {}: {:.1}%, {}: {:.1}%)",
            "Usage:".white().bold(),
            usage_colored,
            "user".dimmed(),
            cpu.total.user,
            "system".dimmed(),
            cpu.total.system,
            "idle".dimmed(),
            cpu.total.idle
        );

        println!("\n  {}", "Per-Core Usage:".white().bold());

        let cores_per_row = 4;
        for chunk in cpu.cores.chunks(cores_per_row) {
            print!("    ");
            for core in chunk {
                if core.online {
                    let usage = 100.0 - core.idle.unwrap_or(0.0);
                    let bar = create_usage_bar(usage, 10);
                    let usage_str = format!("{:>5.1}%", usage);
                    let usage_colored = if usage > 90.0 {
                        usage_str.red()
                    } else if usage > 70.0 {
                        usage_str.yellow()
                    } else {
                        usage_str.green()
                    };
                    print!(
                        "{}{:>2}{} {} {} ",
                        "CPU".dimmed(),
                        core.id,
                        ":".dimmed(),
                        bar,
                        usage_colored
                    );
                } else {
                    print!(
                        "{}{:>2}{} {}     ",
                        "CPU".dimmed(),
                        core.id,
                        ":".dimmed(),
                        "OFFLINE".red()
                    );
                }
            }
            println!();
        }
    } else {
        println!("  {}", "No CPU data available".yellow());
    }
}

/// Print memory info using MonitoringBackend
#[cfg(feature = "cli")]
fn print_memory_info_backend(backend: &simonlib::backend::MonitoringBackend) {
    println!("{}", "═══ Memory Information ═══".cyan().bold());

    if let Some(memory) = backend.memory_stats() {
        let ram_usage = memory.ram_usage_percent();
        let ram_bar = create_usage_bar(ram_usage, 20);
        let ram_used_gb = memory.ram.used as f64 / 1024.0 / 1024.0;
        let ram_total_gb = memory.ram.total as f64 / 1024.0 / 1024.0;

        let ram_pct_str = format!("{:.1}%", ram_usage);
        let ram_pct_colored = if ram_usage > 90.0 {
            ram_pct_str.red().bold()
        } else if ram_usage > 75.0 {
            ram_pct_str.yellow()
        } else {
            ram_pct_str.green()
        };

        println!(
            "  {} {} {:.2} / {:.2} GB {}",
            "RAM:".white().bold(),
            ram_bar,
            ram_used_gb,
            ram_total_gb,
            ram_pct_colored
        );

        if memory.swap.total > 0 {
            let swap_usage = memory.swap_usage_percent();
            let swap_bar = create_usage_bar(swap_usage, 20);
            let swap_used_gb = memory.swap.used as f64 / 1024.0 / 1024.0;
            let swap_total_gb = memory.swap.total as f64 / 1024.0 / 1024.0;

            let swap_pct_str = format!("{:.1}%", swap_usage);
            let swap_pct_colored = if swap_usage > 90.0 {
                swap_pct_str.red().bold()
            } else if swap_usage > 75.0 {
                swap_pct_str.yellow()
            } else {
                swap_pct_str.green()
            };

            println!(
                "  {} {} {:.2} / {:.2} GB {}",
                "SWAP:".white().bold(),
                swap_bar,
                swap_used_gb,
                swap_total_gb,
                swap_pct_colored
            );
        }
    } else {
        println!("  {}", "No memory data available".yellow());
    }
}

/// Print process info using MonitoringBackend
#[cfg(feature = "cli")]
fn print_process_info_backend(
    backend: &simonlib::backend::MonitoringBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::ProcessCategory;
    use std::collections::HashMap;

    println!("{}", "═══ Process Information ═══".cyan().bold());

    let processes = backend.processes();

    if processes.is_empty() {
        println!("  {}", "No process data available".yellow());
        return Ok(());
    }

    // Group by category
    let mut by_category: HashMap<ProcessCategory, Vec<&simonlib::ProcessMonitorInfo>> =
        HashMap::new();
    for proc in processes.iter() {
        by_category.entry(proc.category).or_default().push(proc);
    }

    // Print category summary
    println!("\n  {}", "Category Summary:".white().bold());
    println!(
        "  {:<18} {:>8} {:>8} {:>10} {:>12}",
        "CATEGORY".dimmed(),
        "PROCS".dimmed(),
        "GPU".dimmed(),
        "CPU%".dimmed(),
        "MEMORY".dimmed()
    );
    println!("  {}", "─".repeat(60).dimmed());

    let mut categories: Vec<_> = by_category.keys().collect();
    categories.sort_by_key(|c| c.display_name());

    for category in &categories {
        if let Some(procs) = by_category.get(*category) {
            let count = procs.len();
            let gpu_count = procs.iter().filter(|p| p.is_gpu_process()).count();
            let total_cpu: f32 = procs.iter().map(|p| p.cpu_percent).sum();
            let total_mem: u64 = procs.iter().map(|p| p.memory_bytes).sum();
            let mem_mb = total_mem as f64 / 1024.0 / 1024.0;

            let gpu_indicator = if gpu_count > 0 {
                format!("{}", gpu_count).magenta().to_string()
            } else {
                "-".dimmed().to_string()
            };

            let cpu_str = format!("{:.1}", total_cpu);
            let cpu_colored = if total_cpu > 50.0 {
                cpu_str.yellow()
            } else {
                cpu_str.green()
            };

            let mem_str = format!("{:.1}MB", mem_mb);
            let mem_colored = if mem_mb > 1000.0 {
                mem_str.yellow()
            } else {
                mem_str.white()
            };

            println!(
                "  {} {:<15} {:>8} {:>8} {:>10} {:>12}",
                category.icon(),
                category.display_name().white(),
                count.to_string().cyan(),
                gpu_indicator,
                cpu_colored,
                mem_colored,
            );
        }
    }

    // Show top processes in select categories
    println!();
    for category in &[
        ProcessCategory::AiMl,
        ProcessCategory::GpuCompute,
        ProcessCategory::Gaming,
        ProcessCategory::Browser,
        ProcessCategory::Development,
        ProcessCategory::Media,
    ] {
        if let Some(procs) = by_category.get(category) {
            if !procs.is_empty() {
                let mut sorted: Vec<_> = procs.iter().collect();
                sorted.sort_by(|a, b| {
                    b.cpu_percent
                        .partial_cmp(&a.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                println!(
                    "  {} {} {}",
                    category.icon(),
                    category.display_name().white().bold(),
                    "(top 5):".dimmed()
                );
                println!(
                    "    {:<8} {:<30} {:>8} {:>10} {:>10}",
                    "PID".dimmed(),
                    "NAME".dimmed(),
                    "CPU%".dimmed(),
                    "MEM(MB)".dimmed(),
                    "GPU(MB)".dimmed()
                );

                for proc in sorted.iter().take(5) {
                    let gpu_mem = if proc.is_gpu_process() {
                        format!("{:.1}", proc.gpu_memory_mb()).magenta().to_string()
                    } else {
                        "-".dimmed().to_string()
                    };

                    let cpu_str = format!("{:.1}", proc.cpu_percent);
                    let cpu_colored = if proc.cpu_percent > 50.0 {
                        cpu_str.red()
                    } else if proc.cpu_percent > 20.0 {
                        cpu_str.yellow()
                    } else {
                        cpu_str.green()
                    };

                    let name = if proc.name.len() > 30 {
                        &proc.name[..30]
                    } else {
                        &proc.name
                    };

                    println!(
                        "    {:<8} {:<30} {:>8} {:>10.1} {:>10}",
                        proc.pid.to_string().cyan(),
                        name.white(),
                        cpu_colored,
                        proc.memory_mb(),
                        gpu_mem,
                    );
                }
                println!();
            }
        }
    }

    Ok(())
}

/// Print all system info using MonitoringBackend
#[cfg(feature = "cli")]
fn print_all_info(
    backend: &simonlib::backend::MonitoringBackend,
) -> Result<(), Box<dyn std::error::Error>> {
    // System info
    println!("{}", "═══ System Overview ═══".cyan().bold());
    println!(
        "  {} {}",
        "Host:".white().bold(),
        backend.hostname().green()
    );
    println!("  {} {}", "OS:".white().bold(), backend.os_info().green());
    println!("  {} {:?}", "Uptime:".white().bold(), backend.uptime());
    println!();

    // CPU
    print_cpu_info_backend(backend);
    println!();

    // Memory
    print_memory_info_backend(backend);
    println!();

    // GPU
    print_gpu_info_backend(backend);
    println!();

    // Processes summary
    let procs = backend.processes();
    let gpu_procs: Vec<_> = procs.iter().filter(|p| p.is_gpu_process()).collect();
    println!("{}", "═══ Processes ═══".cyan().bold());
    println!(
        "  {} {} ({} using GPU)",
        "Total:".white().bold(),
        procs.len().to_string().cyan(),
        gpu_procs.len().to_string().magenta()
    );

    Ok(())
}

/// Handle all Jetson-specific commands
#[cfg(feature = "cli")]
fn handle_jetson_command(cmd: &JetsonSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    // Platform check - Jetson commands only work on Linux
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cmd; // Suppress unused variable warning
        eprintln!("Error: Jetson commands are only available on Linux");
        eprintln!("       These utilities require NVIDIA Jetson hardware (Xavier, Orin, etc.)");
        std::process::exit(1);
    }

    #[cfg(target_os = "linux")]
    match cmd {
        JetsonSubcommand::Clocks { action } => handle_jetson_clocks(action),
        JetsonSubcommand::Powermode { action } => handle_nvpmodel(action),
        JetsonSubcommand::Swap { action } => handle_swap(action),
    }
}

#[cfg(all(feature = "cli", feature = "jetson-utils", target_os = "linux"))]
fn handle_jetson_clocks(action: &JetsonClocksAction) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::utils::clocks;

    if !clocks::is_available() {
        eprintln!("jetson_clocks is not available on this system");
        std::process::exit(1);
    }

    match action {
        JetsonClocksAction::Enable => {
            println!("Enabling jetson_clocks (maximizing performance)...");
            clocks::enable()?;
            println!("jetson_clocks enabled successfully");
        }
        JetsonClocksAction::Disable => {
            println!("Disabling jetson_clocks (restoring settings)...");
            clocks::disable()?;
            println!("jetson_clocks disabled successfully");
        }
        JetsonClocksAction::Status => {
            let status = clocks::show()?;
            println!("=== Jetson Clocks Status ===");
            println!("Active: {}", if status.active { "YES" } else { "NO" });
            println!("\nConfigured Engines:");
            for engine in &status.engines {
                println!("  - {}", engine);
            }
        }
        JetsonClocksAction::Store => {
            println!("Storing current configuration...");
            clocks::store()?;
            println!("Configuration stored successfully");
        }
    }

    Ok(())
}

#[cfg(all(feature = "cli", feature = "jetson-utils", target_os = "linux"))]
fn handle_nvpmodel(action: &NvpmodelAction) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::utils::power_mode;

    if !power_mode::is_available() {
        eprintln!("nvpmodel is not available on this system");
        std::process::exit(1);
    }

    match action {
        NvpmodelAction::Show => {
            let mode = power_mode::query()?;
            println!("=== Current Power Mode ===");
            println!("ID: {}", mode.id);
            println!("Name: {}", mode.name);
        }
        NvpmodelAction::List => {
            let status = power_mode::list_modes()?;
            println!("=== Available Power Modes ===");
            println!("\nCurrent Mode:");
            println!(
                "  ID: {} - {} {}",
                status.current.id,
                status.current.name,
                if status.current.is_default {
                    "(default)"
                } else {
                    ""
                }
            );

            println!("\nAll Modes:");
            for mode in &status.modes {
                println!(
                    "  ID: {} - {} {}",
                    mode.id,
                    mode.name,
                    if mode.is_default { "(default)" } else { "" }
                );
            }

            println!("\nDefault Mode:");
            println!("  ID: {} - {}", status.default.id, status.default.name);
        }
        NvpmodelAction::Set { mode_id, force } => {
            println!("Setting power mode to ID {}...", mode_id);
            power_mode::set_mode(*mode_id, *force)?;
            println!("Power mode set successfully");

            // Show new mode
            let mode = power_mode::query()?;
            println!("New mode: {} ({})", mode.name, mode.id);
        }
        NvpmodelAction::SetName { name, force } => {
            println!("Setting power mode to '{}'...", name);
            power_mode::set_mode_by_name(name, *force)?;
            println!("Power mode set successfully");

            // Show new mode
            let mode = power_mode::query()?;
            println!("New mode: {} ({})", mode.name, mode.id);
        }
    }

    Ok(())
}

#[cfg(all(feature = "cli", feature = "jetson-utils", target_os = "linux"))]
fn handle_swap(action: &SwapAction) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::utils::swap;

    match action {
        SwapAction::Status => {
            let swaps = swap::status()?;

            if swaps.is_empty() {
                println!("No active swap");
            } else {
                println!("=== Active Swap ===");
                println!(
                    "{:<30} {:<10} {:<10} {:<10} {:<10}",
                    "NAME", "TYPE", "SIZE", "USED", "PRIO"
                );
                println!("{}", "-".repeat(80));

                for swap_info in swaps {
                    println!(
                        "{:<30} {:<10} {:<10} {:<10} {:<10}",
                        swap_info.path,
                        swap_info.swap_type,
                        format_size(swap_info.size_kb),
                        format_size(swap_info.used_kb),
                        swap_info.priority,
                    );
                }
            }
        }
        SwapAction::Create { path, size, auto } => {
            println!("This operation requires sudo privileges");
            swap::create(path, *size, *auto)?;
        }
        SwapAction::Enable { path } => {
            println!("Enabling swap: {}", path.display());
            swap::enable(path)?;
            println!("Swap enabled successfully");
        }
        SwapAction::Disable { path } => {
            println!("Disabling swap: {}", path.display());
            swap::disable(path)?;
            println!("Swap disabled successfully");
        }
        SwapAction::Remove { path } => {
            println!("Removing swap file: {}", path.display());
            println!("This operation requires sudo privileges");
            swap::remove(path)?;
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
fn format_size(kb: u64) -> String {
    if kb < 1024 {
        format!("{}K", kb)
    } else if kb < 1024 * 1024 {
        format!("{:.1}M", kb as f64 / 1024.0)
    } else {
        format!("{:.1}G", kb as f64 / 1024.0 / 1024.0)
    }
}

#[cfg(feature = "cli")]
#[allow(dead_code)]
fn print_process_info(processes: &simonlib::core::process::ProcessStats) {
    println!("=== Process Information ===");
    println!("Total Processes: {}", processes.process_count());
    println!(
        "Total GPU Memory: {:.2} MB\n",
        processes.total_gpu_memory_kb as f64 / 1024.0
    );

    if processes.process_count() > 0 {
        println!(
            "{:<8} {:<12} {:<8} {:<8} {:<8} {:<10} {:<10} {:<20}",
            "PID", "USER", "GPU", "TYPE", "STATE", "CPU%", "GPU MEM", "NAME"
        );
        println!("{}", "-".repeat(100));

        for proc in processes.sorted_by_gpu_memory().iter().take(10) {
            println!(
                "{:<8} {:<12} {:<8} {:<8} {:<8} {:<10.1} {:<10} {:<20}",
                proc.pid,
                &proc.user,
                &proc.gpu,
                &proc.process_type,
                proc.state,
                proc.cpu_percent,
                format_size(proc.gpu_memory_kb),
                &proc.name,
            );
        }
    }
}

#[cfg(feature = "cli")]
#[allow(dead_code)] // Alternative process display format for future use
fn print_process_info_v2(
    monitor: &mut simonlib::ProcessMonitor,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::ProcessCategory;

    println!("{}", "═══ Process Information ═══".cyan().bold());

    // Get category statistics
    let category_stats = monitor.category_stats()?;

    // Print category summary
    println!("\n  {}", "Category Summary:".white().bold());
    println!(
        "  {:<18} {:>8} {:>8} {:>10} {:>12}",
        "CATEGORY".dimmed(),
        "PROCS".dimmed(),
        "GPU".dimmed(),
        "CPU%".dimmed(),
        "MEMORY".dimmed()
    );
    println!("  {}", "─".repeat(60).dimmed());

    for stats in &category_stats {
        if stats.process_count > 0 {
            let gpu_indicator = if stats.gpu_process_count > 0 {
                format!("{}", stats.gpu_process_count).magenta().to_string()
            } else {
                "-".dimmed().to_string()
            };

            let cpu_str = format!("{:.1}", stats.total_cpu_percent);
            let cpu_colored = if stats.total_cpu_percent > 50.0 {
                cpu_str.yellow()
            } else {
                cpu_str.green()
            };

            let mem_str = format!("{:.1}MB", stats.memory_mb());
            let mem_colored = if stats.memory_mb() > 1000.0 {
                mem_str.yellow()
            } else {
                mem_str.white()
            };

            println!(
                "  {} {:<15} {:>8} {:>8} {:>10} {:>12}",
                stats.category.icon(),
                stats.category.display_name().white(),
                stats.process_count.to_string().cyan(),
                gpu_indicator,
                cpu_colored,
                mem_colored,
            );
        }
    }

    println!();

    // Print top CPU consumers by category
    let grouped = monitor.processes_grouped_by_category()?;

    // Show top processes in active categories
    for category in &[
        ProcessCategory::AiMl,
        ProcessCategory::GpuCompute,
        ProcessCategory::Gaming,
        ProcessCategory::Browser,
        ProcessCategory::Development,
        ProcessCategory::Media,
    ] {
        if let Some(procs) = grouped.get(category) {
            if !procs.is_empty() {
                let mut sorted = procs.clone();
                sorted.sort_by(|a, b| {
                    b.cpu_percent
                        .partial_cmp(&a.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                println!(
                    "  {} {} {}",
                    category.icon(),
                    category.display_name().white().bold(),
                    "(top 5):".dimmed()
                );
                println!(
                    "    {:<8} {:<30} {:>8} {:>10} {:>10}",
                    "PID".dimmed(),
                    "NAME".dimmed(),
                    "CPU%".dimmed(),
                    "MEM(MB)".dimmed(),
                    "GPU(MB)".dimmed()
                );

                for proc in sorted.iter().take(5) {
                    let gpu_mem = if proc.is_gpu_process() {
                        format!("{:.1}", proc.gpu_memory_mb()).magenta().to_string()
                    } else {
                        "-".dimmed().to_string()
                    };

                    let cpu_str = format!("{:.1}", proc.cpu_percent);
                    let cpu_colored = if proc.cpu_percent > 50.0 {
                        cpu_str.red()
                    } else if proc.cpu_percent > 20.0 {
                        cpu_str.yellow()
                    } else {
                        cpu_str.green()
                    };

                    let name = if proc.name.len() > 30 {
                        &proc.name[..30]
                    } else {
                        &proc.name
                    };

                    println!(
                        "    {:<8} {:<30} {:>8} {:>10.1} {:>10}",
                        proc.pid.to_string().cyan(),
                        name.white(),
                        cpu_colored,
                        proc.memory_mb(),
                        gpu_mem,
                    );
                }
                println!();
            }
        }
    }

    Ok(())
}

#[cfg(feature = "cli")]
fn print_engine_info(engines: &simonlib::core::engine::EngineStats) {
    println!("{}", "═══ Engine Information ═══".cyan().bold());
    println!(
        "  {} {} (Total: {})",
        "Engine Groups:".white().bold(),
        engines.group_count().to_string().cyan(),
        engines.engine_count().to_string().green()
    );

    if engines.group_count() > 0 {
        for (group_name, group_engines) in &engines.groups {
            println!("\n  {} {}", "▶".green(), group_name.white().bold());

            for (engine_name, info) in group_engines {
                let status = if info.online {
                    "ONLINE".green()
                } else {
                    "OFFLINE".red()
                };
                let freq_info = match (info.min, info.max) {
                    (Some(min), Some(max)) => {
                        format!(
                            "{} ({}-{} MHz)",
                            format!("{} MHz", info.current).cyan(),
                            min,
                            max
                        )
                    }
                    _ => format!("{} MHz", info.current).cyan().to_string(),
                };

                println!("    {:<15} {} {}", engine_name.white(), status, freq_info);
            }
        }
    } else {
        println!("  {}", "No engines detected".yellow());
    }
}

#[cfg(feature = "cli")]
fn handle_ai_query(query: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::agent::{Agent, AgentConfig};
    use simonlib::SiliconMonitor;
    use std::io::{self, Write};

    // Create monitor for system state
    let monitor = SiliconMonitor::new()?;

    // Auto-detect and configure best available backend
    let config = match AgentConfig::auto_detect() {
        Ok(cfg) => {
            // Successfully auto-detected a backend
            if let Some(ref backend) = cfg.backend {
                eprintln!("[*] Using backend: {}", backend.backend_type.display_name());
            }
            cfg
        }
        Err(e) => {
            // No backends available - return error instead of falling back
            eprintln!("[!] No AI backends available: {}", e);
            eprintln!("[!] To use AI features, install Ollama (https://ollama.com) or set an API key (OPENAI_API_KEY, GITHUB_TOKEN, etc.)");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No AI backend configured",
            )));
        }
    }
    .with_caching(true)
    .with_cache_size(50)
    .with_timeout(Duration::from_secs(30)); // Longer timeout for remote backends

    let mut agent = Agent::new(config)?;

    if let Some(question) = query {
        // Single query mode
        println!("[AI Monitor]");
        println!("Question: {}\n", question);

        let response = agent.ask(question, &monitor)?;
        println!("{}", response.response);

        if response.from_cache {
            println!("\n[CACHE] (from cache, <1ms)");
        } else {
            println!("\n[TIME] ({}ms)", response.inference_time_ms);
        }
    } else {
        // Interactive mode
        println!("[AI Monitor - Interactive Mode]");
        println!("Ask questions about your system state. Type 'quit' or 'exit' to leave.\n");

        loop {
            print!("You: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
                println!("Goodbye!");
                break;
            }

            match agent.ask(input, &monitor) {
                Ok(response) => {
                    println!("\n[Agent]: {}\n", response.response);
                    if response.from_cache {
                        println!("[CACHE] (from cache, <1ms)\n");
                    } else {
                        println!("[TIME] ({}ms)\n", response.inference_time_ms);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}\n", e);
                }
            }
        }
    }

    Ok(())
}

/// Run the monitoring daemon from a TOML configuration file.
#[cfg(feature = "cli")]
fn handle_daemon_command(
    config_path: Option<&str>,
    sample_config: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::daemon::{DaemonConfig, MonitoringDaemon};

    if sample_config {
        print!("{}", DaemonConfig::sample_toml());
        return Ok(());
    }

    let daemon = match config_path {
        Some(path) => MonitoringDaemon::from_config_file(path)?,
        None => MonitoringDaemon::new(DaemonConfig::default()),
    };

    // Fail before announcing settings we are about to reject.
    daemon.validate()?;

    let config = daemon.config();

    println!("{}", "═══ Simon Monitoring Daemon ═══".cyan().bold());
    println!(
        "  {} {}",
        "Listening:".white().bold(),
        daemon.listen_address().cyan()
    );
    println!(
        "  {} {}s",
        "Poll interval:".white().bold(),
        config.poll_interval_secs
    );
    println!(
        "  {} {}",
        "Authentication:".white().bold(),
        if config.api_key.is_some() {
            "api key required".green()
        } else {
            "anonymous (loopback only)".yellow()
        }
    );
    if let Some(ref pid) = config.pid_file {
        println!("  {} {}", "PID file:".white().bold(), pid);
    }
    println!();
    println!("{}", "Press Ctrl+C to stop.".yellow().italic());

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { daemon.run().await })?;

    Ok(())
}

/// Start the HTTP API and Prometheus metrics server.
///
/// `src/http_server.rs`, `src/observability/server.rs` and `src/prometheus.rs` —
/// about 1300 lines — shipped with no entry point in any binary or example, so the
/// REST API and Prometheus exporter could not be started at all. `grafana/README.md`
/// documented `simon --serve --port 9100` and `simon daemon --config simon.toml`;
/// neither command existed, so anyone following it got "unrecognized subcommand".
#[cfg(feature = "cli")]
fn handle_serve_command(
    port: u16,
    bind: &str,
    log_requests: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::http_server::{HttpServer, HttpServerConfig};

    let config = HttpServerConfig {
        bind_address: bind.to_string(),
        port,
        request_logging: log_requests,
        ..Default::default()
    };

    let server = HttpServer::new(config)?;

    println!("{}", "═══ Simon HTTP Server ═══".cyan().bold());
    println!("  {} http://{}:{}", "Listening:".white().bold(), bind, port);
    println!(
        "  {} http://{}:{}/api/v1/metrics/prometheus",
        "Prometheus:".white().bold(),
        bind,
        port
    );
    println!(
        "  {} http://{}:{}/api/v1/context",
        "Context:".white().bold(),
        bind,
        port
    );
    println!(
        "  {} http://{}:{}/health",
        "Health:".white().bold(),
        bind,
        port
    );

    if bind == "0.0.0.0" {
        println!(
            "  {} bound to all interfaces — full hardware telemetry is reachable \
             from the network",
            "Warning:".yellow().bold()
        );
    }

    println!();
    println!("{}", "Press Ctrl+C to stop.".yellow().italic());

    // `HttpServer::run` is async; main is not, so drive it on a runtime here rather
    // than making every command pay for one.
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async { server.run().await })?;

    Ok(())
}

/// Handle record subcommands for time-series database operations
#[cfg(feature = "cli")]
fn handle_record_command(action: &RecordSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::tsdb::{
        format_size, parse_size, MetricsRecorder, ProcessSnapshot, SystemSnapshot, TimeSeriesDb,
    };
    use std::io::{self, Write};

    match action {
        RecordSubcommand::Start {
            database,
            max_size,
            interval,
            max_processes,
            format,
        } => {
            let max_bytes = parse_size(max_size)?;
            let interval_duration = Duration::from_secs_f64(*interval);

            println!("{}", "═══ Starting Metrics Recording ═══".cyan().bold());
            println!(
                "  {} {}",
                "Database:".white().bold(),
                database.display().to_string().green()
            );
            println!(
                "  {} {}",
                "Max Size:".white().bold(),
                format_size(max_bytes).yellow()
            );
            println!(
                "  {} {}s",
                "Interval:".white().bold(),
                interval.to_string().cyan()
            );
            println!(
                "  {} {}",
                "Max Processes:".white().bold(),
                max_processes.to_string().cyan()
            );
            println!();
            println!("{}", "Press Ctrl+C to stop recording...".yellow().italic());
            println!();

            // Create recorder
            let mut recorder =
                MetricsRecorder::new(database, max_bytes, interval_duration, *max_processes)?;

            // Collect on a background thread rather than inline.
            //
            // Inline serial collection cost roughly 1.2s, and the loop slept the
            // requested interval *on top* of that — so `record --interval 1` actually
            // sampled every ~2.2s. Recorded timestamps therefore never matched the
            // requested cadence, and any rate derived from the database was wrong by
            // the ratio between them.
            //
            // The collector now runs at its own cadence and the loop only reads the
            // newest published snapshot, which is an atomic load.
            let collector =
                simonlib::pipeline::Collector::spawn(simonlib::pipeline::CollectorConfig {
                    interval: interval_duration,
                    collect_processes: *max_processes > 0,
                    ..Default::default()
                });
            let snapshots = collector.handle();

            // Wait for the first fully-populated snapshot so the database does not
            // open with a row whose GPU and process columns are empty purely because
            // driver enumeration had not finished yet.
            let warmup_deadline = std::time::Instant::now() + Duration::from_secs(30);
            while snapshots.latest().gpu_static.is_empty()
                && snapshots.latest().processes.is_empty()
                && std::time::Instant::now() < warmup_deadline
            {
                std::thread::sleep(Duration::from_millis(20));
            }

            // Install Ctrl+C handler
            let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let r = running.clone();
            ctrlc::set_handler(move || {
                r.store(false, std::sync::atomic::Ordering::SeqCst);
            })
            .expect("Error setting Ctrl-C handler");

            let mut record_count = 0u64;
            let start_time = std::time::Instant::now();

            while running.load(std::sync::atomic::Ordering::SeqCst) {
                let iteration_start = std::time::Instant::now();

                // Read the newest published snapshot. Lock-free; never blocks on a
                // driver, so a slow GPU query cannot stretch the sampling interval.
                let state = snapshots.latest();

                // Create system snapshot
                let timestamp = simonlib::tsdb::TimeSeriesDb::now_millis();

                let cpu_per_core: Vec<f32> = state
                    .cpu
                    .as_ref()
                    .map(|c| {
                        c.cores
                            .iter()
                            .map(|core| 100.0 - core.idle.unwrap_or(100.0))
                            .collect()
                    })
                    .unwrap_or_default();

                let cpu_percent = state.cpu_utilization();

                // Platform collectors report memory in KB; the database stores bytes.
                let (memory_used, memory_total, swap_used, swap_total) = state
                    .memory
                    .as_ref()
                    .map(|m| {
                        (
                            m.ram.used * 1024,
                            m.ram.total * 1024,
                            m.swap.used * 1024,
                            m.swap.total * 1024,
                        )
                    })
                    .unwrap_or((0, 0, 0, 0));

                // Collect GPU stats. `gpu_dynamic` is index-aligned with the static
                // descriptors and carries None for a device whose query failed this
                // tick; those keep their column rather than shifting their neighbours.
                let mut gpu_percent = Vec::new();
                let mut gpu_memory_used = Vec::new();
                let mut gpu_temperature = Vec::new();
                let mut gpu_power_mw = Vec::new();

                for gpu in &state.gpu_dynamic {
                    match gpu {
                        Some(g) => {
                            gpu_percent.push(g.utilization as f32);
                            gpu_memory_used.push(g.memory.used);
                            gpu_temperature.push(g.thermal.temperature.unwrap_or(0) as f32);
                            gpu_power_mw.push(g.power.draw.unwrap_or(0));
                        }
                        None => {
                            gpu_percent.push(0.0);
                            gpu_memory_used.push(0);
                            gpu_temperature.push(0.0);
                            gpu_power_mw.push(0);
                        }
                    }
                }

                // Record the heaviest processes by CPU.
                let mut ranked: Vec<_> = state.processes.iter().collect();
                ranked.sort_by(|a, b| {
                    b.cpu_percent
                        .partial_cmp(&a.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let processes: Vec<ProcessSnapshot> = ranked
                    .iter()
                    .take(*max_processes)
                    .map(|p| ProcessSnapshot {
                        pid: p.pid,
                        name: p.name.clone(),
                        cpu_percent: p.cpu_percent,
                        memory_bytes: p.memory_bytes,
                        gpu_memory_bytes: p.total_gpu_memory_bytes,
                        gpu_percent: p.gpu_usage_percent.unwrap_or(0.0),
                        disk_read_bps: 0, // Per-process I/O rates need delta tracking across snapshots
                        disk_write_bps: 0, // Absolute I/O bytes available in ProcessMonitorInfo
                        net_rx_bps: 0,    // Per-process network rates not tracked by OS
                        net_tx_bps: 0,    // System-level net rates are in SystemSnapshot
                    })
                    .collect();

                let net_rx_bps = state.total_rx_rate() as u64;
                let net_tx_bps = state.total_tx_rate() as u64;

                let snapshot = SystemSnapshot {
                    timestamp,
                    cpu_percent,
                    cpu_per_core,
                    memory_used,
                    memory_total,
                    swap_used,
                    swap_total,
                    gpu_percent,
                    gpu_memory_used,
                    gpu_temperature,
                    gpu_power_mw,
                    net_rx_bps,
                    net_tx_bps,
                    processes,
                };

                // Record snapshot
                recorder.record_snapshot(snapshot)?;
                record_count += 1;

                // Print status
                if *format == "text" {
                    let elapsed = start_time.elapsed().as_secs();
                    let stats = recorder.stats();
                    print!(
                        "\r{} {} records | {} | {:.1}% full | {} elapsed",
                        "Recording:".green(),
                        record_count.to_string().cyan(),
                        format_size(stats.current_size).yellow(),
                        stats.usage_percent(),
                        format_duration(elapsed)
                    );
                    io::stdout().flush()?;
                } else {
                    // JSON format - print each record
                    println!(
                        "{{\"record\":{},\"size\":{},\"timestamp\":{}}}",
                        record_count,
                        recorder.stats().current_size,
                        timestamp
                    );
                }

                // Sleep only the remainder of the interval. Sleeping the full
                // interval regardless of how long the iteration took is what made the
                // effective sample period drift past the requested one.
                let spent = iteration_start.elapsed();
                std::thread::sleep(interval_duration.saturating_sub(spent));
            }

            println!();
            println!();
            println!("{}", "Recording stopped.".yellow());
            recorder.close()?;

            let stats = recorder.stats();
            println!("{}", "═══ Recording Summary ═══".cyan().bold());
            println!(
                "  {} {}",
                "Total Records:".white().bold(),
                record_count.to_string().green()
            );
            println!(
                "  {} {}",
                "Database Size:".white().bold(),
                format_size(stats.current_size).yellow()
            );
            if let Some(span) = stats.time_span() {
                println!("  {} {}", "Time Span:".white().bold(), span.cyan());
            }
        }

        RecordSubcommand::Info { database } => {
            if !database.exists() {
                println!(
                    "{} Database file not found: {}",
                    "Error:".red().bold(),
                    database.display()
                );
                return Ok(());
            }

            let db = TimeSeriesDb::new(database, 0)?;
            let stats = db.stats();

            println!("{}", "═══ Database Information ═══".cyan().bold());
            println!(
                "  {} {}",
                "Path:".white().bold(),
                stats.path.display().to_string().green()
            );
            println!(
                "  {} {}",
                "Max Size:".white().bold(),
                format_size(stats.max_size).yellow()
            );
            println!(
                "  {} {} ({:.1}%)",
                "Current Size:".white().bold(),
                format_size(stats.current_size).cyan(),
                stats.usage_percent()
            );
            println!(
                "  {} {}",
                "Record Count:".white().bold(),
                stats.record_count.to_string().green()
            );

            if let Some(first) = stats.first_timestamp {
                let dt = chrono::DateTime::from_timestamp_millis(first as i64)
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                println!("  {} {}", "First Record:".white().bold(), dt.cyan());
            }
            if let Some(last) = stats.last_timestamp {
                let dt = chrono::DateTime::from_timestamp_millis(last as i64)
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                println!("  {} {}", "Last Record:".white().bold(), dt.cyan());
            }
            if let Some(span) = stats.time_span() {
                println!("  {} {}", "Time Span:".white().bold(), span.magenta());
            }

            // Print usage bar
            let bar_width = 40;
            let filled = (stats.usage_percent() / 100.0 * bar_width as f32) as usize;
            let empty = bar_width - filled;
            let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
            let bar_colored = if stats.usage_percent() > 90.0 {
                bar.red()
            } else if stats.usage_percent() > 70.0 {
                bar.yellow()
            } else {
                bar.green()
            };
            println!("  {} [{}]", "Usage:".white().bold(), bar_colored);
        }

        RecordSubcommand::Query {
            database,
            start,
            end,
            format,
            limit,
        } => {
            if !database.exists() {
                println!(
                    "{} Database file not found: {}",
                    "Error:".red().bold(),
                    database.display()
                );
                return Ok(());
            }

            let mut db = TimeSeriesDb::new(database, 0)?;

            // Parse time range
            let now = TimeSeriesDb::now_millis();
            let start_time = if let Some(s) = start {
                parse_time_spec(s, now)?
            } else {
                0 // From beginning
            };
            let end_time = if let Some(e) = end {
                parse_time_spec(e, now)?
            } else {
                now // Until now
            };

            let mut snapshots = db.query_range(start_time, end_time)?;

            // Apply limit
            if let Some(l) = limit {
                snapshots.truncate(*l);
            }

            if *format == "json" {
                println!("{}", serde_json::to_string_pretty(&snapshots)?);
            } else {
                println!("{}", "═══ Query Results ═══".cyan().bold());
                println!(
                    "  {} {} records",
                    "Found:".white().bold(),
                    snapshots.len().to_string().green()
                );
                println!();

                for snapshot in snapshots.iter().take(20) {
                    let dt = chrono::DateTime::from_timestamp_millis(snapshot.timestamp as i64)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());

                    println!("  {} {}", "Time:".white().bold(), dt.cyan());
                    println!(
                        "    CPU: {:.1}%  MEM: {:.1}%",
                        snapshot.cpu_percent,
                        (snapshot.memory_used as f64 / snapshot.memory_total as f64) * 100.0
                    );
                    if !snapshot.gpu_percent.is_empty() {
                        for (i, gpu) in snapshot.gpu_percent.iter().enumerate() {
                            println!(
                                "    GPU{}: {:.1}%  Temp: {:.0}°C",
                                i,
                                gpu,
                                snapshot.gpu_temperature.get(i).unwrap_or(&0.0)
                            );
                        }
                    }
                    println!();
                }

                if snapshots.len() > 20 {
                    println!("  {} (showing 20 of {})", "...".dimmed(), snapshots.len());
                }
            }
        }

        RecordSubcommand::Export {
            database,
            output,
            format,
        } => {
            if !database.exists() {
                println!(
                    "{} Database file not found: {}",
                    "Error:".red().bold(),
                    database.display()
                );
                return Ok(());
            }

            let mut db = TimeSeriesDb::new(database, 0)?;
            let snapshots = db.read_all_system_snapshots()?;

            println!("{}", "═══ Exporting Data ═══".cyan().bold());
            println!(
                "  {} {} records",
                "Records:".white().bold(),
                snapshots.len().to_string().green()
            );
            println!(
                "  {} {}",
                "Output:".white().bold(),
                output.display().to_string().yellow()
            );

            if *format == "csv" {
                // Export as CSV
                let mut file = std::fs::File::create(output)?;
                writeln!(
                    file,
                    "timestamp,cpu_percent,memory_used,memory_total,swap_used,swap_total,net_rx_bps,net_tx_bps"
                )?;
                for s in &snapshots {
                    writeln!(
                        file,
                        "{},{:.2},{},{},{},{},{},{}",
                        s.timestamp,
                        s.cpu_percent,
                        s.memory_used,
                        s.memory_total,
                        s.swap_used,
                        s.swap_total,
                        s.net_rx_bps,
                        s.net_tx_bps
                    )?;
                }
            } else {
                // Export as JSON
                let file = std::fs::File::create(output)?;
                serde_json::to_writer_pretty(file, &snapshots)?;
            }

            println!(
                "  {} {}",
                "Status:".white().bold(),
                "Export complete!".green()
            );
        }

        RecordSubcommand::Clear { database, force } => {
            if !database.exists() {
                println!(
                    "{} Database file not found: {}",
                    "Info:".yellow().bold(),
                    database.display()
                );
                return Ok(());
            }

            if !force {
                print!(
                    "{} Delete database {}? [y/N]: ",
                    "Confirm:".yellow().bold(),
                    database.display()
                );
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("{}", "Cancelled.".yellow());
                    return Ok(());
                }
            }

            std::fs::remove_file(database)?;
            println!(
                "{} Database deleted: {}",
                "Success:".green().bold(),
                database.display()
            );
        }
    }

    Ok(())
}

/// Handle privacy subcommands for managing data collection consent
#[cfg(feature = "cli")]
fn handle_privacy_command(action: &PrivacySubcommand) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::consent::{ConsentManager, ConsentScope};

    match action {
        PrivacySubcommand::Status => {
            let manager = ConsentManager::load()?;

            println!(
                "{}",
                "╔════════════════════════════════════════════════════════════════╗".cyan()
            );
            println!(
                "{}",
                "║           Silicon Monitor - Privacy Status                    ║".cyan()
            );
            println!(
                "{}",
                "╚════════════════════════════════════════════════════════════════╝".cyan()
            );
            println!();

            // Check runtime flags
            let no_telemetry = std::env::var("SIMON_NO_TELEMETRY").is_ok();
            let offline = std::env::var("SIMON_OFFLINE").is_ok();

            if no_telemetry || offline {
                println!("{}", "Session Flags:".white().bold());
                if offline {
                    println!("  {} Offline mode active (--offline)", "⦿".yellow());
                } else if no_telemetry {
                    println!(
                        "  {} Telemetry disabled for this session (--no-telemetry)",
                        "⦿".yellow()
                    );
                }
                println!();
            }

            println!("{}", "Consent Status:".white().bold());
            println!("─────────────────────────────────────────────────────────────────");

            for scope in [
                ConsentScope::BasicTelemetry,
                ConsentScope::HardwareInfo,
                ConsentScope::PerformanceMetrics,
                ConsentScope::DetailedDiagnostics,
                ConsentScope::Analytics,
            ] {
                let status = if manager.has_consent(scope) {
                    format!("{} GRANTED", "●".green())
                } else {
                    format!("{} DENIED", "○".red())
                };
                println!("  {:<30} {}", scope.name(), status);
            }

            println!();
            println!("{}", "Commands:".white().bold());
            println!("  simon privacy opt-out    # Disable all data collection");
            println!("  simon privacy opt-in     # Enable all data collection");
            println!("  simon privacy review     # Review settings interactively");
            println!("  simon privacy info       # Show what data each category collects");
            println!();
            println!("{}", "Session Flags:".white().bold());
            println!("  simon --no-telemetry     # Disable telemetry for one session");
            println!("  simon --offline          # Run completely offline");
        }

        PrivacySubcommand::OptOut => {
            let mut manager = ConsentManager::load()?;
            manager.revoke_all()?;

            println!(
                "{}",
                "╔════════════════════════════════════════════════════════════════╗".green()
            );
            println!(
                "{}",
                "║           Data Collection Disabled                            ║".green()
            );
            println!(
                "{}",
                "╚════════════════════════════════════════════════════════════════╝".green()
            );
            println!();
            println!(
                "All data collection has been disabled. Your privacy choices have been saved."
            );
            println!();
            println!("You can re-enable data collection at any time with:");
            println!("  simon privacy opt-in");
            println!("  simon privacy review");
        }

        PrivacySubcommand::OptIn => {
            let mut manager = ConsentManager::load()?;

            println!(
                "{}",
                "╔════════════════════════════════════════════════════════════════╗".cyan()
            );
            println!(
                "{}",
                "║           Enable Data Collection                              ║".cyan()
            );
            println!(
                "{}",
                "╚════════════════════════════════════════════════════════════════╝".cyan()
            );
            println!();
            println!("Enabling all data collection categories...");
            println!();

            for scope in [
                ConsentScope::BasicTelemetry,
                ConsentScope::HardwareInfo,
                ConsentScope::PerformanceMetrics,
                ConsentScope::DetailedDiagnostics,
                ConsentScope::Analytics,
            ] {
                manager.record_consent(scope, true)?;
                println!("  {} {} enabled", "✓".green(), scope.name());
            }

            println!();
            println!(
                "{}",
                "All data collection categories have been enabled.".green()
            );
            println!();
            println!("You can review or change these settings at any time with:");
            println!("  simon privacy status");
            println!("  simon privacy review");
        }

        PrivacySubcommand::Review => {
            let mut manager = ConsentManager::load()?;
            manager.request_all_consents()?;
        }

        PrivacySubcommand::Info => {
            println!(
                "{}",
                "╔════════════════════════════════════════════════════════════════╗".cyan()
            );
            println!(
                "{}",
                "║           Data Collection Categories                          ║".cyan()
            );
            println!(
                "{}",
                "╚════════════════════════════════════════════════════════════════╝".cyan()
            );
            println!();

            for scope in [
                ConsentScope::BasicTelemetry,
                ConsentScope::HardwareInfo,
                ConsentScope::PerformanceMetrics,
                ConsentScope::DetailedDiagnostics,
                ConsentScope::Analytics,
            ] {
                println!("{}", format!("═══ {} ═══", scope.name()).yellow().bold());
                println!();
                println!("{}", "Description:".white().bold());
                println!("  {}", scope.description());
                println!();
                println!("{}", "Data collected:".white().bold());
                for point in scope.data_points() {
                    println!("  • {}", point);
                }
                println!();
            }
        }
    }

    Ok(())
}

/// Parse a time specification (e.g., "30m", "2h", Unix timestamp)
#[cfg(feature = "cli")]
fn parse_time_spec(spec: &str, now: u64) -> Result<u64, Box<dyn std::error::Error>> {
    let spec = spec.trim();

    // Check if it's a relative time (e.g., "30m", "2h", "1d")
    if spec.ends_with('m') || spec.ends_with('M') {
        let minutes: u64 = spec[..spec.len() - 1].parse()?;
        return Ok(now.saturating_sub(minutes * 60 * 1000));
    }
    if spec.ends_with('h') || spec.ends_with('H') {
        let hours: u64 = spec[..spec.len() - 1].parse()?;
        return Ok(now.saturating_sub(hours * 60 * 60 * 1000));
    }
    if spec.ends_with('d') || spec.ends_with('D') {
        let days: u64 = spec[..spec.len() - 1].parse()?;
        return Ok(now.saturating_sub(days * 24 * 60 * 60 * 1000));
    }

    // Otherwise, treat as Unix timestamp (seconds or milliseconds)
    let ts: u64 = spec.parse()?;
    if ts < 10_000_000_000 {
        // Seconds
        Ok(ts * 1000)
    } else {
        // Milliseconds
        Ok(ts)
    }
}

/// Format duration in human-readable format
#[cfg(feature = "cli")]
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Handle `simon profile ...` — hardware profile inspector
#[cfg(feature = "cli")]
fn handle_profile_command(action: &ProfileSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::profile::{ProfileInspector, Subsystem};
    let mut inspector = ProfileInspector::new();

    match action {
        ProfileSubcommand::List { json } => {
            let snapshot = inspector.snapshot_all();
            if *json {
                let summary: Vec<_> = Subsystem::ALL
                    .iter()
                    .map(|s| {
                        let groups = snapshot.providers.get(s).cloned().unwrap_or_default();
                        serde_json::json!({
                            "subsystem": s.as_str(),
                            "group_count": groups.len(),
                            "setting_count": groups.iter().map(|g| g.settings.len()).sum::<usize>(),
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!("{}", "Hardware Profile Inspector".bold().cyan());
                println!("Subsystems with readable profile data:\n");
                for sub in Subsystem::ALL {
                    let groups = snapshot.providers.get(sub);
                    let (group_count, setting_count) = match groups {
                        Some(g) => (g.len(), g.iter().map(|x| x.settings.len()).sum::<usize>()),
                        None => (0, 0),
                    };
                    println!(
                        "  {:<10} {} groups, {} settings",
                        sub.as_str().bold(),
                        group_count,
                        setting_count
                    );
                }
                println!();
                println!(
                    "Use {} for details.",
                    "simon profile show <subsystem>".italic()
                );
            }
        }
        ProfileSubcommand::Show { subsystem, json } => {
            let sub = Subsystem::parse(subsystem).ok_or_else(|| {
                format!(
                    "Unknown subsystem: {} (valid: gpu, cpu, nvme, display, memory)",
                    subsystem
                )
            })?;
            let groups = inspector.snapshot(sub);
            if *json {
                println!("{}", serde_json::to_string_pretty(&groups)?);
            } else {
                print_profile_groups(sub, &groups);
            }
        }
        ProfileSubcommand::Search { query, json } => {
            let snapshot = inspector.snapshot_all();
            let hits = snapshot.search(query);
            if *json {
                let out: Vec<_> = hits
                    .iter()
                    .map(|(sub, group, setting)| {
                        serde_json::json!({
                            "subsystem": sub.as_str(),
                            "device": &group.device,
                            "profile": &group.display_name,
                            "setting": setting,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "{} matches for {:?}:\n",
                    hits.len().to_string().bold(),
                    query
                );
                for (sub, group, setting) in hits {
                    println!(
                        "  [{}] {} / {}",
                        sub.as_str().cyan(),
                        group.device,
                        group.display_name.dimmed()
                    );
                    println!(
                        "      {} = {}{}",
                        setting.display_name.bold(),
                        setting.value,
                        setting
                            .unit
                            .as_deref()
                            .map(|u| format!(" {}", u))
                            .unwrap_or_default()
                    );
                }
            }
        }
        ProfileSubcommand::Set {
            setting_id,
            value,
            confirm,
            json,
        } => {
            let parsed = parse_setting_value(value);
            let outcome = simonlib::profile::apply::apply_setting(setting_id, parsed, *confirm);
            if *json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                let status_color = match outcome.status {
                    simonlib::profile::apply::ApplyStatus::Applied => "applied".green(),
                    simonlib::profile::apply::ApplyStatus::Refused => "refused".red(),
                    simonlib::profile::apply::ApplyStatus::Failed => "failed".red(),
                    simonlib::profile::apply::ApplyStatus::NotWritable => "not writable".yellow(),
                    simonlib::profile::apply::ApplyStatus::NeedsConfirm => {
                        "needs --confirm".yellow()
                    }
                };
                println!("status:    {}", status_color);
                println!("setting:   {}", outcome.setting_id.bold());
                println!("requested: {}", outcome.requested);
                println!("message:   {}", outcome.message);
                println!(
                    "audit log: {}",
                    simonlib::profile::apply::audit_log_path().display()
                );
            }
            // Non-zero exit on non-success so scripts can detect failure.
            if !matches!(
                outcome.status,
                simonlib::profile::apply::ApplyStatus::Applied
            ) {
                std::process::exit(1);
            }
        }
        ProfileSubcommand::Schemes { json } => {
            #[cfg(windows)]
            {
                let schemes = simonlib::profile::cpu::enumerate_power_schemes();
                if *json {
                    let arr: Vec<_> = schemes
                        .iter()
                        .map(|(g, n)| serde_json::json!({"guid": g, "name": n}))
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&arr)?);
                } else if schemes.is_empty() {
                    println!("(no power schemes enumerable)");
                } else {
                    println!(
                        "{} available Windows power scheme(s):\n",
                        schemes.len().to_string().bold()
                    );
                    for (guid, name) in schemes {
                        println!("  {}  {}", guid.cyan(), name.bold());
                    }
                    println!(
                        "\nApply with: {} {} <guid> --confirm",
                        "simon profile set".italic(),
                        "active_scheme_guid".italic()
                    );
                }
            }
            #[cfg(not(windows))]
            {
                let _ = json;
                eprintln!("Power scheme enumeration is only available on Windows.");
                std::process::exit(2);
            }
        }
        ProfileSubcommand::Writable { json } => {
            let ids = simonlib::profile::apply::writable_setting_ids();
            if *json {
                println!("{}", serde_json::to_string_pretty(&ids)?);
            } else if ids.is_empty() {
                println!("(no writable handlers registered for this build)");
            } else {
                println!("{} writable setting id(s):", ids.len().to_string().bold());
                for id in ids {
                    println!("  · {}", id);
                }
                println!(
                    "\nUse: {} {} <value> --confirm",
                    "simon profile set".italic(),
                    "<id>".italic()
                );
            }
        }
        ProfileSubcommand::Bench { json } => {
            let report = simonlib::profile::bench::run_bench();
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Provider benchmark — total {:.1} ms, {} group(s), {} setting(s)\n",
                    report.total_elapsed_ms, report.total_groups, report.total_settings
                );
                println!(
                    "  {:<10} {:>10} {:>8} {:>10} {:>14}",
                    "subsystem".bold(),
                    "elapsed".bold(),
                    "groups".bold(),
                    "settings".bold(),
                    "settings/ms".bold()
                );
                let mut providers = report.providers.clone();
                providers.sort_by(|a, b| {
                    b.elapsed_ms
                        .partial_cmp(&a.elapsed_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for p in providers {
                    println!(
                        "  {:<10} {:>8.2} ms {:>8} {:>10} {:>14.2}",
                        p.subsystem.as_str(),
                        p.elapsed_ms,
                        p.group_count,
                        p.setting_count,
                        p.settings_per_ms
                    );
                }
            }
        }
        ProfileSubcommand::Watch {
            interval,
            ticks,
            subsystem,
        } => {
            use simonlib::profile::diff::diff_snapshots;
            use simonlib::profile::{ProfileSnapshot, Subsystem};
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            let interval_ms = (interval * 1000.0).max(100.0) as u64;
            let running = Arc::new(AtomicBool::new(true));
            let r2 = running.clone();
            let _ = ctrlc::set_handler(move || {
                r2.store(false, Ordering::SeqCst);
            });

            let filter_sub: Option<Subsystem> = match subsystem {
                Some(s) => match Subsystem::parse(s) {
                    Some(parsed) => Some(parsed),
                    None => {
                        eprintln!(
                            "Unknown subsystem {:?}. Valid: gpu, cpu, nvme, display, memory",
                            s
                        );
                        std::process::exit(2);
                    }
                },
                None => None,
            };

            println!(
                "Watching {} subsystem(s) every {}s. Press Ctrl-C to stop.",
                filter_sub
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_else(|| "all".into()),
                interval
            );

            let snapshot =
                |inspector: &mut simonlib::profile::ProfileInspector| -> ProfileSnapshot {
                    match filter_sub {
                        Some(sub) => {
                            let groups = inspector.snapshot(sub);
                            let mut providers = std::collections::BTreeMap::new();
                            providers.insert(sub, groups);
                            ProfileSnapshot {
                                timestamp: 0,
                                providers,
                                errors: Default::default(),
                            }
                        }
                        None => inspector.snapshot_all(),
                    }
                };

            let mut prev = snapshot(&mut inspector);
            let mut tick = 0u32;
            while running.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(interval_ms));
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                let cur = snapshot(&mut inspector);
                let d = diff_snapshots(&prev, &cur);
                if !d.is_empty() {
                    let when = chrono::Local::now().format("%H:%M:%S");
                    println!(
                        "\n[{}] {} change(s):",
                        when.to_string().dimmed(),
                        d.total_changes().to_string().bold()
                    );
                    for c in &d.changed {
                        println!(
                            "  ~ [{}] {} :: {}: {} → {}",
                            c.subsystem.as_str().cyan(),
                            c.device,
                            c.display_name,
                            c.before.to_string().red(),
                            c.after.to_string().green()
                        );
                    }
                    for a in &d.added {
                        println!(
                            "  + [{}] {} :: {} = {}",
                            a.subsystem.as_str().cyan(),
                            a.device,
                            a.display_name,
                            a.value.to_string().green()
                        );
                    }
                    for r in &d.removed {
                        println!(
                            "  - [{}] {} :: {} (was {})",
                            r.subsystem.as_str().cyan(),
                            r.device,
                            r.display_name,
                            r.value.to_string().red()
                        );
                    }
                }
                prev = cur;
                tick = tick.saturating_add(1);
                if *ticks > 0 && tick >= *ticks {
                    break;
                }
            }
            println!("\nStopped after {} tick(s).", tick);
        }
        ProfileSubcommand::Audit { lines, json } => {
            let path = simonlib::profile::apply::audit_log_path();
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let all: Vec<&str> = content.lines().collect();
            let tail = if all.len() > *lines {
                &all[all.len() - lines..]
            } else {
                &all[..]
            };
            if *json {
                let parsed: Vec<serde_json::Value> = tail
                    .iter()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect();
                println!("{}", serde_json::to_string_pretty(&parsed)?);
            } else {
                println!("Audit log: {}", path.display().to_string().italic());
                if tail.is_empty() {
                    println!("(no entries)");
                }
                for line in tail {
                    if let Ok(v) =
                        serde_json::from_str::<simonlib::profile::apply::ApplyOutcome>(line)
                    {
                        let st = match v.status {
                            simonlib::profile::apply::ApplyStatus::Applied => "applied".green(),
                            simonlib::profile::apply::ApplyStatus::Refused => "refused".red(),
                            simonlib::profile::apply::ApplyStatus::Failed => "failed".red(),
                            simonlib::profile::apply::ApplyStatus::NotWritable => {
                                "not writable".yellow()
                            }
                            simonlib::profile::apply::ApplyStatus::NeedsConfirm => {
                                "needs confirm".yellow()
                            }
                        };
                        println!(
                            "  [{}] {:>12}  {} = {}",
                            v.timestamp.to_string().dimmed(),
                            st,
                            v.setting_id.bold(),
                            v.requested
                        );
                    } else {
                        println!("  {}", line);
                    }
                }
            }
        }
        ProfileSubcommand::Deviations { json } => {
            let snapshot = inspector.snapshot_all();
            let devs = simonlib::profile::deviation::deviations_from_default(&snapshot);
            if *json {
                println!("{}", serde_json::to_string_pretty(&devs)?);
            } else if devs.is_empty() {
                println!(
                    "{}",
                    "No settings deviate from their declared defaults.".green()
                );
            } else {
                println!(
                    "{} setting(s) deviate from declared defaults:\n",
                    devs.len().to_string().bold()
                );
                for d in devs {
                    let risk = match d.risk {
                        simonlib::profile::SettingRisk::Dangerous => "[dangerous]".red(),
                        simonlib::profile::SettingRisk::Moderate => "[moderate]".yellow(),
                        simonlib::profile::SettingRisk::Safe => "[safe]".green(),
                        simonlib::profile::SettingRisk::Informational => "[info]".dimmed(),
                    };
                    println!(
                        "  {} [{}] {} :: {}",
                        risk,
                        d.subsystem.as_str().cyan(),
                        d.device,
                        d.display_name.bold()
                    );
                    println!(
                        "      default: {}   current: {}",
                        d.default.to_string().dimmed(),
                        d.current.to_string().yellow()
                    );
                }
            }
        }
        ProfileSubcommand::Explain { setting_id, json } => {
            let snapshot = inspector.snapshot_all();
            match simonlib::profile::explain::explain(&snapshot, setting_id) {
                Some(exp) => {
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&exp)?);
                    } else {
                        println!("{}", exp.setting.display_name.bold().cyan());
                        println!("  id:        {}", exp.setting.id.italic());
                        println!(
                            "  scope:     [{}] {} / {}",
                            exp.subsystem.as_str(),
                            exp.device,
                            exp.profile.dimmed()
                        );
                        println!(
                            "  value:     {}{}",
                            exp.setting.value.to_string().bold(),
                            exp.setting
                                .unit
                                .as_deref()
                                .map(|u| format!(" {}", u))
                                .unwrap_or_default()
                        );
                        if let Some(default) = &exp.setting.default {
                            println!("  default:   {}", default);
                        }
                        if let Some((min, max)) = &exp.setting.range {
                            println!("  range:     [{}, {}]", min, max);
                        }
                        if let Some(choices) = &exp.setting.choices {
                            let names: Vec<String> =
                                choices.iter().map(|(n, _)| n.clone()).collect();
                            println!("  choices:   {}", names.join(", "));
                        }
                        let risk = match exp.setting.risk {
                            simonlib::profile::SettingRisk::Dangerous => "dangerous".red(),
                            simonlib::profile::SettingRisk::Moderate => "moderate".yellow(),
                            simonlib::profile::SettingRisk::Safe => "safe".green(),
                            simonlib::profile::SettingRisk::Informational => "info".dimmed(),
                        };
                        println!("  risk:      {}", risk);
                        println!("  source:    {}", exp.setting.source.italic());
                        if let Some(desc) = &exp.setting.description {
                            println!("\n  {}", desc);
                        }
                        if !exp.related.is_empty() {
                            println!("\n  {}", "Related settings:".bold());
                            for r in &exp.related {
                                println!("    · {:<32} = {}", r.id, r.value.to_string().bold());
                            }
                        }
                    }
                }
                None => {
                    let cands = simonlib::profile::explain::candidates(&snapshot, setting_id);
                    if cands.is_empty() {
                        eprintln!("No setting matches id {:?}", setting_id);
                        std::process::exit(1);
                    } else {
                        eprintln!("No exact match for {:?}. Did you mean:", setting_id);
                        for c in cands {
                            eprintln!("  · {}", c);
                        }
                        std::process::exit(1);
                    }
                }
            }
        }
        ProfileSubcommand::Active { matched, json } => {
            let entries = if *matched {
                simonlib::profile::active::matched_active_profiles()
            } else {
                simonlib::profile::active::active_profiles_for_processes()
            };
            if *json {
                println!("{}", serde_json::to_string_pretty(&entries)?);
            } else {
                let total = entries.len();
                let matched_count = entries.iter().filter(|e| e.has_any_profile()).count();
                println!(
                    "{} of {} processes have a known NVIDIA driver profile\n",
                    matched_count.to_string().bold(),
                    total.to_string().bold()
                );
                for e in entries {
                    let marker = if e.has_nvidia_drs_profile {
                        "✓".green().to_string()
                    } else {
                        "·".dimmed().to_string()
                    };
                    println!(
                        "  {} {:>6}  {}",
                        marker,
                        e.pid.to_string().dimmed(),
                        if e.has_nvidia_drs_profile {
                            e.name.bold().to_string()
                        } else {
                            e.name.dimmed().to_string()
                        }
                    );
                }
            }
        }
        ProfileSubcommand::Diff {
            baseline,
            against,
            json,
        } => {
            use simonlib::profile::diff::diff_snapshots;
            use simonlib::profile::ProfileSnapshot;
            let baseline_str = std::fs::read_to_string(baseline)?;
            let baseline_snap: ProfileSnapshot = serde_json::from_str(&baseline_str)?;
            let current_snap = if let Some(path) = against {
                let s = std::fs::read_to_string(path)?;
                serde_json::from_str::<ProfileSnapshot>(&s)?
            } else {
                inspector.snapshot_all()
            };
            let d = diff_snapshots(&baseline_snap, &current_snap);
            if *json {
                println!("{}", serde_json::to_string_pretty(&d)?);
            } else if d.is_empty() {
                println!("{}", "No changes between snapshots.".green());
            } else {
                println!(
                    "{} change(s) detected\n",
                    d.total_changes().to_string().bold()
                );
                if !d.added_groups.is_empty() {
                    println!("{}", "Added groups:".green().bold());
                    for g in &d.added_groups {
                        println!(
                            "  + [{}] {} / {} ({} settings)",
                            g.subsystem.as_str().cyan(),
                            g.device,
                            g.profile.dimmed(),
                            g.setting_count
                        );
                    }
                    println!();
                }
                if !d.removed_groups.is_empty() {
                    println!("{}", "Removed groups:".red().bold());
                    for g in &d.removed_groups {
                        println!(
                            "  - [{}] {} / {} ({} settings)",
                            g.subsystem.as_str().cyan(),
                            g.device,
                            g.profile.dimmed(),
                            g.setting_count
                        );
                    }
                    println!();
                }
                if !d.changed.is_empty() {
                    println!("{}", "Changed:".yellow().bold());
                    for c in &d.changed {
                        println!(
                            "  ~ [{}] {} / {} :: {}",
                            c.subsystem.as_str().cyan(),
                            c.device,
                            c.profile.dimmed(),
                            c.display_name.bold()
                        );
                        println!(
                            "      {} → {}",
                            c.before.to_string().red(),
                            c.after.to_string().green()
                        );
                    }
                    println!();
                }
                if !d.added.is_empty() {
                    println!("{}", "Added settings:".green().bold());
                    for a in &d.added {
                        println!(
                            "  + [{}] {} / {} :: {} = {}",
                            a.subsystem.as_str().cyan(),
                            a.device,
                            a.profile.dimmed(),
                            a.display_name,
                            a.value.to_string().bold()
                        );
                    }
                    println!();
                }
                if !d.removed.is_empty() {
                    println!("{}", "Removed settings:".red().bold());
                    for r in &d.removed {
                        println!(
                            "  - [{}] {} / {} :: {} (was {})",
                            r.subsystem.as_str().cyan(),
                            r.device,
                            r.profile.dimmed(),
                            r.display_name,
                            r.value
                        );
                    }
                }
            }
        }
        ProfileSubcommand::Dump { json, output } => {
            let snapshot = inspector.snapshot_all();
            if *json {
                let serialized = serde_json::to_string_pretty(&snapshot)?;
                if let Some(path) = output {
                    std::fs::write(path, serialized)?;
                    println!("Wrote profile snapshot to {}", path.display());
                } else {
                    println!("{}", serialized);
                }
            } else {
                let mut sink = String::new();
                use std::fmt::Write;
                writeln!(
                    sink,
                    "Hardware Profile Snapshot ({} groups, {} settings)\n",
                    snapshot.total_groups(),
                    snapshot.total_settings()
                )?;
                for (sub, groups) in &snapshot.providers {
                    writeln!(sink, "=== {} ===", sub.as_str().to_uppercase())?;
                    for group in groups {
                        writeln!(sink, "\n[{}] {}", group.device, group.display_name)?;
                        writeln!(sink, "  source: {}", group.source)?;
                        for s in &group.settings {
                            writeln!(
                                sink,
                                "  {:<32} = {}{}",
                                s.id,
                                s.value,
                                s.unit
                                    .as_deref()
                                    .map(|u| format!(" {}", u))
                                    .unwrap_or_default()
                            )?;
                        }
                        for n in &group.notes {
                            writeln!(sink, "  note: {}", n)?;
                        }
                    }
                    writeln!(sink)?;
                }
                if let Some(path) = output {
                    std::fs::write(path, &sink)?;
                    println!("Wrote profile snapshot to {}", path.display());
                } else {
                    print!("{}", sink);
                }
            }
        }
    }
    Ok(())
}

/// Pretty-print profile groups for a subsystem.
#[cfg(feature = "cli")]
fn print_profile_groups(
    sub: simonlib::profile::Subsystem,
    groups: &[simonlib::profile::ProfileGroup],
) {
    if groups.is_empty() {
        println!("(no profile data for {})", sub);
        return;
    }
    println!("{}", format!("== {} ==", sub).bold().cyan());
    for group in groups {
        println!();
        println!("{} {}", "▸".cyan(), group.device.bold(),);
        println!(
            "  {} {}   {} {}",
            "profile:".dimmed(),
            group.display_name,
            "source:".dimmed(),
            group.source.italic()
        );
        for s in &group.settings {
            let unit = s
                .unit
                .as_deref()
                .map(|u| format!(" {}", u))
                .unwrap_or_default();
            let risk = match s.risk {
                simonlib::profile::SettingRisk::Informational => String::new(),
                simonlib::profile::SettingRisk::Safe => " [safe]".green().to_string(),
                simonlib::profile::SettingRisk::Moderate => " [moderate]".yellow().to_string(),
                simonlib::profile::SettingRisk::Dangerous => " [dangerous]".red().to_string(),
            };
            println!(
                "    {:<28} = {}{}{}",
                s.display_name,
                s.value.to_string().bold(),
                unit,
                risk,
            );
        }
        for n in &group.notes {
            println!("    {} {}", "•".dimmed(), n.dimmed());
        }
    }
}

/// Parse a CLI string into a [`SettingValue`]. Recognizes bool literals,
/// integers, and floats; otherwise falls back to [`SettingValue::Text`].
#[cfg(feature = "cli")]
fn parse_setting_value(raw: &str) -> simonlib::profile::SettingValue {
    use simonlib::profile::SettingValue;
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "true" | "yes" | "on" | "enabled" | "enable" => return SettingValue::Bool(true),
        "false" | "no" | "off" | "disabled" | "disable" => return SettingValue::Bool(false),
        _ => {}
    }
    if let Ok(i) = raw.parse::<i64>() {
        return SettingValue::Int(i);
    }
    if let Ok(u) = raw.parse::<u64>() {
        return SettingValue::Uint(u);
    }
    if let Ok(f) = raw.parse::<f64>() {
        return SettingValue::Float(f);
    }
    SettingValue::Text(raw.to_string())
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("CLI features not enabled. Please compile with --features cli");
    std::process::exit(1);
}

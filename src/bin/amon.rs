//! AI Monitor (amon) - Syntactic sugar for `simon ai`
//!
//! This binary automatically routes all commands to the AI agent interface,
//! providing a simpler interface for querying system state via natural language.

#[cfg(feature = "cli")]
use clap::Parser;
use std::time::Duration;

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "amon")]
#[command(about = "AI Monitor: Ask questions about your system state using natural language", long_about = None)]
#[command(version)]
struct Cli {
    /// Question to ask the AI agent (if not provided, enters interactive mode)
    query: Option<String>,

    /// Additional query arguments (combined with the first query)
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    extra_args: Vec<String>,

    /// List available AI backends
    #[arg(long)]
    list_backends: bool,
}

#[cfg(feature = "cli")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use simonlib::agent::{Agent, AgentConfig};
    use simonlib::SiliconMonitor;
    use std::io::{self, Write};

    env_logger::init();

    let cli = Cli::parse();

    // List backends if requested
    if cli.list_backends {
        println!("[*] Available AI Backends:\n");
        let backends = AgentConfig::list_available_backends();
        for (i, backend) in backends.iter().enumerate() {
            println!("{}. {}", i + 1, backend.display_name());
            if let Some(env_var) = backend.api_key_env_var() {
                let status = if std::env::var(env_var).is_ok() {
                    "[+] configured"
                } else {
                    "[-] not configured"
                };
                println!("   API Key: {} {}", env_var, status);
            }
            if let Some(endpoint) = backend.default_endpoint() {
                println!("   Endpoint: {}", endpoint);
            }
            println!();
        }
        return Ok(());
    }

    // Combine query and extra args into one question
    let query = if let Some(first) = cli.query {
        let mut full_query = first;
        if !cli.extra_args.is_empty() {
            full_query.push(' ');
            full_query.push_str(&cli.extra_args.join(" "));
        }
        Some(full_query)
    } else {
        None
    };

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

        let response = agent.ask(&question, &monitor)?;
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
        println!("Examples:");
        println!("  * What's my GPU temperature?");
        println!("  * Show me memory usage");
        println!("  * Is my CPU usage normal?");
        println!("  * How much power am I using?\n");

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

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("CLI features not enabled. Please compile with --features cli");
    std::process::exit(1);
}

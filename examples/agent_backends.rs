//! Agent Backend Discovery and Configuration Example
//!
//! This example demonstrates:
//! - Automatic backend discovery
//! - Backend configuration
//! - Switching between local and remote backends
//! - Using different AI models (OpenAI, Ollama, rule-based)

use simonlib::agent::{Agent, AgentConfig, BackendConfig, BackendDiscovery, BackendType};
use simonlib::SiliconMonitor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Silicon Monitor - AI Agent Backend Discovery\n");
    println!("{}", "=".repeat(60));

    // 1. Discover available backends
    println!("\n1. Discovering Available Backends...\n");
    let discovery = BackendDiscovery::discover();

    println!("Available backends:");
    for backend in discovery.available() {
        println!("  ✓ {}", backend.display_name());
        if backend.requires_api_key() {
            if let Some(env_var) = backend.api_key_env_var() {
                let status = if std::env::var(env_var).is_ok() {
                    "configured ✓"
                } else {
                    "not configured ✗"
                };
                println!("    API Key: {} ({})", env_var, status);
            }
        }
        if let Some(endpoint) = backend.default_endpoint() {
            println!("    Endpoint: {}", endpoint);
        }
    }

    let recommended = discovery.recommended();
    println!("\n  Recommended: {}", recommended.display_name());

    // 2. Rule-Based Backend (always available)
    println!("\n{}", "=".repeat(60));
    println!("\n2. Using Rule-Based Backend (Built-in)\n");

    let config_rule_based = AgentConfig::new(simonlib::agent::ModelSize::Medium);
    let mut agent_rule_based = Agent::new(config_rule_based)?;

    let monitor = SiliconMonitor::new()?;

    let response = agent_rule_based.ask("What's my GPU temperature?", &monitor)?;
    println!("Query:    What's my GPU temperature?");
    println!("Response: {}", response.response);
    println!("Time:     {}ms (rule-based)", response.inference_time_ms);

    // 3. Ollama Backend (if available)
    println!("\n{}", "=".repeat(60));
    println!("\n3. Checking Ollama Backend (Local Server)\n");

    if discovery.is_available(&BackendType::RemoteOllama) {
        println!("✓ Ollama is running locally");

        #[cfg(feature = "remote-backends")]
        {
            // List available models
            let ollama_config = BackendConfig::ollama("llama3");
            let client = simonlib::agent::RemoteClient::new(ollama_config)?;

            match client.list_models() {
                Ok(models) => {
                    println!("\nAvailable models:");
                    for model in models.iter().take(5) {
                        println!("  • {}", model);
                    }
                }
                Err(e) => println!("Could not list models: {}", e),
            }

            // Create agent with Ollama
            println!("\nCreating agent with Ollama (llama3)...");
            let ollama_backend = BackendConfig::ollama("llama3");
            let config_ollama = AgentConfig::with_backend(ollama_backend);

            match Agent::new(config_ollama) {
                Ok(mut agent_ollama) => {
                    println!("✓ Agent created successfully");

                    let response = agent_ollama.ask("What's my GPU temperature?", &monitor)?;
                    println!("\nQuery:    What's my GPU temperature?");
                    println!("Response: {}", response.response);
                    println!("Time:     {}ms (Ollama)", response.inference_time_ms);
                }
                Err(e) => println!("✗ Failed to create agent: {}", e),
            }
        }

        #[cfg(not(feature = "remote-backends"))]
        println!(
            "  ℹ️  Remote backends require 'remote-backends' feature\n\
            Build with: cargo run --features remote-backends --example agent_backends"
        );
    } else {
        println!("✗ Ollama is not running");
        println!("  To use Ollama:");
        println!("  1. Install: https://ollama.com/download");
        println!("  2. Run: ollama serve");
        println!("  3. Pull model: ollama pull llama3");
    }

    // 4. OpenAI Backend (if API key available)
    println!("\n{}", "=".repeat(60));
    println!("\n4. Checking OpenAI Backend\n");

    if discovery.is_available(&BackendType::RemoteOpenAI) {
        println!("✓ OpenAI API key configured (OPENAI_API_KEY)");

        #[cfg(feature = "remote-backends")]
        {
            println!("\nCreating agent with OpenAI (gpt-4o-mini)...");
            let openai_backend = BackendConfig::openai("gpt-4o-mini", None);
            let config_openai = AgentConfig::with_backend(openai_backend);

            match Agent::new(config_openai) {
                Ok(mut agent_openai) => {
                    println!("✓ Agent created successfully");

                    let response = agent_openai.ask("What's my GPU temperature?", &monitor)?;
                    println!("\nQuery:    What's my GPU temperature?");
                    println!("Response: {}", response.response);
                    println!(
                        "Time:     {}ms (OpenAI GPT-4o-mini)",
                        response.inference_time_ms
                    );
                }
                Err(e) => println!("✗ Failed to create agent: {}", e),
            }
        }
    } else {
        println!("✗ OpenAI API key not found");
        println!("  To use OpenAI:");
        println!("  1. Get API key: https://platform.openai.com/api-keys");
        println!("  2. Set: export OPENAI_API_KEY='your-key-here'");
    }

    // 5. Backend Comparison
    println!("\n{}", "=".repeat(60));
    println!("\n5. Backend Comparison\n");

    println!(
        "{:<25} {:<15} {:<15} {:<10}",
        "Backend", "Cost", "Speed", "Privacy"
    );
    println!("{}", "-".repeat(65));

    let backends = vec![
        ("Rule-Based (built-in)", "Free", "~15ms", "100% local"),
        ("Ollama (local)", "Free", "~500ms", "100% local"),
        ("LM Studio (local)", "Free", "~400ms", "100% local"),
        ("OpenAI GPT-4o-mini", "$0.15/1M", "~300ms", "Cloud"),
        ("Anthropic Claude", "$3.00/1M", "~400ms", "Cloud"),
        ("GitHub Models", "Free*", "~350ms", "Cloud"),
    ];

    for (name, cost, speed, privacy) in backends {
        println!("{:<25} {:<15} {:<15} {:<10}", name, cost, speed, privacy);
    }

    println!("\n  * GitHub Models free for personal use with rate limits");

    // 6. Configuration Examples
    println!("\n{}", "=".repeat(60));
    println!("\n6. Configuration Examples\n");

    println!("Rule-Based (Default):");
    println!(
        "  let config = AgentConfig::new(ModelSize::Medium);\n\
        let agent = Agent::new(config)?;\n"
    );

    println!("Ollama (Local):");
    println!(
        "  let backend = BackendConfig::ollama(\"llama3\");\n\
        let config = AgentConfig::with_backend(backend);\n\
        let agent = Agent::new(config)?;\n"
    );

    println!("OpenAI:");
    println!(
        "  let backend = BackendConfig::openai(\"gpt-4o-mini\", None);\n\
        let config = AgentConfig::with_backend(backend);\n\
        let agent = Agent::new(config)?;\n"
    );

    println!("Anthropic Claude:");
    println!(
        "  let backend = BackendConfig::anthropic(\"claude-3-5-sonnet-20241022\", None);\n\
        let config = AgentConfig::with_backend(backend);\n\
        let agent = Agent::new(config)?;\n"
    );

    // 7. Recommendations
    println!("\n{}", "=".repeat(60));
    println!("\n7. Recommendations\n");

    println!("For quick testing:");
    println!("  → Rule-Based (instant, no setup required)\n");

    println!("For better reasoning (local):");
    println!("  → Ollama with llama3 or mistral");
    println!("  → LM Studio with any local model\n");

    println!("For best reasoning (cloud):");
    println!("  → OpenAI GPT-4o or GPT-4o-mini");
    println!("  → Anthropic Claude 3.5 Sonnet\n");

    println!("For cost-effective:");
    println!("  → GitHub Models (free for personal use)");
    println!("  → OpenAI GPT-4o-mini ($0.15/1M tokens)\n");

    Ok(())
}

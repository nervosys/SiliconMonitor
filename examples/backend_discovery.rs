//! Example: Backend Discovery and Configuration
//!
//! This example demonstrates the automatic backend discovery system
//! and shows how to configure different AI backends.
//!
//! # Usage
//!
//! ```bash
//! cargo run --release --example backend_discovery --features "cli,remote-backends"
//! ```

use simonlib::agent::{BackendCapabilities, BackendDiscovery, BackendType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== AI Backend Discovery System ===\n");

    // Discover available backends
    let discovery = BackendDiscovery::discover();

    println!("Available backends:");
    for backend in discovery.available() {
        println!("  ✓ {}", backend.display_name());

        // Show capabilities
        let caps = BackendCapabilities::for_backend(backend);
        println!("    - Context length: {} tokens", caps.max_context_length);
        println!(
            "    - Streaming: {}",
            if caps.supports_streaming { "Yes" } else { "No" }
        );

        if let Some(cost) = caps.cost_per_million_tokens {
            println!("    - Cost: ${}/1M tokens", cost);
        } else {
            println!("    - Cost: Free");
        }

        // Show configuration
        if backend.requires_api_key() {
            if let Some(env_var) = backend.api_key_env_var() {
                let configured = std::env::var(env_var).is_ok();
                println!(
                    "    - API Key: {} {}",
                    env_var,
                    if configured {
                        "[+] configured"
                    } else {
                        "[-] not set"
                    }
                );
            }
        }

        if let Some(endpoint) = backend.default_endpoint() {
            println!("    - Endpoint: {}", endpoint);
        }

        println!();
    }

    // Show recommended backend
    let recommended = discovery.recommended();
    println!("Recommended backend: {}", recommended.display_name());
    println!();

    // Show all backend types and their properties
    println!("=== All Backend Types ===\n");

    let all_backends = vec![
        BackendType::LocalGGML,
        BackendType::LocalONNX,
        BackendType::LocalCandle,
        BackendType::RemoteOpenAI,
        BackendType::RemoteAnthropic,
        BackendType::RemoteOllama,
        BackendType::RemoteLMStudio,
        BackendType::RemoteVllm,
        BackendType::RemoteTensorRT,
        BackendType::RemoteGitHub,
        BackendType::RemoteAzure,
    ];

    for backend in all_backends {
        let available = discovery.is_available(&backend);
        let status = if available {
            "[+] Available"
        } else {
            "[-] Not available"
        };

        println!("{:<35} {}", backend.display_name(), status);

        // Show why it's not available
        if !available {
            let reason = match backend {
                BackendType::LocalGGML | BackendType::LocalONNX | BackendType::LocalCandle => {
                    "   Reason: No local model files detected"
                }
                BackendType::RemoteOpenAI if std::env::var("OPENAI_API_KEY").is_err() => {
                    "   Reason: OPENAI_API_KEY not set"
                }
                BackendType::RemoteAnthropic if std::env::var("ANTHROPIC_API_KEY").is_err() => {
                    "   Reason: ANTHROPIC_API_KEY not set"
                }
                BackendType::RemoteGitHub if std::env::var("GITHUB_TOKEN").is_err() => {
                    "   Reason: GITHUB_TOKEN not set"
                }
                BackendType::RemoteAzure if std::env::var("AZURE_OPENAI_API_KEY").is_err() => {
                    "   Reason: AZURE_OPENAI_API_KEY not set"
                }
                BackendType::RemoteOllama => {
                    "   Reason: Ollama server not running on localhost:11434"
                }
                BackendType::RemoteLMStudio => {
                    "   Reason: LM Studio server not running on localhost:1234"
                }
                BackendType::RemoteVllm => "   Reason: vLLM server not running on localhost:8000",
                BackendType::RemoteTensorRT => {
                    "   Reason: TensorRT-LLM/Triton server not running on localhost:8001"
                }
                _ => "   Reason: Not configured",
            };
            println!("{}", reason);
        }
    }

    println!("\n=== Configuration Examples ===\n");

    // Show configuration examples
    println!("// Rule-based (always available)");
    println!("let config = BackendConfig::rule_based();");
    println!();

    println!("// Ollama (if running)");
    println!("let config = BackendConfig::ollama(\"llama3.2:3b\");");
    println!();

    println!("// vLLM (if running)");
    println!("// Start with: vllm serve meta-llama/Llama-3-8B-Instruct");
    println!("let config = BackendConfig {{");
    println!("    backend_type: BackendType::RemoteVllm,");
    println!("    model_id: \"meta-llama/Llama-3-8B-Instruct\".to_string(),");
    println!("    endpoint: Some(\"http://localhost:8000\".to_string()),");
    println!("    ..Default");
    println!("}};");
    println!();

    println!("// OpenAI (requires API key)");
    println!("let config = BackendConfig::openai(\"gpt-4o-mini\", None);");
    println!("// Set environment variable: export OPENAI_API_KEY=\"sk-...\"");
    println!();

    println!("\n=== Getting Started ===\n");
    println!("To enable local AI backends:");
    println!();
    println!("1. **Ollama** (Recommended):");
    println!("   - Install: https://ollama.ai");
    println!("   - Pull model: ollama pull llama3.2:3b");
    println!("   - Verify: curl http://localhost:11434/api/tags");
    println!();
    println!("2. **vLLM** (High Performance):");
    println!("   - Install: pip install vllm");
    println!("   - Start: vllm serve meta-llama/Llama-3-8B-Instruct");
    println!("   - Verify: curl http://localhost:8000/v1/models");
    println!();
    println!("3. **Check what's available**:");
    println!("   - CLI: amon --list-backends");
    println!("   - Or run this example again");
    println!();

    Ok(())
}

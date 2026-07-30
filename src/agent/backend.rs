//! Agent Backend System - Local and Remote Model Support
//!
//! This module provides a unified interface for both local and remote AI backends,
//! with automatic discovery and configuration.

use crate::agent::local::CliProvider;
use crate::error::{Result, SimonError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Agent backend type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// IronWorks — simon's built-in inference engine, and the only one.
    ///
    /// Pure-Rust engine served over an OpenAI-compatible API on the local machine.
    /// Every other entry in this enum is an *external provider*: a third-party server,
    /// a command-line tool, or a hosted API. IronWorks is the engine simon itself
    /// ships against, and it is preferred over all of them because it runs locally, so
    /// system telemetry never leaves the host.
    IronWorks,

    /// A locally installed command-line AI tool driven as a subprocess.
    ///
    /// Note that only some of these infer locally — see [`CliProvider::runs_on_host`].
    Cli(CliProvider),

    /// Local GGML/llama.cpp inference, via the `llama-cli` executable.
    ///
    /// Implemented (see [`crate::agent::local::llamacpp`]) and auto-detected on
    /// `PATH`, but never auto-selected: it needs a GGUF model path that cannot be
    /// guessed, so it must be configured explicitly.
    LocalGGML,

    /// Local ONNX Runtime inference.
    ///
    /// **Not implemented.** No client exists and discovery always reports it
    /// unavailable, so this variant is currently unreachable. Retained only because
    /// removing a public enum variant would be a breaking change; do not read its
    /// presence as support.
    LocalONNX,

    /// Local Candle (Rust) inference.
    ///
    /// **Not implemented.** As with [`BackendType::LocalONNX`], no client exists and
    /// discovery always reports it unavailable. An in-process candle backend was
    /// evaluated and rejected: `candle-core` depends unconditionally on `tokenizers`,
    /// which pulls in the `onig` C library and `esaxx-rs` C++, so it does not deliver
    /// the pure-Rust build it appears to promise. IronWorks fills this role instead.
    LocalCandle,

    /// Remote OpenAI API
    RemoteOpenAI,

    /// Remote Anthropic Claude API
    RemoteAnthropic,

    /// Remote Ollama (local server)
    RemoteOllama,

    /// Remote LM Studio (local server)
    RemoteLMStudio,

    /// Remote vLLM (local/remote server)
    RemoteVllm,

    /// Remote TensorRT-LLM (local server, NVIDIA GPUs)
    RemoteTensorRT,

    /// Remote GitHub Models
    RemoteGitHub,

    /// Remote Azure OpenAI
    RemoteAzure,

    /// Custom backend (user-defined)
    Custom(String),
}

impl BackendType {
    /// Check if this is a local backend (runs on user's machine)
    ///
    /// Note that IronWorks is *hosted* locally but reached over HTTP, so it is
    /// "local" for privacy purposes while still needing a running server. Use
    /// [`BackendType::runs_on_host`] when the question is "does telemetry leave this
    /// machine", and [`BackendType::is_local`] when it is "is the model loaded
    /// in-process".
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            BackendType::LocalGGML | BackendType::LocalONNX | BackendType::LocalCandle
        )
    }

    /// Whether inference happens on this machine, so no telemetry is transmitted to
    /// a third party.
    ///
    /// True for in-process backends and for servers listening on localhost.
    pub fn runs_on_host(&self) -> bool {
        match self {
            // A CLI tool is a local *process*, which is not the same as local
            // inference: `claude`, `codex` and `gemini` relay the prompt to their
            // vendor's API. Only the tool itself knows.
            BackendType::Cli(provider) => provider.runs_on_host(),
            _ => matches!(
                self,
                BackendType::IronWorks
                    | BackendType::LocalGGML
                    | BackendType::LocalONNX
                    | BackendType::LocalCandle
                    | BackendType::RemoteOllama
                    | BackendType::RemoteLMStudio
                    | BackendType::RemoteVllm
                    | BackendType::RemoteTensorRT
            ),
        }
    }

    /// Whether this is simon's built-in inference engine.
    ///
    /// Exactly one backend is built in: [`BackendType::IronWorks`]. Everything else is
    /// an external provider that must be installed, started, or authenticated
    /// separately.
    pub fn is_builtin_engine(&self) -> bool {
        matches!(self, BackendType::IronWorks)
    }

    /// Check if this is a remote backend
    pub fn is_remote(&self) -> bool {
        !self.is_local()
    }

    /// Get display name
    pub fn display_name(&self) -> &str {
        match self {
            BackendType::IronWorks => "IronWorks (Built-in Engine)",
            BackendType::Cli(provider) => provider.display_name(),
            BackendType::LocalGGML => "GGML/llama.cpp (Local)",
            BackendType::LocalONNX => "ONNX Runtime (Local)",
            BackendType::LocalCandle => "Candle (Local)",
            BackendType::RemoteOpenAI => "OpenAI API",
            BackendType::RemoteAnthropic => "Anthropic Claude",
            BackendType::RemoteOllama => "Ollama (Local Server)",
            BackendType::RemoteLMStudio => "LM Studio (Local Server)",
            BackendType::RemoteVllm => "vLLM (High-Performance Server)",
            BackendType::RemoteTensorRT => "TensorRT-LLM (NVIDIA Optimized)",
            BackendType::RemoteGitHub => "GitHub Models",
            BackendType::RemoteAzure => "Azure OpenAI",
            BackendType::Custom(name) => name,
        }
    }

    /// Check if backend requires API key
    pub fn requires_api_key(&self) -> bool {
        matches!(
            self,
            BackendType::RemoteOpenAI
                | BackendType::RemoteAnthropic
                | BackendType::RemoteGitHub
                | BackendType::RemoteAzure
        )
    }

    /// Get environment variable name for API key
    pub fn api_key_env_var(&self) -> Option<&str> {
        match self {
            BackendType::RemoteOpenAI => Some("OPENAI_API_KEY"),
            BackendType::RemoteAnthropic => Some("ANTHROPIC_API_KEY"),
            BackendType::RemoteGitHub => Some("GITHUB_TOKEN"),
            BackendType::RemoteAzure => Some("AZURE_OPENAI_API_KEY"),
            _ => None,
        }
    }

    /// Get default endpoint URL
    pub fn default_endpoint(&self) -> Option<String> {
        match self {
            // Port 8080 matches the IronWorks server and CLI default. The `/v1`
            // suffix is required because the OpenAI-compatible request path appends
            // only "/chat/completions" to whatever is configured here.
            BackendType::IronWorks => Some("http://localhost:8080/v1".to_string()),
            BackendType::RemoteOpenAI => Some("https://api.openai.com/v1".to_string()),
            BackendType::RemoteAnthropic => Some("https://api.anthropic.com/v1".to_string()),
            BackendType::RemoteOllama => Some("http://localhost:11434".to_string()),
            BackendType::RemoteLMStudio => Some("http://localhost:1234/v1".to_string()),
            // `/v1` is required: the request path appends only "/chat/completions",
            // so without it every vLLM request went to /chat/completions and 404'd.
            BackendType::RemoteVllm => Some("http://localhost:8000/v1".to_string()),
            BackendType::RemoteTensorRT => Some("http://localhost:8001".to_string()),
            BackendType::RemoteGitHub => Some("https://models.inference.ai.azure.com".to_string()),
            BackendType::RemoteAzure => None, // Requires custom endpoint
            _ => None,
        }
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend type
    pub backend_type: BackendType,

    /// Model identifier (e.g., "gpt-4", "llama-3-8b", "phi-3-mini")
    pub model_id: String,

    /// API endpoint URL (for remote backends)
    pub endpoint: Option<String>,

    /// API key (for remote backends requiring authentication)
    pub api_key: Option<String>,

    /// Local model path (for local backends)
    pub model_path: Option<PathBuf>,

    /// Maximum tokens in response
    pub max_tokens: usize,

    /// Temperature (0.0-1.0)
    pub temperature: f32,

    /// Request timeout
    pub timeout: Duration,

    /// Additional backend-specific options
    pub options: HashMap<String, String>,
}

impl BackendConfig {
    /// Create config for OpenAI
    pub fn openai(model: &str, api_key: Option<String>) -> Self {
        Self {
            backend_type: BackendType::RemoteOpenAI,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteOpenAI.default_endpoint(),
            api_key: api_key.or_else(|| std::env::var("OPENAI_API_KEY").ok()),
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for Anthropic Claude
    pub fn anthropic(model: &str, api_key: Option<String>) -> Self {
        Self {
            backend_type: BackendType::RemoteAnthropic,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteAnthropic.default_endpoint(),
            api_key: api_key.or_else(|| std::env::var("ANTHROPIC_API_KEY").ok()),
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for Ollama (local server)
    /// Create config for an IronWorks server (the default backend).
    ///
    /// The timeout is more generous than the other local servers': IronWorks loads
    /// the model on first request, so a cold start can exceed the 30s used elsewhere.
    pub fn ironworks(model: &str) -> Self {
        Self {
            backend_type: BackendType::IronWorks,
            model_id: model.to_string(),
            endpoint: BackendType::IronWorks.default_endpoint(),
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(120),
            options: HashMap::new(),
        }
    }

    /// Create config for a command-line AI tool.
    ///
    /// No endpoint is set — these are subprocesses, not servers. The timeout is
    /// generous because a CLI tool pays process startup on every query and may also
    /// be waiting on a hosted API behind the scenes.
    pub fn cli(provider: CliProvider, model: &str) -> Self {
        Self {
            backend_type: BackendType::Cli(provider),
            model_id: model.to_string(),
            endpoint: None,
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(120),
            options: HashMap::new(),
        }
    }

    /// Create config for Ollama (local server)
    pub fn ollama(model: &str) -> Self {
        Self {
            backend_type: BackendType::RemoteOllama,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteOllama.default_endpoint(),
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for LM Studio (local server)
    pub fn lm_studio(model: &str) -> Self {
        Self {
            backend_type: BackendType::RemoteLMStudio,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteLMStudio.default_endpoint(),
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for GitHub Models
    pub fn github_models(model: &str, token: Option<String>) -> Self {
        Self {
            backend_type: BackendType::RemoteGitHub,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteGitHub.default_endpoint(),
            api_key: token.or_else(|| std::env::var("GITHUB_TOKEN").ok()),
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for vLLM server
    pub fn vllm(model: &str) -> Self {
        Self {
            backend_type: BackendType::RemoteVllm,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteVllm.default_endpoint(),
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for TensorRT-LLM server
    pub fn tensorrt(model: &str) -> Self {
        Self {
            backend_type: BackendType::RemoteTensorRT,
            model_id: model.to_string(),
            endpoint: BackendType::RemoteTensorRT.default_endpoint(),
            api_key: None,
            model_path: None,
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(30),
            options: HashMap::new(),
        }
    }

    /// Create config for local GGML model
    pub fn ggml(model_path: PathBuf) -> Self {
        Self {
            backend_type: BackendType::LocalGGML,
            model_id: model_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string(),
            endpoint: None,
            api_key: None,
            model_path: Some(model_path),
            max_tokens: 256,
            temperature: 0.3,
            timeout: Duration::from_secs(10),
            options: HashMap::new(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check if API key is required but missing
        if self.backend_type.requires_api_key() && self.api_key.is_none() {
            return Err(SimonError::Configuration(format!(
                "Backend {} requires API key via {}",
                self.backend_type.display_name(),
                self.backend_type
                    .api_key_env_var()
                    .unwrap_or("API_KEY environment variable")
            )));
        }

        // Check if local model path exists
        if self.backend_type.is_local() && self.model_path.is_none() {
            return Err(SimonError::Configuration(format!(
                "Backend {} requires model_path",
                self.backend_type.display_name()
            )));
        }

        Ok(())
    }
}

/// Backend discovery and availability
pub struct BackendDiscovery {
    available_backends: Vec<BackendType>,
}

impl BackendDiscovery {
    /// Discover available backends
    ///
    /// The five server probes are HTTP round trips to localhost ports that are
    /// usually not listening, and each costs up to a 1s connect timeout. Run in
    /// sequence they took 4.7s on a machine with only Ollama running — long enough
    /// that the GUI's AI tab, which gave up waiting after 3s, reported "AI backend
    /// not connected" for a backend that was about to be found. They are independent,
    /// so they now run concurrently and the whole discovery costs about one probe.
    pub fn discover() -> Self {
        let mut available = Vec::new();

        // Order the results deterministically regardless of which probe finishes
        // first: IronWorks is the default backend and must stay ahead of the others,
        // because `recommended()` takes the first available entry.
        let probes: [(BackendType, fn() -> bool); 5] = [
            (BackendType::IronWorks, Self::check_ironworks_available),
            (BackendType::RemoteOllama, Self::check_ollama_available),
            (BackendType::RemoteLMStudio, Self::check_lm_studio_available),
            (BackendType::RemoteVllm, Self::check_vllm_available),
            (BackendType::RemoteTensorRT, Self::check_tensorrt_available),
        ];
        let server_results: Vec<bool> = std::thread::scope(|scope| {
            let handles: Vec<_> = probes
                .iter()
                .map(|(_, probe)| scope.spawn(move || probe()))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or(false))
                .collect()
        });

        if server_results[0] {
            available.push(BackendType::IronWorks);
        }

        // Check for local backends
        if Self::check_ggml_available() {
            available.push(BackendType::LocalGGML);
        }
        if Self::check_onnx_available() {
            available.push(BackendType::LocalONNX);
        }
        if Self::check_candle_available() {
            available.push(BackendType::LocalCandle);
        }

        // Check for remote backends (via environment variables or local servers)
        if std::env::var("OPENAI_API_KEY").is_ok() {
            available.push(BackendType::RemoteOpenAI);
        }
        if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            available.push(BackendType::RemoteAnthropic);
        }
        if std::env::var("GITHUB_TOKEN").is_ok() {
            available.push(BackendType::RemoteGitHub);
        }
        if std::env::var("AZURE_OPENAI_API_KEY").is_ok() {
            available.push(BackendType::RemoteAzure);
        }

        // Local server backends, from the probes run concurrently above.
        for (backend, _) in probes.iter().skip(1) {
            let idx = probes
                .iter()
                .position(|(b, _)| b == backend)
                .expect("backend came from this array");
            if server_results[idx] {
                available.push(backend.clone());
            }
        }

        // Command-line tools. Detection is a PATH lookup, so unlike the server probes
        // above this costs no network round trip.
        for provider in CliProvider::ALL {
            if provider.detect().is_some() {
                available.push(BackendType::Cli(provider));
            }
        }

        Self {
            available_backends: available,
        }
    }

    /// Get list of available backends
    pub fn available(&self) -> &[BackendType] {
        &self.available_backends
    }

    /// Check if specific backend is available
    pub fn is_available(&self, backend: &BackendType) -> bool {
        self.available_backends.contains(backend)
    }

    /// Get recommended backend (prefer local, fallback to remote)
    pub fn recommended(&self) -> BackendType {
        // Ordering encodes two preferences, in this priority:
        //
        //  1. Keep telemetry on the host. Everything above RemoteOpenAI infers
        //     locally, so a hosted provider is only chosen when nothing local exists.
        //  2. Prefer a running server to a subprocess. A CLI tool pays process-spawn
        //     cost on every single query, so it ranks below an equivalent server.
        //
        // The CLI tools that relay to a vendor (claude/codex/gemini) sit alongside the
        // hosted APIs, because that is exactly what they are — but ahead of raw API
        // backends, since they are already authenticated and need no key configured.
        // `LocalGGML` is deliberately absent even though it is discoverable. Finding
        // `llama-cli` on PATH proves the tool exists, but llama.cpp also needs a GGUF
        // model path that simon cannot guess — `BackendConfig::validate` rejects a
        // local backend without one. Recommending it would hand back a config that
        // fails at construction, so it must be configured explicitly.
        for backend in &[
            BackendType::IronWorks,
            BackendType::RemoteTensorRT,
            BackendType::RemoteVllm,
            BackendType::RemoteOllama,
            BackendType::RemoteLMStudio,
            BackendType::Cli(CliProvider::Ollama),
            BackendType::Cli(CliProvider::Claude),
            BackendType::Cli(CliProvider::Codex),
            BackendType::Cli(CliProvider::Gemini),
            BackendType::RemoteOpenAI,
        ] {
            if self.is_available(backend) {
                return backend.clone();
            }
        }

        // Nothing available. Return the default so the resulting error names the
        // backend the user is most likely to want to start.
        BackendType::IronWorks
    }

    /// Check if an IronWorks server is running.
    fn check_ironworks_available() -> bool {
        Self::check_http_endpoint("http://localhost:8080/v1/models")
    }

    /// Check if GGML/llama.cpp is available
    fn check_ggml_available() -> bool {
        // llama.cpp is driven through its CLI (see `agent::local::llamacpp`), so
        // availability means the executable is on PATH.
        //
        // This previously returned a hardcoded false, so a working llama.cpp install
        // was never discovered despite the client being implemented.
        ["llama-cli", "llama-server", "main"]
            .iter()
            .any(|name| crate::agent::local::cli::binary_on_path(name))
    }

    /// Check if ONNX Runtime is available
    /// Always false: there is no ONNX client to route to.
    ///
    /// Reporting availability for an unimplemented backend would let `recommended()`
    /// select it and then fail at construction.
    fn check_onnx_available() -> bool {
        false
    }

    /// Check if Candle is available
    /// Always false: there is no candle client to route to. See
    /// [`BackendType::LocalCandle`] for why one was not added.
    fn check_candle_available() -> bool {
        false
    }

    /// Check if Ollama is running
    fn check_ollama_available() -> bool {
        // Try to connect to Ollama server
        Self::check_http_endpoint("http://localhost:11434/api/tags")
    }

    /// Check if LM Studio is running
    fn check_lm_studio_available() -> bool {
        // Try to connect to LM Studio server
        Self::check_http_endpoint("http://localhost:1234/v1/models")
    }

    /// Check if vLLM is running
    fn check_vllm_available() -> bool {
        // Try to connect to vLLM server
        Self::check_http_endpoint("http://localhost:8000/v1/models")
    }

    /// Check if TensorRT-LLM is running
    fn check_tensorrt_available() -> bool {
        // Try to connect to TensorRT-LLM/Triton server
        Self::check_http_endpoint("http://localhost:8001/v2/health/ready")
    }

    /// Check if HTTP endpoint is accessible
    fn check_http_endpoint(_url: &str) -> bool {
        // Simple HTTP check with timeout for discovery
        #[cfg(feature = "remote-backends")]
        {
            use std::time::Duration;
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(2)) // 2s timeout for discovery
                .connect_timeout(Duration::from_secs(1)) // 1s connect timeout
                .build();

            if let Ok(client) = client {
                match client.get(_url).send() {
                    Ok(response) => {
                        // Check if response is successful (2xx status code)
                        let success = response.status().is_success();
                        log::debug!(
                            "Backend check {}: status={}, success={}",
                            _url,
                            response.status(),
                            success
                        );
                        return success;
                    }
                    Err(e) => {
                        log::debug!("Backend check {} failed: {}", _url, e);
                    }
                }
            }
        }
        false
    }
}

impl Default for BackendDiscovery {
    fn default() -> Self {
        Self::discover()
    }
}

/// Backend capabilities
#[derive(Debug, Clone)]
pub struct BackendCapabilities {
    /// Supports streaming responses
    pub supports_streaming: bool,

    /// Supports function calling
    pub supports_functions: bool,

    /// Supports vision (image input)
    pub supports_vision: bool,

    /// Maximum context length
    pub max_context_length: usize,

    /// Estimated cost per 1M tokens (in USD)
    pub cost_per_million_tokens: Option<f32>,
}

impl BackendCapabilities {
    /// Get capabilities for backend type
    pub fn for_backend(backend: &BackendType) -> Self {
        match backend {
            BackendType::IronWorks => Self {
                supports_streaming: true,
                // IronWorks supports multimodal and tool use behind its own feature
                // flags, so what a given server offers depends on how it was built.
                // Report the conservative baseline rather than advertising a
                // capability a stock server may not have.
                supports_functions: false,
                supports_vision: false,
                max_context_length: 32_000,
                cost_per_million_tokens: None, // Runs on your hardware
            },
            BackendType::Cli(_) => Self {
                // A subprocess returns one complete stdout buffer, so nothing can be
                // streamed regardless of what the underlying model supports.
                supports_streaming: false,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 32_000,
                // Ollama runs locally and is free. The others bill through the vendor
                // account the tool is signed in to, at rates simon cannot observe —
                // reporting a number here would be a guess.
                cost_per_million_tokens: None,
            },
            BackendType::RemoteOpenAI => Self {
                supports_streaming: true,
                supports_functions: true,
                supports_vision: true,
                max_context_length: 128_000,
                cost_per_million_tokens: Some(5.0), // GPT-4o pricing
            },
            BackendType::RemoteAnthropic => Self {
                supports_streaming: true,
                supports_functions: true,
                supports_vision: true,
                max_context_length: 200_000,
                cost_per_million_tokens: Some(3.0), // Claude 3.5 Sonnet
            },
            BackendType::RemoteOllama | BackendType::RemoteLMStudio => Self {
                supports_streaming: true,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 8192,
                cost_per_million_tokens: None, // Local/free
            },
            BackendType::RemoteVllm => Self {
                supports_streaming: true,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 32_000,
                cost_per_million_tokens: None, // Local/free
            },
            BackendType::RemoteTensorRT => Self {
                supports_streaming: true,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 16_000,
                cost_per_million_tokens: None, // Local/free
            },
            BackendType::LocalGGML | BackendType::LocalONNX | BackendType::LocalCandle => Self {
                supports_streaming: false,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 4096,
                cost_per_million_tokens: None, // Local/free
            },
            BackendType::RemoteGitHub => Self {
                supports_streaming: true,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 16_000,
                cost_per_million_tokens: None, // Free for personal use
            },
            BackendType::RemoteAzure => Self {
                supports_streaming: true,
                supports_functions: true,
                supports_vision: true,
                max_context_length: 128_000,
                cost_per_million_tokens: Some(5.0),
            },
            BackendType::Custom(_) => Self {
                supports_streaming: false,
                supports_functions: false,
                supports_vision: false,
                max_context_length: 4096,
                cost_per_million_tokens: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_classification() {
        assert!(BackendType::LocalGGML.is_local());
        assert!(BackendType::RemoteOpenAI.is_remote());
        assert!(BackendType::RemoteOllama.is_remote());
    }

    /// OpenAI-compatible backends must carry `/v1` in their endpoint.
    ///
    /// `RemoteClient::query` builds the URL as `{endpoint}/chat/completions`, so an
    /// endpoint without `/v1` produces `host/chat/completions` and every request
    /// 404s. This is silent — discovery still reports the backend as available,
    /// because availability is probed on a different path.
    #[test]
    fn openai_compatible_endpoints_include_v1() {
        for backend in [
            BackendType::IronWorks,
            BackendType::RemoteOpenAI,
            BackendType::RemoteLMStudio,
            BackendType::RemoteVllm,
        ] {
            let endpoint = backend
                .default_endpoint()
                .unwrap_or_else(|| panic!("{} has no default endpoint", backend.display_name()));
            assert!(
                endpoint.ends_with("/v1"),
                "{} endpoint {endpoint} must end in /v1, or requests resolve to \
                 /chat/completions and 404",
                backend.display_name()
            );
        }
    }

    /// Whatever `recommended()` returns must be usable without further configuration.
    ///
    /// This is the invariant that makes `AgentConfig::auto_detect` safe: it feeds the
    /// recommendation straight into a `BackendConfig` and validates it. A backend that
    /// needs a model path (llama.cpp) or manual setup must therefore never be
    /// recommended, however detectable it is — otherwise auto-detection hands back a
    /// config that fails, and the user gets an error instead of a working agent.
    #[test]
    fn recommended_backend_is_always_auto_configurable() {
        let recommended = BackendDiscovery::discover().recommended();

        assert!(
            !recommended.is_local(),
            "{} requires an explicit model path, so it cannot be auto-configured",
            recommended.display_name()
        );

        // Must round-trip through the same path auto_detect uses.
        let config = crate::agent::AgentConfig::with_backend_type(recommended.clone());
        assert!(
            config.is_ok(),
            "recommended backend {} is not constructible by auto-detection: {:?}",
            recommended.display_name(),
            config.err()
        );
    }

    /// llama.cpp is discoverable but must never be auto-selected.
    ///
    /// The test above only exercises this on a machine that actually has `llama-cli`
    /// installed, so it would pass vacuously in CI. This asserts the underlying
    /// constraint directly: llama.cpp cannot be constructed without a model path, so
    /// it must stay out of the `recommended()` preference list.
    #[test]
    fn llamacpp_is_discoverable_but_not_auto_configurable() {
        assert!(
            BackendType::LocalGGML.is_local(),
            "local backends are the ones requiring an explicit model path"
        );
        assert!(
            crate::agent::AgentConfig::with_backend_type(BackendType::LocalGGML).is_err(),
            "llama.cpp became auto-configurable; only then may it rejoin recommended()"
        );
    }

    /// Exactly one backend is simon's built-in engine.
    #[test]
    fn ironworks_is_the_only_builtin_engine() {
        assert!(BackendType::IronWorks.is_builtin_engine());

        for other in [
            BackendType::LocalGGML,
            BackendType::RemoteOllama,
            BackendType::RemoteVllm,
            BackendType::RemoteTensorRT,
            BackendType::RemoteLMStudio,
            BackendType::RemoteOpenAI,
            BackendType::Cli(CliProvider::Ollama),
            BackendType::Cli(CliProvider::Claude),
        ] {
            assert!(
                !other.is_builtin_engine(),
                "{} is an external provider, not the built-in engine",
                other.display_name()
            );
        }
    }

    /// A CLI tool being a local *process* does not make it local *inference*.
    ///
    /// Getting this backwards would route hardware telemetry to a vendor while
    /// reporting it as staying on the machine.
    #[test]
    fn cli_backends_report_host_locality_per_tool() {
        assert!(BackendType::Cli(CliProvider::Ollama).runs_on_host());

        for relayed in [CliProvider::Claude, CliProvider::Codex, CliProvider::Gemini] {
            assert!(
                !BackendType::Cli(relayed).runs_on_host(),
                "{relayed} relays to a hosted API, so telemetry leaves the host"
            );
        }
    }

    /// CLI backends are subprocesses and must not be given an HTTP endpoint.
    #[test]
    fn cli_config_has_no_endpoint() {
        let config = BackendConfig::cli(CliProvider::Claude, "default");
        assert_eq!(config.backend_type, BackendType::Cli(CliProvider::Claude));
        assert!(
            config.endpoint.is_none(),
            "a CLI backend has no server to address"
        );
        config
            .validate()
            .expect("a CLI config must be valid without an endpoint");
    }

    /// IronWorks is the default backend and must be preferred whenever it is running.
    #[test]
    fn ironworks_is_the_default_backend() {
        let config = BackendConfig::ironworks("default");
        assert_eq!(config.backend_type, BackendType::IronWorks);
        assert_eq!(
            config.endpoint.as_deref(),
            Some("http://localhost:8080/v1"),
            "must match the IronWorks server default port"
        );
        config
            .validate()
            .expect("the default IronWorks config must be valid as constructed");

        // Telemetry stays on the host for IronWorks; that is why it outranks the
        // hosted APIs in `recommended()`.
        assert!(BackendType::IronWorks.runs_on_host());
        assert!(!BackendType::RemoteOpenAI.runs_on_host());
        assert!(!BackendType::IronWorks.requires_api_key());
    }

    #[test]
    fn test_backend_api_key_requirements() {
        assert!(BackendType::RemoteOpenAI.requires_api_key());
        assert!(!BackendType::RemoteOllama.requires_api_key());
    }

    #[test]
    fn test_backend_discovery() {
        let discovery = BackendDiscovery::discover();
        // At least one backend should be available (or none if no backends configured)
        // Test just validates discovery runs without panic
        let _ = discovery.available();
    }
}

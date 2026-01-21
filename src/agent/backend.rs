//! Agent Backend System - Local and Remote Model Support
//!
//! This module provides a unified interface for both local and remote AI backends,
//! with automatic discovery and configuration.

use crate::error::{Result, SimonError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Agent backend type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Local GGML/llama.cpp inference
    LocalGGML,

    /// Local ONNX Runtime inference
    LocalONNX,

    /// Local Candle (Rust) inference
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
    pub fn is_local(&self) -> bool {
        matches!(
            self,
            BackendType::LocalGGML | BackendType::LocalONNX | BackendType::LocalCandle
        )
    }

    /// Check if this is a remote backend
    pub fn is_remote(&self) -> bool {
        !self.is_local()
    }

    /// Get display name
    pub fn display_name(&self) -> &str {
        match self {
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
            BackendType::RemoteOpenAI => Some("https://api.openai.com/v1".to_string()),
            BackendType::RemoteAnthropic => Some("https://api.anthropic.com/v1".to_string()),
            BackendType::RemoteOllama => Some("http://localhost:11434".to_string()),
            BackendType::RemoteLMStudio => Some("http://localhost:1234/v1".to_string()),
            BackendType::RemoteVllm => Some("http://localhost:8000".to_string()),
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
    pub fn discover() -> Self {
        let mut available = Vec::new();

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

        // Check for local server backends
        if Self::check_ollama_available() {
            available.push(BackendType::RemoteOllama);
        }
        if Self::check_lm_studio_available() {
            available.push(BackendType::RemoteLMStudio);
        }
        if Self::check_vllm_available() {
            available.push(BackendType::RemoteVllm);
        }
        if Self::check_tensorrt_available() {
            available.push(BackendType::RemoteTensorRT);
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
        // Preference order: TensorRT > vLLM > Ollama > LM Studio > Local GGML > OpenAI > Rule-based
        for backend in &[
            BackendType::RemoteTensorRT,
            BackendType::RemoteVllm,
            BackendType::RemoteOllama,
            BackendType::RemoteLMStudio,
            BackendType::LocalGGML,
            BackendType::RemoteOpenAI,
        ] {
            if self.is_available(backend) {
                return backend.clone();
            }
        }

        // No backends available - return first one we tried or Ollama as default
        // This will cause an error when trying to use it, which is appropriate
        BackendType::RemoteOllama
    }

    /// Check if GGML/llama.cpp is available
    fn check_ggml_available() -> bool {
        // Check for llama.cpp executable or library
        // For now, return false until implementation
        false
    }

    /// Check if ONNX Runtime is available
    fn check_onnx_available() -> bool {
        // Check for ONNX Runtime library
        false
    }

    /// Check if Candle is available
    fn check_candle_available() -> bool {
        // Candle is a Rust library, check if models are available
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

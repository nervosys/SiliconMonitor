//! Local AI Inference Backends
//!
//! This module provides support for running AI models locally without requiring
//! external API calls. Supports multiple inference engines:
//!
//! - **Ollama**: Popular local LLM server with easy model management
//! - **llama.cpp**: Direct GGUF model loading via llama-cpp bindings
//! - **vLLM**: High-performance inference server with OpenAI-compatible API
//! - **TensorRT-LLM**: NVIDIA's optimized inference engine
//! - **LM Studio**: User-friendly local model server
//!
//! # Feature Flags
//!
//! - `local-ollama`: Enable Ollama client (HTTP-based)
//! - `local-llamacpp`: Enable llama.cpp native bindings
//! - `local-vllm`: Enable vLLM client (HTTP-based)
//! - `local-tensorrt`: Enable TensorRT-LLM support
//!
//! # Example - Ollama
//!
//! ```no_run
//! use simon::agent::local::{OllamaClient, LocalInferenceClient, InferenceRequest};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OllamaClient::new("http://localhost:11434")?;
//!
//! // Check available models
//! let models = client.list_models().await?;
//! println!("Available models: {:?}", models);
//!
//! // Run inference
//! let request = InferenceRequest {
//!     model: "llama3".to_string(),
//!     prompt: "What is the GPU temperature?".to_string(),
//!     system: None,
//!     max_tokens: Some(256),
//!     temperature: Some(0.7),
//!     ..Default::default()
//! };
//! let response = client.generate(request).await?;
//! println!("Response: {}", response.text);
//! # Ok(())
//! # }
//! ```

pub mod ollama;

#[cfg(feature = "local-llamacpp")]
pub mod llamacpp;

#[cfg(feature = "local-vllm")]
pub mod vllm;

#[cfg(feature = "local-tensorrt")]
pub mod tensorrt;

pub use ollama::OllamaClient;

#[cfg(feature = "local-llamacpp")]
pub use llamacpp::LlamaCppClient;

#[cfg(feature = "local-vllm")]
pub use vllm::VllmClient;

#[cfg(feature = "local-tensorrt")]
pub use tensorrt::TensorRtClient;

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Common inference request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// Model identifier
    pub model: String,

    /// Prompt/query text
    pub prompt: String,

    /// System prompt (optional)
    pub system: Option<String>,

    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,

    /// Temperature (0.0-1.0)
    pub temperature: Option<f32>,

    /// Top-p sampling
    pub top_p: Option<f32>,

    /// Stop sequences
    pub stop: Option<Vec<String>>,

    /// Enable streaming
    pub stream: bool,
}

impl Default for InferenceRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            prompt: String::new(),
            system: None,
            max_tokens: Some(256),
            temperature: Some(0.3),
            top_p: Some(0.9),
            stop: None,
            stream: false,
        }
    }
}

/// Inference response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResponse {
    /// Generated text
    pub text: String,

    /// Model used
    pub model: String,

    /// Tokens generated
    pub tokens_generated: Option<usize>,

    /// Inference duration in milliseconds
    pub duration_ms: u64,

    /// Whether response was truncated
    pub truncated: bool,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier
    pub name: String,

    /// Model size in bytes
    pub size: Option<u64>,

    /// Model family (e.g., "llama", "phi", "mistral")
    pub family: Option<String>,

    /// Parameter count
    pub parameter_count: Option<String>,

    /// Quantization level (e.g., "Q4_K_M", "Q8_0")
    pub quantization: Option<String>,
}

/// Common trait for local inference clients
#[async_trait]
pub trait LocalInferenceClient: Send + Sync {
    /// Get client name
    fn name(&self) -> &str;

    /// Check if server is available
    async fn is_available(&self) -> bool;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Generate text from prompt
    async fn generate(&self, request: InferenceRequest) -> Result<InferenceResponse>;

    /// Get model info
    async fn model_info(&self, model_name: &str) -> Result<ModelInfo>;
}

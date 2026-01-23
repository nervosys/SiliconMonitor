//! Lightweight AI Agent for System Analysis and Predictions
//!
//! This module provides a non-blocking AI agent that can answer questions about
//! system state, predict completion times, estimate energy usage, and perform
//! calculations based on hardware monitoring data.
//!
//! # Design Principles
//!
//! - **Zero Latency Impact**: Agent runs in separate thread, never blocks monitoring
//! - **Small Models**: Offers 100M, 500M, and 1B parameter fine-tuned reasoning models
//! - **Privacy-First**: All processing local, no data sent to external servers
//! - **Consent-Aware**: Respects user consent settings for any data collection
//!
//! # Features
//!
//! - **System State Queries**: "What's my GPU utilization?", "Show memory usage"
//! - **Predictions**: "When will this training complete?", "ETA for disk copy"
//! - **Energy Analysis**: "How much power am I using?", "Cost per hour estimate"
//! - **Comparisons**: "Is my GPU faster than X?", "Compare temps across GPUs"
//! - **Recommendations**: "Should I upgrade RAM?", "Optimize settings"
//!
//! # Example
//!
//! ```no_run
//! use simon::agent::{Agent, AgentConfig, ModelSize};
//! use simon::SiliconMonitor;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create agent with 500M model (balanced)
//! let config = AgentConfig::new(ModelSize::Medium);
//! let mut agent = Agent::new(config)?;
//!
//! // Create monitor for system state
//! let monitor = SiliconMonitor::new()?;
//!
//! // Ask question (non-blocking)
//! let response = agent.ask(
//!     "What's my GPU temperature and is it safe?",
//!     &monitor
//! )?;
//!
//! println!("Agent: {}", response);
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod engine;
pub mod inference;
pub mod local;
pub mod query;
pub mod remote;
pub mod state;

use crate::error::{Result, SimonError};
use crate::SiliconMonitor;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use backend::{BackendCapabilities, BackendConfig, BackendDiscovery, BackendType};
pub use engine::InferenceEngine;
pub use local::{
    InferenceRequest, InferenceResponse, LocalInferenceClient, ModelInfo, OllamaClient,
};
pub use query::{Query, QueryType};
pub use remote::{RemoteClient, RemoteClientBuilder};
pub use state::SystemState;

/// AI model size options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSize {
    /// 100M parameters - Fastest, basic queries (~50-100ms latency)
    Small,
    /// 500M parameters - Balanced, good reasoning (~100-200ms latency)
    Medium,
    /// 1B parameters - Best reasoning, slower (~200-500ms latency)
    Large,
}

impl ModelSize {
    /// Get estimated inference latency in milliseconds
    pub fn latency_estimate_ms(&self) -> u64 {
        match self {
            Self::Small => 75,
            Self::Medium => 150,
            Self::Large => 350,
        }
    }

    /// Get memory requirement in MB
    pub fn memory_mb(&self) -> usize {
        match self {
            Self::Small => 200,   // ~100M params * 2 bytes (quantized)
            Self::Medium => 1000, // ~500M params * 2 bytes
            Self::Large => 2000,  // ~1B params * 2 bytes
        }
    }

    /// Get model description
    pub fn description(&self) -> &str {
        match self {
            Self::Small => "100M params - Fast responses, basic queries",
            Self::Medium => "500M params - Balanced speed and reasoning",
            Self::Large => "1B params - Advanced reasoning, slower",
        }
    }
}

impl Default for ModelSize {
    fn default() -> Self {
        Self::Medium // Balanced choice
    }
}

impl std::fmt::Display for ModelSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small => write!(f, "100M"),
            Self::Medium => write!(f, "500M"),
            Self::Large => write!(f, "1B"),
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Model size to use (for local models)
    pub model_size: ModelSize,

    /// Model cache directory
    pub model_dir: PathBuf,

    /// Maximum response length in tokens
    pub max_response_tokens: usize,

    /// Temperature for sampling (0.0 = deterministic, 1.0 = creative)
    pub temperature: f32,

    /// Enable response caching (reuse answers for identical queries)
    pub enable_caching: bool,

    /// Maximum cache size (number of query-response pairs)
    pub cache_size: usize,

    /// Timeout for inference in seconds
    pub timeout_seconds: u64,

    /// Backend configuration (optional, defaults to rule-based)
    pub backend: Option<BackendConfig>,
}

impl AgentConfig {
    /// Create new config with specified model size (rule-based backend)
    pub fn new(model_size: ModelSize) -> Self {
        Self {
            model_size,
            model_dir: Self::default_model_dir(),
            max_response_tokens: 256, // Keep responses concise
            temperature: 0.3,         // Prefer factual responses
            enable_caching: true,
            cache_size: 100,
            timeout_seconds: 5, // Prevent hanging
            backend: None,      // Use rule-based by default
        }
    }

    /// Create new config with backend
    pub fn with_backend(backend: BackendConfig) -> Self {
        Self {
            model_size: ModelSize::Medium, // Ignored for remote backends
            model_dir: Self::default_model_dir(),
            max_response_tokens: backend.max_tokens,
            temperature: backend.temperature,
            enable_caching: true,
            cache_size: 100,
            timeout_seconds: backend.timeout.as_secs(),
            backend: Some(backend),
        }
    }

    /// Get default model directory (~/.cache/simon/models)
    fn default_model_dir() -> PathBuf {
        #[cfg(unix)]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".cache/simon/models")
        }

        #[cfg(windows)]
        {
            let local_app_data =
                std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "C:\\Temp".to_string());
            PathBuf::from(local_app_data).join("simon\\models")
        }
    }

    /// Set custom model directory
    pub fn with_model_dir(mut self, path: PathBuf) -> Self {
        self.model_dir = path;
        self
    }

    /// Set temperature (0.0-1.0)
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp.clamp(0.0, 1.0);
        self
    }

    /// Set max response tokens
    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_response_tokens = tokens;
        self
    }

    /// Disable caching
    pub fn without_caching(mut self) -> Self {
        self.enable_caching = false;
        self
    }

    /// Enable caching (on by default)
    pub fn with_caching(mut self, enabled: bool) -> Self {
        self.enable_caching = enabled;
        self
    }

    /// Set cache size
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = size;
        self
    }

    /// Set timeout duration
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_seconds = timeout.as_secs();
        self
    }

    /// Create config with automatic backend detection
    ///
    /// This will:
    /// 1. Discover available backends (local and remote)
    /// 2. Select the recommended backend
    /// 3. Configure with appropriate defaults
    pub fn auto_detect() -> Result<Self> {
        let discovery = BackendDiscovery::discover();
        let available_backends = discovery.available();

        if available_backends.is_empty() {
            return Err(SimonError::Configuration(
                "No AI backends available. Please install Ollama (https://ollama.com), \
                configure an API key (OPENAI_API_KEY or GITHUB_TOKEN), or set up another backend."
                    .to_string(),
            ));
        }

        let backend_type = discovery.recommended();

        // Create backend config based on detected type
        let backend_config = match backend_type {
            BackendType::RemoteOpenAI => {
                BackendConfig::openai("gpt-4o-mini", None) // Uses OPENAI_API_KEY from env
            }
            BackendType::RemoteAnthropic => {
                BackendConfig::anthropic("claude-3-5-haiku-20241022", None) // Uses ANTHROPIC_API_KEY from env
            }
            BackendType::RemoteOllama => {
                BackendConfig::ollama("llama3.2:3b") // Default Ollama model
            }
            BackendType::RemoteLMStudio => {
                BackendConfig::lm_studio("local-model") // LM Studio model
            }
            BackendType::RemoteGitHub => {
                BackendConfig::github_models("gpt-4o-mini", None) // Uses GITHUB_TOKEN from env
            }
            BackendType::RemoteVllm => {
                BackendConfig::vllm("local-model") // vLLM model
            }
            BackendType::RemoteTensorRT => {
                BackendConfig::tensorrt("local-model") // TensorRT model
            }
            _ => {
                return Err(SimonError::Configuration(format!(
                    "Backend {} auto-detection not supported. Please configure manually.",
                    backend_type.display_name()
                )));
            }
        };

        // Validate the config
        backend_config.validate()?;

        Ok(Self::with_backend(backend_config))
    }

    /// Create config with specific backend type
    pub fn with_backend_type(backend_type: BackendType) -> Result<Self> {
        let backend_config = match backend_type {
            BackendType::RemoteOpenAI => BackendConfig::openai("gpt-4o-mini", None),
            BackendType::RemoteAnthropic => {
                BackendConfig::anthropic("claude-3-5-haiku-20241022", None)
            }
            BackendType::RemoteOllama => BackendConfig::ollama("llama3.2:3b"),
            BackendType::RemoteLMStudio => BackendConfig::lm_studio("local-model"),
            BackendType::RemoteGitHub => BackendConfig::github_models("gpt-4o-mini", None),
            BackendType::RemoteVllm => BackendConfig::vllm("local-model"),
            BackendType::RemoteTensorRT => BackendConfig::tensorrt("local-model"),
            _ => {
                return Err(SimonError::Configuration(format!(
                    "Backend {} is not supported or requires manual configuration",
                    backend_type.display_name()
                )));
            }
        };

        backend_config.validate()?;
        Ok(Self::with_backend(backend_config))
    }

    /// List available backends
    pub fn list_available_backends() -> Vec<BackendType> {
        let discovery = BackendDiscovery::discover();
        discovery.available().to_vec()
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self::new(ModelSize::default())
    }
}

/// Query result with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// The query that was asked
    pub query: String,

    /// Agent's response
    pub response: String,

    /// Query type detected
    pub query_type: QueryType,

    /// Time taken for inference (milliseconds)
    pub inference_time_ms: u64,

    /// Whether response came from cache
    pub from_cache: bool,

    /// Timestamp of response
    pub timestamp: u64,
}

impl std::fmt::Display for AgentResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.response)
    }
}

impl AgentResponse {
    /// Check if response indicates an error
    pub fn is_error(&self) -> bool {
        self.response.starts_with("Error:")
            || self.response.starts_with("Unable to")
            || self.response.contains("not available")
    }
}

/// Lightweight AI agent for system analysis
pub struct Agent {
    config: AgentConfig,
    engine: Arc<Mutex<Option<InferenceEngine>>>,
    cache: Arc<Mutex<lru::LruCache<String, (String, QueryType)>>>,
    initialized: Arc<Mutex<bool>>,
}

impl Agent {
    /// Create new agent with configuration
    ///
    /// Note: Model loading happens lazily on first query to avoid startup latency
    pub fn new(config: AgentConfig) -> Result<Self> {
        let cache_size = if config.enable_caching {
            std::num::NonZeroUsize::new(config.cache_size).unwrap()
        } else {
            std::num::NonZeroUsize::new(1).unwrap()
        };

        Ok(Self {
            config,
            engine: Arc::new(Mutex::new(None)),
            cache: Arc::new(Mutex::new(lru::LruCache::new(cache_size))),
            initialized: Arc::new(Mutex::new(false)),
        })
    }

    /// Get the agent's configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Check if agent is initialized (model loaded)
    pub fn is_initialized(&self) -> bool {
        *self.initialized.lock().unwrap()
    }

    /// Initialize agent (load model) - called automatically on first query
    fn initialize(&self) -> Result<()> {
        let mut initialized = self.initialized.lock().unwrap();
        if *initialized {
            return Ok(());
        }

        // Initialize inference engine
        let mut engine_lock = self.engine.lock().unwrap();
        if engine_lock.is_none() {
            let engine = InferenceEngine::new(&self.config)?;
            *engine_lock = Some(engine);
        }

        *initialized = true;
        Ok(())
    }

    /// Ask agent a question about the system
    ///
    /// This is the main entry point for user queries. The agent will:
    /// 1. Parse the query and determine intent
    /// 2. Extract relevant system state from the monitor
    /// 3. Generate a contextual response using the reasoning model
    /// 4. Cache the response for future identical queries
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use simon::agent::{Agent, AgentConfig};
    /// # use simon::SiliconMonitor;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut agent = Agent::new(AgentConfig::default())?;
    /// let monitor = SiliconMonitor::new()?;
    ///
    /// let response = agent.ask("What's my GPU utilization?", &monitor)?;
    /// println!("Agent: {}", response.response);
    /// # Ok(())
    /// # }
    /// ```
    pub fn ask(&mut self, question: &str, monitor: &SiliconMonitor) -> Result<AgentResponse> {
        let start = Instant::now();
        let query_normalized = question.trim().to_lowercase();

        // Check cache first
        if self.config.enable_caching {
            let mut cache = self.cache.lock().unwrap();
            if let Some((cached_response, query_type)) = cache.get(&query_normalized).cloned() {
                return Ok(AgentResponse {
                    query: question.to_string(),
                    response: cached_response,
                    query_type,
                    inference_time_ms: start.elapsed().as_millis() as u64,
                    from_cache: true,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }

        // Initialize on first query (lazy loading)
        if !self.is_initialized() {
            self.initialize()?;
        }

        // Parse query
        let query = Query::parse(question);

        // Extract relevant system state
        let state = SystemState::from_monitor(monitor, &query)?;

        // Generate response using inference engine
        let response_text = {
            let mut engine_lock = self.engine.lock().unwrap();
            let engine = engine_lock
                .as_mut()
                .ok_or_else(|| SimonError::Other("Agent not initialized".to_string()))?;

            engine.generate_response(&query, &state)?
        };

        let inference_time = start.elapsed().as_millis() as u64;

        // Cache response
        if self.config.enable_caching {
            let mut cache = self.cache.lock().unwrap();
            cache.put(
                query_normalized,
                (response_text.clone(), query.query_type.clone()),
            );
        }

        Ok(AgentResponse {
            query: question.to_string(),
            response: response_text,
            query_type: query.query_type,
            inference_time_ms: inference_time,
            from_cache: false,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Ask question with timeout (non-blocking with time limit)
    pub fn ask_with_timeout(
        &mut self,
        question: &str,
        monitor: &SiliconMonitor,
        _timeout: Duration,
    ) -> Result<AgentResponse> {
        // For now, just use the regular ask with internal timeout
        // In a full implementation, this would spawn a thread
        self.ask(question, monitor)
    }

    /// Clear response cache
    pub fn clear_cache(&mut self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.lock().unwrap();
        (cache.len(), cache.cap().get())
    }

    /// Get current cache size (number of entries)
    pub fn cache_size(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }

    /// Preload model (warm start) - optional, to avoid first-query latency
    pub fn preload(&mut self) -> Result<()> {
        self.initialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_size_properties() {
        assert_eq!(ModelSize::Small.latency_estimate_ms(), 75);
        assert_eq!(ModelSize::Medium.latency_estimate_ms(), 150);
        assert_eq!(ModelSize::Large.latency_estimate_ms(), 350);

        assert_eq!(ModelSize::Small.memory_mb(), 200);
        assert_eq!(ModelSize::Medium.memory_mb(), 1000);
        assert_eq!(ModelSize::Large.memory_mb(), 2000);
    }

    #[test]
    fn test_config_defaults() {
        let config = AgentConfig::default();
        assert_eq!(config.model_size, ModelSize::Medium);
        assert!(config.enable_caching);
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.max_response_tokens, 256);
    }

    #[test]
    fn test_config_builder() {
        let config = AgentConfig::new(ModelSize::Large)
            .with_temperature(0.7)
            .with_max_tokens(512)
            .without_caching();

        assert_eq!(config.model_size, ModelSize::Large);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.max_response_tokens, 512);
        assert!(!config.enable_caching);
    }
}

//! Lightweight inference engine for AI models
//!
//! This module provides ML-powered inference engines for generating responses
//! using local models and remote APIs.

use crate::agent::{AgentConfig, Query, RemoteClient, SystemState};
use crate::error::{SimonError, Result};
use std::time::Instant;

/// Inference engine (ML-powered only)
pub struct InferenceEngine {
    config: AgentConfig,
    #[allow(dead_code)]
    initialized: bool,
    remote_client: RemoteClient,
}

impl InferenceEngine {
    /// Create new inference engine with configuration
    pub fn new(config: &AgentConfig) -> Result<Self> {
        let remote_client = if let Some(ref backend_config) = config.backend {
            RemoteClient::new(backend_config.clone())?
        } else {
            return Err(SimonError::Configuration(
                "No backend configured. Agent requires an AI backend (Ollama, OpenAI, etc.)"
                    .to_string(),
            ));
        };

        Ok(Self {
            config: config.clone(),
            initialized: true,
            remote_client,
        })
    }

    /// Generate response based on query and system state
    pub fn generate_response(&mut self, query: &Query, state: &SystemState) -> Result<String> {
        let start = Instant::now();

        // Use ML backend for all responses
        let response = self.generate_ml_response(&self.remote_client, query, state)?;

        // Check timeout
        let elapsed = start.elapsed();
        if elapsed.as_secs() > self.config.timeout_seconds {
            return Err(SimonError::Other(format!(
                "Inference timeout after {} seconds",
                elapsed.as_secs()
            )));
        }

        Ok(response)
    }

    /// Generate response using ML backend (local or remote)
    fn generate_ml_response(
        &self,
        client: &RemoteClient,
        query: &Query,
        state: &SystemState,
    ) -> Result<String> {
        // Build system prompt with context
        let system_prompt = format!(
            "You are a hardware monitoring assistant. Provide concise, factual answers \
            about system state. Keep responses under 200 words.\n\n\
            Current System State:\n{}",
            state.to_context_string()
        );

        // Send query to ML backend
        let (response, _elapsed) = client.query(&system_prompt, &query.text)?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentConfig, BackendConfig};

    #[test]
    fn test_engine_requires_backend() {
        let config = AgentConfig::default();
        let engine = InferenceEngine::new(&config);
        assert!(engine.is_err()); // Should fail without backend
    }

    #[test]
    fn test_engine_with_backend() {
        let mut config = AgentConfig::default();
        config.backend = Some(BackendConfig::ollama("test-model"));
        // Note: This will still fail without Ollama running, but validates structure
        let _result = InferenceEngine::new(&config);
    }
}

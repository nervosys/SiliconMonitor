//! AI Agent Format Exporters
//!
//! Export tools and ontology in formats understood by various AI agents.

use super::tools::get_all_tool_definitions;
use super::{ToolCategory, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Export format for different AI agent systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// OpenAI (GPT-4o, GPT-4.5, o1, o3, o3-mini)
    OpenAI,
    /// Anthropic (Claude 4 Opus, Claude 4 Sonnet, Claude 3.5)
    Anthropic,
    /// Google (Gemini 2.0, Gemini 1.5)
    Gemini,
    /// xAI (Grok 3, Grok 2)
    Grok,
    /// Meta Llama (Llama 4, Llama 3.3) - OpenAI-compatible format
    Llama,
    /// Mistral (Large, Mixtral) - OpenAI-compatible format
    Mistral,
    /// DeepSeek (R1, V3) - OpenAI-compatible format
    DeepSeek,
    /// JSON-LD for semantic web discovery
    JsonLd,
    /// Model Context Protocol (Claude Desktop, etc.)
    Mcp,
    /// Simple JSON manifest
    SimpleJson,
}

/// Complete AI agent manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub version: String,
    pub name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

impl AgentManifest {
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
            name: "Silicon Monitor".to_string(),
            description: "Comprehensive hardware monitoring for AI agents.".to_string(),
            capabilities: vec![
                "hardware_monitoring".to_string(),
                "gpu_monitoring".to_string(),
                "cpu_monitoring".to_string(),
                "memory_monitoring".to_string(),
                "process_monitoring".to_string(),
            ],
            tools: get_all_tool_definitions(),
        }
    }

    pub fn export(&self, format: ExportFormat) -> Value {
        match format {
            ExportFormat::OpenAI => self.to_openai(),
            ExportFormat::Anthropic => self.to_anthropic(),
            ExportFormat::Gemini => self.to_gemini(),
            ExportFormat::Grok => self.to_grok(),
            ExportFormat::Llama => self.to_llama(),
            ExportFormat::Mistral => self.to_mistral(),
            ExportFormat::DeepSeek => self.to_deepseek(),
            ExportFormat::JsonLd => self.to_json_ld(),
            ExportFormat::Mcp => self.to_mcp(),
            ExportFormat::SimpleJson => serde_json::to_value(self).unwrap_or(json!({})),
        }
    }

    fn to_openai(&self) -> Value {
        let functions: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        }).collect();
        // Supports: gpt-4o, gpt-4.5-preview, o1, o3, o3-mini, gpt-4-turbo, gpt-4
        json!({
            "model": "gpt-4o",
            "tools": functions,
            "tool_choice": "auto",
            "supported_models": [
                "gpt-4o", "gpt-4o-mini", "gpt-4.5-preview",
                "o1", "o1-mini", "o1-preview",
                "o3", "o3-mini",
                "gpt-4-turbo", "gpt-4"
            ]
        })
    }

    fn to_anthropic(&self) -> Value {
        let tools: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters,
            })
        }).collect();
        // Supports: claude-4-opus, claude-4-sonnet, claude-3.5-sonnet, claude-3-opus
        json!({
            "tools": tools,
            "supported_models": [
                "claude-opus-4-20250514", "claude-sonnet-4-20250514",
                "claude-3-5-sonnet-20241022", "claude-3-5-haiku-20241022",
                "claude-3-opus-20240229", "claude-3-sonnet-20240229"
            ]
        })
    }

    fn to_gemini(&self) -> Value {
        let function_declarations: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            })
        }).collect();
        // Supports: gemini-2.0-flash, gemini-2.0-pro, gemini-1.5-pro, gemini-1.5-flash
        json!({
            "tools": [{ "function_declarations": function_declarations }],
            "supported_models": [
                "gemini-2.0-flash", "gemini-2.0-flash-lite", "gemini-2.0-pro",
                "gemini-1.5-pro", "gemini-1.5-flash", "gemini-1.5-flash-8b"
            ]
        })
    }

    fn to_grok(&self) -> Value {
        // xAI Grok uses OpenAI-compatible format
        let functions: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        }).collect();
        // Supports: grok-3, grok-3-mini, grok-2, grok-2-mini
        json!({
            "model": "grok-3",
            "tools": functions,
            "tool_choice": "auto",
            "supported_models": ["grok-3", "grok-3-mini", "grok-2", "grok-2-mini"]
        })
    }

    fn to_llama(&self) -> Value {
        // Meta Llama via various providers (Together, Fireworks, etc.) - OpenAI-compatible
        let functions: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        }).collect();
        // Supports: Llama 4 Scout/Maverick, Llama 3.3, Llama 3.1
        json!({
            "model": "meta-llama/Llama-4-Scout-17B-16E-Instruct",
            "tools": functions,
            "tool_choice": "auto",
            "supported_models": [
                "meta-llama/Llama-4-Scout-17B-16E-Instruct",
                "meta-llama/Llama-4-Maverick-17B-128E-Instruct",
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo",
                "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo"
            ],
            "note": "Use with OpenAI-compatible API providers (Together, Fireworks, Groq, etc.)"
        })
    }

    fn to_mistral(&self) -> Value {
        // Mistral AI - OpenAI-compatible format
        let functions: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        }).collect();
        // Supports: Mistral Large, Codestral, Mixtral
        json!({
            "model": "mistral-large-latest",
            "tools": functions,
            "tool_choice": "auto",
            "supported_models": [
                "mistral-large-latest", "mistral-large-2411",
                "codestral-latest", "codestral-2501",
                "mistral-small-latest",
                "open-mixtral-8x22b", "open-mixtral-8x7b"
            ]
        })
    }

    fn to_deepseek(&self) -> Value {
        // DeepSeek - OpenAI-compatible format
        let functions: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        }).collect();
        // Supports: DeepSeek-R1, DeepSeek-V3, DeepSeek-Coder
        json!({
            "model": "deepseek-chat",
            "tools": functions,
            "tool_choice": "auto",
            "supported_models": [
                "deepseek-chat",
                "deepseek-reasoner"
            ],
            "note": "deepseek-chat = DeepSeek-V3, deepseek-reasoner = DeepSeek-R1"
        })
    }

    fn to_json_ld(&self) -> Value {
        json!({
            "@context": { "@vocab": "https://schema.org/", "simon": "https://schema.siliconmonitor.dev/" },
            "@type": "SoftwareApplication",
            "name": self.name,
            "description": self.description,
            "applicationCategory": "SystemUtility"
        })
    }

    fn to_mcp(&self) -> Value {
        let tools: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.parameters,
            })
        }).collect();
        json!({
            "name": "silicon-monitor",
            "version": self.version,
            "description": self.description,
            "protocol_version": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "tools": tools
        })
    }
}

impl Default for AgentManifest {
    fn default() -> Self { Self::new() }
}

pub fn get_tools_by_category() -> HashMap<ToolCategory, Vec<ToolDefinition>> {
    let tools = get_all_tool_definitions();
    let mut grouped: HashMap<ToolCategory, Vec<ToolDefinition>> = HashMap::new();
    for tool in tools {
        grouped.entry(tool.category).or_default().push(tool);
    }
    grouped
}
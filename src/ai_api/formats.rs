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
    OpenAI,
    Anthropic,
    Gemini,
    JsonLd,
    Mcp,
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
        json!({ "model": "gpt-4", "tools": functions, "tool_choice": "auto" })
    }

    fn to_anthropic(&self) -> Value {
        let tools: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.parameters,
            })
        }).collect();
        json!({ "tools": tools })
    }

    fn to_gemini(&self) -> Value {
        let function_declarations: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            })
        }).collect();
        json!({ "tools": [{ "function_declarations": function_declarations }] })
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

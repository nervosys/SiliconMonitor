# AI Agent Integration Guide

Silicon Monitor provides comprehensive hardware monitoring capabilities that AI agents (ChatGPT, Claude, Gemini, etc.) can use to understand and query system state.

## Quick Start

### For ChatGPT / OpenAI

Export the function calling schema:
```bash
simon ai-manifest --format openai -o openai_tools.json
```

Use in your API calls:
```python
import json
with open("openai_tools.json") as f:
    tools = json.load(f)["tools"]

response = client.chat.completions.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "What is my GPU temperature?"}],
    tools=tools
)
```

### For Claude / Anthropic

Export the tool use schema:
```bash
simon ai-manifest --format anthropic -o claude_tools.json
```

Use with the Anthropic API:
```python
import json
with open("claude_tools.json") as f:
    tools = json.load(f)["tools"]

response = client.messages.create(
    model="claude-sonnet-4-20250514",
    tools=tools,
    messages=[{"role": "user", "content": "How much GPU memory is being used?"}]
)
```

### For Claude Desktop (MCP)

Add to your Claude Desktop configuration (`claude_desktop_config.json`):
```json
{
  "mcpServers": {
    "silicon-monitor": {
      "command": "simon",
      "args": ["mcp-server"]
    }
  }
}
```

Then restart Claude Desktop. Silicon Monitor tools will be available automatically.

### For Gemini / Google AI

Export the function declarations:
```bash
simon ai-manifest --format gemini -o gemini_tools.json
```

## Available Tools (35+)

### System Tools
- `get_system_summary` - Comprehensive overview of all hardware
- `get_system_info` - Hostname, OS, architecture, BIOS info
- `get_platform_info` - Available monitoring capabilities

### GPU Tools
- `get_gpu_status` - All GPU utilization, memory, temp, power
- `get_gpu_list` - List all detected GPUs
- `get_gpu_details` - Detailed info for specific GPU
- `get_gpu_processes` - Processes using GPU resources
- `get_gpu_utilization` - GPU compute utilization
- `get_gpu_memory` - VRAM usage
- `get_gpu_temperature` - Thermal status
- `get_gpu_power` - Power consumption

### CPU Tools
- `get_cpu_status` - Overall CPU utilization
- `get_cpu_cores` - Per-core utilization
- `get_cpu_frequency` - Clock speeds

### Memory Tools
- `get_memory_status` - RAM and swap usage
- `get_memory_breakdown` - Detailed memory breakdown
- `get_swap_status` - Swap space usage

### Disk Tools
- `get_disk_list` - All storage devices
- `get_disk_details` - Specific disk info
- `get_disk_io` - Read/write statistics
- `get_disk_health` - SMART status

### Network Tools
- `get_network_interfaces` - All network interfaces
- `get_network_bandwidth` - Current bandwidth usage
- `get_interface_details` - Specific interface info

### Process Tools
- `get_process_list` - Running processes with resource usage
- `get_process_details` - Detailed process info
- `get_top_cpu_processes` - Highest CPU consumers
- `get_top_memory_processes` - Highest memory consumers
- `get_top_gpu_processes` - Highest GPU memory consumers

## MCP Server

The MCP server allows Claude and other MCP-compatible agents to directly invoke hardware monitoring tools.

Start the server:
```bash
simon mcp-server
```

The server communicates via JSON-RPC over stdio. It supports:
- `initialize` - Protocol handshake
- `tools/list` - List available tools
- `tools/call` - Execute a tool
- `resources/list` - List available resources

## Export Formats

| Format | Command | Use Case |
|--------|---------|----------|
| `openai` | `--format openai` | ChatGPT, GPT-4 function calling |
| `anthropic` | `--format anthropic` | Claude tool use |
| `gemini` | `--format gemini` | Google Gemini |
| `mcp` | `--format mcp` | Model Context Protocol |
| `jsonld` | `--format jsonld` | Semantic web, general discovery |
| `json` | `--format json` | Full manifest with ontology |

## Hardware Ontology

Silicon Monitor provides a structured ontology that describes hardware concepts:

```bash
simon ai-manifest --format json | jq .ontology
```

The ontology includes:
- **GPU** - Utilization, memory, temperature, power, clocks
- **CPU** - Cores, utilization, frequency
- **Memory** - RAM, swap, cached
- **Disk** - Devices, I/O, health
- **Network** - Interfaces, bandwidth
- **Process** - PID, CPU%, memory, GPU memory

## Best Practices for AI Agents

1. **Start with `get_system_summary`** - Get an overview before drilling down
2. **GPU indices are 0-based** - First GPU is index 0
3. **Memory values are in bytes** - Look for `_mb` or `_gb` suffixes for convenience
4. **Percentages are 0-100** - Not 0.0-1.0
5. **Use specific tools for detailed queries** - They are more efficient

## Example Prompts

- "What's the current GPU temperature and is it safe?"
- "How much memory is my Python process using?"
- "Show me the top 5 processes consuming GPU memory"
- "Is my system suitable for running a 7B parameter LLM?"
- "What's causing high CPU usage right now?"

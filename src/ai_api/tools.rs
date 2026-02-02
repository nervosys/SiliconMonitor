//! AI API Tool Implementations
//!
//! This module contains the actual tool implementations that query system data.

use super::types::*;
use super::{AiDataApi, ToolCategory, ToolDefinition};
use crate::error::{Result, SimonError};
use serde_json::json;

/// Get all tool definitions
pub fn get_all_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    // System tools
    tools.push(ToolDefinition {
        name: "get_system_summary".to_string(),
        description: "Get a comprehensive summary of the entire system including GPUs, CPU, memory, disk, network, and top processes. This is the best tool to start with for an overall view.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::System,
        example: Some("get_system_summary()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_system_info".to_string(),
        description: "Get system identification info: hostname, OS, kernel version, architecture, BIOS, manufacturer.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::System,
        example: Some("get_system_info()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_platform_info".to_string(),
        description:
            "Get platform-specific capabilities and what monitoring features are available."
                .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::System,
        example: Some("get_platform_info()".to_string()),
    });

    // GPU tools
    tools.push(ToolDefinition {
        name: "get_gpu_status".to_string(),
        description: "Get current status of all GPUs: utilization, memory, temperature, power. Best for quick GPU health check.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_status()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_list".to_string(),
        description: "List all detected GPUs with basic info (name, vendor, memory size)."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_list()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_details".to_string(),
        description:
            "Get detailed information about a specific GPU by index, including clocks, processes."
                .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "GPU index (0-based). Use get_gpu_list to see available indices."
                }
            },
            "required": ["gpu_index"]
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_details({\"gpu_index\": 0})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_processes".to_string(),
        description: "Get all processes currently using GPU resources with their memory usage."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: specific GPU index. Omit for all GPUs."
                }
            },
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_processes()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_utilization".to_string(),
        description: "Get GPU utilization percentages.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: specific GPU index. Omit for all GPUs."
                }
            },
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_utilization()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_memory".to_string(),
        description: "Get GPU memory usage (used, free, total) for each GPU.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: specific GPU index. Omit for all GPUs."
                }
            },
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_memory()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_temperature".to_string(),
        description: "Get GPU temperatures and thermal status.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: specific GPU index. Omit for all GPUs."
                }
            },
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_temperature()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_gpu_power".to_string(),
        description: "Get GPU power consumption and limits.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: specific GPU index. Omit for all GPUs."
                }
            },
            "required": []
        }),
        category: ToolCategory::Gpu,
        example: Some("get_gpu_power()".to_string()),
    });

    // CPU tools
    tools.push(ToolDefinition {
        name: "get_cpu_status".to_string(),
        description: "Get CPU utilization and status for all cores.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Cpu,
        example: Some("get_cpu_status()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_cpu_cores".to_string(),
        description: "Get detailed per-core CPU information.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Cpu,
        example: Some("get_cpu_cores()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_cpu_frequency".to_string(),
        description: "Get CPU frequency information (current, min, max) for each core.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Cpu,
        example: Some("get_cpu_frequency()".to_string()),
    });

    // Memory tools
    tools.push(ToolDefinition {
        name: "get_memory_status".to_string(),
        description: "Get RAM and swap usage summary.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Memory,
        example: Some("get_memory_status()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_memory_breakdown".to_string(),
        description: "Get detailed memory breakdown (buffers, cached, shared, etc.).".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Memory,
        example: Some("get_memory_breakdown()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_swap_status".to_string(),
        description: "Get swap space usage.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Memory,
        example: Some("get_swap_status()".to_string()),
    });

    // Disk tools
    tools.push(ToolDefinition {
        name: "get_disk_list".to_string(),
        description: "List all disk devices in the system.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Disk,
        example: Some("get_disk_list()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_disk_details".to_string(),
        description: "Get detailed info about a specific disk.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "disk_name": {
                    "type": "string",
                    "description": "Disk device name (e.g., 'sda', 'nvme0n1')"
                }
            },
            "required": ["disk_name"]
        }),
        category: ToolCategory::Disk,
        example: Some("get_disk_details({\"disk_name\": \"nvme0n1\"})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_disk_io".to_string(),
        description: "Get disk I/O statistics (reads, writes).".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "disk_name": {
                    "type": "string",
                    "description": "Optional: specific disk name. Omit for all disks."
                }
            },
            "required": []
        }),
        category: ToolCategory::Disk,
        example: Some("get_disk_io()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_disk_health".to_string(),
        description: "Get disk health status.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "disk_name": {
                    "type": "string",
                    "description": "Optional: specific disk name. Omit for all disks."
                }
            },
            "required": []
        }),
        category: ToolCategory::Disk,
        example: Some("get_disk_health()".to_string()),
    });

    // Network tools
    tools.push(ToolDefinition {
        name: "get_network_interfaces".to_string(),
        description: "List all network interfaces with their status.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "active_only": {
                    "type": "boolean",
                    "description": "Only show active interfaces. Default: true"
                }
            },
            "required": []
        }),
        category: ToolCategory::Network,
        example: Some("get_network_interfaces()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_network_bandwidth".to_string(),
        description: "Get current network bandwidth usage (receive/transmit rates).".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "interface_name": {
                    "type": "string",
                    "description": "Optional: specific interface name. Omit for all interfaces."
                }
            },
            "required": []
        }),
        category: ToolCategory::Network,
        example: Some("get_network_bandwidth()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_interface_details".to_string(),
        description: "Get detailed info about a specific network interface.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "interface_name": {
                    "type": "string",
                    "description": "Interface name (e.g., 'eth0', 'wlan0')"
                }
            },
            "required": ["interface_name"]
        }),
        category: ToolCategory::Network,
        example: Some("get_interface_details({\"interface_name\": \"eth0\"})".to_string()),
    });

    // Process tools
    tools.push(ToolDefinition {
        name: "get_process_list".to_string(),
        description: "Get list of running processes with CPU and memory usage.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of processes to return. Default: 50"
                },
                "sort_by": {
                    "type": "string",
                    "enum": ["cpu", "memory", "gpu_memory", "name", "pid"],
                    "description": "Sort order. Default: cpu"
                }
            },
            "required": []
        }),
        category: ToolCategory::Process,
        example: Some("get_process_list({\"limit\": 20, \"sort_by\": \"memory\"})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_process_details".to_string(),
        description: "Get detailed information about a specific process.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "pid": {
                    "type": "integer",
                    "description": "Process ID"
                }
            },
            "required": ["pid"]
        }),
        category: ToolCategory::Process,
        example: Some("get_process_details({\"pid\": 1234})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_top_cpu_processes".to_string(),
        description: "Get top N processes by CPU usage.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of processes to return. Default: 10"
                }
            },
            "required": []
        }),
        category: ToolCategory::Process,
        example: Some("get_top_cpu_processes({\"count\": 5})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_top_memory_processes".to_string(),
        description: "Get top N processes by memory usage.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of processes to return. Default: 10"
                }
            },
            "required": []
        }),
        category: ToolCategory::Process,
        example: Some("get_top_memory_processes({\"count\": 5})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_top_gpu_processes".to_string(),
        description: "Get top N processes by GPU memory usage.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "description": "Number of processes to return. Default: 10"
                },
                "gpu_index": {
                    "type": "integer",
                    "description": "Optional: filter to specific GPU"
                }
            },
            "required": []
        }),
        category: ToolCategory::Process,
        example: Some("get_top_gpu_processes({\"count\": 10})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "search_processes".to_string(),
        description: "Search for processes by name pattern.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search pattern (case-insensitive substring match)"
                }
            },
            "required": ["pattern"]
        }),
        category: ToolCategory::Process,
        example: Some("search_processes({\"pattern\": \"chrome\"})".to_string()),
    });

    // Hardware tools
    tools.push(ToolDefinition {
        name: "get_motherboard_sensors".to_string(),
        description: "Get all motherboard sensor readings (temperatures, voltages, fans)."
            .to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Hardware,
        example: Some("get_motherboard_sensors()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_system_temperatures".to_string(),
        description: "Get all temperature sensors in the system.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Hardware,
        example: Some("get_system_temperatures()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_fan_speeds".to_string(),
        description: "Get all fan speeds and status.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Hardware,
        example: Some("get_fan_speeds()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_voltage_rails".to_string(),
        description: "Get motherboard voltage rail readings.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        category: ToolCategory::Hardware,
        example: Some("get_voltage_rails()".to_string()),
    });

    tools.push(ToolDefinition {
        name: "get_driver_info".to_string(),
        description: "Get installed driver versions (GPU, network, storage).".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "driver_type": {
                    "type": "string",
                    "enum": ["gpu", "network", "storage", "all"],
                    "description": "Type of drivers to query. Default: all"
                }
            },
            "required": []
        }),
        category: ToolCategory::Hardware,
        example: Some("get_driver_info({\"driver_type\": \"gpu\"})".to_string()),
    });

    // Audio tools
    tools.push(ToolDefinition {
        name: "get_audio_devices".to_string(),
        description: "List all audio devices (speakers, microphones, headphones) with their status.".to_string(),
        parameters: json!({"type": "object", "properties": {"device_type": {"type": "string", "enum": ["output", "input", "all"], "description": "Filter by device type. Default: all"}}, "required": []}),
        category: ToolCategory::Audio,
        example: Some("get_audio_devices()".to_string()),
    });
    tools.push(ToolDefinition {
        name: "get_audio_status".to_string(),
        description: "Get current audio status including master volume and mute state.".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
        category: ToolCategory::Audio,
        example: Some("get_audio_status()".to_string()),
    });
    // Bluetooth tools
    tools.push(ToolDefinition {
        name: "get_bluetooth_adapters".to_string(),
        description: "List all Bluetooth adapters in the system.".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
        category: ToolCategory::Bluetooth,
        example: Some("get_bluetooth_adapters()".to_string()),
    });
    tools.push(ToolDefinition {
        name: "get_bluetooth_devices".to_string(),
        description: "List all paired/discovered Bluetooth devices with connection status.".to_string(),
        parameters: json!({"type": "object", "properties": {"connected_only": {"type": "boolean", "description": "Only show connected devices. Default: false"}}, "required": []}),
        category: ToolCategory::Bluetooth,
        example: Some("get_bluetooth_devices()".to_string()),
    });
    // Display tools
    tools.push(ToolDefinition {
        name: "get_display_list".to_string(),
        description: "List all connected displays/monitors with resolution and refresh rate.".to_string(),
        parameters: json!({"type": "object", "properties": {}, "required": []}),
        category: ToolCategory::Display,
        example: Some("get_display_list()".to_string()),
    });
    tools.push(ToolDefinition {
        name: "get_display_details".to_string(),
        description: "Get detailed information about a specific display.".to_string(),
        parameters: json!({"type": "object", "properties": {"display_id": {"type": "string", "description": "Display identifier"}}, "required": ["display_id"]}),
        category: ToolCategory::Display,
        example: Some("get_display_details()".to_string()),
    });
    // USB tools
    tools.push(ToolDefinition {
        name: "get_usb_devices".to_string(),
        description: "List all connected USB devices with vendor/product info.".to_string(),
        parameters: json!({"type": "object", "properties": {"class": {"type": "string", "enum": ["audio", "hid", "storage", "hub", "video", "all"], "description": "Filter by USB device class. Default: all"}}, "required": []}),
        category: ToolCategory::Usb,
        example: Some("get_usb_devices()".to_string()),
    });
    tools.push(ToolDefinition {
        name: "get_usb_device_details".to_string(),
        description: "Get detailed information about a specific USB device.".to_string(),
        parameters: json!({"type": "object", "properties": {"bus": {"type": "integer", "description": "USB bus number"}, "address": {"type": "integer", "description": "Device address on bus"}}, "required": ["bus", "address"]}),
        category: ToolCategory::Usb,
        example: Some("get_usb_device_details()".to_string()),
    });

    // Historical data tools
    tools.push(ToolDefinition {
        name: "get_historical_data".to_string(),
        description: "Get historical system metrics from a specific time in the past. Useful for questions like 'What was the GPU temperature 5 minutes ago?' or 'What was CPU usage 10 minutes ago?'. Data is available for up to 30 minutes in the past.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "minutes_ago": {
                    "type": "integer",
                    "description": "How many minutes ago to query (0 = current, 1-30 for historical). Default: 0"
                }
            },
            "required": []
        }),
        category: ToolCategory::System,
        example: Some("get_historical_data({\"minutes_ago\": 5})".to_string()),
    });

    tools.push(ToolDefinition {
        name: "compare_metrics".to_string(),
        description: "Compare current system metrics with metrics from a specific time in the past. Shows the change in CPU, memory, GPU temperature and utilization.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "minutes_ago": {
                    "type": "integer",
                    "description": "Compare with metrics from this many minutes ago (1-30). Default: 5"
                }
            },
            "required": []
        }),
        category: ToolCategory::System,
        example: Some("compare_metrics({\"minutes_ago\": 10})".to_string()),
    });

    tools
}

// Tool implementations for AiDataApi
impl AiDataApi {
    // ============== System Tools ==============

    pub(crate) fn tool_get_system_summary(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let summary = self.system_summary()?;
        Ok(serde_json::to_value(summary)?)
    }

    pub(crate) fn tool_get_system_info(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let info = crate::motherboard::get_system_info()
            .map_err(|e| SimonError::NotImplemented(e.to_string()))?;

        let details = SystemInfoDetails {
            hostname: info.hostname.clone().unwrap_or_default(),
            os_name: info.os_name.clone(),
            os_version: info.os_version.clone(),
            kernel_version: info.kernel_version.clone().unwrap_or_default(),
            architecture: info.architecture.clone(),
            bios_vendor: info.bios.vendor.clone(),
            bios_version: info.bios.version.clone(),
            manufacturer: info.manufacturer.clone(),
            model: info.product_name.clone(),
            uptime_seconds: 0, // Would need platform-specific impl
            boot_time: 0,      // Would need platform-specific impl
        };

        Ok(serde_json::to_value(details)?)
    }

    pub(crate) fn tool_get_platform_info(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut capabilities = serde_json::Map::new();

        capabilities.insert("os".to_string(), json!(std::env::consts::OS));
        capabilities.insert("arch".to_string(), json!(std::env::consts::ARCH));
        capabilities.insert("family".to_string(), json!(std::env::consts::FAMILY));

        // Check GPU support
        capabilities.insert("gpu_nvidia".to_string(), json!(cfg!(feature = "nvidia")));
        capabilities.insert("gpu_amd".to_string(), json!(cfg!(feature = "amd")));
        capabilities.insert("gpu_intel".to_string(), json!(cfg!(feature = "intel")));
        capabilities.insert("gpu_apple".to_string(), json!(cfg!(feature = "apple")));

        // Check what's actually available
        capabilities.insert("gpus_detected".to_string(), json!(self.gpus.is_some()));
        capabilities.insert(
            "gpu_count".to_string(),
            json!(self.gpus.as_ref().map(|g| g.device_count()).unwrap_or(0)),
        );
        capabilities.insert(
            "process_monitor".to_string(),
            json!(self.process_monitor.is_some()),
        );
        capabilities.insert(
            "network_monitor".to_string(),
            json!(self.network_monitor.is_some()),
        );

        Ok(json!(capabilities))
    }

    // ============== GPU Tools ==============

    pub(crate) fn tool_get_gpu_status(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let status: Vec<_> = snapshots
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "vendor": format!("{:?}", s.static_info.vendor),
                    "utilization_percent": s.dynamic_info.utilization,
                    "memory": {
                        "used_mb": s.dynamic_info.memory.used / 1024 / 1024,
                        "total_mb": s.dynamic_info.memory.total / 1024 / 1024,
                        "free_mb": s.dynamic_info.memory.free / 1024 / 1024,
                        "usage_percent": if s.dynamic_info.memory.total > 0 {
                            (s.dynamic_info.memory.used as f64 / s.dynamic_info.memory.total as f64) * 100.0
                        } else { 0.0 }
                    },
                    "temperature_c": s.dynamic_info.thermal.temperature,
                    "power_watts": s.dynamic_info.power.draw.map(|p| p as f32 / 1000.0),
                    "power_limit_watts": s.dynamic_info.power.limit.map(|p| p as f32 / 1000.0),
                })
            })
            .collect();

        Ok(json!({
            "gpu_count": status.len(),
            "gpus": status
        }))
    }

    pub(crate) fn tool_get_gpu_list(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let list: Vec<_> = snapshots
            .iter()
            .enumerate()
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "vendor": format!("{:?}", s.static_info.vendor),
                    "total_memory_mb": s.dynamic_info.memory.total / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_get_gpu_details(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params
            .get("gpu_index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| SimonError::InvalidArgument("gpu_index is required".to_string()))?
            as usize;

        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let info = snapshots.get(gpu_index).ok_or_else(|| {
            SimonError::InvalidArgument(format!("GPU index {} not found", gpu_index))
        })?;

        Ok(json!({
            "index": gpu_index,
            "name": info.static_info.name,
            "vendor": format!("{:?}", info.static_info.vendor),
            "pci_bus_id": info.static_info.pci_bus_id,
            "uuid": info.static_info.uuid,
            "driver_version": info.static_info.driver_version,
            "utilization": {
                "gpu_percent": info.dynamic_info.utilization,
            },
            "memory": {
                "total_bytes": info.dynamic_info.memory.total,
                "used_bytes": info.dynamic_info.memory.used,
                "free_bytes": info.dynamic_info.memory.free,
                "total_mb": info.dynamic_info.memory.total / 1024 / 1024,
                "used_mb": info.dynamic_info.memory.used / 1024 / 1024,
                "usage_percent": if info.dynamic_info.memory.total > 0 {
                    (info.dynamic_info.memory.used as f32 / info.dynamic_info.memory.total as f32) * 100.0
                } else { 0.0 }
            },
            "thermal": {
                "temperature_c": info.dynamic_info.thermal.temperature,
                "max_temperature_c": info.dynamic_info.thermal.max_temperature,
                "critical_temperature_c": info.dynamic_info.thermal.critical_temperature,
                "fan_speed_percent": info.dynamic_info.thermal.fan_speed,
                "fan_rpm": info.dynamic_info.thermal.fan_rpm,
            },
            "power": {
                "current_watts": info.dynamic_info.power.draw.map(|p| p as f32 / 1000.0),
                "limit_watts": info.dynamic_info.power.limit.map(|p| p as f32 / 1000.0),
                "default_limit_watts": info.dynamic_info.power.default_limit.map(|p| p as f32 / 1000.0),
            },
            "clocks": {
                "graphics_mhz": info.dynamic_info.clocks.graphics,
                "memory_mhz": info.dynamic_info.clocks.memory,
                "sm_mhz": info.dynamic_info.clocks.sm,
                "max_graphics_mhz": info.dynamic_info.clocks.graphics_max,
                "max_memory_mhz": info.dynamic_info.clocks.memory_max,
            },
            "processes": info.dynamic_info.processes.iter().map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "memory_bytes": p.memory_usage,
                    "gpu_usage_percent": p.gpu_usage,
                })
            }).collect::<Vec<_>>(),
        }))
    }

    pub(crate) fn tool_get_gpu_processes(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let gpu_procs = proc_mon
            .gpu_processes()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        let filtered: Vec<_> = gpu_procs
            .iter()
            .filter(|p| {
                gpu_index
                    .map(|idx| p.gpu_indices.contains(&(idx as usize)))
                    .unwrap_or(true)
            })
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "gpu_indices": p.gpu_indices,
                    "gpu_memory_bytes": p.total_gpu_memory_bytes,
                    "gpu_memory_mb": p.total_gpu_memory_bytes / 1024 / 1024,
                    "cpu_percent": p.cpu_percent,
                    "system_memory_mb": p.memory_bytes / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!({
            "count": filtered.len(),
            "processes": filtered
        }))
    }

    pub(crate) fn tool_get_gpu_utilization(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let util: Vec<_> = snapshots
            .iter()
            .enumerate()
            .filter(|(idx, _)| gpu_index.map(|i| *idx == i as usize).unwrap_or(true))
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "gpu_percent": s.dynamic_info.utilization,
                })
            })
            .collect();

        Ok(json!(util))
    }

    pub(crate) fn tool_get_gpu_memory(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let mem: Vec<_> = snapshots
            .iter()
            .enumerate()
            .filter(|(idx, _)| gpu_index.map(|i| *idx == i as usize).unwrap_or(true))
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "total_bytes": s.dynamic_info.memory.total,
                    "used_bytes": s.dynamic_info.memory.used,
                    "free_bytes": s.dynamic_info.memory.free,
                    "total_mb": s.dynamic_info.memory.total / 1024 / 1024,
                    "used_mb": s.dynamic_info.memory.used / 1024 / 1024,
                    "free_mb": s.dynamic_info.memory.free / 1024 / 1024,
                    "usage_percent": if s.dynamic_info.memory.total > 0 {
                        (s.dynamic_info.memory.used as f64 / s.dynamic_info.memory.total as f64) * 100.0
                    } else { 0.0 }
                })
            })
            .collect();

        Ok(json!(mem))
    }

    pub(crate) fn tool_get_gpu_temperature(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let temps: Vec<_> = snapshots
            .iter()
            .enumerate()
            .filter(|(idx, _)| gpu_index.map(|i| *idx == i as usize).unwrap_or(true))
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "temperature_c": s.dynamic_info.thermal.temperature,
                    "max_temperature_c": s.dynamic_info.thermal.max_temperature,
                    "critical_temperature_c": s.dynamic_info.thermal.critical_temperature,
                    "fan_speed_percent": s.dynamic_info.thermal.fan_speed,
                    "fan_rpm": s.dynamic_info.thermal.fan_rpm,
                })
            })
            .collect();

        Ok(json!(temps))
    }

    pub(crate) fn tool_get_gpu_power(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let gpus = self
            .gpus
            .as_ref()
            .ok_or_else(|| SimonError::NotImplemented("No GPUs detected".to_string()))?;

        let snapshots = gpus
            .snapshot_all()
            .map_err(|e| SimonError::GpuError(e.to_string()))?;

        let power: Vec<_> = snapshots
            .iter()
            .enumerate()
            .filter(|(idx, _)| gpu_index.map(|i| *idx == i as usize).unwrap_or(true))
            .map(|(idx, s)| {
                json!({
                    "index": idx,
                    "name": s.static_info.name,
                    "current_watts": s.dynamic_info.power.draw.map(|p| p as f32 / 1000.0),
                    "limit_watts": s.dynamic_info.power.limit.map(|p| p as f32 / 1000.0),
                    "default_limit_watts": s.dynamic_info.power.default_limit.map(|p| p as f32 / 1000.0),
                })
            })
            .collect();

        Ok(json!(power))
    }

    // ============== CPU Tools ==============

    pub(crate) fn tool_get_cpu_status(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::cpu;
            let stats = cpu::read_cpu_stats().map_err(|e| SimonError::CpuError(e.to_string()))?;

            Ok(json!({
                "core_count": stats.cores.len(),
                "total": {
                    "user_percent": stats.total.user,
                    "system_percent": stats.total.system,
                    "nice_percent": stats.total.nice,
                    "idle_percent": stats.total.idle,
                    "usage_percent": 100.0 - stats.total.idle,
                },
                "model": stats.cores.first().map(|c| &c.model).cloned().unwrap_or_default(),
            }))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "CPU status not implemented for this platform".to_string(),
            ))
        }
    }

    pub(crate) fn tool_get_cpu_cores(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::cpu;
            let stats = cpu::read_cpu_stats().map_err(|e| SimonError::CpuError(e.to_string()))?;

            let cores: Vec<_> = stats
                .cores
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "online": c.online,
                        "governor": c.governor,
                        "model": c.model,
                        "frequency_mhz": c.frequency.as_ref().map(|f| f.current),
                        "user_percent": c.user,
                        "system_percent": c.system,
                        "idle_percent": c.idle,
                    })
                })
                .collect();

            Ok(json!(cores))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "CPU cores not implemented for this platform".to_string(),
            ))
        }
    }

    pub(crate) fn tool_get_cpu_frequency(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::cpu;
            let stats = cpu::read_cpu_stats().map_err(|e| SimonError::CpuError(e.to_string()))?;

            let freqs: Vec<_> = stats
                .cores
                .iter()
                .filter_map(|c| {
                    c.frequency.as_ref().map(|f| {
                        json!({
                            "core_id": c.id,
                            "current_mhz": f.current,
                            "min_mhz": f.min,
                            "max_mhz": f.max,
                        })
                    })
                })
                .collect();

            Ok(json!(freqs))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "CPU frequency not implemented for this platform".to_string(),
            ))
        }
    }

    // ============== Memory Tools ==============

    pub(crate) fn tool_get_memory_status(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::memory;
            let stats =
                memory::read_memory_stats().map_err(|e| SimonError::MemoryError(e.to_string()))?;

            Ok(json!({
                "ram": {
                    "total_mb": stats.ram.total / 1024,
                    "used_mb": stats.ram.used / 1024,
                    "free_mb": stats.ram.free / 1024,
                    "usage_percent": if stats.ram.total > 0 {
                        (stats.ram.used as f64 / stats.ram.total as f64) * 100.0
                    } else { 0.0 },
                },
                "swap": {
                    "total_mb": stats.swap.total / 1024,
                    "used_mb": stats.swap.used / 1024,
                    "usage_percent": if stats.swap.total > 0 {
                        (stats.swap.used as f64 / stats.swap.total as f64) * 100.0
                    } else { 0.0 },
                }
            }))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "Memory status not implemented for this platform".to_string(),
            ))
        }
    }

    pub(crate) fn tool_get_memory_breakdown(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::memory;
            let stats =
                memory::read_memory_stats().map_err(|e| SimonError::MemoryError(e.to_string()))?;

            Ok(json!({
                "total_kb": stats.ram.total,
                "used_kb": stats.ram.used,
                "free_kb": stats.ram.free,
                "buffers_kb": stats.ram.buffers,
                "cached_kb": stats.ram.cached,
                "shared_kb": stats.ram.shared,
                "total_mb": stats.ram.total / 1024,
                "used_mb": stats.ram.used / 1024,
                "free_mb": stats.ram.free / 1024,
                "buffers_mb": stats.ram.buffers / 1024,
                "cached_mb": stats.ram.cached / 1024,
            }))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "Memory breakdown not implemented for this platform".to_string(),
            ))
        }
    }

    pub(crate) fn tool_get_swap_status(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::memory;
            let stats =
                memory::read_memory_stats().map_err(|e| SimonError::MemoryError(e.to_string()))?;

            Ok(json!({
                "total_kb": stats.swap.total,
                "used_kb": stats.swap.used,
                "cached_kb": stats.swap.cached,
                "total_mb": stats.swap.total / 1024,
                "used_mb": stats.swap.used / 1024,
                "usage_percent": if stats.swap.total > 0 {
                    (stats.swap.used as f64 / stats.swap.total as f64) * 100.0
                } else { 0.0 },
            }))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err(SimonError::NotImplemented(
                "Swap status not implemented for this platform".to_string(),
            ))
        }
    }

    // ============== Disk Tools ==============

    pub(crate) fn tool_get_disk_list(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let disks =
            crate::disk::enumerate_disks().map_err(|e| SimonError::DiskError(e.to_string()))?;

        let list: Vec<_> = disks
            .iter()
            .filter_map(|d| {
                d.info().ok().map(|info| {
                    json!({
                        "name": info.name,
                        "model": info.model,
                        "size_gb": info.capacity / 1024 / 1024 / 1024,
                        "disk_type": format!("{:?}", info.disk_type),
                    })
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_get_disk_details(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let disk_name = params
            .get("disk_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SimonError::InvalidArgument("disk_name is required".to_string()))?;

        let disks =
            crate::disk::enumerate_disks().map_err(|e| SimonError::DiskError(e.to_string()))?;

        let disk = disks
            .iter()
            .find(|d| {
                d.info()
                    .map(|i| i.name.contains(disk_name))
                    .unwrap_or(false)
            })
            .ok_or_else(|| SimonError::InvalidArgument(format!("Disk {} not found", disk_name)))?;

        let info = disk
            .info()
            .map_err(|e| SimonError::DiskError(e.to_string()))?;

        Ok(json!({
            "name": info.name,
            "model": info.model,
            "serial": info.serial,
            "firmware": info.firmware,
            "size_bytes": info.capacity,
            "size_gb": info.capacity / 1024 / 1024 / 1024,
            "disk_type": format!("{:?}", info.disk_type),
            "temperature_c": disk.temperature().ok().flatten(),
        }))
    }

    pub(crate) fn tool_get_disk_io(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let disk_name = params.get("disk_name").and_then(|v| v.as_str());

        let disks =
            crate::disk::enumerate_disks().map_err(|e| SimonError::DiskError(e.to_string()))?;

        let io_stats: Vec<_> = disks
            .iter()
            .filter(|d| {
                disk_name
                    .map(|name| d.info().map(|i| i.name.contains(name)).unwrap_or(false))
                    .unwrap_or(true)
            })
            .filter_map(|d| {
                let info = d.info().ok()?;
                let io = d.io_stats().ok()?;
                Some(json!({
                    "name": info.name,
                    "bytes_read": io.read_bytes,
                    "bytes_written": io.write_bytes,
                    "read_ops": io.read_ops,
                    "write_ops": io.write_ops,
                }))
            })
            .collect();

        Ok(json!(io_stats))
    }

    pub(crate) fn tool_get_disk_health(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let disk_name = params.get("disk_name").and_then(|v| v.as_str());

        let disks =
            crate::disk::enumerate_disks().map_err(|e| SimonError::DiskError(e.to_string()))?;

        let health: Vec<_> = disks
            .iter()
            .filter(|d| {
                disk_name
                    .map(|name| d.info().map(|i| i.name.contains(name)).unwrap_or(false))
                    .unwrap_or(true)
            })
            .filter_map(|d| {
                let info = d.info().ok()?;
                let h = d.health().ok()?;
                Some(json!({
                    "name": info.name,
                    "overall_health": format!("{:?}", h),
                    "temperature_c": d.temperature().ok().flatten(),
                }))
            })
            .collect();

        Ok(json!(health))
    }

    // ============== Network Tools ==============

    pub(crate) fn tool_get_network_interfaces(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let active_only = params
            .get("active_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let net_mon = self.network_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Network monitor not available".to_string())
        })?;

        let interfaces = if active_only {
            net_mon
                .active_interfaces()
                .map_err(|e| SimonError::NetworkError(e.to_string()))?
        } else {
            net_mon
                .interfaces()
                .map_err(|e| SimonError::NetworkError(e.to_string()))?
        };

        let list: Vec<_> = interfaces
            .iter()
            .map(|i| {
                json!({
                    "name": i.name,
                    "is_up": i.is_up,
                    "is_running": i.is_running,
                    "rx_bytes": i.rx_bytes,
                    "tx_bytes": i.tx_bytes,
                    "rx_mb": i.rx_bytes / 1024 / 1024,
                    "tx_mb": i.tx_bytes / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_get_network_bandwidth(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let interface_name = params.get("interface_name").and_then(|v| v.as_str());

        let net_mon = self.network_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Network monitor not available".to_string())
        })?;

        let interfaces = net_mon
            .active_interfaces()
            .map_err(|e| SimonError::NetworkError(e.to_string()))?;

        let bandwidth: Vec<_> = interfaces
            .iter()
            .filter(|i| interface_name.map(|name| i.name == name).unwrap_or(true))
            .map(|i| {
                let (rx_rate, tx_rate) = net_mon.bandwidth_rate(&i.name, i);
                json!({
                    "name": i.name,
                    "rx_bytes_per_sec": rx_rate,
                    "tx_bytes_per_sec": tx_rate,
                    "rx_mbps": rx_rate / 1_000_000.0 * 8.0,
                    "tx_mbps": tx_rate / 1_000_000.0 * 8.0,
                })
            })
            .collect();

        Ok(json!(bandwidth))
    }

    pub(crate) fn tool_get_interface_details(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let interface_name = params
            .get("interface_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SimonError::InvalidArgument("interface_name is required".to_string()))?;

        let net_mon = self.network_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Network monitor not available".to_string())
        })?;

        let iface = net_mon
            .interface_by_name(interface_name)
            .map_err(|e| SimonError::NetworkError(e.to_string()))?
            .ok_or_else(|| {
                SimonError::InvalidArgument(format!("Interface {} not found", interface_name))
            })?;

        let (rx_rate, tx_rate) = net_mon.bandwidth_rate(&iface.name, &iface);

        Ok(json!({
            "name": iface.name,
            "is_up": iface.is_up,
            "is_running": iface.is_running,
            "rx_bytes": iface.rx_bytes,
            "tx_bytes": iface.tx_bytes,
            "rx_packets": iface.rx_packets,
            "tx_packets": iface.tx_packets,
            "rx_errors": iface.rx_errors,
            "tx_errors": iface.tx_errors,
            "rx_drops": iface.rx_drops,
            "tx_drops": iface.tx_drops,
            "rx_rate_bytes_sec": rx_rate,
            "tx_rate_bytes_sec": tx_rate,
        }))
    }

    // ============== Process Tools ==============

    pub(crate) fn tool_get_process_list(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        let sort_by = params
            .get("sort_by")
            .and_then(|v| v.as_str())
            .unwrap_or("cpu");

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let mut procs = proc_mon
            .processes()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        // Sort
        match sort_by {
            "cpu" => procs.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "memory" => procs.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
            "gpu_memory" => {
                procs.sort_by(|a, b| b.total_gpu_memory_bytes.cmp(&a.total_gpu_memory_bytes))
            }
            "name" => procs.sort_by(|a, b| a.name.cmp(&b.name)),
            "pid" => procs.sort_by(|a, b| a.pid.cmp(&b.pid)),
            _ => {}
        }

        let list: Vec<_> = procs
            .iter()
            .take(limit)
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "cpu_percent": p.cpu_percent,
                    "memory_mb": p.memory_bytes / 1024 / 1024,
                    "gpu_memory_mb": p.total_gpu_memory_bytes / 1024 / 1024,
                    "thread_count": p.thread_count,
                    "state": p.state,
                })
            })
            .collect();

        Ok(json!({
            "total_count": procs.len(),
            "returned_count": list.len(),
            "sort_by": sort_by,
            "processes": list
        }))
    }

    pub(crate) fn tool_get_process_details(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let pid = params
            .get("pid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| SimonError::InvalidArgument("pid is required".to_string()))?
            as u32;

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let proc = proc_mon
            .process_by_pid(pid)
            .map_err(|e| SimonError::ProcessError(e.to_string()))?
            .ok_or_else(|| SimonError::InvalidArgument(format!("Process {} not found", pid)))?;

        Ok(json!({
            "pid": proc.pid,
            "name": proc.name,
            "state": proc.state,
            "parent_pid": proc.parent_pid,
            "cpu_percent": proc.cpu_percent,
            "memory_bytes": proc.memory_bytes,
            "memory_mb": proc.memory_bytes / 1024 / 1024,
            "virtual_memory_bytes": proc.virtual_memory_bytes,
            "thread_count": proc.thread_count,
            "gpu_indices": proc.gpu_indices,
            "gpu_memory_bytes": proc.total_gpu_memory_bytes,
            "gpu_memory_mb": proc.total_gpu_memory_bytes / 1024 / 1024,
        }))
    }

    pub(crate) fn tool_get_top_cpu_processes(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let top = proc_mon
            .processes_by_cpu()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        let list: Vec<_> = top
            .iter()
            .take(count)
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "cpu_percent": p.cpu_percent,
                    "memory_mb": p.memory_bytes / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_get_top_memory_processes(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let top = proc_mon
            .processes_by_memory()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        let list: Vec<_> = top
            .iter()
            .take(count)
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "memory_bytes": p.memory_bytes,
                    "memory_mb": p.memory_bytes / 1024 / 1024,
                    "cpu_percent": p.cpu_percent,
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_get_top_gpu_processes(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let count = params.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let gpu_index = params.get("gpu_index").and_then(|v| v.as_u64());

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let top = proc_mon
            .processes_by_gpu_memory()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        let list: Vec<_> = top
            .iter()
            .filter(|p| {
                gpu_index
                    .map(|idx| p.gpu_indices.contains(&(idx as usize)))
                    .unwrap_or(true)
            })
            .take(count)
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "gpu_indices": p.gpu_indices,
                    "gpu_memory_bytes": p.total_gpu_memory_bytes,
                    "gpu_memory_mb": p.total_gpu_memory_bytes / 1024 / 1024,
                    "cpu_percent": p.cpu_percent,
                    "system_memory_mb": p.memory_bytes / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!(list))
    }

    pub(crate) fn tool_search_processes(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SimonError::InvalidArgument("pattern is required".to_string()))?
            .to_lowercase();

        let proc_mon = self.process_monitor.as_mut().ok_or_else(|| {
            SimonError::NotImplemented("Process monitor not available".to_string())
        })?;

        let procs = proc_mon
            .processes()
            .map_err(|e| SimonError::ProcessError(e.to_string()))?;

        let matches: Vec<_> = procs
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&pattern))
            .map(|p| {
                json!({
                    "pid": p.pid,
                    "name": p.name,
                    "cpu_percent": p.cpu_percent,
                    "memory_mb": p.memory_bytes / 1024 / 1024,
                    "gpu_memory_mb": p.total_gpu_memory_bytes / 1024 / 1024,
                })
            })
            .collect();

        Ok(json!({
            "pattern": pattern,
            "match_count": matches.len(),
            "processes": matches
        }))
    }

    // ============== Hardware Tools ==============

    pub(crate) fn tool_get_motherboard_sensors(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let sensors = crate::motherboard::enumerate_sensors()
            .map_err(|e| SimonError::HardwareError(e.to_string()))?;

        let mut readings = Vec::new();

        // Collect temperature sensors
        for sensor in &sensors {
            if let Ok(temps) = sensor.temperature_sensors() {
                for t in temps {
                    readings.push(json!({
                        "name": t.label,
                        "sensor_type": "temperature",
                        "value": t.temperature,
                        "unit": "°C",
                        "max": t.max,
                        "critical": t.critical,
                    }));
                }
            }

            // Collect voltage rails
            if let Ok(volts) = sensor.voltage_rails() {
                for v in volts {
                    readings.push(json!({
                        "name": v.label,
                        "sensor_type": "voltage",
                        "value": v.voltage,
                        "unit": "V",
                        "min": v.min,
                        "max": v.max,
                    }));
                }
            }

            // Collect fan info
            if let Ok(fans) = sensor.fans() {
                for f in fans {
                    readings.push(json!({
                        "name": f.label,
                        "sensor_type": "fan",
                        "value": f.rpm,
                        "unit": "RPM",
                        "min": f.min_rpm,
                        "max": f.max_rpm,
                    }));
                }
            }
        }

        Ok(json!(readings))
    }

    pub(crate) fn tool_get_system_temperatures(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let sensors = crate::motherboard::enumerate_sensors()
            .map_err(|e| SimonError::HardwareError(e.to_string()))?;

        let mut temps = Vec::new();

        // Get temperatures from motherboard sensors
        for sensor in &sensors {
            if let Ok(temp_sensors) = sensor.temperature_sensors() {
                for t in temp_sensors {
                    temps.push(json!({
                        "name": t.label,
                        "temperature_c": t.temperature,
                        "max_c": t.max,
                        "critical_c": t.critical,
                        "sensor_type": format!("{:?}", t.sensor_type),
                    }));
                }
            }
        }

        // Add GPU temperatures
        if let Some(ref gpus) = self.gpus {
            if let Ok(snapshots) = gpus.snapshot_all() {
                for (idx, s) in snapshots.iter().enumerate() {
                    if let Some(temp) = s.dynamic_info.thermal.temperature {
                        temps.push(json!({
                            "name": format!("GPU {} ({})", idx, s.static_info.name),
                            "temperature_c": temp,
                        }));
                    }
                }
            }
        }

        // Add disk temperatures
        if let Ok(disks) = crate::disk::enumerate_disks() {
            for disk in &disks {
                if let (Ok(info), Ok(Some(temp))) = (disk.info(), disk.temperature()) {
                    temps.push(json!({
                        "name": format!("Disk ({})", info.name),
                        "temperature_c": temp,
                    }));
                }
            }
        }

        Ok(json!(temps))
    }

    pub(crate) fn tool_get_fan_speeds(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let sensors = crate::motherboard::enumerate_sensors()
            .map_err(|e| SimonError::HardwareError(e.to_string()))?;

        let mut fans = Vec::new();

        for sensor in &sensors {
            if let Ok(fan_info) = sensor.fans() {
                for f in fan_info {
                    fans.push(json!({
                        "name": f.label,
                        "rpm": f.rpm,
                        "pwm": f.pwm,
                        "controllable": f.controllable,
                    }));
                }
            }
        }

        Ok(json!(fans))
    }

    pub(crate) fn tool_get_voltage_rails(
        &mut self,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let sensors = crate::motherboard::enumerate_sensors()
            .map_err(|e| SimonError::HardwareError(e.to_string()))?;

        let mut voltages = Vec::new();

        for sensor in &sensors {
            if let Ok(voltage_rails) = sensor.voltage_rails() {
                for v in voltage_rails {
                    voltages.push(json!({
                        "name": v.label,
                        "voltage_v": v.voltage,
                        "min_v": v.min,
                        "max_v": v.max,
                    }));
                }
            }
        }

        Ok(json!(voltages))
    }

    pub(crate) fn tool_get_driver_info(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let driver_type = params
            .get("driver_type")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let drivers = crate::motherboard::get_driver_versions()
            .map_err(|e| SimonError::HardwareError(e.to_string()))?;

        let filtered: Vec<_> = drivers
            .iter()
            .filter(|d| {
                driver_type == "all"
                    || matches!(
                        (&d.driver_type, driver_type),
                        (crate::motherboard::DriverType::Gpu, "gpu")
                            | (crate::motherboard::DriverType::Network, "network")
                            | (crate::motherboard::DriverType::Storage, "storage")
                    )
            })
            .map(|d| {
                json!({
                    "name": d.name,
                    "driver_type": format!("{:?}", d.driver_type),
                    "version": d.version,
                    "vendor": d.vendor,
                    "date": d.date,
                })
            })
            .collect();

        Ok(json!(filtered))
    }

    // ============== Audio Tools ==============
    pub(crate) fn tool_get_audio_devices(&mut self, params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::audio::{AudioDeviceType, AudioMonitor};
        let device_type = params.get("device_type").and_then(|v| v.as_str()).unwrap_or("all");
        let monitor = AudioMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let devices: Vec<_> = monitor.devices().iter()
            .filter(|d| device_type == "all" || matches!((&d.device_type, device_type), (AudioDeviceType::Output, "output") | (AudioDeviceType::Input, "input") | (AudioDeviceType::Duplex, "output") | (AudioDeviceType::Duplex, "input")))
            .map(|d| json!({"id": d.id, "name": d.name, "type": format!("{:?}", d.device_type), "state": format!("{:?}", d.state), "is_default": d.is_default, "volume": d.volume, "muted": d.muted}))
            .collect();
        Ok(json!(devices))
    }
    pub(crate) fn tool_get_audio_status(&mut self, _params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::audio::AudioMonitor;
        let monitor = AudioMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        Ok(json!({"master_volume": monitor.master_volume(), "is_muted": monitor.is_muted(), "device_count": monitor.devices().len()}))
    }
    // ============== Bluetooth Tools ==============
    pub(crate) fn tool_get_bluetooth_adapters(&mut self, _params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::bluetooth::BluetoothMonitor;
        let monitor = BluetoothMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let adapters: Vec<_> = monitor.adapters().iter().map(|a| json!({"id": a.id, "name": a.name, "address": a.address, "powered": a.powered})).collect();
        Ok(json!({"available": monitor.is_available(), "adapters": adapters}))
    }
    pub(crate) fn tool_get_bluetooth_devices(&mut self, params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::bluetooth::{BluetoothMonitor, BluetoothState};
        let connected_only = params.get("connected_only").and_then(|v| v.as_bool()).unwrap_or(false);
        let monitor = BluetoothMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let devices: Vec<_> = monitor.devices().iter().filter(|d| !connected_only || d.state == BluetoothState::Connected).map(|d| json!({"address": d.address, "name": d.name, "type": format!("{:?}", d.device_type), "state": format!("{:?}", d.state), "battery_percent": d.battery_percent})).collect();
        Ok(json!(devices))
    }
    // ============== Display Tools ==============
    pub(crate) fn tool_get_display_list(&mut self, _params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::display::DisplayMonitor;
        let monitor = DisplayMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let displays: Vec<_> = monitor.displays().iter().map(|d| json!({"id": d.id, "name": d.name, "manufacturer": d.manufacturer, "connection": format!("{:?}", d.connection), "is_primary": d.is_primary, "resolution": format!("{}x{}", d.width, d.height), "aspect_ratio": d.aspect_ratio(), "refresh_rate": d.refresh_rate, "brightness": d.brightness, "hdr": format!("{:?}", d.hdr)})).collect();
        Ok(json!({"count": monitor.count(), "displays": displays}))
    }
    pub(crate) fn tool_get_display_details(&mut self, params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::display::DisplayMonitor;
        let display_id = params.get("display_id").and_then(|v| v.as_str()).ok_or_else(|| SimonError::InvalidArgument("display_id required".to_string()))?;
        let monitor = DisplayMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let display = monitor.displays().iter().find(|d| d.id == display_id).ok_or_else(|| SimonError::DeviceNotFound(format!("Display {} not found", display_id)))?;
        Ok(json!({"id": display.id, "name": display.name, "manufacturer": display.manufacturer, "connection": format!("{:?}", display.connection), "is_primary": display.is_primary, "width": display.width, "height": display.height, "resolution": format!("{}x{}", display.width, display.height), "aspect_ratio": display.aspect_ratio(), "refresh_rate": display.refresh_rate, "brightness": display.brightness, "hdr": format!("{:?}", display.hdr)}))
    }
    // ============== USB Tools ==============
    pub(crate) fn tool_get_usb_devices(&mut self, params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::usb::{UsbDeviceClass, UsbMonitor};
        let class_filter = params.get("class").and_then(|v| v.as_str()).unwrap_or("all");
        let monitor = UsbMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let devices: Vec<_> = monitor.devices().iter().filter(|d| class_filter == "all" || matches!((&d.class, class_filter), (UsbDeviceClass::Audio, "audio") | (UsbDeviceClass::Hid, "hid") | (UsbDeviceClass::MassStorage, "storage") | (UsbDeviceClass::Hub, "hub") | (UsbDeviceClass::Video, "video"))).map(|d| json!({"bus": d.bus_number, "port": d.port_number, "vendor_id": format!("{:04x}", d.vendor_id), "product_id": format!("{:04x}", d.product_id), "vendor_name": d.manufacturer, "product_name": d.product, "class": format!("{:?}", d.class), "speed": format!("{:?}", d.speed)})).collect();
        Ok(json!(devices))
    }
    pub(crate) fn tool_get_usb_device_details(&mut self, params: serde_json::Value) -> Result<serde_json::Value> {
        use crate::usb::UsbMonitor;
        let bus = params.get("bus").and_then(|v| v.as_u64()).ok_or_else(|| SimonError::InvalidArgument("bus required".to_string()))? as u8;
        let address = params.get("address").and_then(|v| v.as_u64()).ok_or_else(|| SimonError::InvalidArgument("address required".to_string()))? as u8;
        let monitor = UsbMonitor::new().map_err(|e| SimonError::HardwareError(e.to_string()))?;
        let device = monitor.devices().iter().find(|d| d.bus_number == bus && d.port_number == address).ok_or_else(|| SimonError::DeviceNotFound(format!("USB device at bus {} address {} not found", bus, address)))?;
        Ok(json!({"bus": device.bus_number, "port": device.port_number, "vendor_id": format!("{:04x}", device.vendor_id), "product_id": format!("{:04x}", device.product_id), "vendor_name": device.manufacturer, "product_name": device.product, "class": format!("{:?}", device.class), "speed": format!("{:?}", device.speed)}))
    }

}

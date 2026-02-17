// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! Monitoring daemon for headless/remote operation
//!
//! Runs simon as a background service with HTTP API, Prometheus metrics,
//! and optional fleet push reporting.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Already running (PID file exists): {0}")]
    AlreadyRunning(String),
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self { LogLevel::Info }
}

/// Fleet push configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetPushConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub host_id: String,
    pub interval_secs: u64,
    pub tags: std::collections::HashMap<String, String>,
}

/// Daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub host: String,
    pub port: u16,
    pub poll_interval_secs: u64,
    pub pid_file: Option<String>,
    pub log_level: LogLevel,
    pub enable_prometheus: bool,
    pub enable_rest_api: bool,
    pub fleet: Option<FleetPushConfig>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".into(),
            port: 9100,
            poll_interval_secs: 5,
            pid_file: None,
            log_level: LogLevel::Info,
            enable_prometheus: true,
            enable_rest_api: true,
            fleet: None,
        }
    }
}

impl DaemonConfig {
    /// Load from TOML file
    pub fn from_toml_file(path: &str) -> Result<Self, DaemonError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DaemonError::Config(format!("Cannot read {}: {}", path, e)))?;
        Self::from_toml(&content)
    }

    /// Parse from TOML string
    pub fn from_toml(content: &str) -> Result<Self, DaemonError> {
        toml::from_str(content)
            .map_err(|e| DaemonError::Config(format!("TOML parse error: {}", e)))
    }

    /// Generate sample config
    pub fn sample_toml() -> String {
        r#"# Simon Monitoring Daemon Configuration
host = "0.0.0.0"
port = 9100
poll_interval_secs = 5
# pid_file = "/var/run/simon.pid"
log_level = "Info"
enable_prometheus = true
enable_rest_api = true

# Optional: Fleet push reporting
# [fleet]
# enabled = true
# endpoint = "http://fleet-server:9200/api/v1/metrics"
# host_id = "host-001"
# interval_secs = 30
# [fleet.tags]
# environment = "production"
# datacenter = "us-east-1"
# rack = "rack-42"
"#.into()
    }
}

/// Monitoring daemon
pub struct MonitoringDaemon {
    config: DaemonConfig,
}

impl MonitoringDaemon {
    pub fn new(config: DaemonConfig) -> Self {
        Self { config }
    }

    /// Load from config file
    pub fn from_config_file(path: &str) -> Result<Self, DaemonError> {
        let config = DaemonConfig::from_toml_file(path)?;
        Ok(Self::new(config))
    }

    /// Get daemon configuration
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// Write PID file
    pub fn write_pid_file(&self) -> Result<(), DaemonError> {
        if let Some(ref pid_path) = self.config.pid_file {
            // Check if already running
            if std::path::Path::new(pid_path).exists() {
                let existing = std::fs::read_to_string(pid_path).unwrap_or_default();
                if !existing.trim().is_empty() {
                    return Err(DaemonError::AlreadyRunning(pid_path.clone()));
                }
            }
            let pid = std::process::id();
            std::fs::write(pid_path, pid.to_string())?;
        }
        Ok(())
    }

    /// Remove PID file
    pub fn remove_pid_file(&self) {
        if let Some(ref pid_path) = self.config.pid_file {
            let _ = std::fs::remove_file(pid_path);
        }
    }

    /// Get listen address
    pub fn listen_address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// Check if prometheus endpoint is enabled
    pub fn prometheus_enabled(&self) -> bool {
        self.config.enable_prometheus
    }

    /// Check if REST API is enabled
    pub fn rest_api_enabled(&self) -> bool {
        self.config.enable_rest_api
    }

    /// Check if fleet push is enabled
    pub fn fleet_push_enabled(&self) -> bool {
        self.config.fleet.as_ref().map(|f| f.enabled).unwrap_or(false)
    }
}

impl Drop for MonitoringDaemon {
    fn drop(&mut self) {
        self.remove_pid_file();
    }
}

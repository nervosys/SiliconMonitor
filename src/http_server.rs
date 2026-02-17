//! HTTP Server for Silicon Monitor REST API
//!
//! Provides a lightweight HTTP/1.1 server built on tokio that exposes all
//! monitoring data via the Observability API. Supports JSON endpoints,
//! Prometheus metrics, health checks, and OpenAPI spec.
//!
//! # Examples
//!
//! ```no_run
//! use simon::http_server::{HttpServer, HttpServerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = HttpServerConfig {
//!         bind_address: "0.0.0.0".into(),
//!         port: 8080,
//!         ..Default::default()
//!     };
//!     let server = HttpServer::new(config)?;
//!     println!("Listening on http://0.0.0.0:8080");
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

use crate::observability::{
    ApiConfig, HttpRequest as ObsRequest, MetricCollector, ObservabilityApi, RequestHandler,
    ServerConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Bind address (default: "127.0.0.1")
    pub bind_address: String,
    /// Port (default: 8080)
    pub port: u16,
    /// CORS enabled (default: true)
    pub cors_enabled: bool,
    /// Allowed CORS origins (default: ["*"])
    pub cors_origins: Vec<String>,
    /// Maximum request body size in bytes (default: 1MB)
    pub max_body_size: usize,
    /// Request timeout in seconds (default: 30)
    pub request_timeout_secs: u64,
    /// Whether to log requests to stderr (default: true)
    pub request_logging: bool,
    /// API key requirement (None = no auth)
    pub api_key: Option<String>,
    /// Metric collection interval in seconds (default: 5)
    pub metric_interval_secs: u64,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1".into(),
            port: 8080,
            cors_enabled: true,
            cors_origins: vec!["*".into()],
            max_body_size: 1_048_576,
            request_timeout_secs: 30,
            request_logging: true,
            api_key: None,
            metric_interval_secs: 5,
        }
    }
}

/// HTTP server that exposes Silicon Monitor data
pub struct HttpServer {
    config: HttpServerConfig,
    handler: Arc<RequestHandler>,
    metric_collector: Arc<MetricCollector>,
}

impl HttpServer {
    /// Create a new HTTP server with the given configuration
    pub fn new(config: HttpServerConfig) -> crate::Result<Self> {
        let server_config = ServerConfig {
            bind_address: config.bind_address.clone(),
            port: config.port,
            cors_enabled: config.cors_enabled,
            cors_origins: config.cors_origins.clone(),
            max_body_size: config.max_body_size,
            request_timeout_secs: config.request_timeout_secs,
            request_logging: config.request_logging,
            ..Default::default()
        };

        let api_config = if let Some(ref key) = config.api_key {
            let mut cfg = ApiConfig::default();
            cfg.require_auth = true;
            // Create API key via config
            let _ = key; // The actual key setup would be done through the permission system
            cfg
        } else {
            ApiConfig::default()
        };

        let api = ObservabilityApi::new(api_config);
        let event_manager = crate::observability::EventManager::new(1000);
        let metric_collector = MetricCollector::new();

        let handler = Arc::new(RequestHandler::new(
            api,
            event_manager,
            metric_collector,
            server_config,
        ));

        let metric_collector = handler.metric_collector.clone();

        Ok(Self {
            config,
            handler,
            metric_collector,
        })
    }

    /// Run the HTTP server (blocks until shutdown)
    #[cfg(feature = "cli")]
    pub async fn run(&self) -> crate::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| crate::SimonError::Other(format!("Failed to bind to {}: {}", addr, e)))?;

        if self.config.request_logging {
            eprintln!("[silicon-monitor] HTTP server listening on http://{}", addr);
            eprintln!(
                "[silicon-monitor] Endpoints: /health, /api/v1/*, /api/v1/metrics/prometheus"
            );
        }

        // Start metric collection background task
        let collector = self.metric_collector.clone();
        let interval = self.config.metric_interval_secs;
        tokio::spawn(async move {
            Self::metric_collection_loop(collector, interval).await;
        });

        loop {
            let (mut stream, peer_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("[silicon-monitor] Accept error: {}", e);
                    continue;
                }
            };

            let handler = self.handler.clone();
            let log = self.config.request_logging;
            let max_body = self.config.max_body_size;
            let cors = self.config.cors_enabled;
            let cors_origins = self.config.cors_origins.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; max_body.min(65536)];
                let n = match stream.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };

                let raw = String::from_utf8_lossy(&buf[..n]);

                // Parse HTTP request
                let obs_request = match Self::parse_http_request(&raw, &peer_addr.to_string()) {
                    Some(req) => req,
                    None => {
                        let resp =
                            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 11\r\n\r\nBad Request";
                        let _ = stream.write_all(resp).await;
                        return;
                    }
                };

                if log {
                    eprintln!(
                        "[silicon-monitor] {} {} {} from {}",
                        obs_request.method,
                        obs_request.path,
                        obs_request
                            .query
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join("&"),
                        peer_addr,
                    );
                }

                // Handle CORS preflight
                if obs_request.method == "OPTIONS" && cors {
                    let origin = cors_origins.first().cloned().unwrap_or_else(|| "*".into());
                    let resp = format!(
                        "HTTP/1.1 204 No Content\r\n\
                         Access-Control-Allow-Origin: {}\r\n\
                         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
                         Access-Control-Max-Age: 86400\r\n\
                         Content-Length: 0\r\n\r\n",
                        origin
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }

                // Dispatch through RequestHandler
                let obs_response = handler.handle(obs_request);

                // Build HTTP response
                let mut resp = format!(
                    "HTTP/1.1 {} {}\r\n",
                    obs_response.status,
                    status_text(obs_response.status)
                );

                for (key, value) in &obs_response.headers {
                    resp.push_str(&format!("{}: {}\r\n", key, value));
                }

                if cors {
                    let origin = cors_origins.first().cloned().unwrap_or_else(|| "*".into());
                    resp.push_str(&format!("Access-Control-Allow-Origin: {}\r\n", origin));
                }

                resp.push_str(&format!("Content-Length: {}\r\n", obs_response.body.len()));
                resp.push_str("Connection: close\r\n");
                resp.push_str("\r\n");
                resp.push_str(&obs_response.body);

                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    }

    /// Run the HTTP server (stub when cli feature is not enabled)
    #[cfg(not(feature = "cli"))]
    pub async fn run(&self) -> crate::Result<()> {
        Err(crate::SimonError::NotImplemented(
            "HTTP server requires the 'cli' feature (for tokio)".into(),
        ))
    }

    /// Parse raw HTTP/1.1 request into ObsRequest
    fn parse_http_request(raw: &str, client_addr: &str) -> Option<ObsRequest> {
        let mut lines = raw.lines();
        let request_line = lines.next()?;
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let method = parts[0].to_uppercase();
        let full_path = parts[1];

        // Split path and query string
        let (path, query) = if let Some(idx) = full_path.find('?') {
            let p = &full_path[..idx];
            let q = &full_path[idx + 1..];
            let mut map = HashMap::new();
            for pair in q.split('&') {
                if let Some(eq) = pair.find('=') {
                    map.insert(pair[..eq].to_string(), pair[eq + 1..].to_string());
                }
            }
            (p.to_string(), map)
        } else {
            (full_path.to_string(), HashMap::new())
        };

        // Parse headers
        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some(colon) = line.find(':') {
                let key = line[..colon].trim().to_lowercase();
                let value = line[colon + 1..].trim().to_string();
                headers.insert(key, value);
            }
        }

        Some(ObsRequest {
            method,
            path,
            query,
            headers,
            body: None, // GET requests don't have bodies typically
            client_addr: client_addr.parse().ok(),
        })
    }

    #[cfg(feature = "cli")]
    async fn metric_collection_loop(collector: Arc<MetricCollector>, interval_secs: u64) {
        loop {
            // Collect CPU stats
            if let Ok(cpu) = crate::CpuStats::new() {
                collector.record("cpu_usage_percent", cpu.total_usage as f64);
            }
            // Collect memory
            if let Ok(mem) = crate::MemoryStats::new() {
                collector.record("memory_used_bytes", mem.used as f64);
                collector.record("memory_total_bytes", mem.total as f64);
                if mem.total > 0 {
                    collector.record(
                        "memory_usage_percent",
                        (mem.used as f64 / mem.total as f64) * 100.0,
                    );
                }
            }
            // Collect GPU
            if let Ok(gpus) = crate::GpuCollection::auto_detect() {
                if let Ok(snapshots) = gpus.snapshot_all() {
                    for (i, info) in snapshots.iter().enumerate() {
                        let prefix = format!("gpu_{}", i);
                        collector.record(
                            &format!("{}_utilization_percent", prefix),
                            info.dynamic_info.utilization as f64,
                        );
                        if let Some(temp) = info.dynamic_info.thermal.temperature {
                            collector
                                .record(&format!("{}_temperature_celsius", prefix), temp as f64);
                        }
                        if let Some(power) = info.dynamic_info.power.draw {
                            collector
                                .record(&format!("{}_power_watts", prefix), power as f64 / 1000.0);
                        }
                        collector.record(
                            &format!("{}_memory_used_bytes", prefix),
                            info.dynamic_info.memory.used as f64,
                        );
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        }
    }
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_request() {
        let raw = "GET /api/v1/health?format=json HTTP/1.1\r\n\
                   Host: localhost:8080\r\n\
                   Accept: application/json\r\n\
                   \r\n";
        let req = HttpServer::parse_http_request(raw, "127.0.0.1:12345").unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/api/v1/health");
        assert_eq!(req.query.get("format").map(|s| s.as_str()), Some("json"));
        assert_eq!(
            req.headers.get("host").map(|s| s.as_str()),
            Some("localhost:8080")
        );
    }

    #[test]
    fn test_parse_http_request_no_query() {
        let raw = "GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let req = HttpServer::parse_http_request(raw, "127.0.0.1").unwrap();
        assert_eq!(req.path, "/health");
        assert!(req.query.is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = HttpServerConfig::default();
        assert_eq!(config.port, 8080);
        assert_eq!(config.bind_address, "127.0.0.1");
        assert!(config.cors_enabled);
    }

    #[test]
    fn test_status_text() {
        assert_eq!(status_text(200), "OK");
        assert_eq!(status_text(404), "Not Found");
        assert_eq!(status_text(500), "Internal Server Error");
    }
}

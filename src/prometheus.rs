//! Prometheus Metrics Exporter
//!
//! Exports Silicon Monitor metrics in Prometheus exposition format with proper
//! `# HELP`, `# TYPE` annotations and label support. Compatible with
//! Prometheus, Grafana, and other metric collection systems.
//!
//! # Examples
//!
//! ```no_run
//! use simon::prometheus::{PrometheusExporter, MetricFamily};
//!
//! let mut exporter = PrometheusExporter::new("simon");
//!
//! // Collect system metrics
//! exporter.collect_system_metrics();
//!
//! // Export in Prometheus text format
//! let output = exporter.export();
//! println!("{}", output);
//! // Output:
//! // # HELP simon_cpu_usage_percent CPU utilization percentage
//! // # TYPE simon_cpu_usage_percent gauge
//! // simon_cpu_usage_percent 42.5
//! // ...
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Prometheus metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// A gauge is a metric that represents a single numerical value that can go up and down
    Gauge,
    /// A counter is a metric that represents a single monotonically increasing counter
    Counter,
    /// A histogram samples observations and counts them in configurable buckets
    Histogram,
    /// A summary is similar to a histogram but calculates configurable quantiles
    Summary,
    /// Untyped metric
    Untyped,
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gauge => write!(f, "gauge"),
            Self::Counter => write!(f, "counter"),
            Self::Histogram => write!(f, "histogram"),
            Self::Summary => write!(f, "summary"),
            Self::Untyped => write!(f, "untyped"),
        }
    }
}

/// A single metric sample with optional labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    /// Metric name suffix (empty for simple metrics)
    pub suffix: String,
    /// Label key-value pairs
    pub labels: BTreeMap<String, String>,
    /// Metric value
    pub value: f64,
}

/// A complete metric family with metadata and samples
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFamily {
    /// Metric name
    pub name: String,
    /// Help text
    pub help: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Samples
    pub samples: Vec<MetricSample>,
}

impl MetricFamily {
    /// Create a new gauge metric
    pub fn gauge(name: &str, help: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Gauge,
            samples: vec![MetricSample {
                suffix: String::new(),
                labels: BTreeMap::new(),
                value,
            }],
        }
    }

    /// Create a gauge with labels
    pub fn gauge_with_labels(
        name: &str,
        help: &str,
        value: f64,
        labels: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Gauge,
            samples: vec![MetricSample {
                suffix: String::new(),
                labels,
                value,
            }],
        }
    }

    /// Create a counter metric
    pub fn counter(name: &str, help: &str, value: f64) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            metric_type: MetricType::Counter,
            samples: vec![MetricSample {
                suffix: String::new(),
                labels: BTreeMap::new(),
                value,
            }],
        }
    }

    /// Add a labeled sample to this family
    pub fn add_sample(&mut self, value: f64, labels: BTreeMap<String, String>) {
        self.samples.push(MetricSample {
            suffix: String::new(),
            labels,
            value,
        });
    }

    /// Format this metric family in Prometheus exposition format
    pub fn format(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# HELP {} {}\n", self.name, self.help));
        out.push_str(&format!("# TYPE {} {}\n", self.name, self.metric_type));

        for sample in &self.samples {
            let full_name = if sample.suffix.is_empty() {
                self.name.clone()
            } else {
                format!("{}_{}", self.name, sample.suffix)
            };

            if sample.labels.is_empty() {
                out.push_str(&format!("{} {}\n", full_name, format_value(sample.value)));
            } else {
                let label_str: Vec<String> = sample
                    .labels
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, escape_label_value(v)))
                    .collect();
                out.push_str(&format!(
                    "{}{{{}}} {}\n",
                    full_name,
                    label_str.join(","),
                    format_value(sample.value)
                ));
            }
        }

        out
    }
}

/// Prometheus metrics exporter for Silicon Monitor
pub struct PrometheusExporter {
    /// Namespace prefix for all metrics
    prefix: String,
    /// Collected metric families
    families: Vec<MetricFamily>,
}

impl PrometheusExporter {
    /// Create a new exporter with the given namespace prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: sanitize_metric_name(prefix),
            families: Vec::new(),
        }
    }

    /// Add a metric family
    pub fn add(&mut self, family: MetricFamily) {
        self.families.push(family);
    }

    /// Collect all available system metrics
    pub fn collect_system_metrics(&mut self) {
        self.collect_cpu_metrics();
        self.collect_memory_metrics();
        self.collect_gpu_metrics();
        self.collect_disk_metrics();
        self.collect_network_metrics();
    }

    fn prefixed(&self, name: &str) -> String {
        format!("{}_{}", self.prefix, name)
    }

    fn collect_cpu_metrics(&mut self) {
        if let Ok(cpu) = crate::CpuStats::new() {
            self.add(MetricFamily::gauge(
                &self.prefixed("cpu_usage_percent"),
                "Total CPU utilization percentage",
                cpu.total_usage as f64,
            ));

            // Per-core utilization
            let mut per_core = MetricFamily {
                name: self.prefixed("cpu_core_usage_percent"),
                help: "Per-core CPU utilization percentage".into(),
                metric_type: MetricType::Gauge,
                samples: Vec::new(),
            };
            for (i, usage) in cpu.per_cpu_usage.iter().enumerate() {
                let mut labels = BTreeMap::new();
                labels.insert("core".into(), i.to_string());
                per_core.samples.push(MetricSample {
                    suffix: String::new(),
                    labels,
                    value: *usage as f64,
                });
            }
            if !per_core.samples.is_empty() {
                self.add(per_core);
            }

            self.add(MetricFamily::gauge(
                &self.prefixed("cpu_cores_total"),
                "Total number of CPU cores",
                cpu.per_cpu_usage.len() as f64,
            ));
        }
    }

    fn collect_memory_metrics(&mut self) {
        if let Ok(mem) = crate::MemoryStats::new() {
            self.add(MetricFamily::gauge(
                &self.prefixed("memory_total_bytes"),
                "Total physical memory in bytes",
                mem.total as f64,
            ));
            self.add(MetricFamily::gauge(
                &self.prefixed("memory_used_bytes"),
                "Used physical memory in bytes",
                mem.used as f64,
            ));
            self.add(MetricFamily::gauge(
                &self.prefixed("memory_free_bytes"),
                "Free physical memory in bytes",
                mem.free as f64,
            ));
            if mem.total > 0 {
                self.add(MetricFamily::gauge(
                    &self.prefixed("memory_usage_percent"),
                    "Memory utilization percentage",
                    (mem.used as f64 / mem.total as f64) * 100.0,
                ));
            }
        }
    }

    fn collect_gpu_metrics(&mut self) {
        if let Ok(gpus) = crate::GpuCollection::auto_detect() {
            if let Ok(snapshots) = gpus.snapshot_all() {
                // GPU count
                self.add(MetricFamily::gauge(
                    &self.prefixed("gpu_count"),
                    "Number of detected GPUs",
                    snapshots.len() as f64,
                ));

                for (i, info) in snapshots.iter().enumerate() {
                    let mut base_labels = BTreeMap::new();
                    base_labels.insert("gpu".into(), i.to_string());
                    base_labels.insert("name".into(), info.static_info.name.clone());
                    base_labels.insert("vendor".into(), info.static_info.vendor.to_string());

                    // Utilization
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("gpu_utilization_percent"),
                        "GPU compute utilization percentage",
                        info.dynamic_info.utilization as f64,
                        base_labels.clone(),
                    ));

                    // Temperature
                    if let Some(temp) = info.dynamic_info.thermal.temperature {
                        self.add(MetricFamily::gauge_with_labels(
                            &self.prefixed("gpu_temperature_celsius"),
                            "GPU temperature in degrees Celsius",
                            temp as f64,
                            base_labels.clone(),
                        ));
                    }

                    // Power
                    if let Some(power) = info.dynamic_info.power.draw {
                        self.add(MetricFamily::gauge_with_labels(
                            &self.prefixed("gpu_power_watts"),
                            "GPU power draw in watts",
                            power as f64 / 1000.0,
                            base_labels.clone(),
                        ));
                    }

                    // Memory
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("gpu_memory_total_bytes"),
                        "GPU total memory in bytes",
                        info.dynamic_info.memory.total as f64,
                        base_labels.clone(),
                    ));
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("gpu_memory_used_bytes"),
                        "GPU used memory in bytes",
                        info.dynamic_info.memory.used as f64,
                        base_labels.clone(),
                    ));
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("gpu_memory_free_bytes"),
                        "GPU free memory in bytes",
                        info.dynamic_info.memory.free as f64,
                        base_labels.clone(),
                    ));

                    // Clocks
                    if let Some(core) = info.dynamic_info.clocks.core {
                        self.add(MetricFamily::gauge_with_labels(
                            &self.prefixed("gpu_clock_core_mhz"),
                            "GPU core clock in MHz",
                            core as f64,
                            base_labels.clone(),
                        ));
                    }
                    if let Some(mem_clk) = info.dynamic_info.clocks.memory {
                        self.add(MetricFamily::gauge_with_labels(
                            &self.prefixed("gpu_clock_memory_mhz"),
                            "GPU memory clock in MHz",
                            mem_clk as f64,
                            base_labels.clone(),
                        ));
                    }

                    // Fan
                    if let Some(fan) = info.dynamic_info.fan.speed_percent {
                        self.add(MetricFamily::gauge_with_labels(
                            &self.prefixed("gpu_fan_speed_percent"),
                            "GPU fan speed percentage",
                            fan as f64,
                            base_labels.clone(),
                        ));
                    }
                }
            }
        }
    }

    fn collect_disk_metrics(&mut self) {
        if let Ok(disks) = crate::disk::enumerate_disks() {
            for disk in &disks {
                let mut labels = BTreeMap::new();
                labels.insert("device".into(), disk.name().to_string());
                labels.insert("mount".into(), disk.mount_point().to_string());

                self.add(MetricFamily::gauge_with_labels(
                    &self.prefixed("disk_total_bytes"),
                    "Total disk capacity in bytes",
                    disk.total_bytes() as f64,
                    labels.clone(),
                ));
                self.add(MetricFamily::gauge_with_labels(
                    &self.prefixed("disk_used_bytes"),
                    "Used disk space in bytes",
                    disk.used_bytes() as f64,
                    labels.clone(),
                ));
                self.add(MetricFamily::gauge_with_labels(
                    &self.prefixed("disk_available_bytes"),
                    "Available disk space in bytes",
                    disk.available_bytes() as f64,
                    labels.clone(),
                ));
                if disk.total_bytes() > 0 {
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("disk_usage_percent"),
                        "Disk utilization percentage",
                        (disk.used_bytes() as f64 / disk.total_bytes() as f64) * 100.0,
                        labels.clone(),
                    ));
                }
            }
        }
    }

    fn collect_network_metrics(&mut self) {
        if let Ok(mut monitor) = crate::NetworkMonitor::new() {
            if let Ok(interfaces) = monitor.interfaces() {
                for iface in &interfaces {
                    let mut labels = BTreeMap::new();
                    labels.insert("interface".into(), iface.name.clone());

                    self.add(MetricFamily::counter(
                        &self.prefixed("network_rx_bytes_total"),
                        "Total bytes received",
                    ));

                    // Use gauges for current rates
                    let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, iface);
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("network_rx_bytes_per_sec"),
                        "Network receive rate in bytes per second",
                        rx_rate,
                        labels.clone(),
                    ));
                    self.add(MetricFamily::gauge_with_labels(
                        &self.prefixed("network_tx_bytes_per_sec"),
                        "Network transmit rate in bytes per second",
                        tx_rate,
                        labels.clone(),
                    ));
                }
            }
        }
    }

    /// Export all metrics in Prometheus text exposition format
    pub fn export(&self) -> String {
        let mut output = String::with_capacity(4096);

        // Add metadata comment
        output.push_str(&format!(
            "# Silicon Monitor v{} Prometheus Metrics\n\n",
            crate::VERSION
        ));

        for family in &self.families {
            output.push_str(&family.format());
            output.push('\n');
        }

        output
    }

    /// Export metrics and clear collected data
    pub fn export_and_clear(&mut self) -> String {
        let output = self.export();
        self.families.clear();
        output
    }

    /// Get the content type for Prometheus exposition format
    pub fn content_type() -> &'static str {
        "text/plain; version=0.0.4; charset=utf-8"
    }
}

/// Sanitize a string for use as a Prometheus metric name
fn sanitize_metric_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Escape a label value for Prometheus format
fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Format a float value for Prometheus (special handling for NaN, Inf)
fn format_value(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "+Inf".to_string()
        } else {
            "-Inf".to_string()
        }
    } else if value == value.floor() && value.abs() < 1e15 {
        format!("{:.0}", value)
    } else {
        format!("{}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauge_format() {
        let family = MetricFamily::gauge("test_metric", "A test metric", 42.5);
        let output = family.format();
        assert!(output.contains("# HELP test_metric A test metric"));
        assert!(output.contains("# TYPE test_metric gauge"));
        assert!(output.contains("test_metric 42.5"));
    }

    #[test]
    fn test_labeled_metric() {
        let mut labels = BTreeMap::new();
        labels.insert("gpu".into(), "0".into());
        labels.insert("name".into(), "RTX 4090".into());

        let family = MetricFamily::gauge_with_labels("gpu_temp", "GPU temperature", 72.0, labels);
        let output = family.format();
        assert!(output.contains("gpu=\"0\""));
        assert!(output.contains("name=\"RTX 4090\""));
        assert!(output.contains("72"));
    }

    #[test]
    fn test_sanitize_metric_name() {
        assert_eq!(sanitize_metric_name("cpu.usage-percent"), "cpu_usage_percent");
        assert_eq!(sanitize_metric_name("valid_name"), "valid_name");
    }

    #[test]
    fn test_escape_label_value() {
        assert_eq!(escape_label_value("hello"), "hello");
        assert_eq!(escape_label_value("hello\"world"), "hello\\\"world");
        assert_eq!(escape_label_value("line\nnewline"), "line\\nnewline");
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value(42.0), "42");
        assert_eq!(format_value(42.5), "42.5");
        assert_eq!(format_value(f64::NAN), "NaN");
        assert_eq!(format_value(f64::INFINITY), "+Inf");
    }

    #[test]
    fn test_exporter_collect_and_export() {
        let mut exporter = PrometheusExporter::new("test");
        exporter.add(MetricFamily::gauge("test_metric", "Help", 1.0));
        let output = exporter.export();
        assert!(output.contains("# HELP test_metric"));
        assert!(output.contains("test_metric 1"));
    }
}

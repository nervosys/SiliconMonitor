//! Observability API - Full System Visibility with MCP-like Permissions
//!
//! This module provides a comprehensive observability API that exposes all system
//! metrics, events, and state to external AI systems. It follows the filesystem-as-context
//! principle where system state is materialized as structured, queryable context.
//!
//! # Architecture
//!
//! The observability stack consists of:
//! - **Metrics**: Real-time hardware metrics (CPU, GPU, memory, disk, network)
//! - **Events**: System events (alerts, state changes, threshold violations)
//! - **Context**: Materialized system state as structured context for AI reasoning
//! - **Permissions**: MCP-like capability-based access control
//!
//! # Permission Model
//!
//! Similar to MCP (Model Context Protocol), the API uses capability-based permissions:
//! - **Capabilities**: Define what categories of data can be accessed
//! - **Scopes**: Fine-grained control within capabilities
//! - **Rate Limits**: Protect against excessive polling
//! - **API Keys**: Authentication for external access
//!
//! # Example
//!
//! ```no_run
//! use simon::observability::{ObservabilityApi, ApiConfig, Permission};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create API with full permissions (local use)
//! let api = ObservabilityApi::new_local()?;
//!
//! // Get system context (materialized state)
//! let context = api.get_context()?;
//! println!("System: {}", context.system.hostname);
//!
//! // Stream metrics
//! for metric in api.metrics_stream()? {
//!     println!("{}: {}", metric.name, metric.value);
//! }
//! # Ok(())
//! # }
//! ```

pub mod api;
pub mod context;
pub mod events;
pub mod metrics;
pub mod permissions;
pub mod server;
pub mod streaming;

// Re-export commonly used types from api
pub use api::{
    ApiResponse, ObservabilityApi, ObservabilityError, RequestContext, ResponseMeta,
    RateLimitInfo,
};

// Re-export context types
pub use context::{
    AlertContext, BootConfigContext, ContextMeta, CpuContext, 
    CpuMetrics as ContextCpuMetrics, DiskContext, DiskMetrics as ContextDiskMetrics, 
    DriverContext, FanContext, GpuClocks, GpuContext, GpuMetrics as ContextGpuMetrics, 
    HardwareContext, MemoryContext, MemoryMetrics as ContextMemoryMetrics, MetricsContext,
    MinimalContext, MotherboardContext, NetworkInterfaceContext,
    NetworkMetrics as ContextNetworkMetrics, PcieContext, PowerSupplyContext,
    ProcessMetrics as ContextProcessMetrics, ServiceContext, SoftwareContext,
    SystemContext, SystemContextBuilder, SystemIdentity, SystemLoadMetrics,
    TemperatureSensorContext,
};

// Re-export event types
pub use events::{
    AlertChecker, EventCategory, EventFilter, EventManager, EventSeverity,
    SubscriptionId, SystemEvent, ThresholdConfig, event_types,
};

// Re-export metric types
pub use metrics::{
    CpuMetricSnapshot, DiskMetricSnapshot, GpuMetricSnapshot, MemoryMetricSnapshot,
    MetricCollector, MetricDefinition, MetricSnapshot, MetricStats, MetricTimeSeries,
    MetricType, MetricValue, NetworkMetricSnapshot, Percentiles, SystemMetricSnapshot,
    TimeSeriesPoint, standard as metric_names,
};

// Re-export permission types
pub use permissions::{
    ApiConfig, ApiKey, Capability, Permission, PermissionChecker, PermissionError,
    RateLimit, RateLimiter, Scope,
};

// Re-export server types
pub use server::{
    HttpRequest, HttpResponse, OpenApiSpec, RequestHandler, ServerConfig, routes,
};

// Re-export streaming types
pub use streaming::{
    ClientMessage, StreamChannel, StreamError, StreamFrame, StreamManager,
    StreamMessage, Subscription,
};


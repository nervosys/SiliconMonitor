//! Real-time Streaming Support
//!
//! This module provides WebSocket-based real-time streaming of:
//! - Metric updates
//! - System events
//! - Context changes

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::context::MinimalContext;
use super::events::{EventFilter, SystemEvent};
use super::metrics::MetricSnapshot;
use super::permissions::Capability;

/// Counter for unique subscription IDs
static SUBSCRIPTION_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique subscription ID using timestamp + counter
fn generate_subscription_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = SUBSCRIPTION_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("sub-{}-{}", timestamp, counter)
}

/// Streaming message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamMessage {
    /// Subscription confirmation
    Subscribed {
        subscription_id: String,
        channels: Vec<String>,
    },
    /// Unsubscription confirmation
    Unsubscribed {
        subscription_id: String,
    },
    /// Metric update
    Metrics {
        timestamp: u64,
        data: MetricSnapshot,
    },
    /// System event
    Event {
        event: SystemEvent,
    },
    /// Context update (minimal)
    ContextUpdate {
        context: MinimalContext,
    },
    /// Error message
    Error {
        code: String,
        message: String,
    },
    /// Heartbeat/ping
    Ping {
        timestamp: u64,
    },
    /// Pong response
    Pong {
        timestamp: u64,
    },
}

/// Client subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to channels
    Subscribe {
        channels: Vec<StreamChannel>,
        /// Optional filter for events
        event_filter: Option<EventFilter>,
    },
    /// Unsubscribe from channels
    Unsubscribe {
        subscription_id: String,
    },
    /// Set update interval
    SetInterval {
        interval_ms: u64,
    },
    /// Request immediate update
    Refresh,
    /// Ping
    Ping,
}

/// Available stream channels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamChannel {
    /// All metrics
    Metrics,
    /// CPU metrics only
    CpuMetrics,
    /// GPU metrics only
    GpuMetrics,
    /// Memory metrics only
    MemoryMetrics,
    /// Disk metrics only
    DiskMetrics,
    /// Network metrics only
    NetworkMetrics,
    /// System events
    Events,
    /// Context updates
    Context,
    /// Process updates
    Processes,
}

impl StreamChannel {
    /// Get required capability for this channel
    pub fn required_capability(&self) -> Capability {
        match self {
            Self::Metrics => Capability::SystemInfo,
            Self::CpuMetrics => Capability::Cpu,
            Self::GpuMetrics => Capability::Gpu,
            Self::MemoryMetrics => Capability::Memory,
            Self::DiskMetrics => Capability::Disk,
            Self::NetworkMetrics => Capability::Network,
            Self::Events => Capability::Events,
            Self::Context => Capability::Context,
            Self::Processes => Capability::Process,
        }
    }
}

/// A client subscription
#[derive(Debug, Clone)]
pub struct Subscription {
    /// Unique subscription ID
    pub id: String,
    /// API key
    pub api_key: String,
    /// Subscribed channels
    pub channels: Vec<StreamChannel>,
    /// Event filter (for Events channel)
    pub event_filter: Option<EventFilter>,
    /// Update interval
    pub interval: Duration,
    /// Last update time
    pub last_update: std::time::Instant,
}

impl Subscription {
    pub fn new(api_key: impl Into<String>, channels: Vec<StreamChannel>) -> Self {
        Self {
            id: generate_subscription_id(),
            api_key: api_key.into(),
            channels,
            event_filter: None,
            interval: Duration::from_secs(1),
            last_update: std::time::Instant::now(),
        }
    }

    pub fn with_event_filter(mut self, filter: EventFilter) -> Self {
        self.event_filter = Some(filter);
        self
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Check if a channel is subscribed
    pub fn has_channel(&self, channel: StreamChannel) -> bool {
        self.channels.contains(&channel)
    }

    /// Check if an update is due
    pub fn update_due(&self) -> bool {
        self.last_update.elapsed() >= self.interval
    }

    /// Mark as updated
    pub fn mark_updated(&mut self) {
        self.last_update = std::time::Instant::now();
    }
}

/// Streaming session manager
pub struct StreamManager {
    /// Active subscriptions by client ID
    subscriptions: RwLock<HashMap<String, Subscription>>,
    /// Pending messages for each client
    pending_messages: RwLock<HashMap<String, Vec<StreamMessage>>>,
    /// Default update interval
    default_interval: Duration,
    /// Max subscriptions per client
    max_subscriptions_per_client: usize,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            subscriptions: RwLock::new(HashMap::new()),
            pending_messages: RwLock::new(HashMap::new()),
            default_interval: Duration::from_secs(1),
            max_subscriptions_per_client: 10,
        }
    }

    pub fn with_default_interval(mut self, interval: Duration) -> Self {
        self.default_interval = interval;
        self
    }

    /// Add a subscription
    pub fn subscribe(
        &self,
        client_id: &str,
        api_key: &str,
        channels: Vec<StreamChannel>,
        event_filter: Option<EventFilter>,
    ) -> Result<Subscription, StreamError> {
        let mut subs = self.subscriptions.write().map_err(|_| StreamError::Internal)?;

        // Check subscription limit
        let client_subs = subs.values().filter(|s| s.api_key == api_key).count();
        if client_subs >= self.max_subscriptions_per_client {
            return Err(StreamError::TooManySubscriptions);
        }

        let mut subscription = Subscription::new(api_key, channels)
            .with_interval(self.default_interval);

        if let Some(filter) = event_filter {
            subscription = subscription.with_event_filter(filter);
        }

        let sub_clone = subscription.clone();
        subs.insert(client_id.to_string(), subscription);

        Ok(sub_clone)
    }

    /// Remove a subscription
    pub fn unsubscribe(&self, client_id: &str) -> bool {
        if let Ok(mut subs) = self.subscriptions.write() {
            subs.remove(client_id).is_some()
        } else {
            false
        }
    }

    /// Get a subscription
    pub fn get_subscription(&self, client_id: &str) -> Option<Subscription> {
        self.subscriptions.read().ok()?.get(client_id).cloned()
    }

    /// Queue a message for a client
    pub fn queue_message(&self, client_id: &str, message: StreamMessage) {
        if let Ok(mut pending) = self.pending_messages.write() {
            pending
                .entry(client_id.to_string())
                .or_insert_with(Vec::new)
                .push(message);
        }
    }

    /// Get pending messages for a client
    pub fn get_pending_messages(&self, client_id: &str) -> Vec<StreamMessage> {
        if let Ok(mut pending) = self.pending_messages.write() {
            pending.remove(client_id).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast_event(&self, event: &SystemEvent) {
        if let Ok(subs) = self.subscriptions.read() {
            for (client_id, sub) in subs.iter() {
                if sub.has_channel(StreamChannel::Events) {
                    // Check event filter
                    let should_send = sub
                        .event_filter
                        .as_ref()
                        .map(|f| f.matches(event))
                        .unwrap_or(true);

                    if should_send {
                        self.queue_message(
                            client_id,
                            StreamMessage::Event {
                                event: event.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    /// Broadcast metrics to all subscribers
    pub fn broadcast_metrics(&self, snapshot: &MetricSnapshot) {
        if let Ok(subs) = self.subscriptions.read() {
            for (client_id, sub) in subs.iter() {
                if sub.has_channel(StreamChannel::Metrics) {
                    self.queue_message(
                        client_id,
                        StreamMessage::Metrics {
                            timestamp: snapshot.timestamp,
                            data: snapshot.clone(),
                        },
                    );
                }
            }
        }
    }

    /// Broadcast context update to all subscribers
    pub fn broadcast_context(&self, context: &MinimalContext) {
        if let Ok(subs) = self.subscriptions.read() {
            for (client_id, sub) in subs.iter() {
                if sub.has_channel(StreamChannel::Context) {
                    self.queue_message(
                        client_id,
                        StreamMessage::ContextUpdate {
                            context: context.clone(),
                        },
                    );
                }
            }
        }
    }

    /// Get all subscriptions that need an update
    pub fn get_due_subscriptions(&self) -> Vec<(String, Subscription)> {
        self.subscriptions
            .read()
            .map(|subs| {
                subs.iter()
                    .filter(|(_, s)| s.update_due())
                    .map(|(id, s)| (id.clone(), s.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Mark a subscription as updated
    pub fn mark_updated(&self, client_id: &str) {
        if let Ok(mut subs) = self.subscriptions.write() {
            if let Some(sub) = subs.get_mut(client_id) {
                sub.mark_updated();
            }
        }
    }

    /// Set interval for a subscription
    pub fn set_interval(&self, client_id: &str, interval: Duration) -> bool {
        if let Ok(mut subs) = self.subscriptions.write() {
            if let Some(sub) = subs.get_mut(client_id) {
                sub.interval = interval;
                return true;
            }
        }
        false
    }

    /// Get active subscription count
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.read().map(|s| s.len()).unwrap_or(0)
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamError {
    /// Too many subscriptions
    TooManySubscriptions,
    /// Invalid channel
    InvalidChannel,
    /// Permission denied
    PermissionDenied,
    /// Internal error
    Internal,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManySubscriptions => write!(f, "Too many subscriptions"),
            Self::InvalidChannel => write!(f, "Invalid channel"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::Internal => write!(f, "Internal error"),
        }
    }
}

impl std::error::Error for StreamError {}

/// WebSocket frame builder for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFrame {
    /// Sequence number
    pub seq: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Message
    pub message: StreamMessage,
}

impl StreamFrame {
    pub fn new(seq: u64, message: StreamMessage) -> Self {
        Self {
            seq,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            message,
        }
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

//! System Events and Change Detection
//!
//! This module provides an event system for:
//! - System state changes (hardware added/removed, config changes)
//! - Metric threshold alerts
//! - Resource exhaustion warnings
//! - Custom event subscriptions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Counter for unique event IDs
static EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique event ID using timestamp + counter
fn generate_event_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = EVENT_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("evt-{}-{}", timestamp, counter)
}

/// Event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventSeverity {
    /// Informational event
    Info,
    /// Warning - attention may be needed
    Warning,
    /// Error - something went wrong
    Error,
    /// Critical - immediate attention required
    Critical,
}

impl std::fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Event categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// CPU-related events
    Cpu,
    /// GPU-related events
    Gpu,
    /// Memory-related events
    Memory,
    /// Disk-related events
    Disk,
    /// Network-related events
    Network,
    /// Power/battery events
    Power,
    /// Temperature events
    Temperature,
    /// Fan events
    Fan,
    /// Process events
    Process,
    /// System events (boot, shutdown, etc.)
    System,
    /// Hardware change events
    Hardware,
    /// Service events
    Service,
    /// Security events
    Security,
    /// Custom events
    Custom,
}

impl std::fmt::Display for EventCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cpu => write!(f, "cpu"),
            Self::Gpu => write!(f, "gpu"),
            Self::Memory => write!(f, "memory"),
            Self::Disk => write!(f, "disk"),
            Self::Network => write!(f, "network"),
            Self::Power => write!(f, "power"),
            Self::Temperature => write!(f, "temperature"),
            Self::Fan => write!(f, "fan"),
            Self::Process => write!(f, "process"),
            Self::System => write!(f, "system"),
            Self::Hardware => write!(f, "hardware"),
            Self::Service => write!(f, "service"),
            Self::Security => write!(f, "security"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// A system event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Unique event ID
    pub id: String,
    /// Event timestamp (unix epoch)
    pub timestamp: u64,
    /// Event category
    pub category: EventCategory,
    /// Event severity
    pub severity: EventSeverity,
    /// Event type (within category)
    pub event_type: String,
    /// Human-readable message
    pub message: String,
    /// Source of the event (e.g., "gpu:0", "cpu", "disk:/dev/sda")
    pub source: String,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Previous value (for change events)
    pub previous_value: Option<serde_json::Value>,
    /// Current value (for change events)
    pub current_value: Option<serde_json::Value>,
    /// Whether this event has been acknowledged
    pub acknowledged: bool,
}

impl SystemEvent {
    /// Create a new event
    pub fn new(
        category: EventCategory,
        severity: EventSeverity,
        event_type: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_event_id(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            category,
            severity,
            event_type: event_type.into(),
            message: message.into(),
            source: source.into(),
            metadata: HashMap::new(),
            previous_value: None,
            current_value: None,
            acknowledged: false,
        }
    }

    /// Add metadata to the event
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(v) = serde_json::to_value(value) {
            self.metadata.insert(key.into(), v);
        }
        self
    }

    /// Add previous/current values for change events
    pub fn with_change(
        mut self,
        previous: impl Serialize,
        current: impl Serialize,
    ) -> Self {
        self.previous_value = serde_json::to_value(previous).ok();
        self.current_value = serde_json::to_value(current).ok();
        self
    }

    /// Create an info event
    pub fn info(category: EventCategory, event_type: &str, message: &str, source: &str) -> Self {
        Self::new(category, EventSeverity::Info, event_type, message, source)
    }

    /// Create a warning event
    pub fn warning(category: EventCategory, event_type: &str, message: &str, source: &str) -> Self {
        Self::new(category, EventSeverity::Warning, event_type, message, source)
    }

    /// Create an error event
    pub fn error(category: EventCategory, event_type: &str, message: &str, source: &str) -> Self {
        Self::new(category, EventSeverity::Error, event_type, message, source)
    }

    /// Create a critical event
    pub fn critical(category: EventCategory, event_type: &str, message: &str, source: &str) -> Self {
        Self::new(category, EventSeverity::Critical, event_type, message, source)
    }
}

/// Predefined event types
pub mod event_types {
    /// CPU events
    pub mod cpu {
        pub const HIGH_USAGE: &str = "high_usage";
        pub const THROTTLING: &str = "throttling";
        pub const HIGH_TEMPERATURE: &str = "high_temperature";
        pub const FREQUENCY_CHANGE: &str = "frequency_change";
    }

    /// GPU events
    pub mod gpu {
        pub const HIGH_USAGE: &str = "high_usage";
        pub const HIGH_MEMORY: &str = "high_memory";
        pub const HIGH_TEMPERATURE: &str = "high_temperature";
        pub const THROTTLING: &str = "throttling";
        pub const DRIVER_ERROR: &str = "driver_error";
        pub const POWER_LIMIT: &str = "power_limit";
        pub const ECC_ERROR: &str = "ecc_error";
    }

    /// Memory events
    pub mod memory {
        pub const HIGH_USAGE: &str = "high_usage";
        pub const LOW_AVAILABLE: &str = "low_available";
        pub const SWAP_ACTIVE: &str = "swap_active";
        pub const HIGH_SWAP: &str = "high_swap";
        pub const OOM_RISK: &str = "oom_risk";
    }

    /// Disk events
    pub mod disk {
        pub const LOW_SPACE: &str = "low_space";
        pub const HIGH_IO: &str = "high_io";
        pub const SMART_WARNING: &str = "smart_warning";
        pub const MOUNT_CHANGE: &str = "mount_change";
    }

    /// Network events
    pub mod network {
        pub const HIGH_BANDWIDTH: &str = "high_bandwidth";
        pub const LINK_UP: &str = "link_up";
        pub const LINK_DOWN: &str = "link_down";
        pub const HIGH_ERRORS: &str = "high_errors";
        pub const HIGH_DROPPED: &str = "high_dropped";
    }

    /// Power events
    pub mod power {
        pub const LOW_BATTERY: &str = "low_battery";
        pub const CRITICAL_BATTERY: &str = "critical_battery";
        pub const POWER_CONNECTED: &str = "power_connected";
        pub const POWER_DISCONNECTED: &str = "power_disconnected";
        pub const HIGH_POWER_DRAW: &str = "high_power_draw";
    }

    /// Temperature events
    pub mod temperature {
        pub const HIGH_TEMP: &str = "high_temp";
        pub const CRITICAL_TEMP: &str = "critical_temp";
        pub const SENSOR_ERROR: &str = "sensor_error";
    }

    /// Fan events
    pub mod fan {
        pub const HIGH_SPEED: &str = "high_speed";
        pub const FAN_STOPPED: &str = "fan_stopped";
        pub const FAN_ERROR: &str = "fan_error";
    }

    /// Process events
    pub mod process {
        pub const HIGH_CPU: &str = "high_cpu";
        pub const HIGH_MEMORY: &str = "high_memory";
        pub const STARTED: &str = "started";
        pub const STOPPED: &str = "stopped";
        pub const CRASHED: &str = "crashed";
    }

    /// Hardware events
    pub mod hardware {
        pub const DEVICE_ADDED: &str = "device_added";
        pub const DEVICE_REMOVED: &str = "device_removed";
        pub const DEVICE_ERROR: &str = "device_error";
    }
}

/// Event filter for subscriptions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    /// Filter by categories
    pub categories: Option<Vec<EventCategory>>,
    /// Filter by severity (minimum)
    pub min_severity: Option<EventSeverity>,
    /// Filter by source pattern
    pub source_pattern: Option<String>,
    /// Filter by event type
    pub event_types: Option<Vec<String>>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_categories(mut self, categories: Vec<EventCategory>) -> Self {
        self.categories = Some(categories);
        self
    }

    pub fn with_min_severity(mut self, severity: EventSeverity) -> Self {
        self.min_severity = Some(severity);
        self
    }

    pub fn with_source_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.source_pattern = Some(pattern.into());
        self
    }

    pub fn with_event_types(mut self, types: Vec<String>) -> Self {
        self.event_types = Some(types);
        self
    }

    /// Check if an event matches this filter
    pub fn matches(&self, event: &SystemEvent) -> bool {
        // Check category
        if let Some(ref categories) = self.categories {
            if !categories.contains(&event.category) {
                return false;
            }
        }

        // Check severity
        if let Some(min_severity) = self.min_severity {
            let event_level = match event.severity {
                EventSeverity::Info => 0,
                EventSeverity::Warning => 1,
                EventSeverity::Error => 2,
                EventSeverity::Critical => 3,
            };
            let min_level = match min_severity {
                EventSeverity::Info => 0,
                EventSeverity::Warning => 1,
                EventSeverity::Error => 2,
                EventSeverity::Critical => 3,
            };
            if event_level < min_level {
                return false;
            }
        }

        // Check source pattern (simple contains for now)
        if let Some(ref pattern) = self.source_pattern {
            if !event.source.contains(pattern) {
                return false;
            }
        }

        // Check event type
        if let Some(ref types) = self.event_types {
            if !types.contains(&event.event_type) {
                return false;
            }
        }

        true
    }
}

/// Event subscriber callback type
pub type EventCallback = Box<dyn Fn(&SystemEvent) + Send + Sync>;

/// Subscription handle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

/// Event manager
pub struct EventManager {
    /// Event history (ring buffer)
    events: RwLock<Vec<SystemEvent>>,
    /// Max events to keep
    max_events: usize,
    /// Subscribers
    subscribers: RwLock<HashMap<SubscriptionId, (EventFilter, Arc<EventCallback>)>>,
    /// Next subscription ID
    next_sub_id: RwLock<u64>,
}

impl EventManager {
    /// Create a new event manager
    pub fn new(max_events: usize) -> Self {
        Self {
            events: RwLock::new(Vec::with_capacity(max_events)),
            max_events,
            subscribers: RwLock::new(HashMap::new()),
            next_sub_id: RwLock::new(0),
        }
    }

    /// Emit a new event
    pub fn emit(&self, event: SystemEvent) {
        // Notify subscribers
        if let Ok(subs) = self.subscribers.read() {
            for (_, (filter, callback)) in subs.iter() {
                if filter.matches(&event) {
                    callback(&event);
                }
            }
        }

        // Store event
        if let Ok(mut events) = self.events.write() {
            if events.len() >= self.max_events {
                events.remove(0);
            }
            events.push(event);
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self, filter: EventFilter, callback: EventCallback) -> SubscriptionId {
        let mut next_id = self.next_sub_id.write().unwrap();
        let id = SubscriptionId(*next_id);
        *next_id += 1;

        if let Ok(mut subs) = self.subscribers.write() {
            subs.insert(id, (filter, Arc::new(callback)));
        }

        id
    }

    /// Unsubscribe
    pub fn unsubscribe(&self, id: SubscriptionId) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.remove(&id);
        }
    }

    /// Get recent events
    pub fn get_events(&self, filter: Option<&EventFilter>, limit: Option<usize>) -> Vec<SystemEvent> {
        if let Ok(events) = self.events.read() {
            let filtered: Vec<_> = events
                .iter()
                .filter(|e| filter.map(|f| f.matches(e)).unwrap_or(true))
                .cloned()
                .collect();

            match limit {
                Some(n) => filtered.into_iter().rev().take(n).collect(),
                None => filtered.into_iter().rev().collect(),
            }
        } else {
            Vec::new()
        }
    }

    /// Get events since a timestamp
    pub fn get_events_since(&self, since: u64) -> Vec<SystemEvent> {
        if let Ok(events) = self.events.read() {
            events
                .iter()
                .filter(|e| e.timestamp >= since)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Acknowledge an event
    pub fn acknowledge(&self, event_id: &str) -> bool {
        if let Ok(mut events) = self.events.write() {
            for event in events.iter_mut() {
                if event.id == event_id {
                    event.acknowledged = true;
                    return true;
                }
            }
        }
        false
    }

    /// Get unacknowledged events
    pub fn get_unacknowledged(&self) -> Vec<SystemEvent> {
        if let Ok(events) = self.events.read() {
            events
                .iter()
                .filter(|e| !e.acknowledged)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Clear all events
    pub fn clear(&self) {
        if let Ok(mut events) = self.events.write() {
            events.clear();
        }
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Threshold configuration for alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// CPU usage threshold (0-100)
    pub cpu_high_percent: Option<f32>,
    /// Memory usage threshold (0-100)
    pub memory_high_percent: Option<f32>,
    /// GPU usage threshold (0-100)
    pub gpu_high_percent: Option<f32>,
    /// GPU temperature threshold (Celsius)
    pub gpu_high_temp_c: Option<f32>,
    /// CPU temperature threshold (Celsius)
    pub cpu_high_temp_c: Option<f32>,
    /// Disk usage threshold (0-100)
    pub disk_high_percent: Option<f32>,
    /// Battery low threshold (0-100)
    pub battery_low_percent: Option<f32>,
    /// Network error rate threshold
    pub network_error_rate: Option<f32>,
    /// Process CPU threshold
    pub process_cpu_percent: Option<f32>,
    /// Process memory threshold (MB)
    pub process_memory_mb: Option<u64>,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            cpu_high_percent: Some(90.0),
            memory_high_percent: Some(90.0),
            gpu_high_percent: Some(95.0),
            gpu_high_temp_c: Some(85.0),
            cpu_high_temp_c: Some(90.0),
            disk_high_percent: Some(90.0),
            battery_low_percent: Some(20.0),
            network_error_rate: Some(5.0),
            process_cpu_percent: Some(80.0),
            process_memory_mb: Some(4096),
        }
    }
}

/// Alert checker that monitors thresholds
pub struct AlertChecker {
    thresholds: ThresholdConfig,
    event_manager: Arc<EventManager>,
    /// Track last alert times to avoid spam
    last_alerts: RwLock<HashMap<String, u64>>,
    /// Minimum time between same alerts (seconds)
    alert_cooldown: Duration,
}

impl AlertChecker {
    pub fn new(thresholds: ThresholdConfig, event_manager: Arc<EventManager>) -> Self {
        Self {
            thresholds,
            event_manager,
            last_alerts: RwLock::new(HashMap::new()),
            alert_cooldown: Duration::from_secs(60),
        }
    }

    /// Set alert cooldown period
    pub fn with_cooldown(mut self, cooldown: Duration) -> Self {
        self.alert_cooldown = cooldown;
        self
    }

    /// Check if we should emit an alert (cooldown)
    fn should_alert(&self, key: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Ok(mut last) = self.last_alerts.write() {
            if let Some(&last_time) = last.get(key) {
                if now - last_time < self.alert_cooldown.as_secs() {
                    return false;
                }
            }
            last.insert(key.to_string(), now);
            true
        } else {
            true
        }
    }

    /// Check CPU metrics
    pub fn check_cpu(&self, usage: f32, temperature: Option<f32>) {
        if let Some(threshold) = self.thresholds.cpu_high_percent {
            if usage > threshold && self.should_alert("cpu_usage") {
                self.event_manager.emit(
                    SystemEvent::warning(
                        EventCategory::Cpu,
                        event_types::cpu::HIGH_USAGE,
                        &format!("CPU usage at {:.1}% (threshold: {:.1}%)", usage, threshold),
                        "cpu",
                    )
                    .with_metadata("usage_percent", usage)
                    .with_metadata("threshold_percent", threshold),
                );
            }
        }

        if let Some(temp) = temperature {
            if let Some(threshold) = self.thresholds.cpu_high_temp_c {
                if temp > threshold && self.should_alert("cpu_temp") {
                    self.event_manager.emit(
                        SystemEvent::warning(
                            EventCategory::Temperature,
                            event_types::cpu::HIGH_TEMPERATURE,
                            &format!("CPU temperature at {:.1}°C (threshold: {:.1}°C)", temp, threshold),
                            "cpu",
                        )
                        .with_metadata("temperature_c", temp)
                        .with_metadata("threshold_c", threshold),
                    );
                }
            }
        }
    }

    /// Check GPU metrics
    pub fn check_gpu(&self, index: usize, usage: f32, memory_percent: f32, temperature: Option<f32>) {
        let source = format!("gpu:{}", index);

        if let Some(threshold) = self.thresholds.gpu_high_percent {
            if usage > threshold && self.should_alert(&format!("gpu_{}_usage", index)) {
                self.event_manager.emit(
                    SystemEvent::warning(
                        EventCategory::Gpu,
                        event_types::gpu::HIGH_USAGE,
                        &format!("GPU {} usage at {:.1}% (threshold: {:.1}%)", index, usage, threshold),
                        &source,
                    )
                    .with_metadata("usage_percent", usage)
                    .with_metadata("threshold_percent", threshold),
                );
            }
        }

        // Check GPU memory
        if memory_percent > 90.0 && self.should_alert(&format!("gpu_{}_memory", index)) {
            self.event_manager.emit(
                SystemEvent::warning(
                    EventCategory::Gpu,
                    event_types::gpu::HIGH_MEMORY,
                    &format!("GPU {} memory at {:.1}%", index, memory_percent),
                    &source,
                )
                .with_metadata("memory_percent", memory_percent),
            );
        }

        if let Some(temp) = temperature {
            if let Some(threshold) = self.thresholds.gpu_high_temp_c {
                if temp > threshold && self.should_alert(&format!("gpu_{}_temp", index)) {
                    self.event_manager.emit(
                        SystemEvent::warning(
                            EventCategory::Temperature,
                            event_types::gpu::HIGH_TEMPERATURE,
                            &format!("GPU {} temperature at {:.1}°C (threshold: {:.1}°C)", index, temp, threshold),
                            &source,
                        )
                        .with_metadata("temperature_c", temp)
                        .with_metadata("threshold_c", threshold),
                    );
                }
            }
        }
    }

    /// Check memory metrics
    pub fn check_memory(&self, usage_percent: f32, swap_percent: f32) {
        if let Some(threshold) = self.thresholds.memory_high_percent {
            if usage_percent > threshold && self.should_alert("memory_usage") {
                self.event_manager.emit(
                    SystemEvent::warning(
                        EventCategory::Memory,
                        event_types::memory::HIGH_USAGE,
                        &format!("Memory usage at {:.1}% (threshold: {:.1}%)", usage_percent, threshold),
                        "memory",
                    )
                    .with_metadata("usage_percent", usage_percent)
                    .with_metadata("threshold_percent", threshold),
                );
            }
        }

        // High swap usage
        if swap_percent > 50.0 && self.should_alert("swap_usage") {
            self.event_manager.emit(
                SystemEvent::warning(
                    EventCategory::Memory,
                    event_types::memory::HIGH_SWAP,
                    &format!("Swap usage at {:.1}%", swap_percent),
                    "memory",
                )
                .with_metadata("swap_percent", swap_percent),
            );
        }
    }

    /// Check disk metrics
    pub fn check_disk(&self, device: &str, usage_percent: f32) {
        if let Some(threshold) = self.thresholds.disk_high_percent {
            if usage_percent > threshold && self.should_alert(&format!("disk_{}", device)) {
                self.event_manager.emit(
                    SystemEvent::warning(
                        EventCategory::Disk,
                        event_types::disk::LOW_SPACE,
                        &format!("Disk {} usage at {:.1}% (threshold: {:.1}%)", device, usage_percent, threshold),
                        &format!("disk:{}", device),
                    )
                    .with_metadata("usage_percent", usage_percent)
                    .with_metadata("threshold_percent", threshold),
                );
            }
        }
    }

    /// Check battery level
    pub fn check_battery(&self, percent: f32, charging: bool) {
        if !charging {
            if let Some(threshold) = self.thresholds.battery_low_percent {
                if percent < threshold && self.should_alert("battery_low") {
                    let severity = if percent < 10.0 {
                        EventSeverity::Critical
                    } else {
                        EventSeverity::Warning
                    };

                    self.event_manager.emit(SystemEvent::new(
                        EventCategory::Power,
                        severity,
                        if percent < 10.0 {
                            event_types::power::CRITICAL_BATTERY
                        } else {
                            event_types::power::LOW_BATTERY
                        },
                        &format!("Battery at {:.1}% (threshold: {:.1}%)", percent, threshold),
                        "battery",
                    )
                    .with_metadata("battery_percent", percent)
                    .with_metadata("threshold_percent", threshold));
                }
            }
        }
    }
}

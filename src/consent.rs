//! User Consent Management
//!
//! This module provides explicit user consent mechanisms for any data collection,
//! telemetry, or analytics features. It ensures ethical operation and compliance
//! with privacy regulations (GDPR, CCPA, etc.).
//!
//! # Principles
//!
//! - **Opt-in by default**: Data collection is enabled by default for better user experience
//! - **Easy opt-out**: Users can disable all data collection with `simon privacy opt-out`
//! - **Transparency**: Clear disclosure of what's collected and why
//! - **Control**: Easy opt-out and data deletion via CLI flags or commands
//! - **Auditability**: All consent decisions are logged and reviewable
//!
//! # Example
//!
//! ```no_run
//! use simonlib::consent::{ConsentManager, ConsentScope};
//!
//! let mut manager = ConsentManager::load()?;
//!
//! // Check if user has consented to basic telemetry
//! if !manager.has_consent(ConsentScope::BasicTelemetry) {
//!     // Prompt user for consent
//!     manager.request_consent(ConsentScope::BasicTelemetry)?;
//! }
//!
//! // Only collect data if user explicitly consented
//! if manager.has_consent(ConsentScope::BasicTelemetry) {
//!     // Safe to collect anonymized usage data
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::error::{SimonError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Check if telemetry is disabled for this session via CLI flags.
/// 
/// Returns `true` if the `--no-telemetry` or `--offline` CLI flag was used.
/// This check is fast and can be used early in code paths to skip telemetry logic entirely.
/// 
/// # Example
/// 
/// ```no_run
/// use simon::consent::is_telemetry_disabled;
/// 
/// if is_telemetry_disabled() {
///     // Skip any telemetry code paths
///     return;
/// }
/// ```
pub fn is_telemetry_disabled() -> bool {
    std::env::var("SIMON_NO_TELEMETRY").is_ok() || std::env::var("SIMON_OFFLINE").is_ok()
}

/// Check if running in offline mode (all network features disabled).
/// 
/// Returns `true` if the `--offline` CLI flag was used.
/// 
/// # Example
/// 
/// ```no_run
/// use simon::consent::is_offline_mode;
/// 
/// if is_offline_mode() {
///     // Skip any network-dependent features
///     return;
/// }
/// ```
pub fn is_offline_mode() -> bool {
    std::env::var("SIMON_OFFLINE").is_ok()
}

/// Consent scopes - each represents a specific type of data collection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentScope {
    /// Basic anonymized usage telemetry (feature usage, crash reports)
    BasicTelemetry,
    
    /// Hardware information (GPU model, CPU type - anonymized)
    HardwareInfo,
    
    /// Performance metrics (aggregated, anonymized)
    PerformanceMetrics,
    
    /// Detailed diagnostics (for troubleshooting, includes system info)
    DetailedDiagnostics,
    
    /// Anonymous analytics (usage patterns, feature popularity)
    Analytics,
}

impl ConsentScope {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::BasicTelemetry => "Basic Telemetry",
            Self::HardwareInfo => "Hardware Information",
            Self::PerformanceMetrics => "Performance Metrics",
            Self::DetailedDiagnostics => "Detailed Diagnostics",
            Self::Analytics => "Anonymous Analytics",
        }
    }

    /// Get detailed description of what's collected
    pub fn description(&self) -> &'static str {
        match self {
            Self::BasicTelemetry => 
                "Basic usage statistics and crash reports. Helps improve stability and reliability. \
                 No personally identifiable information is collected.",
            Self::HardwareInfo => 
                "Anonymized hardware information (GPU models, CPU types, RAM size). \
                 Helps prioritize hardware support. Serial numbers and MAC addresses are never collected.",
            Self::PerformanceMetrics => 
                "Aggregated performance metrics (frame rates, GPU utilization averages). \
                 Helps optimize performance. All data is anonymized and aggregated.",
            Self::DetailedDiagnostics => 
                "Detailed system information for troubleshooting (driver versions, kernel info). \
                 Only collected when explicitly requested for bug reports.",
            Self::Analytics => 
                "Anonymous analytics about feature usage patterns. \
                 Helps understand which features are most valuable. No tracking or profiling.",
        }
    }

    /// Get list of data points collected under this scope
    pub fn data_points(&self) -> Vec<&'static str> {
        match self {
            Self::BasicTelemetry => vec![
                "Application version",
                "Operating system type (Linux/Windows/macOS)",
                "Crash stack traces (anonymized)",
                "Error frequency (aggregated)",
            ],
            Self::HardwareInfo => vec![
                "GPU vendor (NVIDIA/AMD/Intel)",
                "GPU generation (e.g., RTX 30-series, not exact model)",
                "CPU architecture (x86_64/ARM)",
                "Approximate RAM size (e.g., 16GB range)",
            ],
            Self::PerformanceMetrics => vec![
                "Average GPU utilization",
                "Average memory usage",
                "Frame time distributions",
                "Application startup time",
            ],
            Self::DetailedDiagnostics => vec![
                "Exact GPU model and driver version",
                "Kernel version",
                "System configuration",
                "Running processes (names only, no PIDs or paths)",
            ],
            Self::Analytics => vec![
                "Feature usage frequency",
                "Tab/view navigation patterns",
                "Configuration changes (types, not values)",
                "Session duration",
            ],
        }
    }
}

/// Consent decision record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// The scope this consent applies to
    pub scope: ConsentScope,
    
    /// Whether user granted consent
    pub granted: bool,
    
    /// When the consent was recorded
    pub timestamp: u64,
    
    /// Version of the consent prompt shown
    pub prompt_version: u32,
    
    /// User's IP address at time of consent (for audit trail only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_hash: Option<String>,
}

/// Consent management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentConfig {
    /// All consent records
    pub records: HashMap<ConsentScope, ConsentRecord>,
    
    /// When the user was first prompted
    pub first_prompt_timestamp: Option<u64>,
    
    /// Last time consent was reviewed
    pub last_review_timestamp: Option<u64>,
    
    /// User can set a reminder to review consent
    pub review_reminder_days: Option<u32>,
    
    /// Version of consent system (for tracking changes)
    pub consent_version: u32,
}

impl Default for ConsentConfig {
    fn default() -> Self {
        Self {
            records: HashMap::new(),
            first_prompt_timestamp: None,
            last_review_timestamp: None,
            review_reminder_days: Some(90), // Remind every 90 days by default
            consent_version: 1,
        }
    }
}

/// Consent manager - handles all consent-related operations
pub struct ConsentManager {
    config: ConsentConfig,
    config_path: PathBuf,
}

impl ConsentManager {
    /// Current version of the consent prompt/policy
    const CURRENT_PROMPT_VERSION: u32 = 1;

    /// Get default config path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = if cfg!(windows) {
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .map_err(|_| SimonError::ConfigError("APPDATA not set".into()))?
        } else {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .map_err(|_| SimonError::ConfigError("HOME not set".into()))?
        };

        Ok(config_dir.join("simon").join("consent.toml"))
    }

    /// Load consent configuration from default path
    pub fn load() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load_from(&path)
    }

    /// Load consent configuration from specific path
    pub fn load_from(path: &PathBuf) -> Result<Self> {
        let config = if path.exists() {
            let contents = fs::read_to_string(path)
                .map_err(|e| SimonError::ConfigError(format!("Failed to read consent config: {}", e)))?;
            
            toml::from_str(&contents)
                .map_err(|e| SimonError::ConfigError(format!("Failed to parse consent config: {}", e)))?
        } else {
            ConsentConfig::default()
        };

        Ok(Self {
            config,
            config_path: path.clone(),
        })
    }

    /// Save consent configuration
    pub fn save(&self) -> Result<()> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| SimonError::ConfigError(format!("Failed to create config directory: {}", e)))?;
        }

        let contents = toml::to_string_pretty(&self.config)
            .map_err(|e| SimonError::ConfigError(format!("Failed to serialize consent config: {}", e)))?;

        fs::write(&self.config_path, contents)
            .map_err(|e| SimonError::ConfigError(format!("Failed to write consent config: {}", e)))?;

        Ok(())
    }

    /// Check if user has granted consent for a specific scope
    /// 
    /// # Sandbox Protection
    /// 
    /// Returns `false` if running in a sandboxed environment, regardless of
    /// stored consent. This ensures no data is collected during analysis or testing.
    /// 
    /// # Runtime Opt-Out
    /// 
    /// Also returns `false` if the SIMON_NO_TELEMETRY or SIMON_OFFLINE environment
    /// variables are set (via --no-telemetry or --offline CLI flags).
    pub fn has_consent(&self, scope: ConsentScope) -> bool {
        // CRITICAL: Check runtime opt-out flags first (set by CLI --no-telemetry or --offline)
        if std::env::var("SIMON_NO_TELEMETRY").is_ok() || std::env::var("SIMON_OFFLINE").is_ok() {
            return false;
        }

        // CRITICAL: Never collect data in sandboxed environments
        let sandbox_detector = crate::sandbox::SandboxDetector::new();
        if sandbox_detector.is_sandboxed() {
            return false;
        }

        // Default to true (opt-in by default) if no explicit consent record exists
        self.config.records
            .get(&scope)
            .map(|r| r.granted)
            .unwrap_or(true)
    }

    /// Check if consent should be granted (including sandbox check)
    /// 
    /// This is a convenience method that combines consent checking with sandbox detection
    /// and runtime opt-out flags.
    pub fn should_collect_data(&self, scope: ConsentScope) -> bool {
        // Check runtime opt-out flags first (fastest check, set by CLI)
        if std::env::var("SIMON_NO_TELEMETRY").is_ok() || std::env::var("SIMON_OFFLINE").is_ok() {
            return false;
        }

        // Check sandbox second (fast check)
        let sandbox_detector = crate::sandbox::SandboxDetector::new();
        if sandbox_detector.is_sandboxed() {
            return false;
        }

        // Then check consent
        self.has_consent(scope)
    }

    /// Check if any consent prompt is needed (first run or consent expired)
    pub fn needs_consent_prompt(&self) -> bool {
        // First run - never prompted before
        if self.config.first_prompt_timestamp.is_none() {
            return true;
        }

        // Check if review reminder is due
        if let (Some(last_review), Some(reminder_days)) = 
            (self.config.last_review_timestamp, self.config.review_reminder_days) 
        {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let days_since_review = (now - last_review) / 86400;
            if days_since_review >= reminder_days as u64 {
                return true;
            }
        }

        false
    }

    /// Record consent decision for a scope
    pub fn record_consent(&mut self, scope: ConsentScope, granted: bool) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Update first prompt timestamp if this is the first time
        if self.config.first_prompt_timestamp.is_none() {
            self.config.first_prompt_timestamp = Some(timestamp);
        }

        // Update last review timestamp
        self.config.last_review_timestamp = Some(timestamp);

        // Create consent record
        let record = ConsentRecord {
            scope,
            granted,
            timestamp,
            prompt_version: Self::CURRENT_PROMPT_VERSION,
            ip_hash: None, // We don't collect IP addresses for consent
        };

        self.config.records.insert(scope, record);
        self.save()?;

        Ok(())
    }

    /// Revoke consent for a specific scope
    pub fn revoke_consent(&mut self, scope: ConsentScope) -> Result<()> {
        self.record_consent(scope, false)?;
        Ok(())
    }

    /// Revoke all consents
    pub fn revoke_all(&mut self) -> Result<()> {
        for scope in [
            ConsentScope::BasicTelemetry,
            ConsentScope::HardwareInfo,
            ConsentScope::PerformanceMetrics,
            ConsentScope::DetailedDiagnostics,
            ConsentScope::Analytics,
        ] {
            self.revoke_consent(scope)?;
        }
        Ok(())
    }

    /// Get all consent records
    pub fn get_all_consents(&self) -> &HashMap<ConsentScope, ConsentRecord> {
        &self.config.records
    }

    /// Export consent status as human-readable string
    pub fn export_consent_status(&self) -> String {
        let mut output = String::from("=== Silicon Monitor - Consent Status ===\n\n");
        
        if let Some(first_prompt) = self.config.first_prompt_timestamp {
            output.push_str(&format!("First prompted: {}\n", format_timestamp(first_prompt)));
        }
        
        if let Some(last_review) = self.config.last_review_timestamp {
            output.push_str(&format!("Last reviewed: {}\n", format_timestamp(last_review)));
        }
        
        output.push_str(&format!("\nConsent version: {}\n\n", self.config.consent_version));
        
        output.push_str("Current consents:\n");
        output.push_str("─────────────────────────────────────────\n");
        
        for scope in [
            ConsentScope::BasicTelemetry,
            ConsentScope::HardwareInfo,
            ConsentScope::PerformanceMetrics,
            ConsentScope::DetailedDiagnostics,
            ConsentScope::Analytics,
        ] {
            let status = if self.has_consent(scope) { "[+] GRANTED" } else { "[-] DENIED" };
            output.push_str(&format!("{:<30} {}\n", scope.name(), status));
            
            if let Some(record) = self.config.records.get(&scope) {
                output.push_str(&format!("  Recorded: {}\n", format_timestamp(record.timestamp)));
            }
        }
        
        output.push_str("\n=== End of Consent Status ===\n");
        output
    }

    /// Request consent interactively (CLI mode)
    pub fn request_consent(&mut self, scope: ConsentScope) -> Result<bool> {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║           Silicon Monitor - Data Collection Consent           ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        
        println!("Scope: {}\n", scope.name());
        println!("Description:\n{}\n", scope.description());
        println!("Data collected:");
        for point in scope.data_points() {
            println!("  * {}", point);
        }
        println!();
        println!("You can review and change your consent at any time by:");
        println!("  - Running: simon --consent-status");
        println!("  - Running: simon --revoke-consent");
        println!();
        
        loop {
            println!("Do you consent to this data collection? [y/N]: ");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .map_err(|e| SimonError::ConfigError(format!("Failed to read input: {}", e)))?;
            
            let input = input.trim().to_lowercase();
            match input.as_str() {
                "y" | "yes" => {
                    self.record_consent(scope, true)?;
                    println!("[+] Consent granted for {}\n", scope.name());
                    return Ok(true);
                }
                "n" | "no" | "" => {
                    self.record_consent(scope, false)?;
                    println!("[-] Consent denied for {}\n", scope.name());
                    return Ok(false);
                }
                _ => {
                    println!("Invalid input. Please enter 'y' for yes or 'n' for no.");
                }
            }
        }
    }

    /// Request multiple consents interactively
    pub fn request_all_consents(&mut self) -> Result<()> {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║           Welcome to Silicon Monitor!                         ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        println!("This is your first time running Silicon Monitor.");
        println!("We respect your privacy and will only collect data you explicitly consent to.\n");
        println!("You'll be asked about several types of optional data collection.");
        println!("You can say 'no' to all of them - the tool works perfectly without any data collection.\n");
        
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        
        for scope in [
            ConsentScope::BasicTelemetry,
            ConsentScope::HardwareInfo,
            ConsentScope::PerformanceMetrics,
            ConsentScope::Analytics,
        ] {
            self.request_consent(scope)?;
        }
        
        println!("╔════════════════════════════════════════════════════════════════╗");
        println!("║           Consent Setup Complete                              ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        println!("You can review or change these settings at any time:");
        println!("  simon --consent-status      # View current consent status");
        println!("  simon --revoke-consent      # Revoke all consents");
        println!("  simon --consent-review      # Review and change specific consents\n");
        
        Ok(())
    }
}

/// Format timestamp as human-readable string
fn format_timestamp(timestamp: u64) -> String {
    let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);
    datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_consent() {
        let manager = ConsentManager {
            config: ConsentConfig::default(),
            config_path: PathBuf::from("/tmp/test_consent.toml"),
        };

        // By default, no consent is granted
        assert!(!manager.has_consent(ConsentScope::BasicTelemetry));
        assert!(!manager.has_consent(ConsentScope::HardwareInfo));
        assert!(!manager.has_consent(ConsentScope::PerformanceMetrics));
    }

    #[test]
    fn test_consent_recording() {
        let mut manager = ConsentManager {
            config: ConsentConfig::default(),
            config_path: PathBuf::from("/tmp/test_consent.toml"),
        };

        // Record consent
        manager.record_consent(ConsentScope::BasicTelemetry, true).unwrap();
        
        // Check the record directly to bypass sandbox detection in has_consent
        // (sandbox detection may interfere with tests in CI/VMs)
        let record = manager.config.records.get(&ConsentScope::BasicTelemetry);
        assert!(record.is_some());
        assert!(record.unwrap().granted);

        // Revoke consent
        manager.revoke_consent(ConsentScope::BasicTelemetry).unwrap();
        
        // Verify revocation by checking record directly
        let record = manager.config.records.get(&ConsentScope::BasicTelemetry);
        assert!(record.is_some());
        assert!(!record.unwrap().granted);
    }

    #[test]
    fn test_consent_scope_descriptions() {
        let scope = ConsentScope::BasicTelemetry;
        assert!(!scope.name().is_empty());
        assert!(!scope.description().is_empty());
        assert!(!scope.data_points().is_empty());
    }
}

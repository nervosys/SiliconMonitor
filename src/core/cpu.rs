//! CPU monitoring

use serde::{Deserialize, Serialize};

/// CPU frequency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuFrequency {
    /// Current frequency in MHz
    pub current: u32,
    /// Minimum frequency in MHz
    pub min: u32,
    /// Maximum frequency in MHz
    pub max: u32,
}

/// Per-core CPU information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCore {
    /// Core ID
    pub id: usize,
    /// Online status
    pub online: bool,
    /// Governor type
    pub governor: String,
    /// Frequency information
    pub frequency: Option<CpuFrequency>,
    /// User mode utilization percentage
    pub user: Option<f32>,
    /// Nice mode utilization percentage
    pub nice: Option<f32>,
    /// System mode utilization percentage
    pub system: Option<f32>,
    /// Idle percentage
    pub idle: Option<f32>,
    /// CPU model name
    pub model: String,
}

/// Aggregate CPU totals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuTotal {
    /// Total user mode percentage
    pub user: f32,
    /// Total nice mode percentage
    pub nice: f32,
    /// Total system mode percentage
    pub system: f32,
    /// Total idle percentage
    pub idle: f32,
}

/// CPU statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats {
    /// Per-core information
    pub cores: Vec<CpuCore>,
    /// Aggregate totals
    pub total: CpuTotal,
}

impl CpuStats {
    /// A zeroed struct for a platform reader to fill in.
    ///
    /// **This reads nothing.** It returns no cores and 100% idle, and it was called `new()`
    /// until 6.0.0 — a name that reads like a constructor that gathers
    /// data. Two GUI defects, one in the HTTP server and one in the
    /// library's own `snapshot_cpu` came from exactly that misreading,
    /// found over three separate occasions.
    ///
    /// The real values come from the per-platform readers. Use those
    /// unless you are a reader building your own starting struct.
    pub fn empty() -> Self {
        Self {
            cores: Vec::new(),
            total: CpuTotal {
                user: 0.0,
                nice: 0.0,
                system: 0.0,
                idle: 100.0,
            },
        }
    }

    /// Get number of CPU cores
    pub fn core_count(&self) -> usize {
        self.cores.len()
    }

    /// Get number of online cores
    pub fn online_count(&self) -> usize {
        self.cores.iter().filter(|c| c.online).count()
    }
}

impl Default for CpuStats {
    fn default() -> Self {
        Self::empty()
    }
}

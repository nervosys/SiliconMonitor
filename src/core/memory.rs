//! Memory monitoring

use crate::error::Result;
use serde::{Deserialize, Serialize};

/// RAM information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamInfo {
    /// Total RAM in KB
    pub total: u64,
    /// Used RAM in KB
    pub used: u64,
    /// Free RAM in KB
    pub free: u64,
    /// Buffered RAM in KB
    pub buffers: u64,
    /// Cached RAM in KB
    pub cached: u64,
    /// Shared RAM in KB (GPU shared on Jetson)
    pub shared: u64,
    /// Large Free Blocks (4MB blocks on Jetson)
    pub lfb: Option<u32>,
}

/// SWAP information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapInfo {
    /// Total SWAP in KB
    pub total: u64,
    /// Used SWAP in KB
    pub used: u64,
    /// Cached SWAP in KB
    pub cached: u64,
}

/// EMC (External Memory Controller) information (Jetson only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmcInfo {
    /// EMC online status
    pub online: bool,
    /// Percentage of bandwidth used
    pub value: u32,
    /// Current frequency in kHz
    pub current: u32,
    /// Maximum frequency in kHz
    pub max: u32,
    /// Minimum frequency in kHz
    pub min: u32,
}

/// IRAM information (Jetson only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IramInfo {
    /// Total IRAM in KB
    pub total: u64,
    /// Used IRAM in KB
    pub used: u64,
    /// Large Free Blocks
    pub lfb: Option<u32>,
}

/// Memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// RAM information
    pub ram: RamInfo,
    /// SWAP information
    pub swap: SwapInfo,
    /// EMC information (Jetson only)
    pub emc: Option<EmcInfo>,
    /// IRAM information (Jetson only)
    pub iram: Option<IramInfo>,
}

impl MemoryStats {
    /// Create a new memory stats instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            ram: RamInfo {
                total: 0,
                used: 0,
                free: 0,
                buffers: 0,
                cached: 0,
                shared: 0,
                lfb: None,
            },
            swap: SwapInfo {
                total: 0,
                used: 0,
                cached: 0,
            },
            emc: None,
            iram: None,
        })
    }

    /// Get RAM usage percentage
    pub fn ram_usage_percent(&self) -> f32 {
        if self.ram.total == 0 {
            0.0
        } else {
            (self.ram.used as f32 / self.ram.total as f32) * 100.0
        }
    }

    /// Get SWAP usage percentage
    pub fn swap_usage_percent(&self) -> f32 {
        if self.swap.total == 0 {
            0.0
        } else {
            (self.swap.used as f32 / self.swap.total as f32) * 100.0
        }
    }
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            ram: RamInfo {
                total: 0,
                used: 0,
                free: 0,
                buffers: 0,
                cached: 0,
                shared: 0,
                lfb: None,
            },
            swap: SwapInfo {
                total: 0,
                used: 0,
                cached: 0,
            },
            emc: None,
            iram: None,
        })
    }
}

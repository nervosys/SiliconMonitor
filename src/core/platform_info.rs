//! Platform and hardware information

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// Model name
    pub model: String,
    /// P-Number (Jetson)
    pub p_number: Option<String>,
    /// Module name
    pub module: Option<String>,
    /// SoC name
    pub soc: Option<String>,
    /// CUDA architecture
    pub cuda_arch: Option<String>,
    /// Codename
    pub codename: Option<String>,
    /// Serial number
    pub serial_number: Option<String>,
    /// L4T version (Jetson)
    pub l4t: Option<String>,
    /// JetPack version (Jetson)
    pub jetpack: Option<String>,
}

/// Platform information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// Machine type
    pub machine: String,
    /// Operating system
    pub system: String,
    /// OS distribution
    pub distribution: Option<String>,
    /// Kernel release
    pub release: String,
}

/// Library versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryVersions {
    /// CUDA version
    pub cuda: Option<String>,
    /// cuDNN version
    pub cudnn: Option<String>,
    /// TensorRT version
    pub tensorrt: Option<String>,
    /// Other libraries
    pub other: HashMap<String, String>,
}

/// Complete board information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardInfo {
    /// Platform information
    pub platform: PlatformInfo,
    /// Hardware information
    pub hardware: HardwareInfo,
    /// Library versions
    pub libraries: LibraryVersions,
}

impl BoardInfo {
    /// Create a new board info instance
    pub fn new() -> Result<Self> {
        Ok(Self {
            platform: PlatformInfo {
                machine: String::new(),
                system: String::new(),
                distribution: None,
                release: String::new(),
            },
            hardware: HardwareInfo {
                model: String::new(),
                p_number: None,
                module: None,
                soc: None,
                cuda_arch: None,
                codename: None,
                serial_number: None,
                l4t: None,
                jetpack: None,
            },
            libraries: LibraryVersions {
                cuda: None,
                cudnn: None,
                tensorrt: None,
                other: HashMap::new(),
            },
        })
    }
}

impl Default for BoardInfo {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

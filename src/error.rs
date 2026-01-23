//! Error types for GPU Interface (gpui)

use std::io;
use thiserror::Error;

/// Result type alias for Simon operations (legacy compatibility)
pub type Result<T> = std::result::Result<T, SimonError>;

/// Legacy error type for backward compatibility
#[derive(Error, Debug)]
pub enum SimonError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// NVML error
    #[cfg(feature = "nvidia")]
    #[error("NVML error: {0}")]
    Nvml(#[from] nvml_wrapper::error::NvmlError),

    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),

    /// Device not found
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Unsupported platform
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// Feature not available
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),

    /// Invalid value
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    /// Command failed
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// System error
    #[error("System error: {0}")]
    System(String),

    /// Initialization error
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Network error (for remote backends)
    #[error("Network error: {0}")]
    Network(String),

    /// Agent/AI backend error
    #[error("Agent error: {0}")]
    Agent(String),

    /// Not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Configuration error (alias for ConfigError)
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// GPU-specific error
    #[error("GPU error: {0}")]
    GpuError(String),

    /// Process-related error
    #[error("Process error: {0}")]
    ProcessError(String),

    /// CPU-related error
    #[error("CPU error: {0}")]
    CpuError(String),

    /// Memory-related error
    #[error("Memory error: {0}")]
    MemoryError(String),

    /// Disk-related error
    #[error("Disk error: {0}")]
    DiskError(String),

    /// Network-related error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Hardware-related error
    #[error("Hardware error: {0}")]
    HardwareError(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Main error type for unified GPU interface
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// NVML error (NVIDIA GPUs)
    #[cfg(feature = "nvidia")]
    #[error("NVML error: {0}")]
    Nvml(#[from] nvml_wrapper::error::NvmlError),

    /// GPU-specific error
    #[error("GPU error: {0}")]
    GpuError(String),

    /// Process-related error
    #[error("Process error: {0}")]
    ProcessError(String),

    /// Feature not supported
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Device not found
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Parse error
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Command execution failed
    #[error("Command execution failed: {0}")]
    CommandExecutionFailed(String),

    /// Feature not supported
    #[error("Unsupported: {0}")]
    Unsupported(String),

    /// System error
    #[error("System error: {0}")]
    SystemError(String),

    /// Nix error (Unix)
    #[cfg(unix)]
    #[error("Nix error: {0}")]
    Nix(#[from] nix::Error),

    /// Legacy error for backward compatibility
    #[error("Legacy error: {0}")]
    Legacy(#[from] SimonError),

    /// Other error
    #[error("{0}")]
    Other(String),
}

impl From<Error> for SimonError {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(e) => SimonError::Io(e),
            #[cfg(feature = "nvidia")]
            Error::Nvml(e) => SimonError::Nvml(e),
            Error::GpuError(s)
            | Error::ProcessError(s)
            | Error::SystemError(s)
            | Error::CommandExecutionFailed(s)
            | Error::Other(s) => SimonError::Other(s),
            Error::NotSupported(s) | Error::Unsupported(s) => SimonError::FeatureNotAvailable(s),
            Error::PermissionDenied(s) => SimonError::PermissionDenied(s),
            Error::DeviceNotFound(s) => SimonError::DeviceNotFound(s),
            Error::InvalidParameter(s) | Error::ParseError(s) => SimonError::Parse(s),
            #[cfg(unix)]
            Error::Nix(e) => SimonError::System(e.to_string()),
            Error::Legacy(e) => e,
        }
    }
}

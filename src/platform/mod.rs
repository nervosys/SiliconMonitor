//! Platform-specific implementations

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(windows)]
pub mod windows;

// Common utilities
pub mod common;

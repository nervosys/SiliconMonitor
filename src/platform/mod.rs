//! Platform-specific implementations

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
mod windows_pdh;

// Not target-gated: the parsers are pure functions over captured command output,
// and gating them would mean their tests only ran on the one platform where no one
// here can run anything. See the module comment.
pub mod macos;

// Common utilities
pub mod common;

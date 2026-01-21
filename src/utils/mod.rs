//! Utility modules for advanced Jetson management

pub mod clocks;
pub mod power_mode;
pub mod swap;

mod security;
pub(crate) use security::verify_sudo_available;

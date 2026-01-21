//! Linux platform implementation

pub mod cpu;
pub mod gpu;
pub mod jetson;
pub mod memory;
pub mod platform_detect;
pub mod power;
pub mod temperature;

pub use cpu::read_cpu_stats;
pub use gpu::read_gpu_stats;
pub use memory::read_memory_stats;
pub use platform_detect::detect_platform;
pub use power::read_power_stats;
pub use temperature::read_temperature_stats;

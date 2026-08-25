//! Probe every reader that the ontology does not yet cover.
//!
//! Plan item F adds ontology clusters only for readers that demonstrably answer
//! on the machine at hand -- declaring a cluster for a reader that returns
//! nothing here would put entities in the graph that no test can confirm. This
//! example constructs each uncovered monitor and prints what it produced, so
//! the choice of which clusters to add is made from evidence rather than from
//! the module list.
//!
//! Run with `cargo run --example probe_readers --all-features`.

macro_rules! probe {
    ($name:literal, $ty:path, $count:expr) => {{
        match <$ty>::new() {
            Ok(m) => {
                let n: usize = ($count)(&m);
                println!("{:<22} ok      {}", $name, n);
            }
            Err(e) => println!("{:<22} err     {}", $name, e),
        }
    }};
}

fn main() {
    println!("{:<22} {:<7} items", "reader", "status");

    probe!(
        "input",
        simonlib::input::InputMonitor,
        |m: &simonlib::input::InputMonitor| m.devices().len()
    );
    probe!(
        "services",
        simonlib::services::ServiceMonitor,
        |m: &simonlib::services::ServiceMonitor| m.services().len()
    );
    probe!(
        "storage_controller",
        simonlib::storage_controller::StorageControllerMonitor,
        |m: &simonlib::storage_controller::StorageControllerMonitor| m.controllers().len()
    );
    probe!(
        "iommu",
        simonlib::iommu::IommuMonitor,
        |m: &simonlib::iommu::IommuMonitor| m.groups().len()
    );
    probe!(
        "interrupt_map",
        simonlib::interrupt_map::InterruptMapMonitor,
        |m: &simonlib::interrupt_map::InterruptMapMonitor| m.interrupts().len()
    );
    probe!(
        "io_scheduler",
        simonlib::io_scheduler::IoSchedulerMonitor,
        |m: &simonlib::io_scheduler::IoSchedulerMonitor| m.devices().len()
    );
    probe!(
        "dma_engine",
        simonlib::dma_engine::DmaEngineMonitor,
        |m: &simonlib::dma_engine::DmaEngineMonitor| m.controllers().len()
    );
    probe!(
        "gpu_topology",
        simonlib::gpu_topology::GpuTopologyMonitor,
        |m: &simonlib::gpu_topology::GpuTopologyMonitor| m.gpus().len()
    );
    probe!(
        "power_profile",
        simonlib::power_profile::PowerProfileMonitor,
        |m: &simonlib::power_profile::PowerProfileMonitor| m.power_plans().len()
    );
    probe!(
        "thermal_zone",
        simonlib::thermal_zone::ThermalZoneMonitor,
        |m: &simonlib::thermal_zone::ThermalZoneMonitor| m.zones().len()
    );
    probe!(
        "voltage_regulator",
        simonlib::voltage_regulator::VoltageRegulatorMonitor,
        |m: &simonlib::voltage_regulator::VoltageRegulatorMonitor| m.regulators().len()
    );
    probe!(
        "watchdog",
        simonlib::watchdog::WatchdogMonitor,
        |m: &simonlib::watchdog::WatchdogMonitor| m.devices().len()
    );
    probe!(
        "audio",
        simonlib::audio::AudioMonitor,
        |m: &simonlib::audio::AudioMonitor| m.devices().len()
    );
    probe!(
        "bluetooth",
        simonlib::bluetooth::BluetoothMonitor,
        |m: &simonlib::bluetooth::BluetoothMonitor| m.adapters().len()
    );
    probe!(
        "camera",
        simonlib::camera::CameraMonitor,
        |m: &simonlib::camera::CameraMonitor| m.cameras().len()
    );
    probe!(
        "codec",
        simonlib::codec::CodecMonitor,
        |m: &simonlib::codec::CodecMonitor| m.capabilities().len()
    );
    probe!(
        "printer",
        simonlib::printer::PrinterMonitor,
        |m: &simonlib::printer::PrinterMonitor| m.printers().len()
    );

    // Singleton reports rather than collections: one item when they construct.
    probe!(
        "kernel_params",
        simonlib::kernel_params::KernelParamsMonitor,
        |_: &simonlib::kernel_params::KernelParamsMonitor| 1
    );
    probe!(
        "memory_bandwidth",
        simonlib::memory_bandwidth::MemoryBandwidthMonitor,
        |_: &simonlib::memory_bandwidth::MemoryBandwidthMonitor| 1
    );
    probe!(
        "memory_topology",
        simonlib::memory_topology::MemoryTopologyMonitor,
        |m: &simonlib::memory_topology::MemoryTopologyMonitor| m.populated_dimms().len()
    );
    probe!(
        "cpu_microarch",
        simonlib::cpu_microarch::CpuMicroarchMonitor,
        |m: &simonlib::cpu_microarch::CpuMicroarchMonitor| m.supported_extensions().len()
    );
    probe!(
        "crypto_accel",
        simonlib::crypto_accel::CryptoAccelMonitor,
        |_: &simonlib::crypto_accel::CryptoAccelMonitor| 1
    );
    probe!(
        "interconnect",
        simonlib::interconnect::InterconnectMonitor,
        |m: &simonlib::interconnect::InterconnectMonitor| m.inter_socket_links().len()
    );
    probe!(
        "security_mitigations",
        simonlib::security_mitigations::SecurityMitigationsMonitor,
        |m: &simonlib::security_mitigations::SecurityMitigationsMonitor| m.unmitigated().len()
    );
}

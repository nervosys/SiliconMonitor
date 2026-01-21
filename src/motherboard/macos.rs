// macOS motherboard and system monitoring implementation
//
// Data sources:
// - IOKit IOPlatformExpertDevice - System information
// - IOKit AppleSMC - SMC sensors (temperatures, fans, voltages)
// - system_profiler SPHardwareDataType - Hardware information
// - system_profiler SPSoftwareDataType - OS information
// - kextstat - Loaded kernel extensions (drivers)

use super::traits::*;

/// macOS motherboard sensor (SMC-based)
pub struct MacSensor {
    name: String,
}

impl MacSensor {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

impl MotherboardDevice for MacSensor {
    fn name(&self) -> &str {
        &self.name
    }

    fn device_path(&self) -> Option<String> {
        Some("SMC".to_string())
    }

    fn temperature_sensors(&self) -> Result<Vec<TemperatureSensor>, Error> {
        // TODO: Implement via IOKit AppleSMC
        // SMC keys: TC0P (CPU proximity), TG0P (GPU proximity), Th0H (HDD), etc.
        Ok(Vec::new())
    }

    fn voltage_rails(&self) -> Result<Vec<VoltageRail>, Error> {
        // TODO: Implement via IOKit AppleSMC
        // SMC voltage keys
        Ok(Vec::new())
    }

    fn fans(&self) -> Result<Vec<FanInfo>, Error> {
        // TODO: Implement via IOKit AppleSMC
        // SMC fan keys: F0Ac (actual speed), F0Mn (minimum), F0Mx (maximum)
        Ok(Vec::new())
    }
}

/// Enumerate SMC sensors
pub fn enumerate() -> Result<Vec<Box<dyn MotherboardDevice>>, Error> {
    // TODO: Implement IOKit SMC enumeration
    Ok(Vec::new())
}

/// Get system information
pub fn get_system_info() -> Result<SystemInfo, Error> {
    // TODO: Implement via IOKit and system_profiler
    // IOKit IOPlatformExpertDevice properties: manufacturer, model, serial-number
    // system_profiler SPSoftwareDataType: System Version, Kernel Version
    // system_profiler SPHardwareDataType: Model Name, Processor, Cores

    Ok(SystemInfo {
        os_name: "macOS".to_string(),
        os_version: "Unknown".to_string(),
        kernel_version: None,
        architecture: std::env::consts::ARCH.to_string(),
        hostname: None,
        bios: BiosInfo {
            vendor: Some("Apple".to_string()),
            version: None,
            release_date: None,
            revision: None,
            firmware_type: FirmwareType::Uefi,
            secure_boot: None,
        },
        manufacturer: Some("Apple".to_string()),
        product_name: None,
        serial_number: None,
        uuid: None,
        board_vendor: Some("Apple".to_string()),
        board_name: None,
        board_version: None,
        cpu_name: None,
        cpu_cores: None,
        cpu_threads: None,
    })
}

/// Get driver/kext versions
pub fn get_driver_versions() -> Result<Vec<DriverInfo>, Error> {
    // TODO: Implement via kextstat parsing
    // Parse output for specific kexts (IOGraphics, IONVMe, etc.)
    Ok(Vec::new())
}

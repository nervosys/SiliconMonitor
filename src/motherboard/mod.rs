// Motherboard and system hardware monitoring
//
// This module provides:
// - Motherboard sensor monitoring (temperatures, voltages, fan speeds)
// - BIOS/UEFI information
// - System information (manufacturer, model, serial)
// - Hardware driver versions

pub mod traits;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

// Re-export key types
pub use traits::{
    AudioDeviceInfo, AudioDeviceType, BiosInfo, BluetoothDeviceInfo, DisplayOutputInfo,
    DisplayOutputType, DriverInfo, DriverType, Error, FanControl, FanInfo, MotherboardDevice,
    NetworkPortInfo, NetworkPortType, PcieDeviceInfo, PeripheralsInfo, SataDeviceInfo,
    SataMediaType, SensorReading, SensorType, SystemInfo, SystemTemperatures, TemperatureSensor,
    UsbDeviceInfo, UsbVersion, VoltageRail,
};

/// Enumerate all available motherboard devices/sensors
pub fn enumerate_sensors() -> Result<Vec<Box<dyn MotherboardDevice>>, Error> {
    #[cfg(target_os = "linux")]
    {
        linux::enumerate()
    }

    #[cfg(target_os = "windows")]
    {
        windows::enumerate()
    }

    #[cfg(target_os = "macos")]
    {
        macos::enumerate()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "Platform not supported for motherboard monitoring".into(),
        ))
    }
}

/// Get system information (OS, BIOS, hardware details)
pub fn get_system_info() -> Result<SystemInfo, Error> {
    #[cfg(target_os = "linux")]
    {
        linux::get_system_info()
    }

    #[cfg(target_os = "windows")]
    {
        windows::get_system_info()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_system_info()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "Platform not supported for system information".into(),
        ))
    }
}

/// Get installed driver versions
pub fn get_driver_versions() -> Result<Vec<DriverInfo>, Error> {
    #[cfg(target_os = "linux")]
    {
        linux::get_driver_versions()
    }

    #[cfg(target_os = "windows")]
    {
        windows::get_driver_versions()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_driver_versions()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "Platform not supported for driver information".into(),
        ))
    }
}

/// Get PCIe devices
pub fn get_pcie_devices() -> Result<Vec<PcieDeviceInfo>, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::get_pcie_devices()
    }

    #[cfg(target_os = "linux")]
    {
        linux::get_pcie_devices()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_pcie_devices()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "PCIe device enumeration not yet implemented for this platform".into(),
        ))
    }
}

/// Get SATA/storage devices
pub fn get_sata_devices() -> Result<Vec<SataDeviceInfo>, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::get_sata_devices()
    }

    #[cfg(target_os = "linux")]
    {
        linux::get_sata_devices()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_sata_devices()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "SATA device enumeration not yet implemented for this platform".into(),
        ))
    }
}

/// Get system temperatures
pub fn get_system_temperatures() -> Result<SystemTemperatures, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::get_system_temperatures()
    }

    #[cfg(target_os = "linux")]
    {
        linux::get_system_temperatures()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_system_temperatures()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "System temperature monitoring not yet implemented for this platform".into(),
        ))
    }
}

/// Get all peripheral devices (USB, display outputs, audio, etc.)
pub fn get_peripherals() -> Result<PeripheralsInfo, Error> {
    #[cfg(target_os = "windows")]
    {
        windows::get_peripherals()
    }

    #[cfg(target_os = "linux")]
    {
        linux::get_peripherals()
    }

    #[cfg(target_os = "macos")]
    {
        macos::get_peripherals()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        Err(Error::NotSupported(
            "Peripheral enumeration not yet implemented for this platform".into(),
        ))
    }
}

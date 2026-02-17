// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! IPMI/BMC Sensor Interface

use serde::{Deserialize, Serialize};
use std::process::Command;

/// IPMI sensor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpmiSensorType {
    Temperature,
    Voltage,
    Fan,
    Power,
    Current,
    Other,
}

/// An IPMI sensor reading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiSensor {
    pub name: String,
    pub sensor_type: IpmiSensorType,
    pub value: f64,
    pub unit: String,
    pub status: super::SensorStatus,
    pub lower_critical: Option<f64>,
    pub upper_critical: Option<f64>,
}

/// BMC (Baseboard Management Controller) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcInfo {
    pub firmware_version: String,
    pub manufacturer: String,
    pub ip_address: Option<String>,
    pub mac_address: Option<String>,
}

/// Power reading from IPMI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerReading {
    pub current_watts: f64,
    pub minimum_watts: Option<f64>,
    pub maximum_watts: Option<f64>,
    pub average_watts: Option<f64>,
}

/// System Event Log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelEntry {
    pub id: u32,
    pub timestamp: String,
    pub sensor: String,
    pub event: String,
    pub severity: SelSeverity,
}

/// SEL entry severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelSeverity {
    Info,
    Warning,
    Critical,
}

/// IPMI controller for reading sensors via ipmitool
pub struct IpmiController {
    host: Option<String>,
    user: Option<String>,
    pass: Option<String>,
}

impl IpmiController {
    pub fn new() -> Self {
        Self { host: None, user: None, pass: None }
    }

    pub fn remote(host: String, user: String, pass: String) -> Self {
        Self { host: Some(host), user: Some(user), pass: Some(pass) }
    }

    fn ipmitool_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let (Some(h), Some(u), Some(p)) = (&self.host, &self.user, &self.pass) {
            args.extend(["-H".into(), h.clone(), "-U".into(), u.clone(), "-P".into(), p.clone()]);
        }
        args
    }

    pub fn read_sensors(&self) -> Result<Vec<IpmiSensor>, super::DatacenterError> {
        let mut cmd = Command::new("ipmitool");
        cmd.args(self.ipmitool_args());
        cmd.args(["sensor", "list"]);

        let output = cmd.output().map_err(|e|
            super::DatacenterError::IpmiError(format!("Failed to run ipmitool: {}", e))
        )?;

        if !output.status.success() {
            return self.read_sensors_sysfs();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sensors = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
            if parts.len() < 4 { continue; }

            let name = parts[0].to_string();
            let value = parts[1].parse::<f64>().unwrap_or(0.0);
            let unit = parts[2].to_string();
            let status_str = parts[3];

            let sensor_type = match unit.to_lowercase().as_str() {
                u if u.contains("degrees") || u.contains("celsius") => IpmiSensorType::Temperature,
                u if u.contains("volts") => IpmiSensorType::Voltage,
                u if u.contains("rpm") => IpmiSensorType::Fan,
                u if u.contains("watts") => IpmiSensorType::Power,
                u if u.contains("amps") => IpmiSensorType::Current,
                _ => IpmiSensorType::Other,
            };

            let status = match status_str.to_lowercase().as_str() {
                "ok" => super::SensorStatus::Ok,
                s if s.contains("warn") => super::SensorStatus::Warning,
                s if s.contains("crit") => super::SensorStatus::Critical,
                s if s.contains("nr") => super::SensorStatus::NonRecoverable,
                _ => super::SensorStatus::Unknown,
            };

            let lower_critical = parts.get(5).and_then(|s| s.parse().ok());
            let upper_critical = parts.get(8).and_then(|s| s.parse().ok());

            sensors.push(IpmiSensor {
                name, sensor_type, value, unit, status, lower_critical, upper_critical,
            });
        }

        Ok(sensors)
    }

    fn read_sensors_sysfs(&self) -> Result<Vec<IpmiSensor>, super::DatacenterError> {
        let sensors = Vec::new();
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            use std::path::Path;
            let hwmon = Path::new("/sys/class/hwmon");
            if let Ok(entries) = fs::read_dir(hwmon) {
                for entry in entries.flatten() {
                    let name_path = entry.path().join("name");
                    if let Ok(name) = fs::read_to_string(&name_path) {
                        if name.trim().contains("ipmi") {
                            // Read temp/fan/voltage inputs
                            for i in 1..=16 {
                                let temp_path = entry.path().join(format!("temp{}_input", i));
                                if let Ok(val) = fs::read_to_string(&temp_path) {
                                    if let Ok(v) = val.trim().parse::<f64>() {
                                        let label = fs::read_to_string(entry.path().join(format!("temp{}_label", i)))
                                            .unwrap_or_else(|_| format!("Temp {}", i));
                                        sensors.push(IpmiSensor {
                                            name: label.trim().to_string(),
                                            sensor_type: IpmiSensorType::Temperature,
                                            value: v / 1000.0,
                                            unit: "degrees C".into(),
                                            status: super::SensorStatus::Ok,
                                            lower_critical: None,
                                            upper_critical: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(sensors)
    }

    pub fn bmc_info(&self) -> Result<BmcInfo, super::DatacenterError> {
        let mut cmd = Command::new("ipmitool");
        cmd.args(self.ipmitool_args());
        cmd.args(["bmc", "info"]);

        let output = cmd.output().map_err(|e|
            super::DatacenterError::IpmiError(format!("Failed to run ipmitool: {}", e))
        )?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut firmware = String::new();
        let mut manufacturer = String::new();

        for line in stdout.lines() {
            if line.contains("Firmware Revision") {
                firmware = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.contains("Manufacturer Name") {
                manufacturer = line.split(':').nth(1).unwrap_or("").trim().to_string();
            }
        }

        Ok(BmcInfo { firmware_version: firmware, manufacturer, ip_address: None, mac_address: None })
    }

    pub fn power_reading(&self) -> Result<PowerReading, super::DatacenterError> {
        let mut cmd = Command::new("ipmitool");
        cmd.args(self.ipmitool_args());
        cmd.args(["dcmi", "power", "reading"]);

        let output = cmd.output().map_err(|e|
            super::DatacenterError::IpmiError(format!("Failed to run ipmitool: {}", e))
        )?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current = 0.0;
        let mut minimum = None;
        let mut maximum = None;
        let mut average = None;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 2 {
                let val = parts[1].trim().split_whitespace().next()
                    .and_then(|v| v.parse::<f64>().ok());
                if line.contains("Instantaneous") || line.contains("Current") {
                    if let Some(v) = val { current = v; }
                } else if line.contains("Minimum") {
                    minimum = val;
                } else if line.contains("Maximum") {
                    maximum = val;
                } else if line.contains("Average") {
                    average = val;
                }
            }
        }

        Ok(PowerReading { current_watts: current, minimum_watts: minimum, maximum_watts: maximum, average_watts: average })
    }
}

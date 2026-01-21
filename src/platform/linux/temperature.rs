//! Linux temperature monitoring

use crate::core::temperature::{TemperatureSensor, TemperatureStats};
use crate::error::Result;
use crate::platform::common::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const THERMAL_ZONE_PATH: &str = "/sys/class/thermal";

/// Read temperature statistics
pub fn read_temperature_stats() -> Result<TemperatureStats> {
    let mut stats = TemperatureStats::new()?;
    
    // Read thermal zones
    stats.sensors = read_thermal_zones()?;
    
    Ok(stats)
}

fn read_thermal_zones() -> Result<HashMap<String, TemperatureSensor>> {
    let mut sensors = HashMap::new();
    
    if !Path::new(THERMAL_ZONE_PATH).exists() {
        return Ok(sensors);
    }
    
    for entry in fs::read_dir(THERMAL_ZONE_PATH)? {
        let entry = entry?;
        let path = entry.path();
        
        if !path.is_dir() {
            continue;
        }
        
        let filename = entry.file_name();
        let name_str = filename.to_string_lossy();
        
        if !name_str.starts_with("thermal_zone") {
            continue;
        }
        
        if let Ok(sensor) = read_thermal_zone(&path) {
            // Get zone name/type
            let zone_type = read_file_string(path.join("type"))
                .unwrap_or_else(|_| name_str.to_string());
            
            sensors.insert(zone_type, sensor);
        }
    }
    
    Ok(sensors)
}

fn read_thermal_zone(zone_path: &Path) -> Result<TemperatureSensor> {
    let temp_path = zone_path.join("temp");
    
    // Read temperature in millidegrees Celsius
    let temp_millidegrees = read_file_u32(&temp_path)?;
    let temp = temp_millidegrees as f32 / 1000.0;
    
    // Read trip points if available
    let mut max = None;
    let mut crit = None;
    
    // Try to read trip point 0 (usually max)
    if let Ok(trip0) = read_file_u32(zone_path.join("trip_point_0_temp")) {
        max = Some(trip0 as f32 / 1000.0);
    }
    
    // Try to read critical trip point
    if let Ok(trip_crit) = read_file_u32(zone_path.join("trip_point_1_temp")) {
        crit = Some(trip_crit as f32 / 1000.0);
    }
    
    Ok(TemperatureSensor {
        online: true,
        temp,
        max,
        crit,
    })
}

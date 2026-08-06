//! S.M.A.R.T. disk health monitoring — drive health, temperature, wear, predictions.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/block/*/device/`, `/sys/class/nvme/*/`, `smartctl` output
//! - **Windows**: Uses WMI (`MSFT_PhysicalDisk`, `MSFT_StorageReliabilityCounter`)
//! - **macOS**: Uses `smartmontools` or `diskutil info`
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::smart::SmartMonitor;
//!
//! let monitor = SmartMonitor::new().unwrap();
//! for disk in monitor.disks() {
//!     println!("{}: health={:?}", disk.device, disk.health);
//!     // Every counter is `Option`: `None` means the platform would not report it,
//!     // which on Windows is the unelevated case. It does not mean zero.
//!     match disk.temperature_celsius {
//!         Some(c) => println!("  temperature: {c}°C"),
//!         None => println!("  temperature: not reported (needs elevation on Windows)"),
//!     }
//!     if let Some(hours) = disk.power_on_hours {
//!         println!("  power-on hours: {hours}");
//!     }
//!     if let Some(wear) = disk.wear_leveling_percent {
//!         println!("  SSD wear: {wear}%");
//!     }
//! }
//! ```

use crate::error::SimonError;
use serde::{Deserialize, Serialize};

/// Overall disk health assessment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskHealth {
    /// All attributes within normal ranges
    Good,
    /// Some attributes approaching thresholds
    Warning,
    /// Critical attributes exceeded, failure likely
    Critical,
    /// S.M.A.R.T. test failed
    Failed,
    /// Health could not be determined
    Unknown,
}

/// Drive media type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriveMediaType {
    SSD,
    HDD,
    NVMe,
    Hybrid,
    Unknown,
}

/// A single S.M.A.R.T. attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAttribute {
    /// Attribute ID (e.g., 5 = Reallocated Sectors)
    pub id: u16,
    /// Attribute name
    pub name: String,
    /// Current normalized value (1-253, higher = better)
    pub value: u64,
    /// Worst recorded value
    pub worst: u64,
    /// Threshold for failure
    pub threshold: u64,
    /// Raw value
    pub raw_value: u64,
    /// Whether this attribute is pre-fail (true) or old-age (false)
    pub pre_fail: bool,
}

/// S.M.A.R.T. data for a single disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartDiskInfo {
    /// Device path (e.g., "/dev/sda", "\\.\PhysicalDrive0")
    pub device: String,
    /// Device model name
    pub model: String,
    /// Serial number
    pub serial: String,
    /// Firmware version
    pub firmware: String,
    /// Drive media type
    pub media_type: DriveMediaType,
    /// Capacity in bytes
    pub capacity_bytes: u64,
    /// Overall health assessment
    pub health: DiskHealth,
    // Every counter below is `Option` because "the platform would not tell us" and
    // "the drive reports zero" are different facts, and this struct used to render
    // both as `0`.
    //
    // On Windows these come from `Get-StorageReliabilityCounter`, which requires
    // elevation: unelevated it fails with `PermissionDenied`. That failure was
    // being swallowed and replaced with zeros, so simon reported every drive as
    // healthy with a temperature of 0 °C and zero power-on hours — an access
    // error presented as a measurement.
    /// Current temperature in Celsius, if the platform reported one
    pub temperature_celsius: Option<u32>,
    /// Power-on hours
    pub power_on_hours: Option<u64>,
    /// Power cycle count
    pub power_cycle_count: Option<u64>,
    /// Reallocated sector count (HDD/SSD)
    pub reallocated_sectors: Option<u64>,
    /// Pending sector count
    pub pending_sectors: Option<u64>,
    /// Uncorrectable error count
    pub uncorrectable_errors: Option<u64>,
    /// SSD wear leveling percentage used (0-100, None for HDD)
    pub wear_leveling_percent: Option<f32>,
    /// Total bytes written (lifetime)
    pub total_bytes_written: Option<u64>,
    /// Total bytes read (lifetime)
    pub total_bytes_read: Option<u64>,
    /// NVMe percentage used (0-100, None for SATA)
    pub nvme_percentage_used: Option<u8>,
    /// NVMe available spare percentage
    pub nvme_available_spare: Option<u8>,
    /// All raw S.M.A.R.T. attributes
    pub attributes: Vec<SmartAttribute>,
    /// Estimated remaining life percentage (0-100)
    pub estimated_life_remaining: Option<f32>,
    /// Estimated days until failure (if predictable)
    pub estimated_days_remaining: Option<u32>,
}

/// Monitor for S.M.A.R.T. disk health
pub struct SmartMonitor {
    disks: Vec<SmartDiskInfo>,
}

impl SmartMonitor {
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self { disks: Vec::new() };
        monitor.refresh()?;
        Ok(monitor)
    }

    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.disks.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        // Run inference on all collected disks
        for disk in &mut self.disks {
            Self::infer_health(disk);
        }

        Ok(())
    }

    pub fn disks(&self) -> &[SmartDiskInfo] {
        &self.disks
    }

    /// Get disks with health warnings or worse.
    pub fn unhealthy_disks(&self) -> Vec<&SmartDiskInfo> {
        self.disks
            .iter()
            .filter(|d| {
                matches!(
                    d.health,
                    DiskHealth::Warning | DiskHealth::Critical | DiskHealth::Failed
                )
            })
            .collect()
    }

    /// Get the hottest disk temperature, if any disk reported one.
    pub fn max_temperature(&self) -> Option<u32> {
        self.disks
            .iter()
            .filter_map(|d| d.temperature_celsius)
            .max()
    }

    /// Infer health status and remaining life from raw attributes.
    ///
    /// Scoring only reflects evidence that was actually read. Previously every
    /// counter was a plain integer, so a drive whose counters could not be read at
    /// all arrived here as zeros, took no penalties, scored 100 and was declared
    /// `Good` — a confident verdict derived from nothing. Where no factor applies,
    /// this now leaves whatever the platform itself reported (Windows supplies a
    /// `HealthStatus`) rather than overwriting it with a computed judgement.
    fn infer_health(disk: &mut SmartDiskInfo) {
        let mut score: f32 = 100.0;
        let mut had_evidence = false;

        // Factor 1: Reallocated sectors (very bad for HDDs, concerning for SSDs)
        if let Some(reallocated) = disk.reallocated_sectors.filter(|n| *n > 0) {
            had_evidence = true;
            let penalty = match disk.media_type {
                DriveMediaType::HDD => (reallocated as f32 * 2.0).min(40.0),
                _ => (reallocated as f32 * 0.5).min(20.0),
            };
            score -= penalty;
        }

        // Factor 2: Pending sectors
        if let Some(pending) = disk.pending_sectors.filter(|n| *n > 0) {
            had_evidence = true;
            score -= (pending as f32 * 3.0).min(30.0);
        }

        // Factor 3: Uncorrectable errors
        if let Some(errors) = disk.uncorrectable_errors.filter(|n| *n > 0) {
            had_evidence = true;
            score -= (errors as f32 * 5.0).min(40.0);
        }

        // Factor 4: Temperature (above 55°C is concerning, above 70°C is critical)
        if let Some(temp) = disk.temperature_celsius {
            had_evidence = true;
            if temp > 70 {
                score -= 20.0;
            } else if temp > 55 {
                score -= 5.0;
            }
        }

        // Factor 5: SSD wear leveling
        if let Some(wear) = disk.wear_leveling_percent {
            had_evidence = true;
            if wear > 90.0 {
                score -= 30.0;
            } else if wear > 75.0 {
                score -= 10.0;
            }
        }

        // Factor 6: NVMe percentage used
        if let Some(pct) = disk.nvme_percentage_used {
            had_evidence = true;
            if pct > 90 {
                score -= 30.0;
            } else if pct > 75 {
                score -= 10.0;
            }
        }

        // Factor 7: NVMe available spare
        if let Some(spare) = disk.nvme_available_spare {
            had_evidence = true;
            if spare < 10 {
                score -= 25.0;
            } else if spare < 25 {
                score -= 10.0;
            }
        }

        // Factor 8: Power-on hours (age penalty)
        // HDDs: 30k+ hours is old; SSDs: 50k+ hours is old
        let age_threshold = match disk.media_type {
            DriveMediaType::HDD => 30_000,
            _ => 50_000,
        };
        if let Some(hours) = disk.power_on_hours {
            had_evidence = true;
            if hours > age_threshold * 2 {
                score -= 15.0;
            } else if hours > age_threshold {
                score -= 5.0;
            }
        }

        // With nothing to go on, keep the platform's own verdict and publish no
        // life estimate. A score of 100 here would mean "no penalties applied",
        // which is not the same as "healthy".
        if !had_evidence {
            return;
        }

        // Determine health from score
        disk.health = if score >= 80.0 {
            DiskHealth::Good
        } else if score >= 50.0 {
            DiskHealth::Warning
        } else if score >= 20.0 {
            DiskHealth::Critical
        } else {
            DiskHealth::Failed
        };

        // Estimate remaining life
        disk.estimated_life_remaining = Some(score.max(0.0));

        // Estimate days remaining via wear rate extrapolation
        if let Some(hours) = disk.power_on_hours.filter(|h| *h > 100) {
            if let Some(wear) = disk.wear_leveling_percent {
                if wear > 5.0 {
                    let hours_per_percent = hours as f32 / wear;
                    let remaining_percent = 100.0 - wear;
                    let remaining_hours = hours_per_percent * remaining_percent;
                    disk.estimated_days_remaining = Some((remaining_hours / 24.0) as u32);
                }
            } else if let Some(pct) = disk.nvme_percentage_used {
                if pct > 5 {
                    let hours_per_percent = hours as f32 / pct as f32;
                    let remaining_percent = 100.0 - pct as f32;
                    let remaining_hours = hours_per_percent * remaining_percent;
                    disk.estimated_days_remaining = Some((remaining_hours / 24.0) as u32);
                }
            }
        }

        // Check SMART attributes for pre-fail conditions
        for attr in &disk.attributes {
            if attr.pre_fail && attr.value <= attr.threshold && attr.threshold > 0 {
                disk.health = DiskHealth::Critical;
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        // NVMe drives from /sys/class/nvme
        let nvme_base = std::path::Path::new("/sys/class/nvme");
        if nvme_base.exists() {
            if let Ok(entries) = std::fs::read_dir(nvme_base) {
                for entry in entries.flatten() {
                    let ctrl_name = entry.file_name().to_string_lossy().to_string();
                    let base = entry.path();
                    let model = Self::read_sys(&base.join("model"));
                    let serial = Self::read_sys(&base.join("serial"));
                    let firmware = Self::read_sys(&base.join("firmware_rev"));

                    // Try to read smart-log from namespace
                    let device = format!("/dev/{}", ctrl_name);
                    let mut disk = SmartDiskInfo {
                        device,
                        model,
                        serial,
                        firmware,
                        media_type: DriveMediaType::NVMe,
                        capacity_bytes: 0,
                        health: DiskHealth::Unknown,
                        temperature_celsius: None,
                        power_on_hours: None,
                        power_cycle_count: None,
                        reallocated_sectors: None,
                        pending_sectors: None,
                        uncorrectable_errors: None,
                        wear_leveling_percent: None,
                        total_bytes_written: None,
                        total_bytes_read: None,
                        nvme_percentage_used: None,
                        nvme_available_spare: None,
                        attributes: Vec::new(),
                        estimated_life_remaining: None,
                        estimated_days_remaining: None,
                    };

                    // Try nvme smart-log
                    if let Ok(output) = std::process::Command::new("nvme")
                        .args(["smart-log", &format!("/dev/{}n1", ctrl_name), "-o", "json"])
                        .output()
                    {
                        if let Ok(text) = String::from_utf8(output.stdout) {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                // Absent keys stay `None`: `nvme smart-log` omits
                                // what the drive did not report, and the previous
                                // `.unwrap_or(0)` turned every omission into a
                                // reading of zero.
                                disk.temperature_celsius = val["temperature"]
                                    .as_u64()
                                    .or_else(|| val["composite_temperature"].as_u64())
                                    .map(|t| if t > 273 { (t - 273) as u32 } else { t as u32 });
                                disk.power_on_hours = val["power_on_hours"].as_u64();
                                disk.power_cycle_count = val["power_cycles"].as_u64();
                                disk.nvme_percentage_used =
                                    val["percent_used"].as_u64().map(|v| v as u8);
                                disk.nvme_available_spare =
                                    val["avail_spare"].as_u64().map(|v| v as u8);
                                disk.uncorrectable_errors = val["media_errors"].as_u64();
                                // NVMe reports data units of 512 bytes, in
                                // thousands.
                                disk.total_bytes_written = val["data_units_written"]
                                    .as_u64()
                                    .map(|units| units * 512_000);
                                disk.total_bytes_read =
                                    val["data_units_read"].as_u64().map(|units| units * 512_000);
                            }
                        }
                    }

                    self.disks.push(disk);
                }
            }
        }

        // SATA/SCSI drives from /sys/block
        if let Ok(entries) = std::fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("sd") && !name.starts_with("hd") {
                    continue;
                }
                let base = entry.path();
                let device = format!("/dev/{}", name);

                // Detect rotational (1=HDD, 0=SSD)
                let rotational = Self::read_sys(&base.join("queue/rotational"));
                let media_type = if rotational == "0" {
                    DriveMediaType::SSD
                } else {
                    DriveMediaType::HDD
                };

                let size_sectors: u64 = Self::read_sys(&base.join("size")).parse().unwrap_or(0);
                let capacity_bytes = size_sectors * 512;

                let model = Self::read_sys(&base.join("device/model"));
                let serial = Self::read_sys(&base.join("device/serial"))
                    .chars()
                    .filter(|c| !c.is_whitespace() || *c == ' ')
                    .collect::<String>()
                    .trim()
                    .to_string();

                let mut disk = SmartDiskInfo {
                    device: device.clone(),
                    model,
                    serial,
                    firmware: Self::read_sys(&base.join("device/firmware_rev")),
                    media_type,
                    capacity_bytes,
                    health: DiskHealth::Unknown,
                    temperature_celsius: None,
                    power_on_hours: None,
                    power_cycle_count: None,
                    reallocated_sectors: None,
                    pending_sectors: None,
                    uncorrectable_errors: None,
                    wear_leveling_percent: None,
                    total_bytes_written: None,
                    total_bytes_read: None,
                    nvme_percentage_used: None,
                    nvme_available_spare: None,
                    attributes: Vec::new(),
                    estimated_life_remaining: None,
                    estimated_days_remaining: None,
                };

                // Try smartctl for detailed attributes
                if let Ok(output) = std::process::Command::new("smartctl")
                    .args(["-A", "-j", &device])
                    .output()
                {
                    if let Ok(text) = String::from_utf8(output.stdout) {
                        Self::parse_smartctl_json(&text, &mut disk);
                    }
                }

                self.disks.push(disk);
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn parse_smartctl_json(text: &str, disk: &mut SmartDiskInfo) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(attrs) = val["ata_smart_attributes"]["table"].as_array() {
                for attr in attrs {
                    let id = attr["id"].as_u64().unwrap_or(0) as u16;
                    let name = attr["name"].as_str().unwrap_or("").to_string();
                    let value = attr["value"].as_u64().unwrap_or(0);
                    let worst = attr["worst"].as_u64().unwrap_or(0);
                    let thresh = attr["thresh"].as_u64().unwrap_or(0);
                    let raw_val = attr["raw"]["value"].as_u64().unwrap_or(0);
                    let pre_fail = attr["flags"]["prefailure"].as_bool().unwrap_or(false);

                    // Each arm here means the drive actually published that
                    // attribute, so `Some` is the truthful wrapper — unlike the
                    // struct's defaults, which stay `None` for attributes the
                    // drive never reported.
                    match id {
                        5 => disk.reallocated_sectors = Some(raw_val), // Reallocated_Sector_Ct
                        9 => disk.power_on_hours = Some(raw_val),      // Power_On_Hours
                        12 => disk.power_cycle_count = Some(raw_val),  // Power_Cycle_Count
                        177 | 233 => {
                            // Wear_Leveling_Count or Media_Wearout_Indicator
                            disk.wear_leveling_percent = Some(100.0 - value as f32);
                        }
                        194 | 190 => disk.temperature_celsius = Some(raw_val as u32), // Temperature
                        197 => disk.pending_sectors = Some(raw_val), // Current_Pending_Sector
                        198 => disk.uncorrectable_errors = Some(raw_val), // Offline_Uncorrectable
                        241 => disk.total_bytes_written = Some(raw_val * 512), // Total_LBAs_Written
                        242 => disk.total_bytes_read = Some(raw_val * 512), // Total_LBAs_Read
                        _ => {}
                    }

                    disk.attributes.push(SmartAttribute {
                        id,
                        name,
                        value,
                        worst,
                        threshold: thresh,
                        raw_value: raw_val,
                        pre_fail,
                    });
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn read_sys(path: &std::path::Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        // Use Get-PhysicalDisk + Get-StorageReliabilityCounter
        if let Ok(output) = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                r#"
Get-PhysicalDisk | ForEach-Object {
    $disk = $_
    $rel = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
    [PSCustomObject]@{
        DeviceId = $disk.DeviceId
        FriendlyName = $disk.FriendlyName
        Model = $disk.Model
        SerialNumber = $disk.SerialNumber
        FirmwareVersion = $disk.FirmwareVersion
        MediaType = $disk.MediaType
        BusType = $disk.BusType
        Size = $disk.Size
        HealthStatus = $disk.HealthStatus
        Temperature = if ($rel) { $rel.Temperature } else { $null }
        PowerOnHours = if ($rel) { $rel.PowerOnHours } else { $null }
        PowerCycleCount = if ($rel) { $rel.StartStopCycleCount } else { $null }
        ReadErrorsTotal = if ($rel) { $rel.ReadErrorsTotal } else { $null }
        WriteErrorsTotal = if ($rel) { $rel.WriteErrorsTotal } else { $null }
        Wear = if ($rel) { $rel.Wear } else { $null }
    }
} | ConvertTo-Json -Compress
"#,
            ])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for item in &items {
                        // Windows answers these separately: MediaType describes the
                        // storage medium ("SSD"), BusType the transport ("NVMe").
                        // An NVMe drive therefore reports MediaType=SSD, and
                        // matching on MediaType alone classified every NVMe drive
                        // on this machine as a plain SSD — which then made
                        // `nvme_info()` refuse to answer for them.
                        let media_str = item["MediaType"].as_str().unwrap_or("");
                        let bus_str = item["BusType"].as_str().unwrap_or("");
                        let media_type = if bus_str.eq_ignore_ascii_case("nvme") {
                            DriveMediaType::NVMe
                        } else {
                            match media_str {
                                "SSD" => DriveMediaType::SSD,
                                "HDD" => DriveMediaType::HDD,
                                "NVMe" | "SCM" => DriveMediaType::NVMe,
                                _ => {
                                    // Infer from model name
                                    let model = item["Model"].as_str().unwrap_or("").to_lowercase();
                                    if model.contains("nvme") {
                                        DriveMediaType::NVMe
                                    } else if model.contains("ssd") {
                                        DriveMediaType::SSD
                                    } else {
                                        DriveMediaType::Unknown
                                    }
                                }
                            }
                        };

                        let health_str = item["HealthStatus"].as_str().unwrap_or("");
                        let health = match health_str {
                            "Healthy" => DiskHealth::Good,
                            "Warning" => DiskHealth::Warning,
                            "Unhealthy" => DiskHealth::Critical,
                            _ => DiskHealth::Unknown,
                        };

                        let wear = item["Wear"].as_u64().map(|w| w as f32);

                        // `DeviceId` arrives as a JSON *string* ("1"), not a
                        // number. `as_u64()` returned None for every disk and the
                        // fallback made them all PhysicalDrive0 — four drives with
                        // one name, indistinguishable to any caller.
                        let device_id = item["DeviceId"]
                            .as_u64()
                            .or_else(|| item["DeviceId"].as_str()?.trim().parse().ok());
                        let Some(device_id) = device_id else {
                            // Without an id there is no way to name the device or
                            // match it to anything; a placeholder would collide.
                            continue;
                        };

                        self.disks.push(SmartDiskInfo {
                            device: format!(r"\\.\PhysicalDrive{device_id}"),
                            model: item["Model"].as_str().unwrap_or("").trim().to_string(),
                            serial: item["SerialNumber"]
                                .as_str()
                                .unwrap_or("")
                                .trim()
                                .to_string(),
                            firmware: item["FirmwareVersion"]
                                .as_str()
                                .unwrap_or("")
                                .trim()
                                .to_string(),
                            media_type,
                            capacity_bytes: item["Size"].as_u64().unwrap_or(0),
                            health,
                            // Null throughout when Get-StorageReliabilityCounter
                            // was unavailable, which is the unelevated case. These
                            // stay None rather than becoming zero.
                            temperature_celsius: item["Temperature"].as_u64().map(|t| t as u32),
                            power_on_hours: item["PowerOnHours"].as_u64(),
                            power_cycle_count: item["PowerCycleCount"].as_u64(),
                            // SATA reallocated/pending sector counts are not
                            // exposed by the Windows storage stack at all; they
                            // need a SMART passthrough this path does not make.
                            reallocated_sectors: None,
                            pending_sectors: None,
                            uncorrectable_errors: match (
                                item["ReadErrorsTotal"].as_u64(),
                                item["WriteErrorsTotal"].as_u64(),
                            ) {
                                (None, None) => None,
                                (r, w) => Some(r.unwrap_or(0) + w.unwrap_or(0)),
                            },
                            wear_leveling_percent: wear,
                            total_bytes_written: None,
                            total_bytes_read: None,
                            nvme_percentage_used: wear.map(|w| w as u8),
                            nvme_available_spare: None,
                            attributes: Vec::new(),
                            estimated_life_remaining: None,
                            estimated_days_remaining: None,
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        // Try smartmontools
        if let Ok(output) = std::process::Command::new("smartctl")
            .args(["--scan", "-j"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(devices) = val["devices"].as_array() {
                        for dev in devices {
                            let name = dev["name"].as_str().unwrap_or_default();
                            if name.is_empty() {
                                continue;
                            }

                            let mut disk = SmartDiskInfo {
                                device: name.to_string(),
                                model: String::new(),
                                serial: String::new(),
                                firmware: String::new(),
                                media_type: DriveMediaType::Unknown,
                                capacity_bytes: 0,
                                health: DiskHealth::Unknown,
                                temperature_celsius: None,
                                power_on_hours: None,
                                power_cycle_count: None,
                                reallocated_sectors: None,
                                pending_sectors: None,
                                uncorrectable_errors: None,
                                wear_leveling_percent: None,
                                total_bytes_written: None,
                                total_bytes_read: None,
                                nvme_percentage_used: None,
                                nvme_available_spare: None,
                                attributes: Vec::new(),
                                estimated_life_remaining: None,
                                estimated_days_remaining: None,
                            };

                            let dev_type = dev["type"].as_str().unwrap_or("");
                            disk.media_type = if dev_type.contains("nvme") {
                                DriveMediaType::NVMe
                            } else {
                                DriveMediaType::Unknown
                            };

                            // Get info
                            if let Ok(info_out) = std::process::Command::new("smartctl")
                                .args(["-i", "-A", "-j", name])
                                .output()
                            {
                                if let Ok(info_text) = String::from_utf8(info_out.stdout) {
                                    if let Ok(info) =
                                        serde_json::from_str::<serde_json::Value>(&info_text)
                                    {
                                        disk.model =
                                            info["model_name"].as_str().unwrap_or("").to_string();
                                        disk.serial = info["serial_number"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        disk.firmware = info["firmware_version"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string();
                                        if let Some(temp) = info["temperature"]["current"].as_u64()
                                        {
                                            disk.temperature_celsius = Some(temp as u32);
                                        }
                                        if let Some(hours) = info["power_on_time"]["hours"].as_u64()
                                        {
                                            disk.power_on_hours = Some(hours);
                                        }
                                    }
                                }
                            }

                            self.disks.push(disk);
                        }
                    }
                }
            }
        }

        // Fallback: diskutil
        if self.disks.is_empty() {
            if let Ok(output) = std::process::Command::new("diskutil")
                .args(["list", "-plist"])
                .output()
            {
                // Basic enumeration — just record device names
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                for line in text.lines() {
                    if line.contains("/dev/disk") {
                        let device = line.trim().to_string();
                        if !device.contains("s")
                            || device.ends_with("disk0")
                            || device.ends_with("disk1")
                        {
                            self.disks.push(SmartDiskInfo {
                                device,
                                model: String::new(),
                                serial: String::new(),
                                firmware: String::new(),
                                media_type: DriveMediaType::Unknown,
                                capacity_bytes: 0,
                                health: DiskHealth::Unknown,
                                temperature_celsius: None,
                                power_on_hours: None,
                                power_cycle_count: None,
                                reallocated_sectors: None,
                                pending_sectors: None,
                                uncorrectable_errors: None,
                                wear_leveling_percent: None,
                                total_bytes_written: None,
                                total_bytes_read: None,
                                nvme_percentage_used: None,
                                nvme_available_spare: None,
                                attributes: Vec::new(),
                                estimated_life_remaining: None,
                                estimated_days_remaining: None,
                            });
                        }
                    }
                }
            }
        }
    }
}

impl Default for SmartMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self { disks: Vec::new() })
    }
}

impl std::fmt::Display for DiskHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Good => write!(f, "Good"),
            Self::Warning => write!(f, "Warning"),
            Self::Critical => write!(f, "Critical"),
            Self::Failed => write!(f, "Failed"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_monitor_creation() {
        let monitor = SmartMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_smart_monitor_default() {
        let monitor = SmartMonitor::default();
        let _ = monitor.disks();
        let _ = monitor.unhealthy_disks();
        let _ = monitor.max_temperature();
    }

    #[test]
    fn test_health_inference() {
        let mut disk = SmartDiskInfo {
            device: "test".into(),
            model: "Test SSD".into(),
            serial: "ABC".into(),
            firmware: "1.0".into(),
            media_type: DriveMediaType::SSD,
            capacity_bytes: 500_000_000_000,
            health: DiskHealth::Unknown,
            temperature_celsius: Some(35),
            power_on_hours: Some(5000),
            power_cycle_count: Some(1000),
            reallocated_sectors: None,
            pending_sectors: None,
            uncorrectable_errors: None,
            wear_leveling_percent: Some(10.0),
            total_bytes_written: None,
            total_bytes_read: None,
            nvme_percentage_used: None,
            nvme_available_spare: None,
            attributes: Vec::new(),
            estimated_life_remaining: None,
            estimated_days_remaining: None,
        };
        SmartMonitor::infer_health(&mut disk);
        assert_eq!(disk.health, DiskHealth::Good);
        assert!(disk.estimated_life_remaining.unwrap() > 80.0);
    }

    #[test]
    fn test_critical_health() {
        let mut disk = SmartDiskInfo {
            device: "test".into(),
            model: "Test HDD".into(),
            serial: "DEF".into(),
            firmware: "2.0".into(),
            media_type: DriveMediaType::HDD,
            capacity_bytes: 1_000_000_000_000,
            health: DiskHealth::Unknown,
            temperature_celsius: Some(72),
            power_on_hours: Some(80000),
            power_cycle_count: Some(5000),
            reallocated_sectors: Some(50),
            pending_sectors: Some(10),
            uncorrectable_errors: Some(5),
            wear_leveling_percent: None,
            total_bytes_written: None,
            total_bytes_read: None,
            nvme_percentage_used: None,
            nvme_available_spare: None,
            attributes: Vec::new(),
            estimated_life_remaining: None,
            estimated_days_remaining: None,
        };
        SmartMonitor::infer_health(&mut disk);
        assert!(matches!(
            disk.health,
            DiskHealth::Critical | DiskHealth::Failed
        ));
    }

    #[test]
    fn test_serialization() {
        let disk = SmartDiskInfo {
            device: "/dev/sda".into(),
            model: "Samsung".into(),
            serial: "S123".into(),
            firmware: "1.0".into(),
            media_type: DriveMediaType::SSD,
            capacity_bytes: 500_000_000_000,
            health: DiskHealth::Good,
            temperature_celsius: Some(35),
            power_on_hours: Some(1000),
            power_cycle_count: Some(100),
            reallocated_sectors: None,
            pending_sectors: None,
            uncorrectable_errors: None,
            wear_leveling_percent: Some(5.0),
            total_bytes_written: Some(1_000_000_000),
            total_bytes_read: Some(2_000_000_000),
            nvme_percentage_used: None,
            nvme_available_spare: None,
            attributes: Vec::new(),
            estimated_life_remaining: Some(95.0),
            estimated_days_remaining: Some(3650),
        };
        let json = serde_json::to_string(&disk).unwrap();
        assert!(json.contains("Samsung"));
        let _: SmartDiskInfo = serde_json::from_str(&json).unwrap();
    }

    /// A drive whose counters could not be read must not be graded.
    ///
    /// Every counter used to be a plain integer, so a drive whose reliability
    /// counters were unavailable — the unelevated Windows case, which is the
    /// common one — arrived at `infer_health` as zeros. Zero reallocated sectors,
    /// zero errors and zero degrees took no penalties, scored 100, and produced a
    /// confident `Good` plus an "estimated life remaining: 100%" derived from no
    /// measurement whatsoever.
    #[test]
    fn a_drive_with_no_readable_counters_is_not_graded_healthy() {
        let mut disk = SmartDiskInfo {
            device: r"\.\PhysicalDrive0".into(),
            model: "Unreadable".into(),
            serial: String::new(),
            firmware: String::new(),
            media_type: DriveMediaType::NVMe,
            capacity_bytes: 0,
            // What the platform itself said; the scorer must not overwrite it.
            health: DiskHealth::Unknown,
            temperature_celsius: None,
            power_on_hours: None,
            power_cycle_count: None,
            reallocated_sectors: None,
            pending_sectors: None,
            uncorrectable_errors: None,
            wear_leveling_percent: None,
            total_bytes_written: None,
            total_bytes_read: None,
            nvme_percentage_used: None,
            nvme_available_spare: None,
            attributes: Vec::new(),
            estimated_life_remaining: None,
            estimated_days_remaining: None,
        };

        SmartMonitor::infer_health(&mut disk);

        assert_eq!(
            disk.health,
            DiskHealth::Unknown,
            "a drive with no readable counters was graded from an empty score"
        );
        assert!(
            disk.estimated_life_remaining.is_none(),
            "a life estimate was published for a drive nothing was read from"
        );
    }
}

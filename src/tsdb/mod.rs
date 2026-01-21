//! Time-Series Database (TSDB) for recording system metrics over time
//!
//! This module provides a simple, file-based time-series database for recording
//! process resource utilization and system metrics. Data is stored in a compact
//! binary format with automatic rotation when the maximum size is reached.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Result, SimonError};

/// Default maximum database size (100 MB)
const DEFAULT_MAX_SIZE: u64 = 100 * 1024 * 1024;

/// Magic bytes for database file identification
const MAGIC_BYTES: &[u8; 8] = b"SIMONDB\0";

/// Current database version
const DB_VERSION: u32 = 1;

/// Size of the database header
const HEADER_SIZE: u64 = 128;

/// Parse a size string like "500MB", "1GB", "100KB" into bytes
pub fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, multiplier) = if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024 * 1024)
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024)
    } else if s.ends_with('B') {
        (&s[..s.len() - 1], 1)
    } else {
        // Assume bytes if no suffix
        (s.as_str(), 1)
    };

    let num: u64 = num_str
        .trim()
        .parse()
        .map_err(|_| SimonError::Configuration(format!("Invalid size: {}", s)))?;

    Ok(num * multiplier)
}

/// Format bytes as human-readable string
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// A single metric sample
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    /// Timestamp (Unix milliseconds)
    pub timestamp: u64,
    /// Metric value
    pub value: f64,
}

/// Process resource snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    /// Process ID
    pub pid: u32,
    /// Process name
    pub name: String,
    /// CPU usage percentage (0-100)
    pub cpu_percent: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// GPU memory usage in bytes (if applicable)
    pub gpu_memory_bytes: u64,
    /// GPU utilization percentage (if applicable)
    pub gpu_percent: f32,
    /// Disk read bytes per second
    pub disk_read_bps: u64,
    /// Disk write bytes per second
    pub disk_write_bps: u64,
    /// Network receive bytes per second
    pub net_rx_bps: u64,
    /// Network transmit bytes per second
    pub net_tx_bps: u64,
}

/// System-wide resource snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    /// Timestamp (Unix milliseconds)
    pub timestamp: u64,
    /// Overall CPU usage percentage
    pub cpu_percent: f32,
    /// Per-core CPU usage
    pub cpu_per_core: Vec<f32>,
    /// Memory used bytes
    pub memory_used: u64,
    /// Memory total bytes
    pub memory_total: u64,
    /// Swap used bytes
    pub swap_used: u64,
    /// Swap total bytes
    pub swap_total: u64,
    /// GPU utilization (per GPU)
    pub gpu_percent: Vec<f32>,
    /// GPU memory used (per GPU)
    pub gpu_memory_used: Vec<u64>,
    /// GPU temperature (per GPU)
    pub gpu_temperature: Vec<f32>,
    /// GPU power draw (per GPU, milliwatts)
    pub gpu_power_mw: Vec<u32>,
    /// Network RX rate (bytes/sec)
    pub net_rx_bps: u64,
    /// Network TX rate (bytes/sec)
    pub net_tx_bps: u64,
    /// Process snapshots (top N by resource usage)
    pub processes: Vec<ProcessSnapshot>,
}

/// Database header stored at the beginning of the file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatabaseHeader {
    /// Magic bytes for identification
    magic: [u8; 8],
    /// Database version
    version: u32,
    /// Maximum database size in bytes
    max_size: u64,
    /// Current data size (excluding header)
    data_size: u64,
    /// Number of records
    record_count: u64,
    /// First record timestamp
    first_timestamp: u64,
    /// Last record timestamp
    last_timestamp: u64,
    /// Reserved for future use
    _reserved: [u8; 16],
}

impl Default for DatabaseHeader {
    fn default() -> Self {
        let mut magic = [0u8; 8];
        magic.copy_from_slice(MAGIC_BYTES);
        Self {
            magic,
            version: DB_VERSION,
            max_size: DEFAULT_MAX_SIZE,
            data_size: 0,
            record_count: 0,
            first_timestamp: 0,
            last_timestamp: 0,
            _reserved: [0u8; 16],
        }
    }
}

/// Record type marker
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Process variant reserved for future use
enum RecordType {
    System = 1,
    Process = 2,
}

/// Time-Series Database for recording metrics
pub struct TimeSeriesDb {
    /// Path to the database file
    path: PathBuf,
    /// Maximum database size
    max_size: u64,
    /// Current header
    header: DatabaseHeader,
    /// File handle (when open)
    file: Option<File>,
}

impl TimeSeriesDb {
    /// Create a new time-series database
    pub fn new<P: AsRef<Path>>(path: P, max_size: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let max_size = if max_size == 0 {
            DEFAULT_MAX_SIZE
        } else {
            max_size
        };

        let mut db = Self {
            path,
            max_size,
            header: DatabaseHeader::default(),
            file: None,
        };

        db.header.max_size = max_size;
        db.open_or_create()?;

        Ok(db)
    }

    /// Open existing database or create new one
    fn open_or_create(&mut self) -> Result<()> {
        if self.path.exists() {
            self.open_existing()?;
        } else {
            self.create_new()?;
        }
        Ok(())
    }

    /// Open existing database
    fn open_existing(&mut self) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|e| SimonError::Other(e.to_string()))?;

        // Read and validate header
        let mut header_bytes = vec![0u8; HEADER_SIZE as usize];
        file.read_exact(&mut header_bytes)
            .map_err(|e| SimonError::Other(e.to_string()))?;

        let header: DatabaseHeader = bincode::deserialize(&header_bytes)
            .map_err(|e| SimonError::Configuration(format!("Invalid database header: {}", e)))?;

        // Validate magic bytes
        if &header.magic != MAGIC_BYTES {
            return Err(SimonError::Configuration(
                "Invalid database file (magic bytes mismatch)".to_string(),
            ));
        }

        // Check version
        if header.version > DB_VERSION {
            return Err(SimonError::Configuration(format!(
                "Database version {} is newer than supported version {}",
                header.version, DB_VERSION
            )));
        }

        self.header = header;
        // Update max_size if a new one was specified
        if self.max_size != DEFAULT_MAX_SIZE {
            self.header.max_size = self.max_size;
        }
        self.file = Some(file);

        Ok(())
    }

    /// Create new database
    fn create_new(&mut self) -> Result<()> {
        // Create parent directories if needed
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| SimonError::Other(e.to_string()))?;
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| SimonError::Other(e.to_string()))?;

        // Write header
        self.write_header(&mut file)?;
        self.file = Some(file);

        Ok(())
    }

    /// Write header to file
    fn write_header(&self, file: &mut File) -> Result<()> {
        file.seek(SeekFrom::Start(0))
            .map_err(|e| SimonError::Other(e.to_string()))?;

        let header_bytes = bincode::serialize(&self.header)
            .map_err(|e| SimonError::Other(format!("Failed to serialize header: {}", e)))?;

        // Pad to HEADER_SIZE
        let mut padded = vec![0u8; HEADER_SIZE as usize];
        padded[..header_bytes.len().min(HEADER_SIZE as usize)]
            .copy_from_slice(&header_bytes[..header_bytes.len().min(HEADER_SIZE as usize)]);

        file.write_all(&padded)
            .map_err(|e| SimonError::Other(e.to_string()))?;

        Ok(())
    }

    /// Record a system snapshot
    pub fn record_system(&mut self, snapshot: &SystemSnapshot) -> Result<()> {
        let data = bincode::serialize(snapshot)
            .map_err(|e| SimonError::Other(format!("Failed to serialize snapshot: {}", e)))?;

        self.write_record(RecordType::System, &data, snapshot.timestamp)?;

        Ok(())
    }

    /// Write a record to the database
    fn write_record(&mut self, record_type: RecordType, data: &[u8], timestamp: u64) -> Result<()> {
        // Record format: [type: 1 byte][length: 4 bytes][data: N bytes]
        let record_size = 1 + 4 + data.len() as u64;

        // Check if we need to rotate
        let current_size = HEADER_SIZE + self.header.data_size;
        if current_size + record_size > self.header.max_size {
            self.rotate()?;
        }

        {
            let file = self
                .file
                .as_mut()
                .ok_or_else(|| SimonError::Other("Database not open".to_string()))?;

            // Seek to end of data
            file.seek(SeekFrom::Start(HEADER_SIZE + self.header.data_size))
                .map_err(|e| SimonError::Other(e.to_string()))?;

            // Write record type
            file.write_all(&[record_type as u8])
                .map_err(|e| SimonError::Other(e.to_string()))?;

            // Write data length
            let len_bytes = (data.len() as u32).to_le_bytes();
            file.write_all(&len_bytes)
                .map_err(|e| SimonError::Other(e.to_string()))?;

            // Write data
            file.write_all(data)
                .map_err(|e| SimonError::Other(e.to_string()))?;
        }

        // Update header
        self.header.data_size += record_size;
        self.header.record_count += 1;
        self.header.last_timestamp = timestamp;
        if self.header.first_timestamp == 0 {
            self.header.first_timestamp = timestamp;
        }

        // Flush header
        self.flush_header()?;

        Ok(())
    }

    /// Flush header to disk
    fn flush_header(&mut self) -> Result<()> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| SimonError::Other("Database not open".to_string()))?;

        file.seek(SeekFrom::Start(0))
            .map_err(|e| SimonError::Other(e.to_string()))?;

        let header_bytes = bincode::serialize(&self.header)
            .map_err(|e| SimonError::Other(format!("Failed to serialize header: {}", e)))?;

        // Pad to HEADER_SIZE
        let mut padded = vec![0u8; HEADER_SIZE as usize];
        padded[..header_bytes.len().min(HEADER_SIZE as usize)]
            .copy_from_slice(&header_bytes[..header_bytes.len().min(HEADER_SIZE as usize)]);

        file.write_all(&padded)
            .map_err(|e| SimonError::Other(e.to_string()))?;

        file.flush().map_err(|e| SimonError::Other(e.to_string()))?;

        Ok(())
    }

    /// Rotate the database (remove oldest 50% of records)
    fn rotate(&mut self) -> Result<()> {
        // Simple rotation: truncate and start fresh
        // In a production system, we'd keep the recent half
        log::info!(
            "Database size limit reached ({} / {}), rotating...",
            format_size(self.header.data_size),
            format_size(self.header.max_size)
        );

        // Read all records
        let records = self.read_all_system_snapshots()?;
        let records_len = records.len();

        // Keep the most recent 50%
        let keep_count = records_len / 2;
        let to_keep: Vec<_> = records.into_iter().skip(records_len - keep_count).collect();

        // Close and recreate
        self.file = None;
        self.header = DatabaseHeader::default();
        self.header.max_size = self.max_size;
        self.create_new()?;

        // Re-write kept records
        for snapshot in to_keep {
            self.record_system(&snapshot)?;
        }

        log::info!(
            "Rotation complete, kept {} records",
            self.header.record_count
        );

        Ok(())
    }

    /// Read all system snapshots from the database
    pub fn read_all_system_snapshots(&mut self) -> Result<Vec<SystemSnapshot>> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| SimonError::Other("Database not open".to_string()))?;

        let mut snapshots = Vec::new();

        // Seek to start of data
        file.seek(SeekFrom::Start(HEADER_SIZE))
            .map_err(|e| SimonError::Other(e.to_string()))?;

        let mut offset = 0u64;
        while offset < self.header.data_size {
            // Read record type
            let mut type_byte = [0u8; 1];
            if file.read_exact(&mut type_byte).is_err() {
                break;
            }

            // Read length
            let mut len_bytes = [0u8; 4];
            if file.read_exact(&mut len_bytes).is_err() {
                break;
            }
            let len = u32::from_le_bytes(len_bytes) as usize;

            // Read data
            let mut data = vec![0u8; len];
            if file.read_exact(&mut data).is_err() {
                break;
            }

            offset += 1 + 4 + len as u64;

            // Parse if system record
            if type_byte[0] == RecordType::System as u8 {
                if let Ok(snapshot) = bincode::deserialize::<SystemSnapshot>(&data) {
                    snapshots.push(snapshot);
                }
            }
        }

        Ok(snapshots)
    }

    /// Query snapshots within a time range
    pub fn query_range(&mut self, start_time: u64, end_time: u64) -> Result<Vec<SystemSnapshot>> {
        let all = self.read_all_system_snapshots()?;
        Ok(all
            .into_iter()
            .filter(|s| s.timestamp >= start_time && s.timestamp <= end_time)
            .collect())
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        DatabaseStats {
            path: self.path.clone(),
            max_size: self.header.max_size,
            current_size: HEADER_SIZE + self.header.data_size,
            record_count: self.header.record_count,
            first_timestamp: if self.header.first_timestamp > 0 {
                Some(self.header.first_timestamp)
            } else {
                None
            },
            last_timestamp: if self.header.last_timestamp > 0 {
                Some(self.header.last_timestamp)
            } else {
                None
            },
        }
    }

    /// Get current timestamp in milliseconds
    pub fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Close the database
    pub fn close(&mut self) -> Result<()> {
        if let Some(mut file) = self.file.take() {
            self.write_header(&mut file)?;
            file.flush().map_err(|e| SimonError::Other(e.to_string()))?;
        }
        Ok(())
    }
}

impl Drop for TimeSeriesDb {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    /// Path to database file
    pub path: PathBuf,
    /// Maximum size in bytes
    pub max_size: u64,
    /// Current size in bytes
    pub current_size: u64,
    /// Number of records
    pub record_count: u64,
    /// First record timestamp
    pub first_timestamp: Option<u64>,
    /// Last record timestamp
    pub last_timestamp: Option<u64>,
}

impl DatabaseStats {
    /// Get usage percentage
    pub fn usage_percent(&self) -> f32 {
        (self.current_size as f32 / self.max_size as f32) * 100.0
    }

    /// Get time span in human-readable format
    pub fn time_span(&self) -> Option<String> {
        if let (Some(first), Some(last)) = (self.first_timestamp, self.last_timestamp) {
            let duration_ms = last.saturating_sub(first);
            let duration = Duration::from_millis(duration_ms);
            let secs = duration.as_secs();

            if secs < 60 {
                Some(format!("{} seconds", secs))
            } else if secs < 3600 {
                Some(format!("{} minutes", secs / 60))
            } else if secs < 86400 {
                Some(format!("{:.1} hours", secs as f64 / 3600.0))
            } else {
                Some(format!("{:.1} days", secs as f64 / 86400.0))
            }
        } else {
            None
        }
    }
}

/// Recorder that periodically captures system state
pub struct MetricsRecorder {
    /// Database instance
    db: TimeSeriesDb,
    /// Recording interval
    interval: Duration,
    /// Maximum number of processes to record per snapshot
    #[allow(dead_code)] // Reserved for future per-process recording
    max_processes: usize,
    /// Whether recording is active
    recording: bool,
}

impl MetricsRecorder {
    /// Create a new metrics recorder
    pub fn new<P: AsRef<Path>>(
        path: P,
        max_size: u64,
        interval: Duration,
        max_processes: usize,
    ) -> Result<Self> {
        let db = TimeSeriesDb::new(path, max_size)?;
        Ok(Self {
            db,
            interval,
            max_processes,
            recording: false,
        })
    }

    /// Get database statistics
    pub fn stats(&self) -> DatabaseStats {
        self.db.stats()
    }

    /// Get recording interval
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Record a single snapshot
    pub fn record_snapshot(&mut self, snapshot: SystemSnapshot) -> Result<()> {
        self.db.record_system(&snapshot)
    }

    /// Get all snapshots
    pub fn get_all_snapshots(&mut self) -> Result<Vec<SystemSnapshot>> {
        self.db.read_all_system_snapshots()
    }

    /// Query snapshots in time range
    pub fn query_range(&mut self, start: u64, end: u64) -> Result<Vec<SystemSnapshot>> {
        self.db.query_range(start, end)
    }

    /// Close the recorder
    pub fn close(&mut self) -> Result<()> {
        self.recording = false;
        self.db.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("100KB").unwrap(), 100 * 1024);
        assert_eq!(parse_size("100MB").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_size("1GB").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_size("500mb").unwrap(), 500 * 1024 * 1024);
        assert_eq!(parse_size(" 100 MB ").unwrap(), 100 * 1024 * 1024);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}

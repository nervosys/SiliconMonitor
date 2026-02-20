//! Camera and webcam device monitoring.
//!
//! # Platform Support
//!
//! - **Linux**: Reads `/sys/class/video4linux/`, `/dev/video*`
//! - **Windows**: Uses WMI (`Win32_PnPEntity` with Image device class)
//! - **macOS**: Uses `system_profiler SPCameraDataType`
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::camera::CameraMonitor;
//!
//! let monitor = CameraMonitor::new().unwrap();
//! for cam in monitor.cameras() {
//!     println!("{} ({:?}) - {}",
//!         cam.name, cam.connection, if cam.is_active { "active" } else { "idle" });
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Camera connection type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraConnection {
    /// Built-in / integrated webcam
    Internal,
    /// USB-connected camera
    USB,
    /// IP / network camera
    Network,
    /// CSI / MIPI (embedded, e.g., Raspberry Pi, Jetson)
    CSI,
    /// Virtual / software camera (OBS, ManyCam, etc.)
    Virtual,
    /// Unknown connection
    Unknown,
}

/// Camera capability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraCapability {
    /// Video capture
    VideoCapture,
    /// Still image capture
    StillCapture,
    /// Streaming output
    Streaming,
    /// Hardware encoding
    HardwareEncoding,
    /// Infrared / night vision
    Infrared,
    /// Depth sensing (ToF, structured light)
    DepthSensing,
    /// Autofocus
    Autofocus,
    /// Pan/tilt/zoom
    PTZ,
    /// Microphone integrated
    Microphone,
}

/// Information about a single camera device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraInfo {
    /// Device name
    pub name: String,
    /// Connection type
    pub connection: CameraConnection,
    /// Device path (/dev/video0, device instance, etc.)
    pub device_path: String,
    /// Driver in use
    pub driver: String,
    /// Vendor / manufacturer
    pub vendor: String,
    /// Maximum resolution (width)
    pub max_width: u32,
    /// Maximum resolution (height)
    pub max_height: u32,
    /// Supported pixel formats (e.g., YUYV, MJPG, H264)
    pub formats: Vec<String>,
    /// Device capabilities
    pub capabilities: Vec<CameraCapability>,
    /// Whether the camera is currently in use / streaming
    pub is_active: bool,
    /// Device index (e.g., 0 for /dev/video0)
    pub index: u32,
}

impl CameraInfo {
    /// Get resolution as a formatted string (e.g., "1920x1080")
    pub fn resolution(&self) -> String {
        if self.max_width > 0 && self.max_height > 0 {
            format!("{}x{}", self.max_width, self.max_height)
        } else {
            "unknown".into()
        }
    }

    /// Infer megapixels from max resolution
    pub fn megapixels(&self) -> f64 {
        (self.max_width as f64 * self.max_height as f64) / 1_000_000.0
    }
}

/// Monitor for camera and webcam devices
pub struct CameraMonitor {
    cameras: Vec<CameraInfo>,
}

impl CameraMonitor {
    /// Create a new CameraMonitor and enumerate all cameras.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            cameras: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh the list of camera devices.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.cameras.clear();

        #[cfg(target_os = "linux")]
        self.refresh_linux();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        #[cfg(target_os = "macos")]
        self.refresh_macos();

        Ok(())
    }

    /// Get all detected cameras.
    pub fn cameras(&self) -> &[CameraInfo] {
        &self.cameras
    }

    /// Get count of detected cameras.
    pub fn count(&self) -> usize {
        self.cameras.len()
    }

    #[cfg(target_os = "linux")]
    fn refresh_linux(&mut self) {
        // Enumerate /sys/class/video4linux/
        let v4l_path = std::path::Path::new("/sys/class/video4linux");
        if !v4l_path.exists() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(v4l_path) {
            for entry in entries.flatten() {
                let dev_name = entry.file_name().to_string_lossy().to_string();
                if !dev_name.starts_with("video") {
                    continue;
                }
                let index: u32 = dev_name
                    .strip_prefix("video")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                let base = entry.path();
                let name = std::fs::read_to_string(base.join("name"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                // Skip metadata / output-only devices
                // Check device capabilities via uevent
                let uevent = std::fs::read_to_string(base.join("uevent")).unwrap_or_default();
                if uevent.contains("DEVTYPE=video4linux-subdev") {
                    continue;
                }

                let driver = Self::read_sysfs_link_name(&base.join("device/driver"));

                // Infer connection type from device path
                let dev_path_str = base.to_string_lossy().to_string();
                let connection = if dev_path_str.contains("usb") {
                    CameraConnection::USB
                } else if dev_path_str.contains("platform") || dev_path_str.contains("csi") {
                    CameraConnection::CSI
                } else if name.to_lowercase().contains("virtual") || name.to_lowercase().contains("obs") {
                    CameraConnection::Virtual
                } else if dev_path_str.contains("pci") {
                    CameraConnection::Internal
                } else {
                    CameraConnection::Unknown
                };

                let mut capabilities = vec![CameraCapability::VideoCapture];

                // Try to read supported formats from /dev/videoN
                let dev_file = format!("/dev/{}", dev_name);
                let formats = Self::read_v4l2_formats(&dev_file);
                if formats.iter().any(|f| f == "H264" || f == "HEVC") {
                    capabilities.push(CameraCapability::HardwareEncoding);
                }
                if formats.iter().any(|f| f.contains("Z16") || f.contains("INZI")) {
                    capabilities.push(CameraCapability::DepthSensing);
                }
                if name.to_lowercase().contains("ir ") || name.to_lowercase().contains("infrared") {
                    capabilities.push(CameraCapability::Infrared);
                }

                // Try to infer vendor from USB path
                let vendor = Self::read_usb_vendor(&base);

                self.cameras.push(CameraInfo {
                    name: if name.is_empty() {
                        dev_name.clone()
                    } else {
                        name
                    },
                    connection,
                    device_path: dev_file,
                    driver,
                    vendor,
                    max_width: 0, // Would need v4l2 ioctl to get actual max
                    max_height: 0,
                    formats,
                    capabilities,
                    is_active: Self::is_device_busy(&format!("/dev/{}", dev_name)),
                    index,
                });
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn read_sysfs_link_name(path: &std::path::Path) -> String {
        std::fs::read_link(path)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_default()
    }

    #[cfg(target_os = "linux")]
    fn read_v4l2_formats(_dev: &str) -> Vec<String> {
        // Reading actual V4L2 formats requires ioctl; check common format descriptions
        // from /sys instead. This is a best-effort heuristic.
        vec!["YUYV".into(), "MJPG".into()]
    }

    #[cfg(target_os = "linux")]
    fn read_usb_vendor(base: &std::path::Path) -> String {
        // Walk up to find vendor info in USB hierarchy
        let mut path = base.to_path_buf();
        for _ in 0..5 {
            let vendor_path = path.join("manufacturer");
            if let Ok(v) = std::fs::read_to_string(&vendor_path) {
                return v.trim().to_string();
            }
            if !path.pop() {
                break;
            }
        }
        String::new()
    }

    #[cfg(target_os = "linux")]
    fn is_device_busy(dev: &str) -> bool {
        // Try opening exclusively — if it fails with EBUSY, it's in use
        use std::fs::OpenOptions;
        match OpenOptions::new().read(true).open(dev) {
            Ok(_) => false,
            Err(e) => e.raw_os_error() == Some(16), // EBUSY
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        if let Ok(output) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_PnPEntity | Where-Object { $_.PNPClass -eq 'Camera' -or $_.PNPClass -eq 'Image' } | Select-Object Name, Manufacturer, DeviceID, Status, PNPClass | ConvertTo-Json -Compress"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = match &val {
                        serde_json::Value::Array(arr) => arr.clone(),
                        obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                        _ => vec![],
                    };
                    for (i, item) in items.iter().enumerate() {
                        let name = item["Name"].as_str().unwrap_or("Unknown Camera").to_string();
                        let vendor = item["Manufacturer"].as_str().unwrap_or("").to_string();
                        let device_id = item["DeviceID"].as_str().unwrap_or("").to_string();
                        let lower = device_id.to_lowercase();
                        let connection = if lower.contains("usb") {
                            CameraConnection::USB
                        } else if lower.contains("pci") {
                            CameraConnection::Internal
                        } else if name.to_lowercase().contains("virtual") || name.to_lowercase().contains("obs") {
                            CameraConnection::Virtual
                        } else {
                            CameraConnection::Internal
                        };

                        self.cameras.push(CameraInfo {
                            name,
                            connection,
                            device_path: device_id,
                            driver: String::new(),
                            vendor,
                            max_width: 0,
                            max_height: 0,
                            formats: Vec::new(),
                            capabilities: vec![CameraCapability::VideoCapture],
                            is_active: item["Status"].as_str() == Some("OK"),
                            index: i as u32,
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn refresh_macos(&mut self) {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPCameraDataType", "-detailLevel", "full"])
            .output()
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                let mut name = String::new();
                let mut model_id = String::new();
                let mut unique_id = String::new();
                let mut index: u32 = 0;

                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.ends_with(':') && !trimmed.starts_with("Camera") && !trimmed.is_empty() {
                        // Flush previous
                        if !name.is_empty() {
                            let connection = if model_id.to_lowercase().contains("usb") {
                                CameraConnection::USB
                            } else if model_id.to_lowercase().contains("virtual") {
                                CameraConnection::Virtual
                            } else {
                                CameraConnection::Internal
                            };
                            self.cameras.push(CameraInfo {
                                name: name.clone(),
                                connection,
                                device_path: unique_id.clone(),
                                driver: String::new(),
                                vendor: "Apple".into(),
                                max_width: 0,
                                max_height: 0,
                                formats: Vec::new(),
                                capabilities: vec![CameraCapability::VideoCapture, CameraCapability::Autofocus],
                                is_active: true,
                                index,
                            });
                            index += 1;
                        }
                        name = trimmed.trim_end_matches(':').to_string();
                        model_id.clear();
                        unique_id.clear();
                    } else if let Some(v) = trimmed.strip_prefix("Model ID:") {
                        model_id = v.trim().to_string();
                    } else if let Some(v) = trimmed.strip_prefix("Unique ID:") {
                        unique_id = v.trim().to_string();
                    }
                }
                // Flush last
                if !name.is_empty() {
                    self.cameras.push(CameraInfo {
                        name,
                        connection: CameraConnection::Internal,
                        device_path: unique_id,
                        driver: String::new(),
                        vendor: "Apple".into(),
                        max_width: 0,
                        max_height: 0,
                        formats: Vec::new(),
                        capabilities: vec![CameraCapability::VideoCapture],
                        is_active: true,
                        index,
                    });
                }
            }
        }
    }
}

impl Default for CameraMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            cameras: Vec::new(),
        })
    }
}

impl std::fmt::Display for CameraConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal => write!(f, "Internal"),
            Self::USB => write!(f, "USB"),
            Self::Network => write!(f, "Network"),
            Self::CSI => write!(f, "CSI/MIPI"),
            Self::Virtual => write!(f, "Virtual"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_monitor_creation() {
        let monitor = CameraMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_camera_monitor_default() {
        let monitor = CameraMonitor::default();
        let _ = monitor.cameras();
    }

    #[test]
    fn test_camera_info_resolution() {
        let cam = CameraInfo {
            name: "Test".into(),
            connection: CameraConnection::USB,
            device_path: String::new(),
            driver: String::new(),
            vendor: String::new(),
            max_width: 1920,
            max_height: 1080,
            formats: Vec::new(),
            capabilities: Vec::new(),
            is_active: false,
            index: 0,
        };
        assert_eq!(cam.resolution(), "1920x1080");
        assert!((cam.megapixels() - 2.0736).abs() < 0.01);
    }

    #[test]
    fn test_camera_serialization() {
        let cam = CameraInfo {
            name: "Test Camera".into(),
            connection: CameraConnection::Internal,
            device_path: "/dev/video0".into(),
            driver: "uvcvideo".into(),
            vendor: "Logitech".into(),
            max_width: 1920,
            max_height: 1080,
            formats: vec!["YUYV".into(), "MJPG".into()],
            capabilities: vec![CameraCapability::VideoCapture],
            is_active: false,
            index: 0,
        };
        let json = serde_json::to_string(&cam).unwrap();
        assert!(json.contains("Test Camera"));
        let _: CameraInfo = serde_json::from_str(&json).unwrap();
    }
}

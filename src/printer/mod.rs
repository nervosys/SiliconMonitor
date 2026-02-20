//! Printer and print queue monitoring — connected printers, status, drivers.
//!
//! # Platform Support
//!
//! - **Linux**: Uses `lpstat` (CUPS) to enumerate printers and their status
//! - **Windows**: Uses WMI (`Win32_Printer`) to enumerate printers
//! - **macOS**: Uses `lpstat` (CUPS) to enumerate printers
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::printer::PrinterMonitor;
//!
//! let monitor = PrinterMonitor::new().unwrap();
//! for printer in monitor.printers() {
//!     println!("{}: {} ({})", printer.name, printer.status, printer.driver);
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::error::SimonError;

/// Printer connection type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterConnection {
    /// Directly attached via USB
    USB,
    /// Network printer (TCP/IP, IPP, etc.)
    Network,
    /// Shared printer via SMB/Windows sharing
    Shared,
    /// Bluetooth connection
    Bluetooth,
    /// Virtual / software printer (e.g., PDF printer)
    Virtual,
    /// Serial / parallel port
    Legacy,
    /// Unknown connection
    Unknown,
}

/// Printer status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterStatus {
    /// Ready and idle
    Idle,
    /// Currently printing
    Printing,
    /// Paused / stopped
    Paused,
    /// Offline or not responding
    Offline,
    /// Error condition (paper jam, out of paper, etc.)
    Error,
    /// Unknown status
    Unknown,
}

/// Type of printer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterType {
    /// Laser printer
    Laser,
    /// Inkjet printer
    Inkjet,
    /// Thermal printer (receipt, label)
    Thermal,
    /// Dot matrix / impact
    DotMatrix,
    /// 3D printer
    Printer3D,
    /// Virtual / PDF printer
    Virtual,
    /// Plotter
    Plotter,
    /// Unknown type
    Unknown,
}

/// Information about a single printer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterInfo {
    /// Printer name
    pub name: String,
    /// Printer description or model
    pub description: String,
    /// Driver name
    pub driver: String,
    /// Port or URI (e.g., "usb://...", "ipp://...")
    pub port: String,
    /// Connection type
    pub connection: PrinterConnection,
    /// Current status
    pub status: PrinterStatus,
    /// Printer type
    pub printer_type: PrinterType,
    /// Whether this is the default printer
    pub is_default: bool,
    /// Whether the printer accepts new jobs
    pub accepting_jobs: bool,
    /// Whether the printer is shared over the network
    pub shared: bool,
    /// Whether the printer supports color
    pub color: bool,
    /// Whether the printer supports duplex (double-sided)
    pub duplex: bool,
    /// Number of jobs in queue
    pub jobs_in_queue: u32,
    /// Location string (if configured)
    pub location: String,
}

/// Monitor for printers and print queues
pub struct PrinterMonitor {
    printers: Vec<PrinterInfo>,
}

impl PrinterMonitor {
    /// Create a new PrinterMonitor and detect printers.
    pub fn new() -> Result<Self, SimonError> {
        let mut monitor = Self {
            printers: Vec::new(),
        };
        monitor.refresh()?;
        Ok(monitor)
    }

    /// Refresh printer detection.
    pub fn refresh(&mut self) -> Result<(), SimonError> {
        self.printers.clear();

        #[cfg(target_os = "linux")]
        self.refresh_cups();

        #[cfg(target_os = "macos")]
        self.refresh_cups();

        #[cfg(target_os = "windows")]
        self.refresh_windows();

        Ok(())
    }

    /// Get all detected printers.
    pub fn printers(&self) -> &[PrinterInfo] {
        &self.printers
    }

    /// Get the default printer, if any.
    pub fn default_printer(&self) -> Option<&PrinterInfo> {
        self.printers.iter().find(|p| p.is_default)
    }

    /// Get printers with a specific status.
    pub fn printers_by_status(&self, status: PrinterStatus) -> Vec<&PrinterInfo> {
        self.printers
            .iter()
            .filter(|p| p.status == status)
            .collect()
    }

    /// Get network printers.
    pub fn network_printers(&self) -> Vec<&PrinterInfo> {
        self.printers
            .iter()
            .filter(|p| p.connection == PrinterConnection::Network)
            .collect()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn refresh_cups(&mut self) {
        // Get default printer
        let default_name = std::process::Command::new("lpstat")
            .args(["-d"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                s.split(':')
                    .nth(1)
                    .map(|n| n.trim().to_string())
            })
            .unwrap_or_default();

        // List printers with lpstat -p -l
        let output = match std::process::Command::new("lpstat")
            .args(["-p", "-l"])
            .output()
        {
            Ok(o) => String::from_utf8(o.stdout).unwrap_or_default(),
            Err(_) => return,
        };

        // Parse devices
        let device_output = std::process::Command::new("lpstat")
            .args(["-v"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        // Build device URI map: name -> uri
        let mut device_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for line in device_output.lines() {
            // "device for PrinterName: ipp://..."
            if let Some(rest) = line.strip_prefix("device for ") {
                if let Some(colon_pos) = rest.find(':') {
                    let name = rest[..colon_pos].trim().to_string();
                    let uri = rest[colon_pos + 1..].trim().to_string();
                    device_map.insert(name, uri);
                }
            }
        }

        // Get accepting status
        let accept_output = std::process::Command::new("lpstat")
            .args(["-a"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        let mut accept_map: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        for line in accept_output.lines() {
            let parts: Vec<&str> = line.splitn(2, ' ').collect();
            if parts.len() >= 2 {
                let accepting = line.contains("accepting");
                accept_map.insert(parts[0].to_string(), accepting);
            }
        }

        // Parse printer blocks
        let mut current_name = String::new();
        let mut current_desc = String::new();
        let mut current_status = PrinterStatus::Unknown;
        let mut in_printer = false;

        for line in output.lines() {
            if line.starts_with("printer ") {
                // Save previous
                if in_printer && !current_name.is_empty() {
                    let port = device_map.get(&current_name).cloned().unwrap_or_default();
                    let connection = Self::classify_connection_cups(&port);
                    let accepting = accept_map.get(&current_name).copied().unwrap_or(false);
                    self.printers.push(PrinterInfo {
                        name: current_name.clone(),
                        description: current_desc.clone(),
                        driver: String::new(),
                        port,
                        connection,
                        status: current_status.clone(),
                        printer_type: PrinterType::Unknown,
                        is_default: current_name == default_name,
                        accepting_jobs: accepting,
                        shared: false,
                        color: false,
                        duplex: false,
                        jobs_in_queue: 0,
                        location: String::new(),
                    });
                }

                // Parse: "printer NAME is idle."  or "printer NAME disabled since ..."
                let rest = &line["printer ".len()..];
                let name_end = rest
                    .find(" is ")
                    .or_else(|| rest.find(" disabled"))
                    .unwrap_or(rest.len());
                current_name = rest[..name_end].to_string();
                current_desc = String::new();
                in_printer = true;

                if rest.contains("idle") {
                    current_status = PrinterStatus::Idle;
                } else if rest.contains("printing") {
                    current_status = PrinterStatus::Printing;
                } else if rest.contains("disabled") || rest.contains("stopped") {
                    current_status = PrinterStatus::Paused;
                } else {
                    current_status = PrinterStatus::Unknown;
                }
            } else if in_printer && line.starts_with("\tDescription:") {
                current_desc = line
                    .strip_prefix("\tDescription:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
            } else if in_printer && line.starts_with("\tLocation:") {
                // Will store with the printer when we flush it
            }
        }

        // Flush last printer
        if in_printer && !current_name.is_empty() {
            let port = device_map.get(&current_name).cloned().unwrap_or_default();
            let connection = Self::classify_connection_cups(&port);
            let accepting = accept_map.get(&current_name).copied().unwrap_or(false);
            self.printers.push(PrinterInfo {
                name: current_name.clone(),
                description: current_desc,
                driver: String::new(),
                port,
                connection,
                status: current_status,
                printer_type: PrinterType::Unknown,
                is_default: current_name == default_name,
                accepting_jobs: accepting,
                shared: false,
                color: false,
                duplex: false,
                jobs_in_queue: 0,
                location: String::new(),
            });
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn classify_connection_cups(uri: &str) -> PrinterConnection {
        let u = uri.to_lowercase();
        if u.starts_with("usb:") {
            PrinterConnection::USB
        } else if u.starts_with("ipp:")
            || u.starts_with("ipps:")
            || u.starts_with("http:")
            || u.starts_with("https:")
            || u.starts_with("socket:")
            || u.starts_with("lpd:")
        {
            PrinterConnection::Network
        } else if u.starts_with("smb:") {
            PrinterConnection::Shared
        } else if u.contains("pdf") || u.contains("cups-pdf") || u.contains("virtual") {
            PrinterConnection::Virtual
        } else if u.starts_with("serial:") || u.starts_with("parallel:") {
            PrinterConnection::Legacy
        } else {
            PrinterConnection::Unknown
        }
    }

    #[cfg(target_os = "windows")]
    fn refresh_windows(&mut self) {
        let output = match std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command",
                "Get-CimInstance Win32_Printer | Select-Object Name, DriverName, PortName, PrinterStatus, Comment, Location, Shared, Color, Default, PrinterState, JobCountSinceLastReset, Network | ConvertTo-Json -Compress"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return,
        };

        let text = String::from_utf8(output.stdout).unwrap_or_default();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            let items = match &val {
                serde_json::Value::Array(arr) => arr.clone(),
                obj @ serde_json::Value::Object(_) => vec![obj.clone()],
                _ => return,
            };

            for item in &items {
                let name = item["Name"].as_str().unwrap_or("").to_string();
                let driver = item["DriverName"].as_str().unwrap_or("").to_string();
                let port = item["PortName"].as_str().unwrap_or("").to_string();
                let location = item["Location"].as_str().unwrap_or("").to_string();
                let description = item["Comment"].as_str().unwrap_or("").to_string();
                let shared = item["Shared"].as_bool().unwrap_or(false);
                let color = item["Color"].as_bool().unwrap_or(false);
                let is_default = item["Default"].as_bool().unwrap_or(false);
                let is_network = item["Network"].as_bool().unwrap_or(false);

                let wmi_status = item["PrinterStatus"].as_u64().unwrap_or(0);
                let status = match wmi_status {
                    1 | 2 => PrinterStatus::Unknown, // Other, Unknown
                    3 => PrinterStatus::Idle,
                    4 => PrinterStatus::Printing,
                    5 => PrinterStatus::Paused, // Warmup
                    6 => PrinterStatus::Paused, // Stopped Printing
                    7 => PrinterStatus::Offline,
                    _ => PrinterStatus::Unknown,
                };

                let connection = if is_network {
                    PrinterConnection::Network
                } else if port.to_lowercase().contains("usb") {
                    PrinterConnection::USB
                } else if name.to_lowercase().contains("pdf")
                    || name.to_lowercase().contains("xps")
                    || name.to_lowercase().contains("onenote")
                    || name.to_lowercase().contains("fax")
                {
                    PrinterConnection::Virtual
                } else {
                    PrinterConnection::Unknown
                };

                let printer_type = Self::infer_printer_type_win(&name, &driver);

                self.printers.push(PrinterInfo {
                    name,
                    description,
                    driver,
                    port,
                    connection,
                    accepting_jobs: status != PrinterStatus::Offline,
                    status,
                    printer_type,
                    is_default,
                    shared,
                    color,
                    duplex: false,
                    jobs_in_queue: 0,
                    location,
                });
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn infer_printer_type_win(name: &str, driver: &str) -> PrinterType {
        let combined = format!("{} {}", name, driver).to_lowercase();
        if combined.contains("pdf")
            || combined.contains("xps")
            || combined.contains("onenote")
            || combined.contains("fax")
        {
            PrinterType::Virtual
        } else if combined.contains("laser") || combined.contains("laserjet") {
            PrinterType::Laser
        } else if combined.contains("inkjet")
            || combined.contains("deskjet")
            || combined.contains("officejet")
            || combined.contains("pixma")
            || combined.contains("envy")
        {
            PrinterType::Inkjet
        } else if combined.contains("thermal") || combined.contains("receipt") || combined.contains("label") {
            PrinterType::Thermal
        } else if combined.contains("plotter") || combined.contains("designjet") {
            PrinterType::Plotter
        } else {
            PrinterType::Unknown
        }
    }
}

impl Default for PrinterMonitor {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            printers: Vec::new(),
        })
    }
}

impl std::fmt::Display for PrinterStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Printing => write!(f, "Printing"),
            Self::Paused => write!(f, "Paused"),
            Self::Offline => write!(f, "Offline"),
            Self::Error => write!(f, "Error"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for PrinterConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::USB => write!(f, "USB"),
            Self::Network => write!(f, "Network"),
            Self::Shared => write!(f, "Shared"),
            Self::Bluetooth => write!(f, "Bluetooth"),
            Self::Virtual => write!(f, "Virtual"),
            Self::Legacy => write!(f, "Legacy"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_monitor_creation() {
        let monitor = PrinterMonitor::new();
        assert!(monitor.is_ok());
    }

    #[test]
    fn test_printer_monitor_default() {
        let monitor = PrinterMonitor::default();
        let _ = monitor.printers();
        let _ = monitor.default_printer();
    }

    #[test]
    fn test_printer_serialization() {
        let printer = PrinterInfo {
            name: "TestPrinter".into(),
            description: "A test printer".into(),
            driver: "Generic".into(),
            port: "USB001".into(),
            connection: PrinterConnection::USB,
            status: PrinterStatus::Idle,
            printer_type: PrinterType::Laser,
            is_default: true,
            accepting_jobs: true,
            shared: false,
            color: true,
            duplex: true,
            jobs_in_queue: 0,
            location: "Office".into(),
        };
        let json = serde_json::to_string(&printer).unwrap();
        assert!(json.contains("TestPrinter"));
        let _: PrinterInfo = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_status_display() {
        assert_eq!(PrinterStatus::Idle.to_string(), "Idle");
        assert_eq!(PrinterStatus::Printing.to_string(), "Printing");
        assert_eq!(PrinterConnection::USB.to_string(), "USB");
    }
}

//! Hardware AI inference engine — system classification, bottleneck detection,
//! performance estimation, and workload suitability analysis.
//!
//! This module applies heuristic and rule-based "AI" techniques to raw hardware
//! telemetry to produce actionable insights:
//!
//! - **System Classification**: Workstation, server, laptop, embedded, gaming, etc.
//! - **Bottleneck Detection**: CPU-bound, GPU-bound, memory-bound, I/O-bound
//! - **Performance Tier Scoring**: Low-end to ultra-high-end classification
//! - **Workload Suitability**: ML training, video editing, gaming, web server, etc.
//! - **Hardware Age Estimation**: From model numbers and specifications
//! - **Thermal Envelope Modeling**: TDP and cooling capacity analysis
//! - **Upgrade Recommendations**: Cost-effective improvement suggestions
//! - **Anomaly Detection**: Identify hardware configurations that are unusual
//!
//! # Architecture
//!
//! The engine uses a multi-layer inference pipeline:
//!
//! 1. **Feature Extraction**: Normalized hardware metrics
//! 2. **Rule Engine**: Expert-system rules for classification
//! 3. **Scoring Models**: Weighted scoring for performance tiers
//! 4. **Bayesian Reasoning**: Probabilistic workload suitability
//! 5. **Anomaly Detection**: Statistical deviation from expected configurations
//!
//! # Examples
//!
//! ```no_run
//! use simonlib::hardware_ai::HardwareInferenceEngine;
//!
//! let engine = HardwareInferenceEngine::new().unwrap();
//! let report = engine.full_analysis();
//! println!("System: {:?}", report.classification);
//! println!("Performance tier: {:?}", report.performance_tier);
//! for bottleneck in &report.bottlenecks {
//!     println!("Bottleneck: {} (severity: {}%)", bottleneck.component, bottleneck.severity);
//! }
//! for rec in &report.upgrade_recommendations {
//!     println!("Upgrade: {} (impact: {})", rec.description, rec.expected_impact);
//! }
//! ```

use serde::{Deserialize, Serialize};
use crate::error::SimonError;

// ────────────────────────────────────────────────────────────────────
// Classification types
// ────────────────────────────────────────────────────────────────────

/// System form factor / purpose classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemClass {
    /// Desktop workstation (powerful, tower form factor)
    Workstation,
    /// Server (multi-socket, ECC RAM, IPMI)
    Server,
    /// High-end gaming desktop
    GamingDesktop,
    /// General-purpose desktop
    Desktop,
    /// Laptop (battery, mobile CPU)
    Laptop,
    /// Gaming laptop
    GamingLaptop,
    /// Ultra-portable / ultrabook
    Ultrabook,
    /// Embedded / SBC (Raspberry Pi, Jetson, etc.)
    Embedded,
    /// Virtual machine
    VirtualMachine,
    /// Container / cloud instance
    CloudInstance,
    /// Mini PC / NUC
    MiniPc,
    /// Unknown
    Unknown,
}

/// Performance tier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PerformanceTier {
    UltraLow,    // Raspberry Pi, IoT
    Low,         // Atom, Celeron, old hardware
    MidLow,      // i3, Ryzen 3
    Mid,         // i5, Ryzen 5, mid-range GPU
    MidHigh,     // i7, Ryzen 7, high-end GPU
    High,        // i9, Ryzen 9, workstation GPU
    Ultra,       // Threadripper, EPYC, multi-GPU
    Datacenter,  // Server-class, 100+ cores
}

/// Bottleneck type — what's limiting performance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BottleneckType {
    CpuBound,
    GpuBound,
    MemoryBound,
    StorageBound,
    NetworkBound,
    ThermalThrottling,
    PowerLimited,
}

/// A detected bottleneck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Bottleneck type
    pub bottleneck_type: BottleneckType,
    /// Human-readable component name
    pub component: String,
    /// Severity 0-100
    pub severity: u8,
    /// Explanation
    pub reason: String,
    /// Confidence 0.0-1.0
    pub confidence: f32,
}

/// Workload type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Workload {
    /// Machine learning training (GPU VRAM, tensor cores)
    MlTraining,
    /// ML inference (GPU, lower VRAM requirement)
    MlInference,
    /// Video editing / production (GPU, fast storage, RAM)
    VideoEditing,
    /// 3D rendering / CAD (GPU, multi-core CPU)
    Rendering3D,
    /// Gaming (GPU, CPU single-thread, fast storage)
    Gaming,
    /// Software compilation (multi-core CPU, RAM, fast storage)
    Compilation,
    /// Web server (network, RAM, multi-core)
    WebServer,
    /// Database server (storage IOPS, RAM, multi-core)
    DatabaseServer,
    /// Virtualization (cores, RAM, VT-x/AMD-V)
    Virtualization,
    /// Scientific computing (FP performance, RAM)
    ScientificComputing,
    /// General office / productivity
    OfficeProductivity,
    /// Media streaming
    MediaStreaming,
    /// Network appliance (NIC throughput, low latency)
    NetworkAppliance,
}

/// Workload suitability score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadSuitability {
    pub workload: Workload,
    /// Score 0-100 (100 = perfectly suited)
    pub score: u8,
    /// Confidence 0.0-1.0
    pub confidence: f32,
    /// Key limiting factors
    pub limiting_factors: Vec<String>,
    /// Key strengths for this workload
    pub strengths: Vec<String>,
}

/// Inferred hardware age.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAge {
    /// Estimated CPU generation year
    pub cpu_year: Option<u16>,
    /// Estimated GPU generation year
    pub gpu_year: Option<u16>,
    /// Overall system age estimate (years)
    pub estimated_age_years: f32,
    /// Confidence
    pub confidence: f32,
    /// Reasoning
    pub reasoning: String,
}

/// Thermal envelope analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalEnvelope {
    /// Estimated total system TDP (watts)
    pub estimated_total_tdp_watts: f32,
    /// CPU TDP
    pub cpu_tdp_watts: f32,
    /// GPU TDP
    pub gpu_tdp_watts: f32,
    /// Thermal headroom assessment
    pub headroom: ThermalHeadroom,
    /// Cooling adequacy score (0-100)
    pub cooling_score: u8,
    /// Recommendations
    pub recommendations: Vec<String>,
}

/// Thermal headroom classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalHeadroom {
    /// Plenty of cooling capacity
    Ample,
    /// Adequate cooling
    Adequate,
    /// Marginal — may throttle under sustained load
    Marginal,
    /// Insufficient — likely thermal throttling
    Insufficient,
    /// Unknown — insufficient data
    Unknown,
}

/// Upgrade recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeRecommendation {
    /// Component to upgrade
    pub component: String,
    /// Current state
    pub current: String,
    /// Recommended upgrade
    pub recommended: String,
    /// Description of the improvement
    pub description: String,
    /// Expected performance impact (e.g., "20-30% faster compilation")
    pub expected_impact: String,
    /// Priority 1-10 (10 = most impactful)
    pub priority: u8,
    /// Estimated relative cost (1 = cheap, 5 = expensive)
    pub cost_tier: u8,
}

/// Hardware anomaly — something unusual in the configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAnomaly {
    /// What's anomalous
    pub description: String,
    /// Severity (info, warning, critical)
    pub severity: AnomalySeverity,
    /// Possible explanation
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
}

// ────────────────────────────────────────────────────────────────────
// Full analysis report
// ────────────────────────────────────────────────────────────────────

/// Complete hardware analysis report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareAnalysisReport {
    /// System classification
    pub classification: SystemClass,
    /// Classification confidence
    pub classification_confidence: f32,
    /// Performance tier
    pub performance_tier: PerformanceTier,
    /// Overall performance score (0-100)
    pub performance_score: u8,
    /// Detected bottlenecks
    pub bottlenecks: Vec<Bottleneck>,
    /// Workload suitability scores
    pub workload_scores: Vec<WorkloadSuitability>,
    /// Hardware age estimate
    pub hardware_age: HardwareAge,
    /// Thermal analysis
    pub thermal_envelope: ThermalEnvelope,
    /// Upgrade recommendations
    pub upgrade_recommendations: Vec<UpgradeRecommendation>,
    /// Anomalies detected
    pub anomalies: Vec<HardwareAnomaly>,
    /// Hardware fingerprint (unique identifier based on hardware config)
    pub hardware_fingerprint: String,
}

// ────────────────────────────────────────────────────────────────────
// Extracted hardware features (normalized)
// ────────────────────────────────────────────────────────────────────

/// Normalized hardware features used by the inference engine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct HardwareFeatures {
    // CPU features
    cpu_cores_physical: u32,
    cpu_cores_logical: u32,
    cpu_max_freq_mhz: u32,
    cpu_model: String,
    cpu_vendor: String,
    is_server_cpu: bool,
    has_ecc: bool,
    cpu_tdp_watts: f32,

    // Memory features
    ram_total_gb: f32,
    ram_channels: u32,
    ram_speed_mhz: u32,

    // GPU features
    has_discrete_gpu: bool,
    gpu_count: u32,
    gpu_model: String,
    gpu_vram_gb: f32,
    has_tensor_cores: bool,
    has_rt_cores: bool,
    gpu_tdp_watts: f32,

    // Storage features
    has_nvme: bool,
    has_ssd: bool,
    total_storage_gb: f32,
    boot_drive_type: String,

    // Platform features
    has_battery: bool,
    chassis_type: String,
    is_virtual: bool,
    numa_nodes: u32,
    pcie_gen: u32,

    // Network
    max_nic_speed_gbps: f32,
    nic_count: u32,
}

// ────────────────────────────────────────────────────────────────────
// Inference engine
// ────────────────────────────────────────────────────────────────────

/// The hardware inference engine.
pub struct HardwareInferenceEngine {
    features: HardwareFeatures,
}

impl HardwareInferenceEngine {
    /// Create a new inference engine, extracting hardware features from the system.
    pub fn new() -> Result<Self, SimonError> {
        let mut engine = Self {
            features: HardwareFeatures::default(),
        };
        engine.extract_features();
        Ok(engine)
    }

    /// Create from pre-populated features (for testing or external data).
    pub fn from_features(features_json: &str) -> Result<Self, SimonError> {
        let features: HardwareFeatures = serde_json::from_str(features_json)
            .map_err(|e| SimonError::Parse(format!("Invalid features JSON: {}", e)))?;
        Ok(Self { features })
    }

    /// Run the full analysis pipeline.
    pub fn full_analysis(&self) -> HardwareAnalysisReport {
        let (classification, class_confidence) = self.classify_system();
        let (tier, score) = self.compute_performance_tier();
        let bottlenecks = self.detect_bottlenecks();
        let workloads = self.score_workloads();
        let age = self.estimate_hardware_age();
        let thermal = self.analyze_thermal_envelope();
        let upgrades = self.suggest_upgrades(&bottlenecks);
        let anomalies = self.detect_anomalies();
        let fingerprint = self.compute_fingerprint();

        HardwareAnalysisReport {
            classification,
            classification_confidence: class_confidence,
            performance_tier: tier,
            performance_score: score,
            bottlenecks,
            workload_scores: workloads,
            hardware_age: age,
            thermal_envelope: thermal,
            upgrade_recommendations: upgrades,
            anomalies,
            hardware_fingerprint: fingerprint,
        }
    }

    // ────────────────────────────────────────────────────────────────
    // Feature extraction
    // ────────────────────────────────────────────────────────────────

    fn extract_features(&mut self) {
        self.extract_cpu_features();
        self.extract_memory_features();
        self.extract_gpu_features();
        self.extract_storage_features();
        self.extract_platform_features();
        self.extract_network_features();
    }

    fn extract_cpu_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
                let mut model = String::new();
                let mut cores = std::collections::HashSet::new();
                let mut logical = 0u32;
                let mut max_freq = 0u32;
                let mut vendor = String::new();

                for line in cpuinfo.lines() {
                    if line.starts_with("model name") {
                        if let Some(m) = line.split(':').nth(1) {
                            model = m.trim().to_string();
                        }
                    }
                    if line.starts_with("vendor_id") {
                        if let Some(v) = line.split(':').nth(1) {
                            vendor = v.trim().to_string();
                        }
                    }
                    if line.starts_with("core id") {
                        if let Some(c) = line.split(':').nth(1) {
                            if let Ok(id) = c.trim().parse::<u32>() {
                                cores.insert(id);
                            }
                        }
                    }
                    if line.starts_with("processor") {
                        logical += 1;
                    }
                    if line.starts_with("cpu MHz") {
                        if let Some(f) = line.split(':').nth(1) {
                            if let Ok(freq) = f.trim().parse::<f32>() {
                                max_freq = max_freq.max(freq as u32);
                            }
                        }
                    }
                }

                self.features.cpu_model = model;
                self.features.cpu_vendor = vendor;
                self.features.cpu_cores_physical = if cores.is_empty() { logical } else { cores.len() as u32 };
                self.features.cpu_cores_logical = logical;
                self.features.cpu_max_freq_mhz = max_freq;
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "Get-CimInstance Win32_Processor | Select-Object Name,Manufacturer,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed | ConvertTo-Json"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    self.features.cpu_model = json.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    self.features.cpu_vendor = json.get("Manufacturer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    self.features.cpu_cores_physical = json.get("NumberOfCores").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    self.features.cpu_cores_logical = json.get("NumberOfLogicalProcessors").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    self.features.cpu_max_freq_mhz = json.get("MaxClockSpeed").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
            {
                self.features.cpu_model = String::from_utf8(output.stdout).unwrap_or_default().trim().to_string();
            }
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.physicalcpu"])
                .output()
            {
                self.features.cpu_cores_physical = String::from_utf8(output.stdout).unwrap_or_default().trim().parse().unwrap_or(0);
            }
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.logicalcpu"])
                .output()
            {
                self.features.cpu_cores_logical = String::from_utf8(output.stdout).unwrap_or_default().trim().parse().unwrap_or(0);
            }
        }

        // Infer CPU class
        let model_lower = self.features.cpu_model.to_lowercase();
        self.features.is_server_cpu = model_lower.contains("xeon")
            || model_lower.contains("epyc")
            || model_lower.contains("threadripper")
            || model_lower.contains("platinum")
            || model_lower.contains("gold")
            || model_lower.contains("silver");

        // Infer TDP from model
        self.features.cpu_tdp_watts = Self::infer_cpu_tdp(&model_lower);
    }

    fn extract_memory_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb) = line.split_whitespace().nth(1) {
                            if let Ok(k) = kb.parse::<u64>() {
                                self.features.ram_total_gb = k as f32 / 1_048_576.0;
                            }
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "(Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                if let Ok(kb) = text.trim().parse::<u64>() {
                    self.features.ram_total_gb = kb as f32 / 1_048_576.0;
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("sysctl")
                .args(["-n", "hw.memsize"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                if let Ok(bytes) = text.trim().parse::<u64>() {
                    self.features.ram_total_gb = bytes as f32 / 1_073_741_824.0;
                }
            }
        }
    }

    fn extract_gpu_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // Check for discrete GPUs
            if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("card") && !name.contains('-') {
                        // Check if discrete (not integrated)
                        let boot_vga = std::fs::read_to_string(
                            entry.path().join("device/boot_vga"),
                        )
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                        let label = std::fs::read_to_string(entry.path().join("device/label"))
                            .or_else(|_| std::fs::read_to_string(entry.path().join("device/product_name")))
                            .unwrap_or_default()
                            .trim()
                            .to_string();

                        if !label.is_empty() {
                            self.features.gpu_model = label;
                            self.features.gpu_count += 1;
                        }

                        // Check VRAM
                        if let Ok(vram) = std::fs::read_to_string(
                            entry.path().join("device/mem_info_vram_total"),
                        ) {
                            if let Ok(bytes) = vram.trim().parse::<u64>() {
                                self.features.gpu_vram_gb = bytes as f32 / 1_073_741_824.0;
                                self.features.has_discrete_gpu = true;
                            }
                        }

                        let _ = boot_vga;
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                let gpus: Vec<serde_json::Value> = if text.trim_start().starts_with('[') {
                    serde_json::from_str(&text).unwrap_or_default()
                } else {
                    serde_json::from_str::<serde_json::Value>(&text)
                        .map(|v| vec![v])
                        .unwrap_or_default()
                };

                for gpu in gpus {
                    let name = gpu.get("Name").and_then(|v| v.as_str()).unwrap_or("");
                    let vram = gpu.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);

                    if !name.is_empty() {
                        self.features.gpu_model = name.to_string();
                        self.features.gpu_count += 1;
                        self.features.gpu_vram_gb = vram as f32 / 1_073_741_824.0;

                        let lower = name.to_lowercase();
                        if lower.contains("nvidia") || lower.contains("geforce")
                            || lower.contains("rtx") || lower.contains("gtx")
                            || lower.contains("radeon") || lower.contains("rx ")
                        {
                            self.features.has_discrete_gpu = true;
                        }
                    }
                }
            }
        }

        // Infer GPU capabilities from model name
        let gpu_lower = self.features.gpu_model.to_lowercase();
        self.features.has_tensor_cores = gpu_lower.contains("rtx")
            || gpu_lower.contains("a100")
            || gpu_lower.contains("h100")
            || gpu_lower.contains("l40")
            || gpu_lower.contains("apple m");
        self.features.has_rt_cores = gpu_lower.contains("rtx")
            || gpu_lower.contains("m3") || gpu_lower.contains("m4");
        self.features.gpu_tdp_watts = Self::infer_gpu_tdp(&gpu_lower);
    }

    fn extract_storage_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // Check for NVMe drives
            if std::path::Path::new("/sys/class/nvme").exists() {
                self.features.has_nvme = true;
                self.features.has_ssd = true;
            }

            // Check block devices
            if let Ok(entries) = std::fs::read_dir("/sys/block") {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("sd") || name.starts_with("nvme") {
                        let rotational = std::fs::read_to_string(
                            entry.path().join("queue/rotational"),
                        )
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                        if rotational == "0" {
                            self.features.has_ssd = true;
                        }

                        if let Ok(size) = std::fs::read_to_string(entry.path().join("size")) {
                            if let Ok(sectors) = size.trim().parse::<u64>() {
                                self.features.total_storage_gb += (sectors * 512) as f32 / 1e9;
                            }
                        }

                        if name.starts_with("nvme") {
                            self.features.boot_drive_type = "NVMe".into();
                        } else if self.features.boot_drive_type.is_empty() {
                            self.features.boot_drive_type = if rotational == "0" {
                                "SATA SSD".into()
                            } else {
                                "HDD".into()
                            };
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "Get-PhysicalDisk | Select-Object MediaType,Size,BusType | ConvertTo-Json"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                let disks: Vec<serde_json::Value> = if text.trim_start().starts_with('[') {
                    serde_json::from_str(&text).unwrap_or_default()
                } else {
                    serde_json::from_str::<serde_json::Value>(&text)
                        .map(|v| vec![v])
                        .unwrap_or_default()
                };

                for disk in disks {
                    let media_type = disk.get("MediaType").and_then(|v| v.as_u64()).unwrap_or(0);
                    let size = disk.get("Size").and_then(|v| v.as_u64()).unwrap_or(0);
                    let bus_type = disk.get("BusType").and_then(|v| v.as_u64()).unwrap_or(0);

                    self.features.total_storage_gb += size as f32 / 1e9;

                    // MediaType: 3 = HDD, 4 = SSD
                    if media_type == 4 {
                        self.features.has_ssd = true;
                    }
                    // BusType: 17 = NVMe
                    if bus_type == 17 {
                        self.features.has_nvme = true;
                        self.features.has_ssd = true;
                        self.features.boot_drive_type = "NVMe".into();
                    }
                }
            }
        }
    }

    fn extract_platform_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // Battery check
            self.features.has_battery = std::path::Path::new("/sys/class/power_supply/BAT0").exists()
                || std::path::Path::new("/sys/class/power_supply/BAT1").exists();

            // Chassis type from DMI
            if let Ok(chassis) = std::fs::read_to_string("/sys/class/dmi/id/chassis_type") {
                self.features.chassis_type = match chassis.trim() {
                    "3" | "4" | "5" | "6" | "7" => "Desktop".into(),
                    "8" | "9" | "10" | "14" => "Laptop".into(),
                    "11" | "12" => "Handheld".into(),
                    "13" => "All-in-One".into(),
                    "17" | "23" => "Server".into(),
                    "35" | "36" => "Mini PC".into(),
                    _ => "Unknown".into(),
                };
            }

            // VM detection
            if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
                let lower = product.to_lowercase();
                self.features.is_virtual = lower.contains("virtual")
                    || lower.contains("vmware")
                    || lower.contains("kvm")
                    || lower.contains("qemu")
                    || lower.contains("hyperv")
                    || lower.contains("xen");
            }

            // NUMA nodes
            if let Ok(entries) = std::fs::read_dir("/sys/devices/system/node") {
                self.features.numa_nodes = entries
                    .flatten()
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("node")
                    })
                    .count() as u32;
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Battery
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "(Get-CimInstance Win32_Battery).Status"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default().trim().to_string();
                self.features.has_battery = !text.is_empty();
            }

            // VM detection
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "(Get-CimInstance Win32_ComputerSystem).Model"])
                .output()
            {
                let model = String::from_utf8(output.stdout).unwrap_or_default().trim().to_lowercase();
                self.features.is_virtual = model.contains("virtual")
                    || model.contains("vmware")
                    || model.contains("hyper-v");
                self.features.chassis_type = if self.features.is_virtual {
                    "Virtual".into()
                } else if self.features.has_battery {
                    "Laptop".into()
                } else {
                    "Desktop".into()
                };
            }

            self.features.numa_nodes = 1;
        }
    }

    fn extract_network_features(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name == "lo" {
                        continue;
                    }
                    self.features.nic_count += 1;

                    if let Ok(speed) = std::fs::read_to_string(entry.path().join("speed")) {
                        if let Ok(mbps) = speed.trim().parse::<u32>() {
                            let gbps = mbps as f32 / 1000.0;
                            self.features.max_nic_speed_gbps = self.features.max_nic_speed_gbps.max(gbps);
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command",
                    "Get-NetAdapter -Physical | Select-Object LinkSpeed | ConvertTo-Json"])
                .output()
            {
                let text = String::from_utf8(output.stdout).unwrap_or_default();
                let adapters: Vec<serde_json::Value> = if text.trim_start().starts_with('[') {
                    serde_json::from_str(&text).unwrap_or_default()
                } else {
                    serde_json::from_str::<serde_json::Value>(&text)
                        .map(|v| vec![v])
                        .unwrap_or_default()
                };

                for adapter in &adapters {
                    self.features.nic_count += 1;
                    if let Some(speed) = adapter.get("LinkSpeed").and_then(|v| v.as_str()) {
                        // Parse "1 Gbps", "10 Gbps", "100 Mbps"
                        let parts: Vec<&str> = speed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            if let Ok(val) = parts[0].parse::<f32>() {
                                let gbps = if parts[1].starts_with('G') {
                                    val
                                } else if parts[1].starts_with('M') {
                                    val / 1000.0
                                } else {
                                    val / 1000.0
                                };
                                self.features.max_nic_speed_gbps =
                                    self.features.max_nic_speed_gbps.max(gbps);
                            }
                        }
                    }
                }
            }
        }
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: System Classification
    // ────────────────────────────────────────────────────────────────

    fn classify_system(&self) -> (SystemClass, f32) {
        let f = &self.features;
        let mut scores: Vec<(SystemClass, f32)> = Vec::new();

        // Virtual machine
        if f.is_virtual {
            scores.push((SystemClass::VirtualMachine, 0.95));
        }

        // Server indicators
        let server_score = {
            let mut s = 0.0f32;
            if f.is_server_cpu { s += 0.35; }
            if f.has_ecc { s += 0.2; }
            if f.cpu_cores_physical >= 16 { s += 0.15; }
            if f.ram_total_gb >= 64.0 { s += 0.1; }
            if f.numa_nodes >= 2 { s += 0.15; }
            if f.chassis_type.contains("Server") { s += 0.3; }
            if !f.has_battery { s += 0.05; }
            if f.nic_count >= 2 { s += 0.1; }
            s.min(0.95)
        };
        if server_score > 0.3 {
            scores.push((SystemClass::Server, server_score));
        }

        // Laptop indicators
        let laptop_score = {
            let mut s = 0.0f32;
            if f.has_battery { s += 0.4; }
            if f.chassis_type.contains("Laptop") { s += 0.3; }
            if f.cpu_cores_physical <= 8 { s += 0.05; }
            if f.ram_total_gb <= 32.0 { s += 0.05; }
            s.min(0.95)
        };
        if laptop_score > 0.3 {
            let gpu_lower = f.gpu_model.to_lowercase();
            if f.has_discrete_gpu && (gpu_lower.contains("rtx") || gpu_lower.contains("rx ")) {
                scores.push((SystemClass::GamingLaptop, laptop_score * 0.9));
            } else if f.ram_total_gb <= 16.0 && f.cpu_cores_physical <= 4 {
                scores.push((SystemClass::Ultrabook, laptop_score * 0.8));
            } else {
                scores.push((SystemClass::Laptop, laptop_score));
            }
        }

        // Desktop indicators
        if !f.has_battery && !f.is_virtual && !f.chassis_type.contains("Server") {
            let gpu_lower = f.gpu_model.to_lowercase();
            let is_gaming_gpu = gpu_lower.contains("rtx") || gpu_lower.contains("gtx")
                || gpu_lower.contains("rx 6") || gpu_lower.contains("rx 7");

            if f.is_server_cpu || f.ram_total_gb >= 64.0 {
                scores.push((SystemClass::Workstation, 0.6));
            } else if is_gaming_gpu {
                scores.push((SystemClass::GamingDesktop, 0.6));
            } else {
                scores.push((SystemClass::Desktop, 0.5));
            }
        }

        // Embedded
        if f.cpu_cores_physical <= 4 && f.ram_total_gb <= 4.0 {
            let model = f.cpu_model.to_lowercase();
            if model.contains("arm") || model.contains("cortex") || model.contains("tegra") {
                scores.push((SystemClass::Embedded, 0.8));
            }
        }

        // Mini PC
        if f.chassis_type.contains("Mini") {
            scores.push((SystemClass::MiniPc, 0.7));
        }

        // Pick highest-scoring classification
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores
            .first()
            .cloned()
            .unwrap_or((SystemClass::Unknown, 0.0))
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Performance Tier
    // ────────────────────────────────────────────────────────────────

    fn compute_performance_tier(&self) -> (PerformanceTier, u8) {
        let f = &self.features;
        let mut score = 0u32; // 0-1000 internal scale

        // CPU scoring (up to 350 points)
        score += match f.cpu_cores_physical {
            0..=1 => 10,
            2 => 30,
            4 => 80,
            6 => 120,
            8 => 160,
            12 => 200,
            16 => 250,
            24 => 280,
            32 => 300,
            _ => 350, // 64+ cores
        };

        // Frequency bonus
        score += match f.cpu_max_freq_mhz {
            0..=1500 => 0,
            1501..=2500 => 10,
            2501..=3500 => 25,
            3501..=4500 => 40,
            _ => 50, // 4.5+ GHz
        };

        // RAM scoring (up to 200 points)
        score += match f.ram_total_gb as u32 {
            0..=2 => 5,
            3..=4 => 20,
            5..=8 => 50,
            9..=16 => 80,
            17..=32 => 120,
            33..=64 => 160,
            65..=128 => 180,
            _ => 200, // 128+ GB
        };

        // GPU scoring (up to 300 points)
        if f.has_discrete_gpu {
            score += 50; // Base discrete GPU bonus
            score += match f.gpu_vram_gb as u32 {
                0..=2 => 10,
                3..=4 => 40,
                5..=8 => 80,
                9..=12 => 120,
                13..=16 => 160,
                17..=24 => 200,
                _ => 250, // 24+ GB VRAM
            };
            if f.has_tensor_cores {
                score += 30;
            }
            if f.has_rt_cores {
                score += 20;
            }
        }

        // Storage scoring (up to 100 points)
        if f.has_nvme {
            score += 60;
        } else if f.has_ssd {
            score += 30;
        }
        score += match f.total_storage_gb as u32 {
            0..=128 => 5,
            129..=512 => 15,
            513..=2048 => 25,
            _ => 40,
        };

        // Network scoring (up to 50 points)
        score += match f.max_nic_speed_gbps as u32 {
            0 => 0,
            1 => 15,
            10 => 30,
            25 => 40,
            _ => 50, // 40+ Gbps
        };

        // Normalize to 0-100
        let normalized = ((score as f32 / 1000.0) * 100.0).min(100.0) as u8;

        let tier = match normalized {
            0..=10 => PerformanceTier::UltraLow,
            11..=25 => PerformanceTier::Low,
            26..=35 => PerformanceTier::MidLow,
            36..=50 => PerformanceTier::Mid,
            51..=65 => PerformanceTier::MidHigh,
            66..=80 => PerformanceTier::High,
            81..=90 => PerformanceTier::Ultra,
            _ => PerformanceTier::Datacenter,
        };

        (tier, normalized)
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Bottleneck Detection
    // ────────────────────────────────────────────────────────────────

    fn detect_bottlenecks(&self) -> Vec<Bottleneck> {
        let f = &self.features;
        let mut bottlenecks = Vec::new();

        // CPU vs GPU imbalance
        if f.has_discrete_gpu && f.cpu_cores_physical <= 4 && f.gpu_vram_gb >= 8.0 {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::CpuBound,
                component: "CPU".into(),
                severity: 70,
                reason: format!(
                    "Only {} CPU cores paired with {}GB GPU — CPU will bottleneck GPU-intensive tasks",
                    f.cpu_cores_physical, f.gpu_vram_gb as u32
                ),
                confidence: 0.75,
            });
        }

        // RAM too low for GPU workloads
        if f.has_discrete_gpu && f.gpu_vram_gb >= 8.0 && f.ram_total_gb < 16.0 {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::MemoryBound,
                component: "System RAM".into(),
                severity: 60,
                reason: format!(
                    "{:.0}GB RAM is insufficient for GPU workloads with {}GB VRAM",
                    f.ram_total_gb, f.gpu_vram_gb as u32
                ),
                confidence: 0.7,
            });
        }

        // RAM too low for core count
        if f.cpu_cores_physical >= 8 && f.ram_total_gb < 16.0 {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::MemoryBound,
                component: "System RAM".into(),
                severity: 55,
                reason: format!(
                    "{:.0}GB RAM for {} cores — should have at least 2GB/core",
                    f.ram_total_gb, f.cpu_cores_physical
                ),
                confidence: 0.65,
            });
        }

        // No SSD
        if !f.has_ssd && !f.has_nvme {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::StorageBound,
                component: "Storage".into(),
                severity: 80,
                reason: "No SSD detected — HDD will severely bottleneck modern workloads".into(),
                confidence: 0.85,
            });
        }

        // High core count with SATA (not NVMe)
        if f.cpu_cores_physical >= 16 && f.has_ssd && !f.has_nvme {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::StorageBound,
                component: "Storage bus".into(),
                severity: 40,
                reason: "High-core-count CPU with SATA SSD — NVMe would reduce I/O bottleneck".into(),
                confidence: 0.6,
            });
        }

        // Slow network for server
        if f.is_server_cpu && f.max_nic_speed_gbps < 10.0 {
            bottlenecks.push(Bottleneck {
                bottleneck_type: BottleneckType::NetworkBound,
                component: "Network".into(),
                severity: 45,
                reason: format!(
                    "Server CPU with only {:.0} Gbps NIC — consider 10/25 GbE",
                    f.max_nic_speed_gbps
                ),
                confidence: 0.5,
            });
        }

        bottlenecks
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Workload Suitability (Bayesian-inspired scoring)
    // ────────────────────────────────────────────────────────────────

    fn score_workloads(&self) -> Vec<WorkloadSuitability> {
        let f = &self.features;
        let mut results = Vec::new();

        // ML Training
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.has_tensor_cores {
                score += 30;
                strengths.push("Tensor cores available".into());
            }
            if f.gpu_vram_gb >= 24.0 {
                score += 25;
                strengths.push(format!("{:.0}GB VRAM for large models", f.gpu_vram_gb));
            } else if f.gpu_vram_gb >= 12.0 {
                score += 15;
            } else if f.gpu_vram_gb >= 8.0 {
                score += 8;
                limiting.push("Limited VRAM for large models".into());
            } else {
                limiting.push("Insufficient VRAM for ML training".into());
            }
            if f.ram_total_gb >= 64.0 {
                score += 15;
            } else if f.ram_total_gb >= 32.0 {
                score += 10;
            } else {
                limiting.push("RAM may limit dataset size".into());
            }
            if f.has_nvme { score += 10; }
            if f.cpu_cores_physical >= 8 { score += 10; }
            if f.gpu_count > 1 {
                score += 10;
                strengths.push("Multi-GPU available".into());
            }

            results.push(WorkloadSuitability {
                workload: Workload::MlTraining,
                score: score.min(100),
                confidence: 0.7,
                limiting_factors: limiting,
                strengths,
            });
        }

        // ML Inference
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.has_discrete_gpu { score += 30; strengths.push("Discrete GPU".into()); }
            if f.gpu_vram_gb >= 8.0 { score += 20; }
            if f.has_tensor_cores { score += 15; }
            if f.cpu_cores_physical >= 4 { score += 15; }
            if f.ram_total_gb >= 16.0 { score += 10; }
            if f.has_nvme { score += 10; }
            if !f.has_discrete_gpu {
                limiting.push("No discrete GPU — CPU inference only".into());
            }

            results.push(WorkloadSuitability {
                workload: Workload::MlInference,
                score: score.min(100),
                confidence: 0.75,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Gaming
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.has_discrete_gpu && f.gpu_vram_gb >= 8.0 {
                score += 35;
                strengths.push(format!("Discrete GPU with {:.0}GB VRAM", f.gpu_vram_gb));
            } else if f.has_discrete_gpu {
                score += 15;
            } else {
                limiting.push("No discrete GPU".into());
            }
            if f.has_rt_cores { score += 10; strengths.push("Ray tracing support".into()); }
            if f.cpu_max_freq_mhz >= 4000 { score += 15; strengths.push("High CPU clock speed".into()); }
            else if f.cpu_max_freq_mhz >= 3000 { score += 10; }
            if f.ram_total_gb >= 16.0 { score += 10; }
            else { limiting.push("< 16GB RAM".into()); }
            if f.has_nvme { score += 10; }
            if f.cpu_cores_physical >= 6 { score += 10; }
            if f.has_ssd { score += 10; }

            results.push(WorkloadSuitability {
                workload: Workload::Gaming,
                score: score.min(100),
                confidence: 0.8,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Video Editing
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.has_discrete_gpu { score += 20; }
            if f.cpu_cores_physical >= 8 { score += 20; strengths.push("Multi-core for rendering".into()); }
            else if f.cpu_cores_physical >= 6 { score += 15; }
            if f.ram_total_gb >= 32.0 { score += 20; strengths.push("Plenty of RAM for timelines".into()); }
            else if f.ram_total_gb >= 16.0 { score += 10; }
            else { limiting.push("< 16GB RAM limits timeline length".into()); }
            if f.has_nvme { score += 15; strengths.push("NVMe for fast media reads".into()); }
            if f.total_storage_gb >= 2000.0 { score += 10; }
            else { limiting.push("Limited storage for large projects".into()); }
            if f.gpu_vram_gb >= 8.0 { score += 15; }

            results.push(WorkloadSuitability {
                workload: Workload::VideoEditing,
                score: score.min(100),
                confidence: 0.7,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Compilation
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.cpu_cores_physical >= 16 { score += 35; strengths.push("Many cores for parallel builds".into()); }
            else if f.cpu_cores_physical >= 8 { score += 25; }
            else if f.cpu_cores_physical >= 4 { score += 15; }
            else { limiting.push("Few cores — slow parallel builds".into()); }
            if f.ram_total_gb >= 32.0 { score += 25; }
            else if f.ram_total_gb >= 16.0 { score += 15; }
            if f.has_nvme { score += 20; strengths.push("Fast build I/O with NVMe".into()); }
            else if f.has_ssd { score += 10; }
            if f.cpu_max_freq_mhz >= 4000 { score += 10; }
            if f.total_storage_gb >= 512.0 { score += 10; }

            results.push(WorkloadSuitability {
                workload: Workload::Compilation,
                score: score.min(100),
                confidence: 0.8,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Web Server
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.cpu_cores_physical >= 8 { score += 25; }
            else if f.cpu_cores_physical >= 4 { score += 15; }
            if f.ram_total_gb >= 16.0 { score += 20; }
            if f.max_nic_speed_gbps >= 10.0 { score += 25; strengths.push("10+ GbE networking".into()); }
            else if f.max_nic_speed_gbps >= 1.0 { score += 10; }
            else { limiting.push("Slow network".into()); }
            if f.has_nvme { score += 15; }
            if f.nic_count >= 2 { score += 10; strengths.push("Multiple NICs for redundancy".into()); }
            if f.is_server_cpu { score += 5; }

            results.push(WorkloadSuitability {
                workload: Workload::WebServer,
                score: score.min(100),
                confidence: 0.65,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Database Server
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.ram_total_gb >= 64.0 { score += 30; strengths.push("Large RAM for caching".into()); }
            else if f.ram_total_gb >= 32.0 { score += 20; }
            else { limiting.push("RAM limits index caching".into()); }
            if f.has_nvme { score += 25; strengths.push("NVMe for fast IOPS".into()); }
            else if f.has_ssd { score += 15; }
            else { limiting.push("HDD severely limits database IOPS".into()); }
            if f.cpu_cores_physical >= 8 { score += 20; }
            if f.total_storage_gb >= 2000.0 { score += 15; }
            if f.has_ecc { score += 10; strengths.push("ECC memory for data integrity".into()); }

            results.push(WorkloadSuitability {
                workload: Workload::DatabaseServer,
                score: score.min(100),
                confidence: 0.65,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Virtualization
        {
            let mut score = 0u8;
            let mut strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.cpu_cores_physical >= 16 { score += 30; strengths.push("Many cores for VMs".into()); }
            else if f.cpu_cores_physical >= 8 { score += 15; }
            else { limiting.push("Few cores limits VM density".into()); }
            if f.ram_total_gb >= 128.0 { score += 30; }
            else if f.ram_total_gb >= 64.0 { score += 20; strengths.push("Good RAM for VMs".into()); }
            else if f.ram_total_gb >= 32.0 { score += 10; }
            else { limiting.push("Limited RAM for virtual machines".into()); }
            if f.has_nvme { score += 15; }
            if f.numa_nodes >= 2 { score += 10; strengths.push("NUMA for VM pinning".into()); }
            if f.is_server_cpu { score += 10; }
            if f.total_storage_gb >= 2000.0 { score += 5; }

            results.push(WorkloadSuitability {
                workload: Workload::Virtualization,
                score: score.min(100),
                confidence: 0.7,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Office Productivity
        {
            let mut score = 50u8; // Almost anything can handle office work
            let strengths = Vec::new();
            let mut limiting = Vec::new();

            if f.cpu_cores_physical >= 4 { score += 15; }
            if f.ram_total_gb >= 8.0 { score += 15; }
            else { limiting.push("< 8GB RAM may cause slowdowns".into()); }
            if f.has_ssd { score += 15; }
            else { limiting.push("HDD makes boot and app launch slow".into()); }
            if f.cpu_cores_physical < 2 || f.ram_total_gb < 4.0 { score = score.saturating_sub(30); }

            results.push(WorkloadSuitability {
                workload: Workload::OfficeProductivity,
                score: score.min(100),
                confidence: 0.9,
                limiting_factors: limiting,
                strengths,
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Hardware Age Estimation
    // ────────────────────────────────────────────────────────────────

    fn estimate_hardware_age(&self) -> HardwareAge {
        let f = &self.features;
        let cpu_lower = f.cpu_model.to_lowercase();
        let gpu_lower = f.gpu_model.to_lowercase();

        let cpu_year = Self::infer_cpu_year(&cpu_lower);
        let gpu_year = Self::infer_gpu_year(&gpu_lower);

        let years = match (cpu_year, gpu_year) {
            (Some(cy), Some(gy)) => {
                let avg = (cy + gy) as f32 / 2.0;
                let current = 2025.0;
                (current - avg).max(0.0)
            }
            (Some(cy), None) => (2025.0 - cy as f32).max(0.0),
            (None, Some(gy)) => (2025.0 - gy as f32).max(0.0),
            (None, None) => {
                // Guess from specs
                if f.cpu_cores_physical >= 16 && f.has_nvme {
                    1.0
                } else if f.cpu_cores_physical >= 8 && f.has_ssd {
                    3.0
                } else if f.cpu_cores_physical >= 4 {
                    5.0
                } else {
                    7.0
                }
            }
        };

        let confidence = match (cpu_year, gpu_year) {
            (Some(_), Some(_)) => 0.85,
            (Some(_), None) | (None, Some(_)) => 0.65,
            (None, None) => 0.3,
        };

        let reasoning = format!(
            "CPU: {} (est. {}), GPU: {} (est. {})",
            f.cpu_model,
            cpu_year.map_or("unknown".to_string(), |y| y.to_string()),
            f.gpu_model,
            gpu_year.map_or("unknown".to_string(), |y| y.to_string()),
        );

        HardwareAge {
            cpu_year,
            gpu_year,
            estimated_age_years: years,
            confidence,
            reasoning,
        }
    }

    fn infer_cpu_year(model: &str) -> Option<u16> {
        // Intel Core generations
        if model.contains("14th gen") || model.contains("core ultra") || model.contains("14900") || model.contains("14700") {
            return Some(2024);
        }
        if model.contains("13th gen") || model.contains("13900") || model.contains("13700") || model.contains("13600") {
            return Some(2022);
        }
        if model.contains("12th gen") || model.contains("12900") || model.contains("12700") || model.contains("12600") {
            return Some(2021);
        }
        if model.contains("11th gen") || model.contains("11900") || model.contains("11700") {
            return Some(2021);
        }
        if model.contains("10th gen") || model.contains("10900") || model.contains("10700") {
            return Some(2020);
        }
        if model.contains("9th gen") || model.contains("9900") || model.contains("9700") {
            return Some(2018);
        }
        if model.contains("8th gen") || model.contains("8700") {
            return Some(2017);
        }

        // AMD Ryzen (generation from model number pattern: 3xxx=Zen2, 5xxx=Zen3, 7xxx=Zen4, 9xxx=Zen5)
        if model.contains("ryzen 9 9") || model.contains("ryzen 7 9") || model.contains("zen 5") {
            return Some(2024);
        }
        if model.contains("ryzen 9 7") || model.contains("ryzen 7 7") || model.contains("ryzen 5 7") || model.contains("zen 4") {
            return Some(2022);
        }
        if model.contains("ryzen 9 5") || model.contains("ryzen 7 5") || model.contains("ryzen 5 5") || model.contains("zen 3") {
            return Some(2020);
        }
        if model.contains("ryzen 9 3") || model.contains("ryzen 7 3") || model.contains("ryzen 5 3") || model.contains("zen 2") {
            return Some(2019);
        }

        // AMD EPYC
        if model.contains("epyc 9") { return Some(2023); }
        if model.contains("epyc 7") && model.contains("3") { return Some(2022); }
        if model.contains("epyc 7") { return Some(2019); }

        // Intel Xeon (simplified)
        if model.contains("xeon w9") || model.contains("xeon w7") { return Some(2023); }
        if model.contains("xeon") && model.contains("v5") { return Some(2017); }
        if model.contains("xeon") && model.contains("v4") { return Some(2016); }

        // Apple
        if model.contains("m4") { return Some(2024); }
        if model.contains("m3") { return Some(2023); }
        if model.contains("m2") { return Some(2022); }
        if model.contains("m1") { return Some(2020); }

        None
    }

    fn infer_gpu_year(model: &str) -> Option<u16> {
        // NVIDIA GeForce
        if model.contains("rtx 50") || model.contains("5090") || model.contains("5080") { return Some(2025); }
        if model.contains("rtx 40") || model.contains("4090") || model.contains("4080") { return Some(2022); }
        if model.contains("rtx 30") || model.contains("3090") || model.contains("3080") { return Some(2020); }
        if model.contains("rtx 20") || model.contains("2080") || model.contains("2070") { return Some(2018); }
        if model.contains("gtx 1080") || model.contains("gtx 10") { return Some(2016); }
        if model.contains("gtx 9") { return Some(2014); }

        // NVIDIA Data Center
        if model.contains("h100") || model.contains("h200") { return Some(2023); }
        if model.contains("a100") { return Some(2020); }
        if model.contains("v100") { return Some(2017); }

        // AMD Radeon
        if model.contains("rx 9") { return Some(2025); }
        if model.contains("rx 7") { return Some(2022); }
        if model.contains("rx 6") { return Some(2020); }
        if model.contains("rx 5") { return Some(2019); }

        // Intel Arc
        if model.contains("arc b") { return Some(2024); }
        if model.contains("arc a") { return Some(2022); }

        // Apple (same as CPU year for SoC)
        if model.contains("m4") { return Some(2024); }
        if model.contains("m3") { return Some(2023); }
        if model.contains("m2") { return Some(2022); }
        if model.contains("m1") { return Some(2020); }

        None
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Thermal Envelope Modeling
    // ────────────────────────────────────────────────────────────────

    fn analyze_thermal_envelope(&self) -> ThermalEnvelope {
        let f = &self.features;
        let cpu_tdp = f.cpu_tdp_watts;
        let gpu_tdp = f.gpu_tdp_watts;
        let total_tdp = cpu_tdp + gpu_tdp + 30.0; // +30W for RAM, storage, board

        let headroom = if f.has_battery {
            // Laptop — limited cooling
            if total_tdp > 150.0 {
                ThermalHeadroom::Insufficient
            } else if total_tdp > 100.0 {
                ThermalHeadroom::Marginal
            } else if total_tdp > 60.0 {
                ThermalHeadroom::Adequate
            } else {
                ThermalHeadroom::Ample
            }
        } else if f.chassis_type.contains("Mini") {
            // Mini PC — moderate cooling
            if total_tdp > 120.0 {
                ThermalHeadroom::Marginal
            } else {
                ThermalHeadroom::Adequate
            }
        } else {
            // Desktop/server — good cooling assumed
            if total_tdp > 500.0 {
                ThermalHeadroom::Marginal
            } else {
                ThermalHeadroom::Ample
            }
        };

        let cooling_score = match &headroom {
            ThermalHeadroom::Ample => 90,
            ThermalHeadroom::Adequate => 70,
            ThermalHeadroom::Marginal => 40,
            ThermalHeadroom::Insufficient => 15,
            ThermalHeadroom::Unknown => 50,
        };

        let mut recommendations = Vec::new();
        if headroom == ThermalHeadroom::Insufficient || headroom == ThermalHeadroom::Marginal {
            recommendations.push("Consider improving cooling (better fans, repasting, or external cooling)".into());
        }
        if f.has_battery && gpu_tdp > 80.0 {
            recommendations.push("High GPU TDP in a laptop — may experience thermal throttling under sustained load".into());
        }

        ThermalEnvelope {
            estimated_total_tdp_watts: total_tdp,
            cpu_tdp_watts: cpu_tdp,
            gpu_tdp_watts: gpu_tdp,
            headroom,
            cooling_score,
            recommendations,
        }
    }

    fn infer_cpu_tdp(model_lower: &str) -> f32 {
        // Known TDP ranges by CPU class
        if model_lower.contains("i9-14") || model_lower.contains("i9-13") { return 125.0; }
        if model_lower.contains("i7-14") || model_lower.contains("i7-13") { return 65.0; }
        if model_lower.contains("i5-14") || model_lower.contains("i5-13") { return 65.0; }
        if model_lower.contains("i3") { return 35.0; }
        if model_lower.contains("ryzen 9") { return 120.0; }
        if model_lower.contains("ryzen 7") { return 65.0; }
        if model_lower.contains("ryzen 5") { return 65.0; }
        if model_lower.contains("ryzen 3") { return 35.0; }
        if model_lower.contains("epyc") { return 225.0; }
        if model_lower.contains("xeon") { return 150.0; }
        if model_lower.contains("threadripper") { return 280.0; }
        if model_lower.contains("celeron") || model_lower.contains("atom") { return 15.0; }
        if model_lower.contains("m1") || model_lower.contains("m2") { return 20.0; }
        if model_lower.contains("m3") || model_lower.contains("m4") { return 22.0; }
        if model_lower.contains("arm") || model_lower.contains("cortex") { return 5.0; }
        65.0 // default guess
    }

    fn infer_gpu_tdp(model_lower: &str) -> f32 {
        if model_lower.contains("4090") { return 450.0; }
        if model_lower.contains("4080") { return 320.0; }
        if model_lower.contains("4070 ti") { return 285.0; }
        if model_lower.contains("4070") { return 200.0; }
        if model_lower.contains("4060") { return 115.0; }
        if model_lower.contains("3090") { return 350.0; }
        if model_lower.contains("3080") { return 320.0; }
        if model_lower.contains("3070") { return 220.0; }
        if model_lower.contains("3060") { return 170.0; }
        if model_lower.contains("h100") { return 700.0; }
        if model_lower.contains("a100") { return 300.0; }
        if model_lower.contains("rx 7900") { return 355.0; }
        if model_lower.contains("rx 7800") { return 263.0; }
        if model_lower.contains("rx 7700") { return 245.0; }
        if model_lower.contains("rx 7600") { return 165.0; }
        if model_lower.contains("arc a7") { return 225.0; }
        if model_lower.contains("arc a5") { return 175.0; }
        if model_lower.is_empty() { return 0.0; }
        100.0 // default for unknown discrete GPU
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Upgrade Recommendations
    // ────────────────────────────────────────────────────────────────

    fn suggest_upgrades(&self, bottlenecks: &[Bottleneck]) -> Vec<UpgradeRecommendation> {
        let f = &self.features;
        let mut recs = Vec::new();

        // RAM upgrades
        if f.ram_total_gb < 8.0 {
            recs.push(UpgradeRecommendation {
                component: "RAM".into(),
                current: format!("{:.0}GB", f.ram_total_gb),
                recommended: "16GB+".into(),
                description: "Upgrade to at least 16GB RAM".into(),
                expected_impact: "40-60% reduction in memory-related slowdowns".into(),
                priority: 9,
                cost_tier: 2,
            });
        } else if f.ram_total_gb < 16.0 && f.has_discrete_gpu {
            recs.push(UpgradeRecommendation {
                component: "RAM".into(),
                current: format!("{:.0}GB", f.ram_total_gb),
                recommended: "32GB".into(),
                description: "Upgrade RAM for GPU workloads".into(),
                expected_impact: "20-30% better multitasking and GPU workflow performance".into(),
                priority: 7,
                cost_tier: 2,
            });
        }

        // Storage upgrades
        if !f.has_ssd && !f.has_nvme {
            recs.push(UpgradeRecommendation {
                component: "Boot Drive".into(),
                current: "HDD".into(),
                recommended: "NVMe SSD".into(),
                description: "Replace HDD with NVMe SSD for dramatic speed improvement".into(),
                expected_impact: "5-10x faster boot, app launch, and file operations".into(),
                priority: 10,
                cost_tier: 2,
            });
        } else if f.has_ssd && !f.has_nvme && f.cpu_cores_physical >= 8 {
            recs.push(UpgradeRecommendation {
                component: "Boot Drive".into(),
                current: "SATA SSD".into(),
                recommended: "NVMe Gen4 SSD".into(),
                description: "Upgrade to NVMe for faster I/O".into(),
                expected_impact: "3-5x faster sequential reads, lower I/O latency".into(),
                priority: 5,
                cost_tier: 2,
            });
        }

        // GPU upgrades based on bottlenecks
        for bn in bottlenecks {
            match bn.bottleneck_type {
                BottleneckType::GpuBound if bn.severity > 50 => {
                    recs.push(UpgradeRecommendation {
                        component: "GPU".into(),
                        current: f.gpu_model.clone(),
                        recommended: "Next-tier GPU".into(),
                        description: "GPU is the primary bottleneck".into(),
                        expected_impact: "30-100% improvement in GPU-bound workloads".into(),
                        priority: 8,
                        cost_tier: 4,
                    });
                }
                BottleneckType::CpuBound if bn.severity > 50 => {
                    recs.push(UpgradeRecommendation {
                        component: "CPU".into(),
                        current: f.cpu_model.clone(),
                        recommended: "Higher core-count CPU".into(),
                        description: "CPU bottlenecks the system".into(),
                        expected_impact: "20-50% improvement in CPU-bound workloads".into(),
                        priority: 7,
                        cost_tier: 4,
                    });
                }
                BottleneckType::NetworkBound if bn.severity > 40 => {
                    recs.push(UpgradeRecommendation {
                        component: "Network".into(),
                        current: format!("{:.0} Gbps NIC", f.max_nic_speed_gbps),
                        recommended: "10 GbE NIC".into(),
                        description: "Network is a bottleneck for server workloads".into(),
                        expected_impact: "10x network throughput".into(),
                        priority: 6,
                        cost_tier: 3,
                    });
                }
                _ => {}
            }
        }

        // Sort by priority descending
        recs.sort_by(|a, b| b.priority.cmp(&a.priority));
        recs
    }

    // ────────────────────────────────────────────────────────────────
    // Inference: Anomaly Detection
    // ────────────────────────────────────────────────────────────────

    fn detect_anomalies(&self) -> Vec<HardwareAnomaly> {
        let f = &self.features;
        let mut anomalies = Vec::new();

        // Huge GPU + tiny RAM
        if f.gpu_vram_gb >= 16.0 && f.ram_total_gb < 16.0 {
            anomalies.push(HardwareAnomaly {
                description: format!(
                    "GPU has {:.0}GB VRAM but system only has {:.0}GB RAM",
                    f.gpu_vram_gb, f.ram_total_gb
                ),
                severity: AnomalySeverity::Warning,
                explanation: "GPU VRAM exceeds system RAM — data loading will bottleneck GPU workloads".into(),
            });
        }

        // Server CPU in laptop
        if f.is_server_cpu && f.has_battery {
            anomalies.push(HardwareAnomaly {
                description: "Server-class CPU detected in a battery-powered system".into(),
                severity: AnomalySeverity::Info,
                explanation: "This is unusual but may be a mobile workstation".into(),
            });
        }

        // More GPU VRAM than system RAM
        if f.gpu_vram_gb > f.ram_total_gb && f.gpu_vram_gb > 0.0 {
            anomalies.push(HardwareAnomaly {
                description: format!(
                    "GPU VRAM ({:.0}GB) exceeds system RAM ({:.0}GB)",
                    f.gpu_vram_gb, f.ram_total_gb
                ),
                severity: AnomalySeverity::Warning,
                explanation: "System RAM should typically be at least 2x GPU VRAM for optimal performance".into(),
            });
        }

        // Many cores + low frequency
        if f.cpu_cores_physical >= 32 && f.cpu_max_freq_mhz < 2500 {
            anomalies.push(HardwareAnomaly {
                description: format!(
                    "{} cores at only {} MHz",
                    f.cpu_cores_physical, f.cpu_max_freq_mhz
                ),
                severity: AnomalySeverity::Info,
                explanation: "Many-core + low clock = throughput-optimized (server/HPC), not latency-sensitive".into(),
            });
        }

        // No discrete GPU with tensor expectations
        if !f.has_discrete_gpu && f.ram_total_gb >= 64.0 && f.cpu_cores_physical >= 16 {
            anomalies.push(HardwareAnomaly {
                description: "Powerful CPU and RAM but no discrete GPU".into(),
                severity: AnomalySeverity::Info,
                explanation: "System may be optimized for CPU-bound workloads or virtualization".into(),
            });
        }

        // Very large storage with slow network
        if f.total_storage_gb >= 10000.0 && f.max_nic_speed_gbps < 10.0 {
            anomalies.push(HardwareAnomaly {
                description: format!(
                    "{:.0}TB storage with only {:.0} Gbps network",
                    f.total_storage_gb / 1000.0,
                    f.max_nic_speed_gbps
                ),
                severity: AnomalySeverity::Warning,
                explanation: "Large storage with slow network — data transfers will be slow".into(),
            });
        }

        anomalies
    }

    // ────────────────────────────────────────────────────────────────
    // Hardware Fingerprint
    // ────────────────────────────────────────────────────────────────

    fn compute_fingerprint(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.features.cpu_model.hash(&mut hasher);
        self.features.cpu_cores_physical.hash(&mut hasher);
        self.features.cpu_cores_logical.hash(&mut hasher);
        (self.features.ram_total_gb as u64).hash(&mut hasher);
        self.features.gpu_model.hash(&mut hasher);
        (self.features.gpu_vram_gb as u64).hash(&mut hasher);
        self.features.has_nvme.hash(&mut hasher);
        (self.features.total_storage_gb as u64).hash(&mut hasher);

        format!("{:016x}", hasher.finish())
    }
}

impl Default for HardwareInferenceEngine {
    fn default() -> Self {
        Self::new().unwrap_or(Self {
            features: HardwareFeatures::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = HardwareInferenceEngine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_engine_default() {
        let engine = HardwareInferenceEngine::default();
        let report = engine.full_analysis();
        let _ = &report.classification;
        let _ = &report.performance_tier;
    }

    #[test]
    fn test_full_analysis_report() {
        let engine = HardwareInferenceEngine::new().unwrap();
        let report = engine.full_analysis();
        assert!(!report.hardware_fingerprint.is_empty());
        assert!(!report.workload_scores.is_empty());
    }

    #[test]
    fn test_gaming_desktop_classification() {
        let engine = HardwareInferenceEngine {
            features: HardwareFeatures {
                cpu_cores_physical: 8,
                cpu_cores_logical: 16,
                cpu_max_freq_mhz: 5000,
                cpu_model: "Intel Core i7-14700K".into(),
                cpu_vendor: "Intel".into(),
                is_server_cpu: false,
                has_ecc: false,
                cpu_tdp_watts: 125.0,
                ram_total_gb: 32.0,
                ram_channels: 2,
                ram_speed_mhz: 3600,
                has_discrete_gpu: true,
                gpu_count: 1,
                gpu_model: "NVIDIA GeForce RTX 4070 Ti".into(),
                gpu_vram_gb: 12.0,
                has_tensor_cores: true,
                has_rt_cores: true,
                gpu_tdp_watts: 285.0,
                has_nvme: true,
                has_ssd: true,
                total_storage_gb: 2000.0,
                boot_drive_type: "NVMe".into(),
                has_battery: false,
                chassis_type: "Desktop".into(),
                is_virtual: false,
                numa_nodes: 1,
                pcie_gen: 4,
                max_nic_speed_gbps: 2.5,
                nic_count: 1,
            },
        };

        let report = engine.full_analysis();
        assert!(
            report.classification == SystemClass::GamingDesktop
                || report.classification == SystemClass::Desktop
                || report.classification == SystemClass::Workstation
        );
        assert!(report.performance_tier >= PerformanceTier::MidHigh);
        assert!(report.performance_score >= 50);
    }

    #[test]
    fn test_server_classification() {
        let engine = HardwareInferenceEngine {
            features: HardwareFeatures {
                cpu_cores_physical: 64,
                cpu_cores_logical: 128,
                cpu_max_freq_mhz: 2800,
                cpu_model: "AMD EPYC 9554".into(),
                cpu_vendor: "AMD".into(),
                is_server_cpu: true,
                has_ecc: true,
                cpu_tdp_watts: 225.0,
                ram_total_gb: 512.0,
                ram_channels: 8,
                ram_speed_mhz: 4800,
                has_discrete_gpu: false,
                gpu_count: 0,
                gpu_model: String::new(),
                gpu_vram_gb: 0.0,
                has_tensor_cores: false,
                has_rt_cores: false,
                gpu_tdp_watts: 0.0,
                has_nvme: true,
                has_ssd: true,
                total_storage_gb: 15000.0,
                boot_drive_type: "NVMe".into(),
                has_battery: false,
                chassis_type: "Server".into(),
                is_virtual: false,
                numa_nodes: 2,
                pcie_gen: 5,
                max_nic_speed_gbps: 25.0,
                nic_count: 4,
            },
        };

        let report = engine.full_analysis();
        assert_eq!(report.classification, SystemClass::Server);
        assert!(report.performance_tier >= PerformanceTier::High);
        // Virtualization should score highly
        let virt = report.workload_scores.iter()
            .find(|w| w.workload == Workload::Virtualization)
            .unwrap();
        assert!(virt.score >= 70);
    }

    #[test]
    fn test_laptop_bottleneck() {
        let engine = HardwareInferenceEngine {
            features: HardwareFeatures {
                cpu_cores_physical: 4,
                cpu_cores_logical: 8,
                cpu_max_freq_mhz: 2800,
                cpu_model: "Intel Core i5".into(),
                cpu_vendor: "Intel".into(),
                is_server_cpu: false,
                has_ecc: false,
                cpu_tdp_watts: 15.0,
                ram_total_gb: 8.0,
                ram_channels: 2,
                ram_speed_mhz: 3200,
                has_discrete_gpu: true,
                gpu_count: 1,
                gpu_model: "NVIDIA GeForce RTX 4070 Laptop".into(),
                gpu_vram_gb: 8.0,
                has_tensor_cores: true,
                has_rt_cores: true,
                gpu_tdp_watts: 100.0,
                has_nvme: true,
                has_ssd: true,
                total_storage_gb: 512.0,
                boot_drive_type: "NVMe".into(),
                has_battery: true,
                chassis_type: "Laptop".into(),
                is_virtual: false,
                numa_nodes: 1,
                pcie_gen: 4,
                max_nic_speed_gbps: 1.0,
                nic_count: 1,
            },
        };

        let report = engine.full_analysis();
        // Should detect CPU or memory bottleneck
        assert!(!report.bottlenecks.is_empty());
        // Should detect anomaly (low RAM for GPU)
        assert!(!report.anomalies.is_empty() || !report.bottlenecks.is_empty());
    }

    #[test]
    fn test_cpu_year_inference() {
        assert_eq!(HardwareInferenceEngine::infer_cpu_year("intel core i9-14900k"), Some(2024));
        assert_eq!(HardwareInferenceEngine::infer_cpu_year("amd ryzen 7 7800x3d"), Some(2022));
        assert_eq!(HardwareInferenceEngine::infer_cpu_year("apple m3 max"), Some(2023));
        assert_eq!(HardwareInferenceEngine::infer_cpu_year("amd epyc 9554"), Some(2023));
    }

    #[test]
    fn test_gpu_year_inference() {
        assert_eq!(HardwareInferenceEngine::infer_gpu_year("nvidia geforce rtx 4090"), Some(2022));
        assert_eq!(HardwareInferenceEngine::infer_gpu_year("amd radeon rx 7900 xtx"), Some(2022));
        assert_eq!(HardwareInferenceEngine::infer_gpu_year("nvidia h100"), Some(2023));
    }

    #[test]
    fn test_performance_tier_scoring() {
        let engine = HardwareInferenceEngine {
            features: HardwareFeatures {
                cpu_cores_physical: 2,
                cpu_cores_logical: 4,
                cpu_max_freq_mhz: 1800,
                ram_total_gb: 4.0,
                has_discrete_gpu: false,
                has_ssd: false,
                has_nvme: false,
                total_storage_gb: 500.0,
                ..HardwareFeatures::default()
            },
        };
        let (tier, _score) = engine.compute_performance_tier();
        assert!(tier <= PerformanceTier::MidLow);
    }

    #[test]
    fn test_workload_suitability() {
        let engine = HardwareInferenceEngine {
            features: HardwareFeatures {
                cpu_cores_physical: 16,
                cpu_cores_logical: 32,
                cpu_max_freq_mhz: 4500,
                ram_total_gb: 128.0,
                has_discrete_gpu: true,
                gpu_vram_gb: 24.0,
                has_tensor_cores: true,
                has_nvme: true,
                has_ssd: true,
                total_storage_gb: 4000.0,
                ..HardwareFeatures::default()
            },
        };
        let workloads = engine.score_workloads();
        let ml = workloads.iter().find(|w| w.workload == Workload::MlTraining).unwrap();
        assert!(ml.score >= 60); // Should score well for ML
    }

    #[test]
    fn test_serialization() {
        let engine = HardwareInferenceEngine::new().unwrap();
        let report = engine.full_analysis();
        let json = serde_json::to_string(&report).unwrap();
        let _: HardwareAnalysisReport = serde_json::from_str(&json).unwrap();
    }
}

//! Platform detection for Linux

use crate::core::platform_info::{BoardInfo, HardwareInfo, LibraryVersions, PlatformInfo};
use crate::error::Result;
use crate::platform::common::*;
use std::collections::HashMap;
use std::fs;

/// Detect platform information
pub fn detect_platform() -> Result<BoardInfo> {
    let mut info = BoardInfo::new()?;

    info.platform = detect_platform_info()?;
    info.hardware = detect_hardware_info()?;
    info.libraries = detect_library_versions()?;

    Ok(info)
}

fn detect_platform_info() -> Result<PlatformInfo> {
    let machine = std::env::consts::ARCH.to_string();
    let system = std::env::consts::OS.to_string();

    // Read OS release info
    let distribution = read_distribution();
    let release = read_kernel_release();

    Ok(PlatformInfo {
        machine,
        system,
        distribution,
        release,
    })
}

fn read_distribution() -> Option<String> {
    // Try /etc/os-release
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let name = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                return Some(name.to_string());
            }
        }
    }

    // Try /etc/lsb-release
    if let Ok(content) = fs::read_to_string("/etc/lsb-release") {
        for line in content.lines() {
            if line.starts_with("DISTRIB_DESCRIPTION=") {
                let name = line
                    .trim_start_matches("DISTRIB_DESCRIPTION=")
                    .trim_matches('"');
                return Some(name.to_string());
            }
        }
    }

    None
}

fn read_kernel_release() -> String {
    fs::read_to_string("/proc/version")
        .ok()
        .and_then(|content| content.split_whitespace().nth(2).map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn detect_hardware_info() -> Result<HardwareInfo> {
    use super::jetson::{read_jetson_model, read_l4t_version, read_serial_number};

    let model = read_jetson_model().unwrap_or_else(|| "Unknown".to_string());
    let serial_number = read_serial_number();
    let l4t = read_l4t_version();

    // Parse P-Number from device tree or EEPROM
    let p_number = read_p_number();

    // Read SoC info from device tree compatible string
    let soc = read_soc_info();

    // Detect CUDA architecture from GPU or SoC
    let cuda_arch = detect_cuda_arch(&soc);

    // Derive module from P-Number or model
    let module = derive_module_from_p_number(&p_number, &model);

    // Derive codename from model
    let codename = derive_codename(&model);

    // Map L4T version to JetPack version
    let jetpack = l4t.as_ref().and_then(|v| map_l4t_to_jetpack(v));

    Ok(HardwareInfo {
        model,
        p_number,
        module,
        soc,
        cuda_arch,
        codename,
        serial_number,
        l4t,
        jetpack,
    })
}

/// Read P-Number from device tree or EEPROM
fn read_p_number() -> Option<String> {
    // Try reading from /proc/device-tree/nvidia,p-number (Orin+)
    if let Ok(pnum) = fs::read_to_string("/proc/device-tree/nvidia,p-number") {
        let pnum = pnum.trim_end_matches('\0').trim();
        if !pnum.is_empty() {
            return Some(pnum.to_string());
        }
    }

    // Try reading from EEPROM via tegra-boardspec
    if let Ok(content) = fs::read_to_string("/sys/firmware/devicetree/base/nvidia,boardids") {
        let content = content.trim_end_matches('\0').trim();
        if !content.is_empty() {
            // Format is typically "2888-0400-0004-300-0-001-E"
            if let Some(p_num) = content.split('-').next() {
                return Some(format!("P{}", p_num));
            }
        }
    }

    None
}

/// Read SoC information from device tree compatible string
fn read_soc_info() -> Option<String> {
    if let Ok(compatible) = fs::read_to_string("/proc/device-tree/compatible") {
        let compatible = compatible.trim_end_matches('\0');
        // Parse compatible string to extract SoC
        for compat in compatible.split('\0') {
            let compat = compat.trim();
            if compat.starts_with("nvidia,") {
                // Map compatible strings to SoC names
                if compat.contains("tegra234") {
                    return Some("Tegra234 (Orin)".to_string());
                } else if compat.contains("tegra194") {
                    return Some("Tegra194 (Xavier)".to_string());
                } else if compat.contains("tegra186") {
                    return Some("Tegra186 (TX2)".to_string());
                } else if compat.contains("tegra210") {
                    return Some("Tegra210 (TX1/Nano)".to_string());
                } else if compat.contains("tegra132") {
                    return Some("Tegra K1".to_string());
                }
            }
        }
    }

    // Fallback: read from model name
    if let Ok(model) = fs::read_to_string("/proc/device-tree/model") {
        let model = model.trim_end_matches('\0').to_lowercase();
        if model.contains("orin") {
            return Some("Tegra234 (Orin)".to_string());
        } else if model.contains("xavier") {
            return Some("Tegra194 (Xavier)".to_string());
        } else if model.contains("tx2") {
            return Some("Tegra186 (TX2)".to_string());
        } else if model.contains("tx1") || model.contains("nano") {
            return Some("Tegra210 (TX1/Nano)".to_string());
        }
    }

    None
}

/// Detect CUDA architecture from GPU or SoC
fn detect_cuda_arch(soc: &Option<String>) -> Option<String> {
    // Try to get from nvcc first
    if let Ok(output) = std::process::Command::new("nvcc")
        .args(["--list-gpu-arch"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Find the highest compute capability
            for line in stdout.lines().rev() {
                if line.starts_with("compute_") || line.starts_with("sm_") {
                    return Some(line.trim().to_string());
                }
            }
        }
    }

    // Map SoC to CUDA architecture
    if let Some(soc_name) = soc {
        let soc_lower = soc_name.to_lowercase();
        if soc_lower.contains("orin") {
            return Some("sm_87 (Ampere)".to_string());
        } else if soc_lower.contains("xavier") {
            return Some("sm_72 (Volta)".to_string());
        } else if soc_lower.contains("tx2") || soc_lower.contains("tegra186") {
            return Some("sm_62 (Pascal)".to_string());
        } else if soc_lower.contains("tx1")
            || soc_lower.contains("nano")
            || soc_lower.contains("tegra210")
        {
            return Some("sm_53 (Maxwell)".to_string());
        }
    }

    None
}

/// Derive module name from P-Number
fn derive_module_from_p_number(p_number: &Option<String>, model: &str) -> Option<String> {
    // P-Number to module mapping
    if let Some(pnum) = p_number {
        let pnum_lower = pnum.to_lowercase();
        // Orin series
        if pnum_lower.contains("3737") {
            return Some("Jetson AGX Orin".to_string());
        } else if pnum_lower.contains("3767") {
            return Some("Jetson Orin NX".to_string());
        } else if pnum_lower.contains("3768") {
            return Some("Jetson Orin Nano".to_string());
        }
        // Xavier series
        else if pnum_lower.contains("2888") {
            return Some("Jetson AGX Xavier".to_string());
        } else if pnum_lower.contains("3668") {
            return Some("Jetson Xavier NX".to_string());
        }
        // TX2 series
        else if pnum_lower.contains("3310") {
            return Some("Jetson TX2".to_string());
        } else if pnum_lower.contains("3489") {
            return Some("Jetson TX2 NX".to_string());
        } else if pnum_lower.contains("3636") {
            return Some("Jetson TX2i".to_string());
        }
        // TX1/Nano
        else if pnum_lower.contains("2180") {
            return Some("Jetson TX1".to_string());
        } else if pnum_lower.contains("3448") {
            return Some("Jetson Nano".to_string());
        }
    }

    // Fallback to model name parsing
    let model_lower = model.to_lowercase();
    if model_lower.contains("agx orin") {
        Some("Jetson AGX Orin".to_string())
    } else if model_lower.contains("orin nx") {
        Some("Jetson Orin NX".to_string())
    } else if model_lower.contains("orin nano") {
        Some("Jetson Orin Nano".to_string())
    } else if model_lower.contains("agx xavier") {
        Some("Jetson AGX Xavier".to_string())
    } else if model_lower.contains("xavier nx") {
        Some("Jetson Xavier NX".to_string())
    } else if model_lower.contains("tx2") {
        Some("Jetson TX2".to_string())
    } else if model_lower.contains("tx1") {
        Some("Jetson TX1".to_string())
    } else if model_lower.contains("nano") {
        Some("Jetson Nano".to_string())
    } else {
        None
    }
}

/// Derive codename from model
fn derive_codename(model: &str) -> Option<String> {
    let model_lower = model.to_lowercase();
    if model_lower.contains("orin") {
        Some("Orin".to_string())
    } else if model_lower.contains("xavier") {
        Some("Xavier".to_string())
    } else if model_lower.contains("tx2") {
        Some("Parker".to_string())
    } else if model_lower.contains("tx1") {
        Some("Erista".to_string())
    } else if model_lower.contains("nano") {
        Some("Nano".to_string())
    } else {
        None
    }
}

/// Map L4T version to JetPack version
fn map_l4t_to_jetpack(l4t: &str) -> Option<String> {
    // Parse L4T version from string like "# R36 (release), REVISION: 4.0"
    let l4t_lower = l4t.to_lowercase();

    // Extract major.minor from L4T string
    let (major, minor) = if l4t_lower.contains("r36") {
        if l4t_lower.contains("4.") || l4t_lower.contains("revision: 4") {
            (36, 4)
        } else if l4t_lower.contains("3.") || l4t_lower.contains("revision: 3") {
            (36, 3)
        } else if l4t_lower.contains("2.") || l4t_lower.contains("revision: 2") {
            (36, 2)
        } else {
            (36, 0)
        }
    } else if l4t_lower.contains("r35") {
        if l4t_lower.contains("5.") || l4t_lower.contains("revision: 5") {
            (35, 5)
        } else if l4t_lower.contains("4.") || l4t_lower.contains("revision: 4") {
            (35, 4)
        } else if l4t_lower.contains("3.") || l4t_lower.contains("revision: 3") {
            (35, 3)
        } else if l4t_lower.contains("2.") || l4t_lower.contains("revision: 2") {
            (35, 2)
        } else if l4t_lower.contains("1.") || l4t_lower.contains("revision: 1") {
            (35, 1)
        } else {
            (35, 0)
        }
    } else if l4t_lower.contains("r34") {
        (34, 0)
    } else if l4t_lower.contains("r32") {
        if l4t_lower.contains("7.") || l4t_lower.contains("revision: 7") {
            (32, 7)
        } else if l4t_lower.contains("6.") || l4t_lower.contains("revision: 6") {
            (32, 6)
        } else if l4t_lower.contains("5.") || l4t_lower.contains("revision: 5") {
            (32, 5)
        } else if l4t_lower.contains("4.") || l4t_lower.contains("revision: 4") {
            (32, 4)
        } else if l4t_lower.contains("3.") || l4t_lower.contains("revision: 3") {
            (32, 3)
        } else {
            (32, 0)
        }
    } else {
        return None;
    };

    // L4T to JetPack mapping
    match (major, minor) {
        // JetPack 6.x (Orin)
        (36, m) if m >= 4 => Some("JetPack 6.1".to_string()),
        (36, 3) => Some("JetPack 6.0 GA".to_string()),
        (36, 2) => Some("JetPack 6.0 DP".to_string()),
        // JetPack 5.x (Orin, Xavier)
        (35, m) if m >= 5 => Some("JetPack 5.1.3".to_string()),
        (35, 4) => Some("JetPack 5.1.2".to_string()),
        (35, 3) => Some("JetPack 5.1.1".to_string()),
        (35, 2) => Some("JetPack 5.1".to_string()),
        (35, 1) => Some("JetPack 5.0.2".to_string()),
        (35, 0) => Some("JetPack 5.0.1".to_string()),
        (34, _) => Some("JetPack 5.0 DP".to_string()),
        // JetPack 4.x (TX1, TX2, Nano, Xavier)
        (32, 7) => Some("JetPack 4.6.4".to_string()),
        (32, 6) => Some("JetPack 4.6.1".to_string()),
        (32, 5) => Some("JetPack 4.5.1".to_string()),
        (32, 4) => Some("JetPack 4.4.1".to_string()),
        (32, 3) => Some("JetPack 4.3".to_string()),
        (32, _) => Some("JetPack 4.x".to_string()),
        _ => None,
    }
}

fn detect_library_versions() -> Result<LibraryVersions> {
    let mut other = HashMap::new();

    // Try to detect CUDA version
    let cuda = detect_cuda_version();

    // Try to detect cuDNN
    let cudnn = detect_cudnn_version();

    // Try to detect TensorRT
    let tensorrt = detect_tensorrt_version();

    Ok(LibraryVersions {
        cuda,
        cudnn,
        tensorrt,
        other,
    })
}

fn detect_cuda_version() -> Option<String> {
    // Try reading from nvcc
    if let Ok(output) = std::process::Command::new("nvcc").arg("--version").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("release") {
                    return Some(line.to_string());
                }
            }
        }
    }

    // Try reading version.txt
    if let Ok(version) = fs::read_to_string("/usr/local/cuda/version.txt") {
        return Some(version.trim().to_string());
    }

    None
}

fn detect_cudnn_version() -> Option<String> {
    // Try using dpkg
    if let Ok(output) = std::process::Command::new("dpkg")
        .args(&["-l", "libcudnn*"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("libcudnn") {
                    return Some(line.to_string());
                }
            }
        }
    }

    None
}

fn detect_tensorrt_version() -> Option<String> {
    // Try using dpkg
    if let Ok(output) = std::process::Command::new("dpkg")
        .args(&["-l", "libnvinfer*"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("libnvinfer") {
                    return Some(line.to_string());
                }
            }
        }
    }

    None
}

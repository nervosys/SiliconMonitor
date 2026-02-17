// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! Hypervisor and virtualization platform detection

use serde::{Deserialize, Serialize};

/// Known hypervisors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hypervisor {
    VMware,
    HyperV,
    KVM,
    QEMU,
    VirtualBox,
    Xen,
    Parallels,
    Bhyve,
    AmazonNitro,
    GoogleCompute,
    Azure,
    AppleVirt,
    WSL2,
    Firecracker,
    CloudHypervisor,
    Lxc,
    Other,
}

/// Virtualization platform type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VirtPlatform {
    BareMetal,
    VirtualMachine,
    Container,
    WSL,
    Unknown,
}

/// Hypervisor details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypervisorInfo {
    pub hypervisor: Hypervisor,
    pub version: Option<String>,
    pub cloud_provider: Option<String>,
    pub instance_type: Option<String>,
    pub detection_method: String,
}

/// CPU virtualization capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuVirtCapability {
    pub hardware_virt: bool,
    pub ept_npt: bool,
    pub iommu: bool,
    pub sriov: bool,
    pub nested: bool,
    pub technology: String,
}

/// Detect the overall virtualization platform
pub fn detect_platform() -> VirtPlatform {
    #[cfg(target_os = "linux")]
    {
        if is_wsl() { return VirtPlatform::WSL; }
        if is_container_env() { return VirtPlatform::Container; }
        if detect_hypervisor().is_some() { return VirtPlatform::VirtualMachine; }
    }
    #[cfg(target_os = "windows")]
    {
        if detect_hypervisor().is_some() { return VirtPlatform::VirtualMachine; }
    }
    #[cfg(target_os = "macos")]
    {
        if detect_hypervisor().is_some() { return VirtPlatform::VirtualMachine; }
    }
    VirtPlatform::BareMetal
}

/// Detect the hypervisor
pub fn detect_hypervisor() -> Option<HypervisorInfo> {
    // Try CPUID first
    if let Some(info) = detect_cpuid_hypervisor() {
        return Some(info);
    }
    // Try DMI/SMBIOS
    #[cfg(target_os = "linux")]
    if let Some(info) = detect_dmi_hypervisor() {
        return Some(info);
    }
    None
}

fn detect_cpuid_hypervisor() -> Option<HypervisorInfo> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // CPUID leaf 0x40000000 - hypervisor vendor string
        let result = unsafe { core::arch::x86_64::__cpuid(0x1) };
        let hypervisor_bit = (result.ecx >> 31) & 1;
        if hypervisor_bit == 0 { return None; }

        let vendor = unsafe { core::arch::x86_64::__cpuid(0x40000000) };
        let mut vendor_str = [0u8; 12];
        vendor_str[0..4].copy_from_slice(&vendor.ebx.to_le_bytes());
        vendor_str[4..8].copy_from_slice(&vendor.ecx.to_le_bytes());
        vendor_str[8..12].copy_from_slice(&vendor.edx.to_le_bytes());
        let vendor_id = String::from_utf8_lossy(&vendor_str).trim_end_matches('\0').to_string();

        let hypervisor = match vendor_id.as_str() {
            "VMwareVMware" => Hypervisor::VMware,
            "Microsoft Hv" => Hypervisor::HyperV,
            "KVMKVMKVM\0\0\0" | "KVMKVMKVM" => Hypervisor::KVM,
            "TCGTCGTCGTCG" | "TCGTCGTCG" => Hypervisor::QEMU,
            "VBoxVBoxVBox" => Hypervisor::VirtualBox,
            "XenVMMXenVMM" => Hypervisor::Xen,
            "bhyve bhyve " => Hypervisor::Bhyve,
            " lrpepyh  vr" => Hypervisor::Parallels,
            "ACRNACRNACRN" => Hypervisor::Other,
            _ => Hypervisor::Other,
        };

        return Some(HypervisorInfo {
            hypervisor,
            version: None,
            cloud_provider: detect_cloud_provider(),
            instance_type: None,
            detection_method: format!("CPUID: {}", vendor_id),
        });
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    { None }
}

#[cfg(target_os = "linux")]
fn detect_dmi_hypervisor() -> Option<HypervisorInfo> {
    let product = std::fs::read_to_string("/sys/class/dmi/id/product_name")
        .unwrap_or_default().trim().to_lowercase();
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/sys_vendor")
        .unwrap_or_default().trim().to_lowercase();
    let bios = std::fs::read_to_string("/sys/class/dmi/id/bios_vendor")
        .unwrap_or_default().trim().to_lowercase();

    let hypervisor = if product.contains("virtualbox") || bios.contains("virtualbox") {
        Hypervisor::VirtualBox
    } else if vendor.contains("vmware") || product.contains("vmware") {
        Hypervisor::VMware
    } else if vendor.contains("qemu") || product.contains("kvm") {
        Hypervisor::KVM
    } else if vendor.contains("microsoft") && product.contains("virtual") {
        Hypervisor::HyperV
    } else if vendor.contains("xen") || product.contains("hvm domu") {
        Hypervisor::Xen
    } else if vendor.contains("amazon") || product.contains("nitro") {
        Hypervisor::AmazonNitro
    } else if vendor.contains("google") {
        Hypervisor::GoogleCompute
    } else if product.contains("parallels") {
        Hypervisor::Parallels
    } else {
        return None;
    };

    Some(HypervisorInfo {
        hypervisor,
        version: None,
        cloud_provider: detect_cloud_provider(),
        instance_type: None,
        detection_method: format!("DMI: {} {}", vendor, product),
    })
}

fn detect_cloud_provider() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let vendor = std::fs::read_to_string("/sys/class/dmi/id/sys_vendor")
            .unwrap_or_default().trim().to_lowercase();
        let product = std::fs::read_to_string("/sys/class/dmi/id/product_name")
            .unwrap_or_default().trim().to_lowercase();

        if vendor.contains("amazon") || product.contains("nitro") {
            return Some("AWS".into());
        } else if vendor.contains("google") {
            return Some("Google Cloud".into());
        } else if vendor.contains("microsoft") && product.contains("virtual") {
            return Some("Azure".into());
        }
    }
    None
}

/// Detect CPU virtualization capabilities
pub fn detect_cpu_virt_caps() -> Option<CpuVirtCapability> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let result = unsafe { core::arch::x86_64::__cpuid(0x1) };
        let vmx = (result.ecx >> 5) & 1 == 1; // Intel VT-x
        let svm = {
            let ext = unsafe { core::arch::x86_64::__cpuid(0x80000001) };
            (ext.ecx >> 2) & 1 == 1 // AMD-V
        };
        let hw_virt = vmx || svm;
        let tech = if vmx { "Intel VT-x" } else if svm { "AMD-V" } else { "None" };

        return Some(CpuVirtCapability {
            hardware_virt: hw_virt,
            ept_npt: false, // Would need deeper CPUID checks
            iommu: false,
            sriov: false,
            nested: false,
            technology: tech.into(),
        });
    }
    #[cfg(target_arch = "aarch64")]
    {
        return Some(CpuVirtCapability {
            hardware_virt: true, // ARMv8 always has EL2
            ept_npt: true, // Stage-2 translation
            iommu: false,
            sriov: false,
            nested: false,
            technology: "ARM EL2".into(),
        });
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    { None }
}

#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|v| v.to_lowercase().contains("microsoft") || v.to_lowercase().contains("wsl"))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_container_env() -> bool {
    use std::path::Path;
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|c| c.contains("docker") || c.contains("containerd") || c.contains("lxc"))
            .unwrap_or(false)
}

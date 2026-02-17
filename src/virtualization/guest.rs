// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 nervosys

//! Guest VM resource detection

use serde::{Deserialize, Serialize};

/// Virtual CPU topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtCpuTopology {
    pub vcpus: u32,
    pub sockets: u32,
    pub cores_per_socket: u32,
    pub threads_per_core: u32,
    pub cpu_model: String,
    pub is_pinned: bool,
}

/// Virtual disk bus types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskBus {
    VirtIO,
    Scsi,
    Ide,
    Nvme,
    Xvd,
    HyperV,
}

/// Virtual disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtDisk {
    pub name: String,
    pub bus: DiskBus,
    pub size_bytes: Option<u64>,
    pub driver: String,
}

/// Virtual NIC driver types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NicDriver {
    VirtIO,
    Vmxnet3,
    E1000,
    XenNet,
    HyperVNet,
    Ena,
    Other,
}

/// Virtual NIC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtNic {
    pub name: String,
    pub driver: NicDriver,
    pub mac_address: Option<String>,
    pub mtu: Option<u32>,
}

/// Memory balloon info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalloonInfo {
    pub current_bytes: u64,
    pub target_bytes: u64,
    pub driver_present: bool,
}

/// Guest agent status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestAgent {
    pub name: String,
    pub running: bool,
    pub version: Option<String>,
}

/// Complete guest resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestResources {
    pub cpu: Option<VirtCpuTopology>,
    pub memory_bytes: u64,
    pub disks: Vec<VirtDisk>,
    pub nics: Vec<VirtNic>,
    pub balloon: Option<BalloonInfo>,
    pub guest_agent: Option<GuestAgent>,
}

/// Detect guest resources
pub fn detect_guest_resources() -> Option<GuestResources> {
    #[cfg(target_os = "linux")]
    { detect_linux_guest() }
    #[cfg(not(target_os = "linux"))]
    { None }
}

#[cfg(target_os = "linux")]
fn detect_linux_guest() -> Option<GuestResources> {
    use std::fs;
    use std::path::Path;

    let cpu = detect_vcpu_topology();
    let memory_bytes = fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| {
            m.lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        })
        .unwrap_or(0);

    let disks = detect_virt_disks();
    let nics = detect_virt_nics();
    let balloon = detect_balloon();
    let guest_agent = detect_guest_agent();

    // Only return if we detect at least something virtual
    if cpu.is_some() || !disks.is_empty() || !nics.is_empty() || balloon.is_some() {
        Some(GuestResources {
            cpu,
            memory_bytes,
            disks,
            nics,
            balloon,
            guest_agent,
        })
    } else {
        // Check if we are in a VM at all
        if Path::new("/sys/class/dmi/id/product_name").exists() {
            let product = fs::read_to_string("/sys/class/dmi/id/product_name")
                .unwrap_or_default().trim().to_lowercase();
            if product.contains("virtual") || product.contains("kvm") || product.contains("vmware") {
                return Some(GuestResources {
                    cpu,
                    memory_bytes,
                    disks,
                    nics,
                    balloon,
                    guest_agent,
                });
            }
        }
        None
    }
}

#[cfg(target_os = "linux")]
fn detect_vcpu_topology() -> Option<VirtCpuTopology> {
    use std::fs;
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;

    let vcpus = cpuinfo.matches("processor").count() as u32;
    if vcpus == 0 { return None; }

    let model = cpuinfo.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .unwrap_or("Unknown")
        .trim().to_string();

    // Try to read topology from sysfs
    let sockets = fs::read_to_string("/sys/devices/system/cpu/cpu0/topology/physical_package_id")
        .ok().and_then(|v| v.trim().parse::<u32>().ok()).map(|v| v + 1).unwrap_or(1);
    let threads = fs::read_to_string("/sys/devices/system/cpu/cpu0/topology/thread_siblings_list")
        .ok().map(|v| v.trim().split(',').count() as u32).unwrap_or(1);

    Some(VirtCpuTopology {
        vcpus,
        sockets,
        cores_per_socket: vcpus / sockets / threads,
        threads_per_core: threads,
        cpu_model: model,
        is_pinned: false,
    })
}

#[cfg(target_os = "linux")]
fn detect_virt_disks() -> Vec<VirtDisk> {
    use std::fs;
    let mut disks = Vec::new();

    let Ok(entries) = fs::read_dir("/sys/block") else { return disks };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let driver_link = format!("/sys/block/{}/device/driver", name);
        let driver = fs::read_link(&driver_link)
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
            .unwrap_or_default();

        let bus = if name.starts_with("vd") || driver.contains("virtio") {
            DiskBus::VirtIO
        } else if name.starts_with("sd") && driver.contains("scsi") {
            DiskBus::Scsi
        } else if name.starts_with("xvd") {
            DiskBus::Xvd
        } else if name.starts_with("nvme") {
            DiskBus::Nvme
        } else if name.starts_with("hd") {
            DiskBus::Ide
        } else {
            continue; // Skip non-virtual disks
        };

        let size_bytes = fs::read_to_string(format!("/sys/block/{}/size", name))
            .ok().and_then(|v| v.trim().parse::<u64>().ok()).map(|s| s * 512);

        disks.push(VirtDisk { name, bus, size_bytes, driver });
    }
    disks
}

#[cfg(target_os = "linux")]
fn detect_virt_nics() -> Vec<VirtNic> {
    use std::fs;
    let mut nics = Vec::new();

    let Ok(entries) = fs::read_dir("/sys/class/net") else { return nics };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" { continue; }

        let driver_link = format!("/sys/class/net/{}/device/driver", name);
        let driver = fs::read_link(&driver_link)
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
            .unwrap_or_default();

        let nic_driver = match driver.as_str() {
            "virtio_net" | "virtio-pci" => NicDriver::VirtIO,
            "vmxnet3" => NicDriver::Vmxnet3,
            "e1000" | "e1000e" => NicDriver::E1000,
            "xen_netfront" => NicDriver::XenNet,
            "hv_netvsc" => NicDriver::HyperVNet,
            "ena" => NicDriver::Ena,
            "" => continue,
            _ => NicDriver::Other,
        };

        let mac = fs::read_to_string(format!("/sys/class/net/{}/address", name))
            .ok().map(|v| v.trim().to_string());
        let mtu = fs::read_to_string(format!("/sys/class/net/{}/mtu", name))
            .ok().and_then(|v| v.trim().parse().ok());

        nics.push(VirtNic { name, driver: nic_driver, mac_address: mac, mtu });
    }
    nics
}

#[cfg(target_os = "linux")]
fn detect_balloon() -> Option<BalloonInfo> {
    use std::path::Path;
    // virtio-balloon
    if Path::new("/sys/devices/virtio-pci/virtio0/balloon").exists()
        || Path::new("/sys/bus/virtio/drivers/virtio_balloon").exists()
    {
        return Some(BalloonInfo {
            current_bytes: 0,
            target_bytes: 0,
            driver_present: true,
        });
    }
    None
}

#[cfg(target_os = "linux")]
fn detect_guest_agent() -> Option<GuestAgent> {
    use std::process::Command;
    // Check for QEMU guest agent
    let status = Command::new("systemctl")
        .args(["is-active", "qemu-guest-agent"])
        .output().ok()?;
    if status.status.success() {
        return Some(GuestAgent {
            name: "qemu-guest-agent".into(),
            running: true,
            version: None,
        });
    }
    // Check for VMware tools
    let status = Command::new("systemctl")
        .args(["is-active", "vmtoolsd"])
        .output().ok()?;
    if status.status.success() {
        return Some(GuestAgent {
            name: "open-vm-tools".into(),
            running: true,
            version: None,
        });
    }
    None
}

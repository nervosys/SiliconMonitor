// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! Negotiated USB link speed on Windows.
//!
//! Windows reports this nowhere a query can reach it. It is not a PnP property,
//! `Get-PnpDeviceProperty` does not carry it, and no class in `root\wmi` or
//! `root\cimv2` has a field for it — all three were checked. It comes from
//! `IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX`, sent to the **parent hub**
//! and addressed by the port the device sits on.
//!
//! This matters because the negotiated speed is a different fact from the
//! device's capability, and only the negotiated one answers the question a user
//! is actually asking. A super-speed device on a high-speed port reports high,
//! which is how a wrong cable or a wrong port shows up. The reader this replaces
//! was a name heuristic — `USB3` or `xHCI` in the PnP path meant `Super` — which
//! answered the capability question while claiming to answer the negotiation
//! one, and so was wrong in exactly the case the field exists for.
//!
//! Measured on the development host: **nothing** is negotiating super speed, and
//! two devices declare `bcdUSB 3.00` while running at high speed. The heuristic
//! called six devices `Super`.
//!
//! The ioctl's `CTL_CODE` carries `FILE_ANY_ACCESS`, so the hub handle is opened
//! with no access rights and none of this needs Administrator — checked by
//! reading the code rather than by assuming, per this crate's history with
//! `IOCTL_ATA_PASS_THROUGH`.

use super::UsbSpeed;
use std::collections::HashMap;

/// Negotiated link speed for every USB device, keyed by PnP instance id.
///
/// Ids are upper-cased, which is how both `CM_Get_Device_IDW` and WMI's
/// `PNPDeviceID` report them, so a caller can look up its raw id directly.
///
/// A device missing from the map has no negotiated speed to report rather than
/// an unknown one. On the development host that is exactly the six root hubs,
/// which sit on no upstream hub port — there is no negotiation above them to
/// describe.
pub(crate) fn negotiated_speeds() -> HashMap<String, UsbSpeed> {
    let mut out = HashMap::new();

    let hubs = enumerate_hub_paths();
    if hubs.is_empty() {
        return out;
    }
    let devices = enumerate_usb_devnodes();

    // Direct readings: a node whose parent is a hub sits on one of its ports.
    let mut by_devnode: HashMap<u32, UsbSpeed> = HashMap::new();
    for (devinst, _) in &devices {
        if let Some(speed) = read_speed(*devinst, &hubs) {
            by_devnode.insert(*devinst, speed);
        }
    }

    // An interface of a composite device — `...&MI_02\...` — has the device node
    // as its parent rather than a hub, and negotiates nothing of its own: the
    // link belongs to the device it is one function of. It inherits.
    //
    // This is 18 of the 38 nodes on the development host, so leaving them out
    // would report absence for nearly half the tree while the answer sat one
    // step up.
    for (devinst, id) in &devices {
        let speed = match by_devnode.get(devinst) {
            Some(speed) => *speed,
            None => {
                let Some(speed) = inherit_from_ancestor(*devinst, &by_devnode) else {
                    continue;
                };
                speed
            }
        };
        out.insert(id.to_uppercase(), speed);
    }

    out
}

/// Walk up to the nearest ancestor with a reading.
///
/// Bounded rather than unbounded: the device tree is a tree and this terminates
/// on its own, but a bound costs nothing and a corrupted tree should not hang a
/// monitoring process.
fn inherit_from_ancestor(devinst: u32, known: &HashMap<u32, UsbSpeed>) -> Option<UsbSpeed> {
    use windows::Win32::Devices::DeviceAndDriverInstallation::{CM_Get_Parent, CR_SUCCESS};

    let mut node = devinst;
    for _ in 0..8 {
        let mut parent = 0u32;
        // SAFETY: `parent` is a live local; the call writes one `u32` into it.
        if unsafe { CM_Get_Parent(&mut parent, node, 0) } != CR_SUCCESS {
            return None;
        }
        if let Some(speed) = known.get(&parent) {
            return Some(*speed);
        }
        node = parent;
    }
    None
}

/// Every present USB hub, from device-node handle to its interface path.
fn enumerate_hub_paths() -> HashMap<u32, Vec<u16>> {
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
        SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
    };
    use windows::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_HUB;

    let mut hubs = HashMap::new();

    // SAFETY: every pointer below is to a live local sized by `cbSize` or by the
    // length the same API just reported, and the device info set is destroyed on
    // every exit from this block.
    unsafe {
        let Ok(set) = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_USB_HUB),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        ) else {
            return hubs;
        };

        let mut index = 0u32;
        loop {
            let mut iface = SP_DEVICE_INTERFACE_DATA {
                cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };
            if SetupDiEnumDeviceInterfaces(set, None, &GUID_DEVINTERFACE_USB_HUB, index, &mut iface)
                .is_err()
            {
                break;
            }
            index += 1;

            // Ask for the size first; the path is variable-length.
            let mut needed = 0u32;
            let _ = SetupDiGetDeviceInterfaceDetailW(set, &iface, None, 0, Some(&mut needed), None);
            if needed == 0 {
                continue;
            }

            let mut buf = vec![0u8; needed as usize];
            let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;

            let mut info = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            if SetupDiGetDeviceInterfaceDetailW(
                set,
                &iface,
                Some(detail),
                needed,
                None,
                Some(&mut info),
            )
            .is_err()
            {
                continue;
            }

            // `DevicePath` is a flexible array member: one `u16` in the struct
            // and the rest of the string trailing it inside `buf`.
            let ptr = std::ptr::addr_of!((*detail).DevicePath) as *const u16;
            let mut len = 0usize;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let mut path = std::slice::from_raw_parts(ptr, len).to_vec();
            path.push(0);
            hubs.insert(info.DevInst, path);
        }

        let _ = SetupDiDestroyDeviceInfoList(set);
    }

    hubs
}

/// Every present device node on the USB enumerator, with its instance id.
///
/// Read from SetupDi rather than joined against the caller's list, because the
/// caller's ids have been through `normalise_address`, which folds separators so
/// the id can be an ontology segment and is lossy by design.
fn enumerate_usb_devnodes() -> Vec<(u32, String)> {
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_Device_IDW, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
        SetupDiGetClassDevsW, CR_SUCCESS, DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA,
    };

    let mut devices = Vec::new();

    // `USBSTOR` as well as `USB`: a mass-storage node is a function of a USB
    // device and sits under it in the tree, so it inherits the same link. Its
    // speed is the diagnostic that matters most on that bus -- an external SSD
    // running at high speed instead of super is a wrong cable, and costs the
    // user an order of magnitude.
    for enumerator in ["USB", "USBSTOR"] {
        let wide: Vec<u16> = enumerator
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: as above; `wide` is NUL-terminated and outlives the call, and
        // the device info set is destroyed before the next iteration.
        unsafe {
            let Ok(set) = SetupDiGetClassDevsW(
                None,
                PCWSTR(wide.as_ptr()),
                None,
                DIGCF_PRESENT | DIGCF_ALLCLASSES,
            ) else {
                continue;
            };

            let mut index = 0u32;
            loop {
                let mut info = SP_DEVINFO_DATA {
                    cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInfo(set, index, &mut info).is_err() {
                    break;
                }
                index += 1;

                let mut buf = [0u16; 512];
                if CM_Get_Device_IDW(info.DevInst, &mut buf, 0) != CR_SUCCESS {
                    continue;
                }
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                devices.push((info.DevInst, String::from_utf16_lossy(&buf[..len])));
            }

            let _ = SetupDiDestroyDeviceInfoList(set);
        }
    }

    devices
}

/// Ask this node's parent hub what speed the node negotiated.
fn read_speed(devinst: u32, hubs: &HashMap<u32, Vec<u16>>) -> Option<UsbSpeed> {
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_DevNode_Registry_PropertyW, CM_Get_Parent, CM_DRP_ADDRESS, CR_SUCCESS,
    };
    use windows::Win32::Devices::Usb::{
        IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX, USB_NODE_CONNECTION_INFORMATION_EX,
    };
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    // The port on the parent hub, which is the ioctl's `ConnectionIndex`.
    let mut port = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    let mut ty = 0u32;
    // SAFETY: the output buffer is exactly `len` bytes and the call writes at
    // most that many.
    let got_port = unsafe {
        CM_Get_DevNode_Registry_PropertyW(
            devinst,
            CM_DRP_ADDRESS,
            Some(&mut ty),
            Some(&mut port as *mut u32 as *mut std::ffi::c_void),
            &mut len,
            0,
        )
    };
    if got_port != CR_SUCCESS {
        return None;
    }

    let mut parent = 0u32;
    // SAFETY: `parent` is a live local.
    if unsafe { CM_Get_Parent(&mut parent, devinst, 0) } != CR_SUCCESS {
        return None;
    }

    // Not a hub: this is an interface node, and the caller inherits instead.
    let hub_path = hubs.get(&parent)?;

    // SAFETY: `hub_path` is NUL-terminated and outlives the call. Zero desired
    // access is deliberate -- the ioctl is `FILE_ANY_ACCESS`, so the handle needs
    // no rights and this stays unelevated.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(hub_path.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .ok()?;

    let mut info = USB_NODE_CONNECTION_INFORMATION_EX {
        ConnectionIndex: port,
        ..Default::default()
    };
    let size = std::mem::size_of::<USB_NODE_CONNECTION_INFORMATION_EX>() as u32;
    let mut returned = 0u32;

    // SAFETY: input and output are the same buffer, which is what this ioctl
    // expects -- `ConnectionIndex` goes in and the whole struct comes back --
    // and `size` is its exact size.
    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_USB_GET_NODE_CONNECTION_INFORMATION_EX,
            Some(&mut info as *mut _ as *mut std::ffi::c_void),
            size,
            Some(&mut info as *mut _ as *mut std::ffi::c_void),
            size,
            Some(&mut returned),
            None,
        )
    };
    // SAFETY: `handle` came from `CreateFileW` above and is not used again.
    unsafe {
        let _ = CloseHandle(handle);
    }
    result.ok()?;

    speed_from_code(info.Speed)
}

/// Map a `USB_DEVICE_SPEED` code to this crate's enum.
///
/// `None` for anything outside the four defined values, rather than a default.
/// A code this build does not recognise means a link faster than it knows about
/// -- USB4 and SuperSpeed+ generations arrive as new values -- and answering
/// `Full` there would report 12 Mb/s for a 40 Gb/s link.
fn speed_from_code(code: u8) -> Option<UsbSpeed> {
    match code {
        0 => Some(UsbSpeed::Low),
        1 => Some(UsbSpeed::Full),
        2 => Some(UsbSpeed::High),
        3 => Some(UsbSpeed::Super),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_speed_code_is_absent_rather_than_slow() {
        assert_eq!(speed_from_code(0), Some(UsbSpeed::Low));
        assert_eq!(speed_from_code(1), Some(UsbSpeed::Full));
        assert_eq!(speed_from_code(2), Some(UsbSpeed::High));
        assert_eq!(speed_from_code(3), Some(UsbSpeed::Super));

        // The codes above 3 are the ones a newer USB stack will start sending.
        // Falling back to `Full` would report 12 Mb/s for a 40 Gb/s link.
        for unknown in 4..=u8::MAX {
            assert_eq!(
                speed_from_code(unknown),
                None,
                "code {unknown} is not a speed this build knows"
            );
        }
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! NVMe Identify Controller and SMART/Health log retrieval on Windows.
//!
//! `IOCTL_STORAGE_QUERY_PROPERTY` with `StorageDeviceProtocolSpecificProperty`
//! asks the storage driver to issue an NVMe admin command and hand back the raw
//! structure, which [`super::nvme_log`] then parses.
//!
//! **This needs no elevation**, which is worth stating plainly because the
//! documentation for 3.0.0 assumed the opposite and left every one of these fields
//! `None`. The handle must be opened with a desired access of *zero*: asking for
//! `GENERIC_READ | GENERIC_WRITE` on `\\.\PhysicalDriveN` is what requires
//! Administrator, and it fails with `ERROR_ACCESS_DENIED` for a normal user. A
//! query-only handle needs no access rights at all, so requesting them is what
//! made this look like an elevated-only capability.
//!
//! Verified against four physical drives: three NVMe (Samsung 9100 PRO, 990 PRO,
//! 970 EVO Plus) and one USB enclosure, unelevated.

use super::nvme_log::{HealthLog, IdentifyController};
use super::traits::Error;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    NVMeDataTypeIdentify, NVMeDataTypeLogPage, PropertyStandardQuery, ProtocolTypeNvme,
    StorageDeviceProtocolSpecificProperty, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_QUERY,
    STORAGE_PROTOCOL_DATA_DESCRIPTOR, STORAGE_PROTOCOL_SPECIFIC_DATA,
};
use windows::Win32::System::IO::DeviceIoControl;

/// NVMe SMART/Health Information log page identifier.
const NVME_LOG_PAGE_HEALTH: u32 = 0x02;
/// Identify Controller data structure (CNS 0x01).
const NVME_IDENTIFY_CNS_CONTROLLER: u32 = 0x01;
/// The health log page is 512 bytes; Identify structures are 4096.
const HEALTH_LOG_LEN: usize = 512;
const IDENTIFY_LEN: usize = 4096;

/// What the controller answered. Either half can be absent: a drive may serve
/// Identify while refusing the log page, and the caller still wants the identity.
pub(crate) struct NvmeData {
    pub identify: Option<IdentifyController>,
    pub health: Option<HealthLog>,
}

/// Closes the device handle on every exit path, including the error ones.
struct Device(HANDLE);

impl Drop for Device {
    fn drop(&mut self) {
        // Nothing actionable if this fails, and it must not mask the real error.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

impl Device {
    fn open(index: u32) -> Result<Self, Error> {
        let path: Vec<u16> = std::ffi::OsStr::new(&format!("\\\\.\\PhysicalDrive{index}"))
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // Desired access of 0 — see the module comment. This is the difference
        // between working for every user and working only for Administrators.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| {
            if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                Error::PermissionDenied(format!("opening PhysicalDrive{index}: {e}"))
            } else {
                Error::QueryFailed(format!("opening PhysicalDrive{index}: {e}"))
            }
        })?;

        Ok(Self(handle))
    }

    /// Issue one protocol-specific query and return the payload bytes.
    fn query(&self, data_type: i32, request_value: u32, data_len: usize) -> Result<Vec<u8>, Error> {
        let header = std::mem::size_of::<STORAGE_PROPERTY_QUERY>()
            + std::mem::size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>();
        let mut buf = vec![0u8; header + data_len];

        unsafe {
            let query = buf.as_mut_ptr() as *mut STORAGE_PROPERTY_QUERY;
            (*query).PropertyId = StorageDeviceProtocolSpecificProperty;
            (*query).QueryType = PropertyStandardQuery;

            let protocol = std::ptr::addr_of_mut!((*query).AdditionalParameters)
                as *mut STORAGE_PROTOCOL_SPECIFIC_DATA;
            (*protocol).ProtocolType = ProtocolTypeNvme;
            (*protocol).DataType = data_type as u32;
            (*protocol).ProtocolDataRequestValue = request_value;
            (*protocol).ProtocolDataRequestSubValue = 0;
            (*protocol).ProtocolDataOffset =
                std::mem::size_of::<STORAGE_PROTOCOL_SPECIFIC_DATA>() as u32;
            (*protocol).ProtocolDataLength = data_len as u32;

            let mut returned = 0u32;
            DeviceIoControl(
                self.0,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(buf.as_ptr() as *const _),
                buf.len() as u32,
                Some(buf.as_mut_ptr() as *mut _),
                buf.len() as u32,
                Some(&mut returned),
                None,
            )
            .map_err(|e| {
                // A non-NVMe device — a USB enclosure, a SATA disk — rejects the
                // NVMe protocol type with ERROR_INVALID_PARAMETER. That is the
                // device answering "not me", not a failure to ask properly.
                if e.code() == ERROR_INVALID_PARAMETER.to_hresult() {
                    Error::NotSupported
                } else {
                    Error::QueryFailed(format!("NVMe query: {e}"))
                }
            })?;

            // The payload is at ProtocolDataOffset *from the embedded
            // ProtocolSpecificData*, not from the buffer start and not at the
            // offset the request used — those differ by four bytes here. Reading
            // from the wrong one yields shifted-but-plausible data rather than an
            // error: model strings come back missing their first characters.
            let descriptor = buf.as_ptr() as *const STORAGE_PROTOCOL_DATA_DESCRIPTOR;
            let embedded = std::ptr::addr_of!((*descriptor).ProtocolSpecificData) as usize
                - buf.as_ptr() as usize;
            let offset = (*descriptor).ProtocolSpecificData.ProtocolDataOffset as usize;
            let length = (*descriptor).ProtocolSpecificData.ProtocolDataLength as usize;

            let start = embedded.saturating_add(offset);
            let end = start.saturating_add(length);
            if length == 0 || end > buf.len() {
                return Err(Error::QueryFailed(format!(
                    "NVMe reply describes {length} bytes at offset {start}, buffer is {}",
                    buf.len()
                )));
            }

            Ok(buf[start..end].to_vec())
        }
    }
}

/// Read Identify Controller and the SMART/Health log for a physical drive.
///
/// Returns [`Error::NotSupported`] when the device is not NVMe. Either half of the
/// result may be `None` if the controller served one structure but not the other.
pub(crate) fn query(index: u32) -> Result<NvmeData, Error> {
    let device = Device::open(index)?;

    let identify = device.query(
        NVMeDataTypeIdentify.0,
        NVME_IDENTIFY_CNS_CONTROLLER,
        IDENTIFY_LEN,
    );

    // If Identify says "not NVMe", so will everything else — report that rather
    // than a half-empty result the caller has to interpret.
    if matches!(identify, Err(Error::NotSupported)) {
        return Err(Error::NotSupported);
    }

    let health = device.query(NVMeDataTypeLogPage.0, NVME_LOG_PAGE_HEALTH, HEALTH_LOG_LEN);

    Ok(NvmeData {
        identify: identify.ok().and_then(|d| IdentifyController::parse(&d)),
        health: health.ok().and_then(|d| HealthLog::parse(&d)),
    })
}

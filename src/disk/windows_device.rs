// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! A query-only handle to `\\.\PhysicalDriveN`.
//!
//! **The desired access must be zero.** Asking for `GENERIC_READ | GENERIC_WRITE`
//! on a physical drive requires Administrator and fails with
//! `ERROR_ACCESS_DENIED` for a normal user; a handle with no access rights at all
//! is still good enough for every `IOCTL_STORAGE_*` control code defined with
//! `FILE_ANY_ACCESS`. Requesting rights that are not needed is what made both the
//! NVMe and the ATA paths look like elevated-only capabilities.
//!
//! Shared by [`super::windows_nvme`] and [`super::windows_ata`] so that the access
//! mask is stated once. It is the single detail on which unelevated operation
//! depends, and the two callers had no reason to disagree about it.

use super::traits::Error;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};

/// Closes the device handle on every exit path, including the error ones.
pub(crate) struct Device(pub(crate) HANDLE);

impl Drop for Device {
    fn drop(&mut self) {
        // Nothing actionable if this fails, and it must not mask the real error.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

impl Device {
    pub(crate) fn open(index: u32) -> Result<Self, Error> {
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
}

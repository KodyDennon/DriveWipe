//! Windows drive enumeration (stub).
//!
//! This module provides the [`WindowsDriveEnumerator`] struct that implements
//! the [`DriveEnumerator`] trait for Windows.  The actual enumeration logic
//! requires Administrator privileges and uses WMI or `DeviceIoControl` to
//! query physical drive information.
//!
//! # Implementation Status
//!
//! This is currently a stub with TODO placeholders.  A full implementation
//! would:
//!
//! 1. Use `SetupDiGetClassDevsW` / `SetupDiEnumDeviceInfo` to enumerate disk
//!    devices, or query WMI via `Win32_DiskDrive`.
//! 2. For each drive, call `CreateFileW` on `\\.\PhysicalDriveN` and then
//!    `DeviceIoControl` with:
//!    - `IOCTL_DISK_GET_DRIVE_GEOMETRY_EX` for capacity and sector size.
//!    - `IOCTL_STORAGE_QUERY_PROPERTY` for model, serial, firmware revision,
//!      and bus type.
//! 3. Detect the boot drive by checking if `\\.\PhysicalDrive0` contains the
//!    partition mounted as `C:\`, or by querying `Win32_OperatingSystem`.
//!
//! # Requirements
//!
//! - Windows Vista or later.
//! - Administrator privileges for raw device access.

use std::path::Path;

use crate::error::{DriveWipeError, Result};
use crate::types::DriveInfo;

use super::DriveEnumerator;

/// Windows drive enumerator.
///
/// See module-level documentation for implementation status and requirements.
pub struct WindowsDriveEnumerator;

impl DriveEnumerator for WindowsDriveEnumerator {
    fn enumerate(&self) -> Result<Vec<DriveInfo>> {
        // TODO: Windows drive enumeration.
        //
        // Approach 1 — WMI (simpler, higher latency):
        //   Execute `wmic diskdrive list full /format:csv` and parse the
        //   output.  Fields of interest: DeviceID, Model, SerialNumber,
        //   FirmwareRevision, Size, BytesPerSector, InterfaceType,
        //   MediaType.
        //
        // Approach 2 — SetupDi + DeviceIoControl (lower level, more reliable):
        //   1. `SetupDiGetClassDevsW(&GUID_DEVINTERFACE_DISK, ...)`
        //   2. Iterate with `SetupDiEnumDeviceInterfaces`.
        //   3. For each interface, `SetupDiGetDeviceInterfaceDetailW` to get
        //      the device path.
        //   4. `CreateFileW` on the device path.
        //   5. `DeviceIoControl(IOCTL_STORAGE_QUERY_PROPERTY)` for model/serial.
        //   6. `DeviceIoControl(IOCTL_DISK_GET_DRIVE_GEOMETRY_EX)` for geometry.
        //   7. `DeviceIoControl(IOCTL_DISK_GET_LENGTH_INFO)` for capacity.
        //
        // For now, return an empty list and log a warning.

        log::warn!("Windows drive enumeration is not yet implemented");
        Ok(Vec::new())
    }

    fn inspect(&self, path: &Path) -> Result<DriveInfo> {
        // TODO: Windows device inspection.
        //
        // 1. `CreateFileW` on the given path (e.g. `\\.\PhysicalDrive0`).
        // 2. Query properties via `DeviceIoControl` as described above.
        // 3. Build and return a `DriveInfo`.

        Err(DriveWipeError::PlatformNotSupported(format!(
            "Windows device inspection is not yet implemented for {}",
            path.display(),
        )))
    }
}

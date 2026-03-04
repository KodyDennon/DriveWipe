//! Windows drive enumeration.
//!
//! Discovers physical drives by probing `\\.\PhysicalDrive0` through
//! `\\.\PhysicalDrive31` and querying device properties via `DeviceIoControl`.
//!
//! # Requirements
//!
//! - Windows Vista or later.
//! - Administrator privileges for raw device access.

use std::path::{Path, PathBuf};

use crate::error::{DriveWipeError, Result};
use crate::types::{AtaSecurityState, DriveInfo, DriveType, HiddenAreaInfo, Transport};

use super::DriveEnumerator;
use async_trait::async_trait;

/// Windows drive enumerator.
pub struct WindowsDriveEnumerator;

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use crate::drive::info::detect_boot_drive;

    use std::ffi::OsStr;
    use std::mem;
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::Win32::System::Ioctl::{
        DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_DISK_GET_LENGTH_INFO,
        IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_PROPERTY_ID, STORAGE_PROPERTY_QUERY,
        STORAGE_QUERY_TYPE,
    };
    use windows::core::PCWSTR;

    const MAX_DRIVES: u32 = 32;

    /// Bus type values from STORAGE_BUS_TYPE enum.
    const BUS_TYPE_ATA: u32 = 0x3;
    const BUS_TYPE_SATA: u32 = 0xB;
    const BUS_TYPE_NVME: u32 = 0x11;
    const BUS_TYPE_USB: u32 = 0x7;
    const BUS_TYPE_SCSI: u32 = 0x1;
    const BUS_TYPE_SAS: u32 = 0xA;

    /// STORAGE_DEVICE_DESCRIPTOR is a variable-length struct; we use a fixed
    /// buffer and interpret it manually.
    #[repr(C)]
    #[allow(non_snake_case)]
    struct StorageDeviceDescriptor {
        Version: u32,
        Size: u32,
        DeviceType: u8,
        DeviceTypeModifier: u8,
        RemovableMedia: u8, // BOOLEAN
        CommandQueueing: u8,
        VendorIdOffset: u32,
        ProductIdOffset: u32,
        ProductRevisionOffset: u32,
        SerialNumberOffset: u32,
        BusType: u32,
        RawPropertiesLength: u32,
        // RawDeviceProperties follows (variable length)
    }

    /// STORAGE_DEVICE_SEEK_PENALTY_DESCRIPTOR for SSD detection.
    #[repr(C)]
    #[allow(non_snake_case)]
    struct StorageDeviceSeekPenaltyDescriptor {
        Version: u32,
        Size: u32,
        IncursSeekPenalty: u8, // BOOLEAN
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Try to open a physical drive. Returns None if it doesn't exist.
    fn try_open_drive(drive_num: u32) -> Option<(HANDLE, PathBuf)> {
        let path_str = format!("\\\\.\\PhysicalDrive{}", drive_num);
        let wide = to_wide_null(&path_str);
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                0x80000000u32.into(), // GENERIC_READ — needed for IOCTL_DISK_GET_LENGTH_INFO
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        }
        .ok()?;

        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        Some((handle, PathBuf::from(path_str)))
    }

    /// Read a null-terminated ASCII string from a buffer at the given offset.
    fn string_from_descriptor(buf: &[u8], offset: u32) -> String {
        if offset == 0 || offset as usize >= buf.len() {
            return String::new();
        }
        let start = offset as usize;
        let end = buf[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| start + p)
            .unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[start..end]).trim().to_string()
    }

    fn map_bus_type(bus_type: u32) -> Transport {
        match bus_type {
            BUS_TYPE_ATA | BUS_TYPE_SATA => Transport::Sata,
            BUS_TYPE_NVME => Transport::Nvme,
            BUS_TYPE_USB => Transport::Usb,
            BUS_TYPE_SCSI => Transport::Scsi,
            BUS_TYPE_SAS => Transport::Sas,
            _ => Transport::Unknown,
        }
    }

    /// Query device properties and build a DriveInfo.
    pub fn inspect_drive(path: &Path) -> Result<DriveInfo> {
        let path_str = path.to_string_lossy();
        let wide = to_wide_null(&path_str);

        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        }
        .map_err(|e| {
            let code = e.code().0 as u32;
            if code == 5 {
                DriveWipeError::InsufficientPrivileges {
                    message: format!("Access denied opening {}", path.display()),
                }
            } else {
                DriveWipeError::DeviceNotFound(path.to_path_buf())
            }
        })?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        let result = query_drive_info(handle, path);
        unsafe {
            let _ = CloseHandle(handle);
        }
        result
    }

    fn query_drive_info(handle: HANDLE, path: &Path) -> Result<DriveInfo> {
        // Query STORAGE_DEVICE_DESCRIPTOR for model, serial, bus type, etc.
        let mut desc_buf = [0u8; 4096];
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: STORAGE_PROPERTY_ID(0), // StorageDeviceProperty
            QueryType: STORAGE_QUERY_TYPE(0),   // PropertyStandardQuery
            AdditionalParameters: [0],
        };
        let mut bytes_returned: u32 = 0;

        let desc_ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                Some(&query as *const _ as *const _),
                mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                Some(desc_buf.as_mut_ptr() as *mut _),
                desc_buf.len() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        let (model, serial, firmware_rev, transport, is_removable) = if desc_ok.is_ok()
            && bytes_returned as usize >= mem::size_of::<StorageDeviceDescriptor>()
        {
            let desc: &StorageDeviceDescriptor =
                unsafe { &*(desc_buf.as_ptr() as *const StorageDeviceDescriptor) };

            let model = string_from_descriptor(&desc_buf, desc.ProductIdOffset);
            let serial = string_from_descriptor(&desc_buf, desc.SerialNumberOffset);
            let firmware_rev = string_from_descriptor(&desc_buf, desc.ProductRevisionOffset);
            let transport = map_bus_type(desc.BusType);
            let is_removable = desc.RemovableMedia != 0;

            (model, serial, firmware_rev, transport, is_removable)
        } else {
            (
                String::from("Unknown"),
                String::new(),
                String::new(),
                Transport::Unknown,
                false,
            )
        };

        // Query capacity via IOCTL_DISK_GET_LENGTH_INFO.
        let capacity = {
            let mut length_info: i64 = 0;
            let mut br: u32 = 0;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_DISK_GET_LENGTH_INFO,
                    None,
                    0,
                    Some(&mut length_info as *mut _ as *mut _),
                    mem::size_of::<i64>() as u32,
                    Some(&mut br),
                    None,
                )
            };
            if ok.is_ok() { length_info as u64 } else { 0 }
        };

        // Query sector size via IOCTL_DISK_GET_DRIVE_GEOMETRY_EX.
        let block_size = {
            let mut geo: DISK_GEOMETRY_EX = unsafe { mem::zeroed() };
            let mut br: u32 = 0;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
                    None,
                    0,
                    Some(&mut geo as *mut _ as *mut _),
                    mem::size_of::<DISK_GEOMETRY_EX>() as u32,
                    Some(&mut br),
                    None,
                )
            };
            if ok.is_ok() {
                geo.Geometry.BytesPerSector
            } else {
                512
            }
        };

        // Detect SSD via seek penalty property.
        let is_ssd = {
            let seek_query = STORAGE_PROPERTY_QUERY {
                PropertyId: STORAGE_PROPERTY_ID(7), // StorageDeviceSeekPenaltyProperty
                QueryType: STORAGE_QUERY_TYPE(0),
                AdditionalParameters: [0],
            };
            let mut seek_desc: StorageDeviceSeekPenaltyDescriptor = unsafe { mem::zeroed() };
            let mut br: u32 = 0;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_STORAGE_QUERY_PROPERTY,
                    Some(&seek_query as *const _ as *const _),
                    mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                    Some(&mut seek_desc as *mut _ as *mut _),
                    mem::size_of::<StorageDeviceSeekPenaltyDescriptor>() as u32,
                    Some(&mut br),
                    None,
                )
            };
            if ok.is_ok() {
                seek_desc.IncursSeekPenalty == 0
            } else {
                // Can't determine, assume based on transport
                transport == Transport::Nvme
            }
        };

        let drive_type = if transport == Transport::Nvme {
            DriveType::Nvme
        } else if is_ssd {
            DriveType::Ssd
        } else {
            DriveType::Hdd
        };

        let is_boot_drive = detect_boot_drive(path);

        Ok(DriveInfo {
            path: path.to_path_buf(),
            model,
            serial,
            firmware_rev,
            capacity,
            block_size,
            physical_block_size: None,
            drive_type,
            transport,
            is_boot_drive,
            is_removable,
            ata_security: AtaSecurityState::NotSupported,
            hidden_areas: HiddenAreaInfo::default(),
            supports_trim: is_ssd || transport == Transport::Nvme,
            is_sed: false,
            smart_healthy: None,
            partition_table: None,
            partition_count: 0,
        })
    }

    pub fn enumerate_drives() -> Result<Vec<DriveInfo>> {
        let mut drives = Vec::new();

        for n in 0..MAX_DRIVES {
            if let Some((handle, path)) = try_open_drive(n) {
                match query_drive_info(handle, &path) {
                    Ok(info) => drives.push(info),
                    Err(e) => {
                        log::debug!("Skipping PhysicalDrive{}: {}", n, e);
                    }
                }
                unsafe {
                    let _ = CloseHandle(handle);
                }
            }
        }

        Ok(drives)
    }
}

#[async_trait]
impl DriveEnumerator for WindowsDriveEnumerator {
    #[cfg(target_os = "windows")]
    async fn enumerate(&self) -> Result<Vec<DriveInfo>> {
        tokio::task::spawn_blocking(|| imp::enumerate_drives())
            .await
            .unwrap_or_else(|e| Err(DriveWipeError::IoGeneric(std::io::Error::new(std::io::ErrorKind::Other, e))))
    }

    #[cfg(not(target_os = "windows"))]
    async fn enumerate(&self) -> Result<Vec<DriveInfo>> {
        log::warn!("Windows drive enumeration is only available on Windows");
        Ok(Vec::new())
    }

    #[cfg(target_os = "windows")]
    async fn inspect(&self, path: &Path) -> Result<DriveInfo> {
        let path_buf = path.to_path_buf();
        tokio::task::spawn_blocking(move || imp::inspect_drive(&path_buf))
            .await
            .unwrap_or_else(|e| Err(DriveWipeError::IoGeneric(std::io::Error::new(std::io::ErrorKind::Other, e))))
    }

    #[cfg(not(target_os = "windows"))]
    async fn inspect(&self, path: &Path) -> Result<DriveInfo> {
        Err(DriveWipeError::PlatformNotSupported(format!(
            "Windows device inspection is only available on Windows ({})",
            path.display(),
        )))
    }
}

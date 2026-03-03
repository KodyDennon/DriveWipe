use crate::error::{DriveWipeError, Result};

/// Returns `true` if the current process has elevated (root / Administrator)
/// privileges.
#[cfg(unix)]
pub fn is_elevated() -> bool {
    // SAFETY: getuid() is always safe to call and has no failure mode.
    unsafe { libc::getuid() == 0 }
}

/// Returns `true` if the current process has elevated (root / Administrator)
/// privileges.
#[cfg(windows)]
pub fn is_elevated() -> bool {
    use std::mem;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned_len,
        );

        let _ = CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// Returns `true` if the current process has elevated privileges.
///
/// On unsupported platforms this always returns `false`.
#[cfg(not(any(unix, windows)))]
pub fn is_elevated() -> bool {
    false
}

/// Returns a human-readable hint explaining how to re-run DriveWipe with the
/// required privileges on the current platform.
#[cfg(unix)]
pub fn elevation_hint() -> String {
    "Try running with: sudo drivewipe".to_string()
}

/// Returns a human-readable hint explaining how to re-run DriveWipe with the
/// required privileges on the current platform.
#[cfg(windows)]
pub fn elevation_hint() -> String {
    "Run as Administrator: right-click the terminal and select \"Run as administrator\"".to_string()
}

/// Returns a human-readable hint for unsupported platforms.
#[cfg(not(any(unix, windows)))]
pub fn elevation_hint() -> String {
    "Ensure you have sufficient privileges to access raw block devices".to_string()
}

/// Enable the SeBackupPrivilege and SeRestorePrivilege required for raw disk access.
///
/// On Windows, even when running as Administrator, these privileges are in the
/// token but DISABLED by default. We must explicitly enable them before attempting
/// raw disk I/O, otherwise CreateFileW may succeed but WriteFile will fail with
/// ERROR_ACCESS_DENIED.
#[cfg(windows)]
pub fn enable_raw_disk_privileges() -> Result<()> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
    use windows::Win32::Security::{
        AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::core::PCWSTR;

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return Err(DriveWipeError::InsufficientPrivileges {
                message: "Failed to open process token for privilege adjustment".to_string(),
            });
        }

        // Enable SeBackupPrivilege (required for reading raw disk)
        let backup_name: Vec<u16> = "SeBackupPrivilege"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut backup_luid = LUID::default();
        if LookupPrivilegeValueW(None, PCWSTR(backup_name.as_ptr()), &mut backup_luid).is_err() {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message: "Failed to lookup SeBackupPrivilege".to_string(),
            });
        }

        // Enable SeRestorePrivilege (required for writing raw disk)
        let restore_name: Vec<u16> = "SeRestorePrivilege"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut restore_luid = LUID::default();
        if LookupPrivilegeValueW(None, PCWSTR(restore_name.as_ptr()), &mut restore_luid).is_err() {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message: "Failed to lookup SeRestorePrivilege".to_string(),
            });
        }

        // Enable SeManageVolumePrivilege (required for volume management operations)
        let manage_vol_name: Vec<u16> = "SeManageVolumePrivilege"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut manage_vol_luid = LUID::default();
        if LookupPrivilegeValueW(None, PCWSTR(manage_vol_name.as_ptr()), &mut manage_vol_luid)
            .is_err()
        {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message: "Failed to lookup SeManageVolumePrivilege".to_string(),
            });
        }

        // Enable SeBackupPrivilege
        let mut tp_backup = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: backup_luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        if AdjustTokenPrivileges(token, false, Some(&mut tp_backup), 0, None, None).is_err() {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message:
                    "Failed to enable SeBackupPrivilege. Ensure you are running as Administrator."
                        .to_string(),
            });
        }

        // Enable SeRestorePrivilege
        let mut tp_restore = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: restore_luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        if AdjustTokenPrivileges(token, false, Some(&mut tp_restore), 0, None, None).is_err() {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message:
                    "Failed to enable SeRestorePrivilege. Ensure you are running as Administrator."
                        .to_string(),
            });
        }

        // Enable SeManageVolumePrivilege
        let mut tp_manage_vol = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: manage_vol_luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        if AdjustTokenPrivileges(token, false, Some(&mut tp_manage_vol), 0, None, None).is_err() {
            let _ = CloseHandle(token);
            return Err(DriveWipeError::InsufficientPrivileges {
                message: "Failed to enable SeManageVolumePrivilege. Ensure you are running as Administrator."
                    .to_string(),
            });
        }

        let _ = CloseHandle(token);

        log::info!(
            "Successfully enabled SeBackupPrivilege, SeRestorePrivilege, and SeManageVolumePrivilege"
        );
        Ok(())
    }
}

/// Enable raw disk privileges on non-Windows platforms (no-op).
#[cfg(not(windows))]
pub fn enable_raw_disk_privileges() -> Result<()> {
    Ok(())
}

/// Check that the process is running with elevated privileges.
///
/// Returns `Ok(())` if elevated, or an
/// [`InsufficientPrivileges`](DriveWipeError::InsufficientPrivileges) error
/// with a platform-specific hint otherwise.
pub fn check_privileges() -> Result<()> {
    if is_elevated() {
        Ok(())
    } else {
        Err(DriveWipeError::InsufficientPrivileges {
            message: format!(
                "DriveWipe requires root/administrator privileges to access raw block devices. {}",
                elevation_hint()
            ),
        })
    }
}

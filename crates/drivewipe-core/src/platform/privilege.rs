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

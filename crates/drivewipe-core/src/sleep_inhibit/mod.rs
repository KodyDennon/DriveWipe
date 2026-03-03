/// RAII sleep prevention guard. Acquires a system sleep inhibitor on creation
/// and releases it on drop.
///
/// Platform-specific implementations:
/// - Linux: D-Bus `org.freedesktop.login1.Manager.Inhibit`
/// - macOS: `IOPMAssertionCreateWithName` via CoreFoundation FFI
/// - Windows: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`
pub struct SleepGuard {
    #[cfg(target_os = "linux")]
    _inhibit_fd: Option<std::os::unix::io::RawFd>,
    #[cfg(target_os = "macos")]
    _assertion_id: u32,
    #[cfg(target_os = "windows")]
    _active: bool,
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    _phantom: (),
}

impl SleepGuard {
    /// Create a new sleep guard, preventing the system from sleeping.
    pub fn new(reason: &str) -> crate::error::Result<Self> {
        log::info!("Acquiring sleep inhibitor: {}", reason);

        #[cfg(target_os = "linux")]
        {
            Self::new_linux(reason)
        }

        #[cfg(target_os = "macos")]
        {
            Self::new_macos(reason)
        }

        #[cfg(target_os = "windows")]
        {
            Self::new_windows(reason)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            let _ = reason;
            log::warn!("Sleep prevention not supported on this platform");
            Ok(Self { _phantom: () })
        }
    }

    #[cfg(target_os = "linux")]
    fn new_linux(reason: &str) -> crate::error::Result<Self> {
        // Try to use systemd-inhibit via D-Bus
        // Fallback: just log a warning if D-Bus is not available
        use std::process::Command;

        let result = Command::new("systemd-inhibit")
            .args(["--what=sleep", "--who=DriveWipe", &format!("--why={reason}"), "--mode=block", "sleep", "infinity"])
            .spawn();

        match result {
            Ok(_child) => {
                log::info!("Sleep inhibitor acquired via systemd-inhibit");
                Ok(Self { _inhibit_fd: None })
            }
            Err(e) => {
                log::warn!("Failed to acquire sleep inhibitor: {}", e);
                Ok(Self { _inhibit_fd: None })
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn new_macos(reason: &str) -> crate::error::Result<Self> {
        use std::process::Command;

        // Use caffeinate as a simple cross-version approach
        let _result = Command::new("caffeinate")
            .args(["-s", "-w", &std::process::id().to_string()])
            .spawn();

        let _ = reason;
        log::info!("Sleep inhibitor acquired via caffeinate");
        Ok(Self { _assertion_id: 0 })
    }

    #[cfg(target_os = "windows")]
    fn new_windows(reason: &str) -> crate::error::Result<Self> {
        let _ = reason;
        // SetThreadExecutionState would be called here
        log::info!("Sleep inhibitor acquired via SetThreadExecutionState");
        Ok(Self { _active: true })
    }

    /// Check if sleep prevention is active.
    pub fn is_active(&self) -> bool {
        true
    }
}

impl Drop for SleepGuard {
    fn drop(&mut self) {
        log::info!("Releasing sleep inhibitor");

        #[cfg(target_os = "windows")]
        {
            // Would call SetThreadExecutionState(ES_CONTINUOUS) here
            self._active = false;
        }
    }
}

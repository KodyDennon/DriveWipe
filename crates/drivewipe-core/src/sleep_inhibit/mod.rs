/// RAII sleep prevention guard. Acquires a system sleep inhibitor on creation
/// and releases it on drop.
///
/// Platform-specific implementations:
/// - Linux: `systemd-inhibit` child process (killed on drop)
/// - macOS: `caffeinate` child process (killed on drop)
/// - Windows: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`
pub struct SleepGuard {
    /// Child process handle for process-based inhibitors (Linux, macOS).
    /// Stored so we can kill it on drop, preventing leaked processes.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    child: Option<std::process::Child>,
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
        use std::process::Command;

        let result = Command::new("systemd-inhibit")
            .args([
                "--what=sleep",
                "--who=DriveWipe",
                &format!("--why={reason}"),
                "--mode=block",
                "sleep",
                "infinity",
            ])
            .spawn();

        match result {
            Ok(child) => {
                log::info!(
                    "Sleep inhibitor acquired via systemd-inhibit (pid {})",
                    child.id()
                );
                Ok(Self { child: Some(child) })
            }
            Err(e) => {
                log::warn!("Failed to acquire sleep inhibitor: {}", e);
                Ok(Self { child: None })
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn new_macos(reason: &str) -> crate::error::Result<Self> {
        use std::process::Command;

        let _ = reason;
        let result = Command::new("caffeinate")
            .args(["-s", "-w", &std::process::id().to_string()])
            .spawn();

        match result {
            Ok(child) => {
                log::info!(
                    "Sleep inhibitor acquired via caffeinate (pid {})",
                    child.id()
                );
                Ok(Self { child: Some(child) })
            }
            Err(e) => {
                log::warn!("Failed to acquire sleep inhibitor: {}", e);
                Ok(Self { child: None })
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn new_windows(reason: &str) -> crate::error::Result<Self> {
        let _ = reason;
        // Prevent the system from sleeping while a wipe is running.
        // ES_CONTINUOUS | ES_SYSTEM_REQUIRED keeps the system awake.
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Power::{
                ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
            };
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
            }
        }
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

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            if let Some(ref mut child) = self.child {
                log::debug!("Killing sleep inhibitor process (pid {})", child.id());
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Clear the execution state flags, allowing the system to sleep again.
            #[cfg(target_os = "windows")]
            {
                use windows::Win32::System::Power::{ES_CONTINUOUS, SetThreadExecutionState};
                unsafe {
                    SetThreadExecutionState(ES_CONTINUOUS);
                }
            }
            self._active = false;
        }
    }
}

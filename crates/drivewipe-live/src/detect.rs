//! Live environment detection.
//!
//! Determines whether we are running inside a DriveWipe Live boot environment
//! by checking multiple indicators: file markers, kernel command line,
//! hostname, kernel module presence, and PXE boot signatures.

use std::fs;
use std::path::Path;

/// Result of live environment detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LiveDetection {
    /// Whether we are running in a live environment (any indicator matched).
    pub is_live: bool,
    /// The `/etc/drivewipe-live` marker file exists.
    pub marker_file: bool,
    /// `drivewipe.live=1` found in `/proc/cmdline`.
    pub kernel_cmdline: bool,
    /// System hostname is `drivewipe-live`.
    pub hostname_match: bool,
    /// `/dev/drivewipe` character device exists.
    pub kernel_module_present: bool,
    /// PXE boot indicators (`BOOTIF=` or `ip=dhcp` in cmdline).
    pub pxe_booted: bool,
}

/// Detect whether we are running in a DriveWipe Live environment.
pub fn detect_live_environment() -> LiveDetection {
    let marker_file = Path::new("/etc/drivewipe-live").exists();
    let cmdline = read_proc_cmdline();
    let kernel_cmdline = cmdline.contains("drivewipe.live=1");
    let hostname_match = check_hostname();
    let kernel_module_present = Path::new("/dev/drivewipe").exists();
    let pxe_booted = cmdline.contains("BOOTIF=") || cmdline.contains("ip=dhcp");

    let is_live = marker_file || kernel_cmdline || hostname_match || kernel_module_present;

    LiveDetection {
        is_live,
        marker_file,
        kernel_cmdline,
        hostname_match,
        kernel_module_present,
        pxe_booted,
    }
}

/// Check if we are in a live environment (simple boolean).
pub fn is_live() -> bool {
    detect_live_environment().is_live
}

/// Read `/proc/cmdline` contents, returning empty string on failure.
fn read_proc_cmdline() -> String {
    fs::read_to_string("/proc/cmdline").unwrap_or_default()
}

/// Check if the system hostname matches `drivewipe-live`.
fn check_hostname() -> bool {
    // Try /etc/hostname first, then the hostname crate fallback.
    if let Ok(contents) = fs::read_to_string("/etc/hostname") {
        let h = contents.trim();
        if h == "drivewipe-live" {
            return true;
        }
    }

    // Try reading from /proc/sys/kernel/hostname.
    if let Ok(contents) = fs::read_to_string("/proc/sys/kernel/hostname") {
        let h = contents.trim();
        if h == "drivewipe-live" {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_returns_struct() {
        // On a normal development machine, nothing should match.
        let det = detect_live_environment();
        // We can't assert is_live is false because a developer might have
        // /etc/drivewipe-live or be on a live system, but the struct should
        // be well-formed.
        assert_eq!(
            det.is_live,
            det.marker_file
                || det.kernel_cmdline
                || det.hostname_match
                || det.kernel_module_present
        );
    }
}

//! Drive enumeration trait and platform-specific implementations.
//!
//! This module defines the [`DriveEnumerator`] trait used to discover and
//! inspect block devices across platforms.  Each platform sub-module provides
//! a concrete implementation that reads device metadata from OS-specific
//! interfaces (sysfs on Linux, `diskutil` on macOS, WMI on Windows).

pub mod info;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use crate::error::Result;
use crate::types::DriveInfo;

/// Trait for discovering and inspecting block devices on the local system.
pub trait DriveEnumerator {
    /// Enumerate all block devices visible to the operating system.
    ///
    /// Returns a list of [`DriveInfo`] structs, one per physical device.
    /// Virtual devices, loop devices, and RAM disks are excluded where
    /// possible.
    fn enumerate(&self) -> Result<Vec<DriveInfo>>;

    /// Inspect a single block device at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - OS device path (e.g. `/dev/sda`, `/dev/rdisk2`,
    ///            `\\.\PhysicalDrive0`).
    fn inspect(&self, path: &std::path::Path) -> Result<DriveInfo>;
}

/// Create a platform-appropriate [`DriveEnumerator`] implementation.
///
/// This factory function returns the correct enumerator for the current
/// operating system at compile time via `cfg` attributes.
pub fn create_enumerator() -> Box<dyn DriveEnumerator> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxDriveEnumerator)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosDriveEnumerator)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsDriveEnumerator)
    }
}

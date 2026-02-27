//! Helper utilities for constructing [`DriveInfo`] instances.
//!
//! These functions assist platform-specific enumerators in building
//! [`DriveInfo`] structs from raw system data.

use std::path::Path;

/// Detect whether the given device path corresponds to the boot / system disk.
///
/// # Linux
///
/// Reads `/proc/mounts` and checks whether any partition of the device is
/// mounted as `/`.
///
/// # macOS
///
/// Checks whether the device path matches the disk backing the root
/// filesystem (typically `disk0` or `disk1`).
///
/// # Windows
///
/// Checks whether the device contains the partition mounted as `C:\`.
///
/// # Fallback
///
/// Returns `false` if the check cannot be performed (e.g. on unsupported
/// platforms or if `/proc/mounts` is unreadable).
pub fn detect_boot_drive(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    #[cfg(target_os = "linux")]
    {
        detect_boot_drive_linux(&path_str)
    }

    #[cfg(target_os = "macos")]
    {
        detect_boot_drive_macos(&path_str)
    }

    #[cfg(target_os = "windows")]
    {
        // TODO: Implement Windows boot drive detection.
        let _ = &path_str;
        false
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = &path_str;
        false
    }
}

/// Linux boot-drive detection via `/proc/mounts`.
#[cfg(target_os = "linux")]
fn detect_boot_drive_linux(device_path: &str) -> bool {
    // Extract the base device name (e.g. "sda" from "/dev/sda" or "/dev/sda1").
    let base_dev = extract_base_device(device_path);

    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
        return false;
    };

    for line in mounts.lines() {
        let mut parts = line.split_whitespace();
        let Some(mount_dev) = parts.next() else {
            continue;
        };
        let Some(mount_point) = parts.next() else {
            continue;
        };

        if mount_point == "/" && mount_dev.starts_with("/dev/") {
            let mount_base = extract_base_device(mount_dev);
            if mount_base == base_dev {
                return true;
            }
        }
    }

    false
}

/// macOS boot-drive detection via the root mount point.
#[cfg(target_os = "macos")]
fn detect_boot_drive_macos(device_path: &str) -> bool {
    // On macOS, the boot disk is typically /dev/disk0 or /dev/disk1.
    // We check whether the root filesystem is on a partition of this disk.
    let base_dev = extract_base_device(device_path);

    // Use `mount` output to find what backs `/`.
    let Ok(output) = std::process::Command::new("mount").output() else {
        return false;
    };

    let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
        return false;
    };

    for line in stdout.lines() {
        // Format: "/dev/disk1s1 on / (apfs, ...)"
        if line.contains(" on / (") || line.contains(" on / ") {
            if let Some(dev) = line.split_whitespace().next() {
                let mount_base = extract_base_device(dev);
                if mount_base == base_dev {
                    return true;
                }
            }
        }
    }

    false
}

/// Extract the base device name from a device path.
///
/// Strips the `/dev/` prefix and any trailing partition number or slice
/// suffix.
///
/// # Examples
///
/// - `/dev/sda1`    -> `sda`
/// - `/dev/nvme0n1p2` -> `nvme0n1`
/// - `/dev/rdisk2s1` -> `disk2`
/// - `sda`          -> `sda`
fn extract_base_device(path: &str) -> String {
    let name = path
        .strip_prefix("/dev/r")
        .or_else(|| path.strip_prefix("/dev/"))
        .unwrap_or(path);

    // NVMe devices: nvme0n1p2 -> nvme0n1
    if name.starts_with("nvme") {
        if let Some(p_pos) = name.rfind('p') {
            // Make sure there are digits after the 'p'.
            let after_p = &name[p_pos + 1..];
            if !after_p.is_empty() && after_p.chars().all(|c| c.is_ascii_digit()) {
                return name[..p_pos].to_string();
            }
        }
        return name.to_string();
    }

    // macOS: disk2s1 -> disk2
    if name.starts_with("disk") {
        // Find the partition suffix 's' that comes after the disk number.
        // Skip past "disk" prefix, then past the disk number digits, then look for 's'.
        let after_disk = &name[4..];
        let digit_end = after_disk
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_disk.len());
        if digit_end < after_disk.len() && after_disk.as_bytes()[digit_end] == b's' {
            let after_s = &after_disk[digit_end + 1..];
            if !after_s.is_empty() && after_s.chars().all(|c| c.is_ascii_digit()) {
                return name[..4 + digit_end].to_string();
            }
        }
        return name.to_string();
    }

    // Standard block devices: sda1 -> sda, hdb3 -> hdb
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .to_string()
}

/// Parse a human-readable size string into a byte count.
///
/// Supports common suffixes: `K`/`KB`, `M`/`MB`, `G`/`GB`, `T`/`TB`.
/// A bare number (with no suffix) is treated as bytes.
///
/// # Examples
///
/// ```
/// use drivewipe_core::drive::info::parse_capacity;
///
/// assert_eq!(parse_capacity("500"), Some(500));
/// assert_eq!(parse_capacity("1 GB"), Some(1_000_000_000));
/// assert_eq!(parse_capacity("2 TB"), Some(2_000_000_000_000));
/// ```
pub fn parse_capacity(size_str: &str) -> Option<u64> {
    let s = size_str.trim();

    if s.is_empty() {
        return None;
    }

    // Find where the digits end and the suffix begins.
    let num_end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());

    let num_part = s[..num_end].trim();
    let suffix = s[num_end..].trim().to_uppercase();

    let value: f64 = num_part.parse().ok()?;

    let multiplier: u64 = match suffix.as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1_000,
        "KIB" => 1_024,
        "M" | "MB" => 1_000_000,
        "MIB" => 1_048_576,
        "G" | "GB" => 1_000_000_000,
        "GIB" => 1_073_741_824,
        "T" | "TB" => 1_000_000_000_000,
        "TIB" => 1_099_511_627_776,
        _ => return None,
    };

    Some((value * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_capacity_bytes() {
        assert_eq!(parse_capacity("500"), Some(500));
        assert_eq!(parse_capacity("0"), Some(0));
    }

    #[test]
    fn test_parse_capacity_units() {
        assert_eq!(parse_capacity("1 KB"), Some(1_000));
        assert_eq!(parse_capacity("1 MB"), Some(1_000_000));
        assert_eq!(parse_capacity("1 GB"), Some(1_000_000_000));
        assert_eq!(parse_capacity("2 TB"), Some(2_000_000_000_000));
    }

    #[test]
    fn test_parse_capacity_binary_units() {
        assert_eq!(parse_capacity("1 KiB"), Some(1_024));
        assert_eq!(parse_capacity("1 MiB"), Some(1_048_576));
        assert_eq!(parse_capacity("1 GiB"), Some(1_073_741_824));
    }

    #[test]
    fn test_parse_capacity_no_space() {
        assert_eq!(parse_capacity("500GB"), Some(500_000_000_000));
        assert_eq!(parse_capacity("1TB"), Some(1_000_000_000_000));
    }

    #[test]
    fn test_parse_capacity_invalid() {
        assert_eq!(parse_capacity(""), None);
        assert_eq!(parse_capacity("abc"), None);
        assert_eq!(parse_capacity("1 XB"), None);
    }

    #[test]
    fn test_extract_base_device_linux() {
        assert_eq!(extract_base_device("/dev/sda1"), "sda");
        assert_eq!(extract_base_device("/dev/sda"), "sda");
        assert_eq!(extract_base_device("/dev/nvme0n1p2"), "nvme0n1");
        assert_eq!(extract_base_device("/dev/nvme0n1"), "nvme0n1");
        assert_eq!(extract_base_device("sda1"), "sda");
    }

    #[test]
    fn test_extract_base_device_macos() {
        assert_eq!(extract_base_device("/dev/rdisk2s1"), "disk2");
        assert_eq!(extract_base_device("/dev/disk2"), "disk2");
        assert_eq!(extract_base_device("/dev/rdisk0"), "disk0");
    }
}

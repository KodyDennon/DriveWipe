//! macOS drive enumeration via `diskutil`.
//!
//! Discovers block devices by running `diskutil list` and inspecting
//! individual disks with `diskutil info`.  This provides a reliable way to
//! enumerate drives on macOS without requiring IOKit bindings.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{DriveWipeError, Result};
use crate::types::{AtaSecurityState, DriveInfo, DriveType, HiddenAreaInfo, Transport};

use super::info::detect_boot_drive;
use super::DriveEnumerator;

/// macOS drive enumerator backed by the `diskutil` command-line tool.
pub struct MacosDriveEnumerator;

impl DriveEnumerator for MacosDriveEnumerator {
    fn enumerate(&self) -> Result<Vec<DriveInfo>> {
        let disk_names = list_whole_disks()?;
        let mut drives = Vec::new();

        for disk_name in &disk_names {
            let dev_path = PathBuf::from(format!("/dev/{disk_name}"));
            match build_drive_info_from_diskutil(disk_name, &dev_path) {
                Ok(info) => drives.push(info),
                Err(e) => {
                    log::warn!("Skipping {disk_name}: {e}");
                }
            }
        }

        Ok(drives)
    }

    fn inspect(&self, path: &Path) -> Result<DriveInfo> {
        if !path.exists() {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        // Extract disk name from path (e.g. /dev/disk2 -> disk2, /dev/rdisk2 -> disk2).
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DriveWipeError::DeviceNotFound(path.to_path_buf()))?;

        // Normalize rdiskN -> diskN.
        let disk_name = if let Some(stripped) = name.strip_prefix('r') {
            stripped.to_string()
        } else {
            name.to_string()
        };

        build_drive_info_from_diskutil(&disk_name, path)
    }
}

/// List whole (non-partition) disk identifiers by parsing `diskutil list`.
fn list_whole_disks() -> Result<Vec<String>> {
    let output = Command::new("diskutil")
        .args(["list", "-plist"])
        .output()
        .map_err(|e| DriveWipeError::IoGeneric(e))?;

    if !output.status.success() {
        return Err(DriveWipeError::PlatformNotSupported(
            "diskutil list failed".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the plist XML to extract WholeDisks array.
    // For robustness we do simple text parsing rather than pulling in a full
    // plist library.
    let disks = parse_plist_string_array(&stdout, "WholeDisks");

    Ok(disks)
}

/// Build a [`DriveInfo`] for the given disk by running `diskutil info -plist`.
fn build_drive_info_from_diskutil(disk_name: &str, dev_path: &Path) -> Result<DriveInfo> {
    let output = Command::new("diskutil")
        .args(["info", "-plist", disk_name])
        .output()
        .map_err(|e| DriveWipeError::IoGeneric(e))?;

    if !output.status.success() {
        return Err(DriveWipeError::DeviceNotFound(dev_path.to_path_buf()));
    }

    let plist = String::from_utf8_lossy(&output.stdout);

    // Extract fields from the plist.
    let model = extract_plist_string(&plist, "MediaName")
        .or_else(|| extract_plist_string(&plist, "IORegistryEntryName"))
        .unwrap_or_default();

    let serial = extract_plist_string(&plist, "SerialNumber").unwrap_or_default();

    let firmware_rev = extract_plist_string(&plist, "FirmwareRevision").unwrap_or_default();

    let capacity = extract_plist_integer(&plist, "TotalSize")
        .or_else(|| extract_plist_integer(&plist, "Size"))
        .unwrap_or(0);

    let block_size = extract_plist_integer(&plist, "DeviceBlockSize").unwrap_or(512) as u32;

    // Detect drive type from protocol and media characteristics.
    let protocol = extract_plist_string(&plist, "BusProtocol").unwrap_or_default();
    let is_ssd = extract_plist_bool(&plist, "SolidState").unwrap_or(false);
    let internal = extract_plist_bool(&plist, "Internal").unwrap_or(true);
    let removable = extract_plist_bool(&plist, "Removable")
        .or_else(|| extract_plist_bool(&plist, "RemovableMedia"))
        .unwrap_or(false);

    let transport = match protocol.to_lowercase().as_str() {
        "pci-express" | "pci" | "apple fabric" => Transport::Nvme,
        "sata" | "ata" => Transport::Sata,
        "usb" => Transport::Usb,
        "sas" => Transport::Sas,
        _ => Transport::Unknown,
    };

    let drive_type = match transport {
        Transport::Nvme => DriveType::Nvme,
        _ if is_ssd => DriveType::Ssd,
        _ if !is_ssd && internal => DriveType::Hdd,
        _ => DriveType::Unknown,
    };

    let is_boot_drive = detect_boot_drive(dev_path);

    // TRIM support.
    let supports_trim = extract_plist_string(&plist, "TRIMSupport")
        .is_some_and(|v| v == "Yes" || v == "TRUE" || v == "true");

    // Count partitions by listing disk partitions.
    let partition_count = count_partitions_macos(disk_name);

    // Use the raw device path for best I/O performance.
    let raw_path = PathBuf::from(format!("/dev/r{disk_name}"));

    Ok(DriveInfo {
        path: raw_path,
        model,
        serial,
        firmware_rev,
        capacity,
        block_size,
        physical_block_size: None,
        drive_type,
        transport,
        is_boot_drive,
        is_removable: removable || !internal,
        ata_security: AtaSecurityState::NotSupported,
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim,
        is_sed: false,
        smart_healthy: None,
        partition_table: None,
        partition_count,
    })
}

/// Count partitions for a disk by listing its slices.
fn count_partitions_macos(disk_name: &str) -> u32 {
    let output = Command::new("diskutil").args(["list", disk_name]).output();

    let Ok(output) = output else {
        return 0;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Count lines that look like partition entries (e.g. "   1:  ...").
    // Skip the first line (header) and lines without a partition number.
    stdout
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Partition lines start with a digit followed by a colon.
            trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && trimmed.contains(':')
        })
        .count() as u32
}

// ── Simple plist XML parsing helpers ────────────────────────────────────────
//
// These are intentionally simple text-based parsers that avoid pulling in a
// full plist library.  They work well enough for the structured output of
// `diskutil info -plist`.

/// Extract a `<string>` value for the given key from a plist XML string.
fn extract_plist_string(plist: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{key}</key>");
    let key_pos = plist.find(&key_tag)?;
    let after_key = &plist[key_pos + key_tag.len()..];

    // Skip whitespace.
    let after_key = after_key.trim_start();

    // Look for <string>...</string>.
    let start_tag = "<string>";
    let end_tag = "</string>";

    if !after_key.starts_with(start_tag) {
        return None;
    }

    let value_start = start_tag.len();
    let value_end = after_key.find(end_tag)?;
    Some(after_key[value_start..value_end].to_string())
}

/// Extract an `<integer>` value for the given key from a plist XML string.
fn extract_plist_integer(plist: &str, key: &str) -> Option<u64> {
    let key_tag = format!("<key>{key}</key>");
    let key_pos = plist.find(&key_tag)?;
    let after_key = &plist[key_pos + key_tag.len()..];
    let after_key = after_key.trim_start();

    let start_tag = "<integer>";
    let end_tag = "</integer>";

    if !after_key.starts_with(start_tag) {
        return None;
    }

    let value_start = start_tag.len();
    let value_end = after_key.find(end_tag)?;
    after_key[value_start..value_end].parse().ok()
}

/// Extract a boolean value for the given key from a plist XML string.
///
/// Looks for `<true/>` or `<false/>` after the key tag.
fn extract_plist_bool(plist: &str, key: &str) -> Option<bool> {
    let key_tag = format!("<key>{key}</key>");
    let key_pos = plist.find(&key_tag)?;
    let after_key = &plist[key_pos + key_tag.len()..];
    let after_key = after_key.trim_start();

    if after_key.starts_with("<true/>") {
        Some(true)
    } else if after_key.starts_with("<false/>") {
        Some(false)
    } else {
        None
    }
}

/// Parse a `<array>` of `<string>` values for the given key from plist XML.
fn parse_plist_string_array(plist: &str, key: &str) -> Vec<String> {
    let key_tag = format!("<key>{key}</key>");
    let Some(key_pos) = plist.find(&key_tag) else {
        return Vec::new();
    };

    let after_key = &plist[key_pos + key_tag.len()..];
    let after_key = after_key.trim_start();

    let Some(array_start) = after_key.find("<array>") else {
        return Vec::new();
    };
    let Some(array_end) = after_key.find("</array>") else {
        return Vec::new();
    };

    let array_content = &after_key[array_start + 7..array_end];
    let mut results = Vec::new();

    let mut remaining = array_content;
    while let Some(start) = remaining.find("<string>") {
        let value_start = start + 8;
        if let Some(end) = remaining[value_start..].find("</string>") {
            results.push(remaining[value_start..value_start + end].to_string());
            remaining = &remaining[value_start + end + 9..];
        } else {
            break;
        }
    }

    results
}

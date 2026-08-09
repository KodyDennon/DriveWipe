//! Linux drive enumeration via sysfs.
//!
//! Discovers block devices by scanning `/sys/block/` and reading device
//! metadata from the sysfs pseudo-filesystem.  This avoids shelling out to
//! external commands and works on minimal Linux environments.

use std::path::{Path, PathBuf};

use crate::error::{DriveWipeError, Result};
use crate::types::{AtaSecurityState, DriveInfo, DriveType, HiddenAreaInfo, Transport};

use super::DriveEnumerator;
use super::info::detect_boot_drive;
use async_trait::async_trait;

/// Linux drive enumerator backed by sysfs.
pub struct LinuxDriveEnumerator;

#[async_trait]
impl DriveEnumerator for LinuxDriveEnumerator {
    async fn enumerate(&self) -> Result<Vec<DriveInfo>> {
        let mut drives = Vec::new();

        let mut entries =
            tokio::fs::read_dir("/sys/block")
                .await
                .map_err(|e| DriveWipeError::Io {
                    path: PathBuf::from("/sys/block"),
                    source: e,
                })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| DriveWipeError::Io {
            path: PathBuf::from("/sys/block"),
            source: e,
        })? {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Only consider real block devices: sd*, nvme*, hd*, vd*.
            let dominated = name_str.starts_with("sd")
                || name_str.starts_with("nvme")
                || name_str.starts_with("hd")
                || name_str.starts_with("vd");

            if !dominated {
                continue;
            }

            // Skip partition entries (e.g. sda1, nvme0n1p1).
            if is_partition(&name_str) {
                continue;
            }

            let dev_path = PathBuf::from(format!("/dev/{name_str}"));
            match build_drive_info(&name_str, &dev_path).await {
                Ok(info) => drives.push(info),
                Err(e) => {
                    log::warn!("Skipping {name_str}: {e}");
                }
            }
        }

        Ok(drives)
    }

    async fn inspect(&self, path: &Path) -> Result<DriveInfo> {
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            return Err(DriveWipeError::DeviceNotFound(path.to_path_buf()));
        }

        // Extract device name from path (e.g. /dev/sda -> sda).
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| DriveWipeError::DeviceNotFound(path.to_path_buf()))?;

        build_drive_info(name, path).await
    }
}

/// Check if a sysfs block device name is a partition rather than a whole disk.
fn is_partition(name: &str) -> bool {
    // sd* partitions: sda1, sdb2, sdaa1, etc. — letters after prefix, then digits.
    if name.starts_with("sd") || name.starts_with("hd") || name.starts_with("vd") {
        let rest = &name[2..];
        // Find first digit — partition names have digits after the letter(s)
        if let Some(digit_start) = rest.find(|c: char| c.is_ascii_digit()) {
            return rest[digit_start..].chars().all(|c| c.is_ascii_digit());
        }
        return false; // No digits = whole device
    }

    // NVMe partitions: nvme0n1p1, nvme0n1p2, etc.
    if name.starts_with("nvme") {
        return name.contains('p')
            && name.rsplit_once('p').is_some_and(|(_, after)| {
                !after.is_empty() && after.chars().all(|c| c.is_ascii_digit())
            });
    }

    false
}

/// Build a [`DriveInfo`] for the given device by reading sysfs attributes.
async fn build_drive_info(name: &str, dev_path: &Path) -> Result<DriveInfo> {
    let sys_block = PathBuf::from(format!("/sys/block/{name}"));

    let model = read_sysfs_string(&sys_block.join("device/model")).await;
    let serial = read_sysfs_string(&sys_block.join("device/serial")).await;
    let firmware_rev = read_sysfs_string(&sys_block.join("device/rev")).await;

    // Size is reported in 512-byte sectors.
    let size_sectors = read_sysfs_u64(&sys_block.join("size")).await.unwrap_or(0);
    let capacity = size_sectors * 512;

    // Logical block size.
    let block_size = read_sysfs_u64(&sys_block.join("queue/logical_block_size"))
        .await
        .unwrap_or(512) as u32;

    // Physical block size.
    let physical_block_size = read_sysfs_u64(&sys_block.join("queue/physical_block_size"))
        .await
        .map(|v| v as u32);

    // Detect transport before interpreting medium hints. USB mass-storage
    // bridges expose SCSI disks and some report a synthetic rotational value,
    // so the hardware ancestry has to take precedence over those leaf-level
    // attributes.
    let transport = detect_transport(name, &sys_block).await;

    // Removable flag.
    let is_removable = read_sysfs_u64(&sys_block.join("removable"))
        .await
        .is_some_and(|v| v == 1);

    // rotational: 0 = SSD/NVMe, 1 = HDD. A removable USB bridge may report 1
    // for flash media (the SanDisk USB mass-storage stack does this on Linux),
    // so do not turn that ambiguous combination into a false HDD claim. The
    // USB transport still selects the bridge-safe wipe method below.
    let rotational = read_sysfs_u64(&sys_block.join("queue/rotational")).await;
    let drive_type = classify_drive_type(name, rotational, transport, is_removable);

    // Boot drive detection.
    let is_boot_drive = detect_boot_drive(dev_path);

    // TRIM support.
    let supports_trim = read_sysfs_string(&sys_block.join("queue/discard_max_bytes"))
        .await
        .parse::<u64>()
        .unwrap_or(0)
        > 0;

    // Partition count: count subdirectories matching <name>N or <name>pN.
    let partition_count = count_partitions(name).await;

    Ok(DriveInfo {
        path: dev_path.to_path_buf(),
        model,
        serial,
        firmware_rev,
        capacity,
        block_size,
        physical_block_size,
        drive_type,
        transport,
        is_boot_drive,
        is_removable,
        ata_security: AtaSecurityState::NotSupported, // Requires ATA passthrough to detect.
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim,
        is_sed: false,         // Requires TCG/OPAL query.
        smart_healthy: None,   // Requires smartctl or ATA passthrough.
        partition_table: None, // Requires reading MBR/GPT header.
        partition_count,
    })
}

/// Detect the connection transport for a block device.
async fn detect_transport(name: &str, sys_block: &Path) -> Transport {
    if name.starts_with("nvme") {
        return Transport::Nvme;
    }

    // USB, SATA, SAS and other transports commonly present a SCSI leaf node.
    // The direct subsystem therefore says only "scsi"; the real bus is higher
    // in the canonical /sys/devices ancestry (for example .../usb2/2-7/...).
    if let Ok(device_path) = tokio::fs::canonicalize(sys_block.join("device")).await
        && let Some(transport) = transport_from_device_ancestry(&device_path)
    {
        return transport;
    }

    // Some kernels or virtual filesystems expose an explicit transport value.
    let transport_str = read_sysfs_string(&sys_block.join("device/transport"))
        .await
        .to_ascii_lowercase();
    match transport_str.as_str() {
        "usb" => return Transport::Usb,
        "sata" | "ata" => return Transport::Sata,
        "sas" => return Transport::Sas,
        "iscsi" | "fc" | "fcoe" => return Transport::Scsi,
        _ => {}
    }

    // Finally use the direct subsystem. Unknown SCSI devices stay SCSI rather
    // than being guessed as SATA; a wrong firmware-capable classification is
    // more dangerous than a conservative generic one.
    let subsystem = sys_block.join("device/subsystem");
    let subsystem_target = tokio::fs::canonicalize(&subsystem)
        .await
        .ok()
        .or_else(|| std::fs::read_link(&subsystem).ok());
    let subsystem_name = subsystem_target
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or("");
    match subsystem_name {
        "usb" => Transport::Usb,
        "ata" => Transport::Sata,
        "sas" => Transport::Sas,
        "scsi" => Transport::Scsi,
        _ => Transport::Unknown,
    }
}

/// Infer the physical bus from a canonical `/sys/devices/...` path.
fn transport_from_device_ancestry(path: &Path) -> Option<Transport> {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();

    // A bridge can contain generic SCSI/ATA-looking descendants, so the USB
    // bus marker gets first priority across the entire ancestry.
    if components
        .iter()
        .any(|component| has_numeric_suffix(component, "usb"))
    {
        return Some(Transport::Usb);
    }
    if components
        .iter()
        .any(|component| has_numeric_suffix(component, "ata"))
    {
        return Some(Transport::Sata);
    }
    None
}

fn has_numeric_suffix(component: &str, prefix: &str) -> bool {
    component
        .strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
}

fn classify_drive_type(
    name: &str,
    rotational: Option<u64>,
    transport: Transport,
    is_removable: bool,
) -> DriveType {
    if name.starts_with("nvme") || transport == Transport::Nvme {
        return DriveType::Nvme;
    }

    match rotational {
        Some(0) => DriveType::Ssd,
        // USB removable media frequently has a fabricated rotational=1 flag.
        // Preserve uncertainty instead of confidently calling flash an HDD.
        Some(1) if transport == Transport::Usb && is_removable => DriveType::Unknown,
        Some(1) => DriveType::Hdd,
        _ => DriveType::Unknown,
    }
}

/// Read a sysfs file and return its trimmed contents, or an empty string on
/// failure.
async fn read_sysfs_string(path: &Path) -> String {
    tokio::fs::read_to_string(path)
        .await
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Read a sysfs file and parse it as a `u64`.
async fn read_sysfs_u64(path: &Path) -> Option<u64> {
    read_sysfs_string(path).await.parse().ok()
}

/// Count the number of partitions for a given block device by scanning
/// `/sys/block/<name>/`.
async fn count_partitions(name: &str) -> u32 {
    let sys_block = PathBuf::from(format!("/sys/block/{name}"));
    let Ok(mut entries) = tokio::fs::read_dir(&sys_block).await else {
        return 0;
    };

    let mut count = 0;
    while let Ok(Some(e)) = entries.next_entry().await {
        let entry_name = e.file_name();
        let entry_str = entry_name.to_string_lossy();
        // Partitions show up as subdirectories named like sda1, nvme0n1p1.
        if entry_str.starts_with(name)
            && entry_str.len() > name.len()
            && e.file_type().await.is_ok_and(|ft| ft.is_dir())
        {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usb_mass_storage_is_detected_from_scsi_device_ancestry() {
        let path = Path::new(
            "/sys/devices/pci0000:00/0000:00:14.0/usb2/2-7/2-7:1.0/host0/target0:0:0/0:0:0:0",
        );
        assert_eq!(transport_from_device_ancestry(path), Some(Transport::Usb));
    }

    #[test]
    fn native_sata_is_detected_from_ata_ancestry() {
        let path = Path::new("/sys/devices/pci0000:00/0000:00:17.0/ata3/host2/target2:0:0/2:0:0:0");
        assert_eq!(transport_from_device_ancestry(path), Some(Transport::Sata));
    }

    #[test]
    fn unrelated_component_names_do_not_look_like_bus_markers() {
        let path = Path::new("/sys/devices/platform/usb-storage-cache/atlas/host0/target0:0:0");
        assert_eq!(transport_from_device_ancestry(path), None);
    }

    #[test]
    fn ambiguous_removable_usb_does_not_claim_to_be_an_hdd() {
        assert_eq!(
            classify_drive_type("sda", Some(1), Transport::Usb, true),
            DriveType::Unknown
        );
    }

    #[test]
    fn non_rotational_usb_media_is_an_ssd() {
        assert_eq!(
            classify_drive_type("sda", Some(0), Transport::Usb, true),
            DriveType::Ssd
        );
    }

    #[test]
    fn fixed_rotational_usb_disk_remains_an_hdd() {
        assert_eq!(
            classify_drive_type("sda", Some(1), Transport::Usb, false),
            DriveType::Hdd
        );
    }
}

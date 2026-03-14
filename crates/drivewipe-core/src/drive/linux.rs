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
    // sd* partitions: sda1, sdb2, etc. — the name ends with digits.
    if name.starts_with("sd") || name.starts_with("hd") || name.starts_with("vd") {
        return name.len() > 3 && name[3..].chars().all(|c| c.is_ascii_digit());
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

    // rotational: 0 = SSD/NVMe, 1 = HDD.
    let rotational = read_sysfs_u64(&sys_block.join("queue/rotational")).await;
    let is_nvme = name.starts_with("nvme");

    let drive_type = match (is_nvme, rotational) {
        (true, _) => DriveType::Nvme,
        (_, Some(0)) => DriveType::Ssd,
        (_, Some(1)) => DriveType::Hdd,
        _ => DriveType::Unknown,
    };

    // Detect transport from device path or sysfs symlinks.
    let transport = detect_transport(name, &sys_block).await;

    // Removable flag.
    let is_removable = read_sysfs_u64(&sys_block.join("removable"))
        .await
        .is_some_and(|v| v == 1);

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

    // Check the device subsystem symlink.
    let subsystem = sys_block.join("device/subsystem");
    if let Ok(target) = tokio::fs::read_link(&subsystem).await {
        let target_str = target.to_string_lossy();
        if target_str.contains("usb") {
            return Transport::Usb;
        }
        if target_str.contains("scsi") || target_str.contains("ata") {
            // Try to distinguish SATA from SAS/SCSI.
            let transport_file = sys_block.join("device/transport");
            let transport_str = read_sysfs_string(&transport_file).await;
            return match transport_str.as_str() {
                "sata" => Transport::Sata,
                "sas" => Transport::Sas,
                "iscsi" | "fc" => Transport::Scsi,
                _ => Transport::Sata, // Default SCSI subsystem to SATA.
            };
        }
    }

    Transport::Unknown
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

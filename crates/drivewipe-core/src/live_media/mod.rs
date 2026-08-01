//! Writing the DriveWipe Live image to removable media.
//!
//! Creating boot media means writing raw sectors to a block device, which is
//! indistinguishable from destroying whatever was on it. Everything here is
//! built around not doing that to the wrong device: candidates are restricted
//! to removable media by default, the boot drive is refused outright, and the
//! write is verified by reading it back.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{DriveWipeError, Result};
use crate::types::DriveInfo;

/// Copy buffer size. Large enough to keep USB 3 saturated, small enough that
/// progress updates stay responsive on slow USB 2 sticks.
const CHUNK: usize = 4 * 1024 * 1024;

/// Why a device is or is not offered as a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetSafety {
    /// Removable media — the expected kind of target.
    Removable,
    /// Fixed disk. Writable only with an explicit override.
    Fixed,
    /// The running system's boot drive. Never writable.
    BootDrive,
}

impl TargetSafety {
    pub fn is_safe_default(&self) -> bool {
        matches!(self, TargetSafety::Removable)
    }

    /// A short reason, for display next to the device.
    pub fn reason(&self) -> &'static str {
        match self {
            TargetSafety::Removable => "removable",
            TargetSafety::Fixed => "fixed disk — not removable",
            TargetSafety::BootDrive => "boot drive — refused",
        }
    }
}

/// A device that could receive the live image, with its safety classification.
#[derive(Debug, Clone)]
pub struct BurnTarget {
    pub path: PathBuf,
    pub model: String,
    pub serial: String,
    pub capacity: u64,
    pub safety: TargetSafety,
}

impl BurnTarget {
    pub fn from_drive(d: &DriveInfo) -> Self {
        let safety = if d.is_boot_drive {
            TargetSafety::BootDrive
        } else if d.is_removable {
            TargetSafety::Removable
        } else {
            TargetSafety::Fixed
        };

        Self {
            path: d.path.clone(),
            model: d.model.clone(),
            serial: d.serial.clone(),
            capacity: d.capacity,
            safety,
        }
    }
}

/// Classify every attached drive as a burn target.
pub fn classify_targets(drives: &[DriveInfo]) -> Vec<BurnTarget> {
    drives.iter().map(BurnTarget::from_drive).collect()
}

/// Progress during a burn.
#[derive(Debug, Clone, Copy)]
pub enum BurnProgress {
    Writing { written: u64, total: u64 },
    Syncing,
    Verifying { checked: u64, total: u64 },
}

/// Reject a target that must never be written to, and require an override for
/// one that is merely unusual.
///
/// Separated from the write itself so a UI can grey out or warn about a device
/// before the operator commits to anything.
pub fn check_target(target: &BurnTarget, allow_fixed: bool) -> Result<()> {
    match target.safety {
        TargetSafety::BootDrive => Err(DriveWipeError::BootDriveRefused(target.path.clone())),
        TargetSafety::Fixed if !allow_fixed => Err(DriveWipeError::DeviceError(format!(
            "{} is a fixed disk, not removable media. Writing the live image to it \
             would destroy its contents. Pass --allow-fixed-disk if this is really \
             what you want.",
            target.path.display()
        ))),
        _ => Ok(()),
    }
}

/// Write `image` to `target`, then read it back and confirm it matches.
///
/// The verification pass is not optional: a USB stick that silently drops
/// writes produces media that fails at boot, in a data centre, at the point
/// where someone is relying on it.
pub fn burn<F>(
    image: &Path,
    target: &BurnTarget,
    allow_fixed: bool,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(BurnProgress),
{
    check_target(target, allow_fixed)?;

    let mut src = std::fs::File::open(image).map_err(|e| DriveWipeError::Io {
        path: image.to_path_buf(),
        source: e,
    })?;
    let total = src
        .metadata()
        .map_err(|e| DriveWipeError::Io {
            path: image.to_path_buf(),
            source: e,
        })?
        .len();

    if total == 0 {
        return Err(DriveWipeError::DeviceError(format!(
            "{} is empty — refusing to write a zero-length image",
            image.display()
        )));
    }
    if total > target.capacity {
        return Err(DriveWipeError::DeviceError(format!(
            "image is {} but {} holds only {}",
            crate::types::format_bytes(total),
            target.path.display(),
            crate::types::format_bytes(target.capacity),
        )));
    }

    let mut dst = std::fs::OpenOptions::new()
        .write(true)
        .open(&target.path)
        .map_err(|e| DriveWipeError::Io {
            path: target.path.clone(),
            source: e,
        })?;

    // ── Write ───────────────────────────────────────────────────────────
    let mut buf = vec![0u8; CHUNK];
    let mut written: u64 = 0;
    let mut hasher_written = Sha256::new();

    loop {
        let n = src.read(&mut buf).map_err(DriveWipeError::IoGeneric)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])
            .map_err(DriveWipeError::IoGeneric)?;
        hasher_written.update(&buf[..n]);
        written += n as u64;
        on_progress(BurnProgress::Writing { written, total });
    }

    on_progress(BurnProgress::Syncing);
    dst.flush().map_err(DriveWipeError::IoGeneric)?;
    dst.sync_all().map_err(DriveWipeError::IoGeneric)?;
    drop(dst);

    let expected = hasher_written.finalize();

    // ── Verify ──────────────────────────────────────────────────────────
    // Re-open so the read is not served from anything the write left behind.
    let mut check = std::fs::File::open(&target.path).map_err(|e| DriveWipeError::Io {
        path: target.path.clone(),
        source: e,
    })?;
    check
        .seek(SeekFrom::Start(0))
        .map_err(DriveWipeError::IoGeneric)?;

    let mut hasher_read = Sha256::new();
    let mut checked: u64 = 0;
    while checked < total {
        let want = ((total - checked) as usize).min(CHUNK);
        let n = check
            .read(&mut buf[..want])
            .map_err(DriveWipeError::IoGeneric)?;
        if n == 0 {
            return Err(DriveWipeError::DeviceError(format!(
                "device returned end-of-data after {} of {} bytes — the write did not stick",
                checked, total
            )));
        }
        hasher_read.update(&buf[..n]);
        checked += n as u64;
        on_progress(BurnProgress::Verifying { checked, total });
    }

    if hasher_read.finalize() != expected {
        return Err(DriveWipeError::DeviceError(format!(
            "verification failed: {} does not contain what was written to it. \
             The media may be faulty or counterfeit — do not boot from it.",
            target.path.display()
        )));
    }

    Ok(())
}

/// Verify a downloaded image against a published SHA-256 digest.
pub fn verify_image_checksum(image: &Path, expected_hex: &str) -> Result<()> {
    let mut f = std::fs::File::open(image).map_err(|e| DriveWipeError::Io {
        path: image.to_path_buf(),
        source: e,
    })?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = f.read(&mut buf).map_err(DriveWipeError::IoGeneric)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex::encode(hasher.finalize());
    let want = expected_hex.trim().to_lowercase();

    if got == want {
        Ok(())
    } else {
        Err(DriveWipeError::DeviceError(format!(
            "checksum mismatch for {}\n  expected {want}\n  actual   {got}\n\
             This image does not match what was published. Do not use it.",
            image.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(removable: bool, boot: bool) -> DriveInfo {
        let mut d = DriveInfo::default();
        d.path = PathBuf::from("/dev/sdz");
        d.is_removable = removable;
        d.is_boot_drive = boot;
        d.capacity = 8 * 1024 * 1024 * 1024;
        d
    }

    #[test]
    fn removable_media_is_the_safe_default() {
        let t = BurnTarget::from_drive(&drive(true, false));
        assert_eq!(t.safety, TargetSafety::Removable);
        assert!(t.safety.is_safe_default());
        assert!(check_target(&t, false).is_ok());
    }

    #[test]
    fn a_fixed_disk_needs_an_explicit_override() {
        let t = BurnTarget::from_drive(&drive(false, false));
        assert_eq!(t.safety, TargetSafety::Fixed);
        assert!(check_target(&t, false).is_err());
        assert!(check_target(&t, true).is_ok());
    }

    #[test]
    fn the_boot_drive_is_refused_even_with_the_override() {
        // Writing a live image over the running system is never what anyone
        // meant, so no flag permits it.
        let t = BurnTarget::from_drive(&drive(false, true));
        assert_eq!(t.safety, TargetSafety::BootDrive);
        assert!(check_target(&t, false).is_err());
        assert!(check_target(&t, true).is_err());
    }

    #[test]
    fn a_removable_boot_drive_is_still_refused() {
        // Booted from the USB stick you are about to overwrite.
        let t = BurnTarget::from_drive(&drive(true, true));
        assert_eq!(t.safety, TargetSafety::BootDrive);
        assert!(check_target(&t, true).is_err());
    }

    #[test]
    fn checksum_verification_accepts_and_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("img");
        std::fs::write(&f, b"drivewipe live image").unwrap();

        let mut h = Sha256::new();
        h.update(b"drivewipe live image");
        let good = hex::encode(h.finalize());

        assert!(verify_image_checksum(&f, &good).is_ok());
        assert!(verify_image_checksum(&f, &good.to_uppercase()).is_ok());
        assert!(verify_image_checksum(&f, &"0".repeat(64)).is_err());
    }
}

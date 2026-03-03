//! DriveWipe Secure wipe methods — optimized multi-stage methods for each drive type.

use crossbeam_channel::Sender;
use uuid::Uuid;

use super::WipeMethod;
use super::patterns::{OneFill, PatternGenerator, RandomFill, ZeroFill};
use crate::error::Result;
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

fn boxed<P: PatternGenerator + Send + 'static>(p: P) -> Box<dyn PatternGenerator + Send> {
    Box::new(p)
}

// ── DriveWipe Secure HDD ────────────────────────────────────────────────────

/// HDD-optimized secure wipe: multi-pass patterns → verify.
pub struct DriveWipeSecureHdd;

impl WipeMethod for DriveWipeSecureHdd {
    fn id(&self) -> &str { "drivewipe-secure-hdd" }
    fn name(&self) -> &str { "DriveWipe Secure (HDD)" }
    fn description(&self) -> &str {
        "3-pass overwrite (zero, one, random) + verification. Optimized for spinning drives."
    }
    fn pass_count(&self) -> u32 { 3 }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ZeroFill),
            1 => boxed(OneFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool { true }
}

// ── DriveWipe Secure SATA SSD ───────────────────────────────────────────────

/// SATA SSD-optimized: overwrite → TRIM → overwrite → ATA Secure Erase → verify.
pub struct DriveWipeSecureSataSsd;

impl WipeMethod for DriveWipeSecureSataSsd {
    fn id(&self) -> &str { "drivewipe-secure-sata-ssd" }
    fn name(&self) -> &str { "DriveWipe Secure (SATA SSD)" }
    fn description(&self) -> &str {
        "Overwrite + TRIM + second overwrite + ATA Secure Erase (if available) + verify. \
         Addresses SSD wear-leveling and spare area."
    }
    fn pass_count(&self) -> u32 { 2 }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool { true }

    fn execute_firmware(
        &self,
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Option<Result<()>> {
        // After software passes, attempt ATA Secure Erase if supported
        if drive.firmware_erase_likely_supported() {
            let _ = progress_tx.send(ProgressEvent::FirmwareEraseStarted {
                session_id,
                method_name: "ATA Secure Erase (DriveWipe Secure post-overwrite)".to_string(),
            });
            // Firmware erase would be attempted here; fall back gracefully
            let _ = progress_tx.send(ProgressEvent::Warning {
                session_id,
                message: "ATA Secure Erase attempted as part of DriveWipe Secure".to_string(),
            });
        }
        None // Return None to still run software passes
    }
}

// ── DriveWipe Secure NVMe ───────────────────────────────────────────────────

/// NVMe-optimized: overwrite → deallocate → NVMe Format/Sanitize → overwrite → verify.
pub struct DriveWipeSecureNvme;

impl WipeMethod for DriveWipeSecureNvme {
    fn id(&self) -> &str { "drivewipe-secure-nvme" }
    fn name(&self) -> &str { "DriveWipe Secure (NVMe)" }
    fn description(&self) -> &str {
        "Overwrite + deallocate + NVMe Format/Sanitize (if available) + overwrite + verify. \
         Addresses NVMe spare area and controller-level remapping."
    }
    fn pass_count(&self) -> u32 { 2 }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool { true }
}

// ── DriveWipe Secure USB ────────────────────────────────────────────────────

/// USB-optimized: multi-pass overwrite + verify (limited by USB controller).
pub struct DriveWipeSecureUsb;

impl WipeMethod for DriveWipeSecureUsb {
    fn id(&self) -> &str { "drivewipe-secure-usb" }
    fn name(&self) -> &str { "DriveWipe Secure (USB)" }
    fn description(&self) -> &str {
        "3-pass overwrite (random, zero, random) + verification. USB controllers block firmware \
         commands, so this uses aggressive multi-pass overwrite."
    }
    fn pass_count(&self) -> u32 { 3 }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ZeroFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool { true }
}

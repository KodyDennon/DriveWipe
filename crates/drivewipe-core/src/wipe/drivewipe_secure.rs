//! DriveWipe Secure wipe methods — optimized multi-stage methods for each drive type.

use async_trait::async_trait;
use crossbeam_channel::Sender;
use uuid::Uuid;

use super::WipeMethod;
use super::firmware::FirmwareWipe;
use super::firmware::{ata, nvme};
use super::patterns::{PatternGenerator, RandomFill, ZeroFill};
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

fn boxed<P: PatternGenerator + Send + 'static>(p: P) -> Box<dyn PatternGenerator + Send> {
    Box::new(p)
}

/// Try each firmware erase in order, stopping at the first that succeeds.
///
/// Runs before the overwrite passes so the passes leave the final, verifiable
/// pattern on the surface. Every outcome is advisory — an unsupported or
/// rejected command is reported and the software passes carry the wipe.
async fn try_firmware_sanitize(
    candidates: &[&dyn FirmwareWipe],
    drive: &DriveInfo,
    session_id: Uuid,
    progress_tx: &Sender<ProgressEvent>,
) -> Vec<String> {
    let mut notes = Vec::new();

    for fw in candidates {
        if !fw.is_supported(drive) {
            continue;
        }
        match fw.execute(drive, session_id, progress_tx).await {
            Ok(()) => {
                notes.push(format!(
                    "Controller sanitize succeeded via {} before overwrite passes",
                    fw.name()
                ));
                return notes;
            }
            Err(e) => {
                notes.push(format!("Controller sanitize via {} failed: {e}", fw.name()));
            }
        }
    }

    if notes.is_empty() {
        notes.push(
            "No controller sanitize command available for this drive; relying on overwrite \
             passes alone, which cannot reach spare or overprovisioned blocks"
                .to_string(),
        );
    }
    notes
}

/// Issue a whole-device TRIM after the overwrite passes.
///
/// Lets the controller erase flash blocks the host cannot address. Safe to run
/// before verification: a discarded range reads back as zeros or as the last
/// data written, and every method using this ends on a zero pass.
fn trim_whole_device(drive: &DriveInfo) -> Vec<String> {
    if !drive.supports_trim {
        return vec!["Drive does not report TRIM support; skipping discard".to_string()];
    }

    match crate::io::discard_all(&drive.path, drive.capacity) {
        Ok(()) => vec![format!("TRIM issued across all {} bytes", drive.capacity)],
        Err(e) => vec![format!("TRIM failed (wipe unaffected): {e}")],
    }
}

// ── DriveWipe Secure HDD ────────────────────────────────────────────────────

/// HDD-optimized secure wipe: multi-pass patterns → verify.
pub struct DriveWipeSecureHdd;

#[async_trait]
impl WipeMethod for DriveWipeSecureHdd {
    fn id(&self) -> &str {
        "drivewipe-secure-hdd"
    }
    fn name(&self) -> &str {
        "DriveWipe Secure (HDD)"
    }
    fn description(&self) -> &str {
        "4-pass overwrite (zero, random, random, zero) + verification. Optimized for spinning \
         drives with full surface coverage."
    }
    fn pass_count(&self) -> u32 {
        4
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ZeroFill),
            1 => boxed(RandomFill::new()),
            2 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── DriveWipe Secure SATA SSD ───────────────────────────────────────────────

/// SATA SSD-optimized: ATA Secure Erase → overwrite → TRIM → verify.
pub struct DriveWipeSecureSataSsd;

#[async_trait]
impl WipeMethod for DriveWipeSecureSataSsd {
    fn id(&self) -> &str {
        "drivewipe-secure-sata-ssd"
    }
    fn name(&self) -> &str {
        "DriveWipe Secure (SATA SSD)"
    }
    fn description(&self) -> &str {
        "ATA Secure Erase (if available) + 4-pass software overwrite (random, zero, random, \
         zero) + whole-device TRIM + verification. Addresses SSD wear-leveling and spare area."
    }
    fn pass_count(&self) -> u32 {
        4
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ZeroFill),
            2 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }

    async fn before_passes(
        &self,
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Vec<String> {
        try_firmware_sanitize(
            &[&ata::AtaEnhancedSecureErase, &ata::AtaSecureErase],
            drive,
            session_id,
            progress_tx,
        )
        .await
    }

    async fn after_passes(
        &self,
        drive: &DriveInfo,
        _session_id: Uuid,
        _progress_tx: &Sender<ProgressEvent>,
    ) -> Vec<String> {
        trim_whole_device(drive)
    }
}

// ── DriveWipe Secure NVMe ───────────────────────────────────────────────────

/// NVMe-optimized: Sanitize/Format → overwrite → deallocate → verify.
pub struct DriveWipeSecureNvme;

#[async_trait]
impl WipeMethod for DriveWipeSecureNvme {
    fn id(&self) -> &str {
        "drivewipe-secure-nvme"
    }
    fn name(&self) -> &str {
        "DriveWipe Secure (NVMe)"
    }
    fn description(&self) -> &str {
        "NVMe Sanitize/Format (if available) + 4-pass software overwrite (random, zero, random, \
         zero) + deallocate + verification. Addresses NVMe spare area and controller-level \
         remapping."
    }
    fn pass_count(&self) -> u32 {
        4
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ZeroFill),
            2 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }

    async fn before_passes(
        &self,
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Vec<String> {
        try_firmware_sanitize(
            &[
                &nvme::NvmeSanitizeBlock,
                &nvme::NvmeSanitizeCrypto,
                &nvme::NvmeFormatUserData,
            ],
            drive,
            session_id,
            progress_tx,
        )
        .await
    }

    async fn after_passes(
        &self,
        drive: &DriveInfo,
        _session_id: Uuid,
        _progress_tx: &Sender<ProgressEvent>,
    ) -> Vec<String> {
        trim_whole_device(drive)
    }
}

// ── DriveWipe Secure USB ────────────────────────────────────────────────────

/// USB-optimized: multi-pass overwrite + verify (limited by USB controller).
pub struct DriveWipeSecureUsb;

#[async_trait]
impl WipeMethod for DriveWipeSecureUsb {
    fn id(&self) -> &str {
        "drivewipe-secure-usb"
    }
    fn name(&self) -> &str {
        "DriveWipe Secure (USB)"
    }
    fn description(&self) -> &str {
        "4-pass overwrite (random, zero, random, zero) + verification. USB controllers block \
         firmware commands, so this uses aggressive multi-pass overwrite."
    }
    fn pass_count(&self) -> u32 {
        4
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ZeroFill),
            2 => boxed(RandomFill::new()),
            _ => boxed(ZeroFill),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }

    async fn after_passes(
        &self,
        drive: &DriveInfo,
        _session_id: Uuid,
        _progress_tx: &Sender<ProgressEvent>,
    ) -> Vec<String> {
        // USB bridges block firmware erase, but many pass TRIM through.
        trim_whole_device(drive)
    }
}

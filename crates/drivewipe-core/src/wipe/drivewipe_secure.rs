//! DriveWipe Secure wipe methods — optimized multi-stage methods for each drive type.

use async_trait::async_trait;

use super::WipeMethod;
use super::patterns::{PatternGenerator, RandomFill, ZeroFill};

fn boxed<P: PatternGenerator + Send + 'static>(p: P) -> Box<dyn PatternGenerator + Send> {
    Box::new(p)
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
            _ => boxed(ZeroFill), // Final zero pass for clean verification
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── DriveWipe Secure SATA SSD ───────────────────────────────────────────────

/// SATA SSD-optimized: overwrite → TRIM → overwrite → ATA Secure Erase → verify.
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
        "4-pass software overwrite (random, zero, random, zero) + verification. \
         Addresses SSD wear-leveling and spare area."
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
}

// ── DriveWipe Secure NVMe ───────────────────────────────────────────────────

/// NVMe-optimized: overwrite → deallocate → NVMe Format/Sanitize → overwrite → verify.
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
        "4-pass software overwrite (random, zero, random, zero) + NVMe Format/Sanitize attempt \
         (if available) + verification. Addresses NVMe spare area and controller-level remapping."
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
            _ => boxed(ZeroFill), // Final zero pass for clean verification
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

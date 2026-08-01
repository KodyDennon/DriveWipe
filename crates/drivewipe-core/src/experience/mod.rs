//! Two ways through the tool: one for people who know disks, one for people
//! who know what they want to happen to them.
//!
//! DriveWipe's method list is 27 entries deep and named after standards
//! documents. That is exactly right for someone sanitising a fleet against a
//! compliance requirement, and exactly wrong for someone who booted a USB stick
//! to erase a laptop before selling it. Basic mode answers the second person's
//! question — how thoroughly, and how long — and picks the standard for them.
//!
//! Nothing is hidden that changes what happens to the data: Basic mode maps
//! onto the same wipe methods, and always names the standard it selected so the
//! choice can be checked or repeated from the command line.

use serde::{Deserialize, Serialize};

use crate::types::{DriveInfo, DriveType, Transport};

/// How much of the tool to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Experience {
    /// Plain language, three choices, sensible defaults.
    #[default]
    Basic,
    /// Every method, every switch, standards named directly.
    Expert,
}

impl Experience {
    pub fn label(&self) -> &'static str {
        match self {
            Experience::Basic => "Basic",
            Experience::Expert => "Expert",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Experience::Basic => {
                "Guided. Three levels of thoroughness, plain language, and a \
                 recommended default for each drive."
            }
            Experience::Expert => {
                "Full control. All 27 methods by name, per-pass verification, \
                 hidden-area handling, and firmware commands."
            }
        }
    }

    pub fn toggled(&self) -> Self {
        match self {
            Experience::Basic => Experience::Expert,
            Experience::Expert => Experience::Basic,
        }
    }
}

/// The three choices offered in Basic mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraseLevel {
    /// One pass. For a drive staying inside the organisation.
    Quick,
    /// The current standard, verified. The right answer for almost everyone.
    Standard,
    /// Multi-pass, for a policy that demands it.
    Thorough,
}

impl EraseLevel {
    pub const ALL: [EraseLevel; 3] = [
        EraseLevel::Standard,
        EraseLevel::Quick,
        EraseLevel::Thorough,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            EraseLevel::Quick => "Quick",
            EraseLevel::Standard => "Standard",
            EraseLevel::Thorough => "Thorough",
        }
    }

    /// One line, in plain language, describing the outcome rather than the
    /// mechanism.
    pub fn summary(&self) -> &'static str {
        match self {
            EraseLevel::Quick => "Overwrites everything once. Fast.",
            EraseLevel::Standard => "Overwrites everything, then reads it all back to prove it.",
            EraseLevel::Thorough => "Overwrites several times, checking after each pass.",
        }
    }

    /// When someone should pick this, in terms of their situation.
    pub fn guidance(&self) -> &'static str {
        match self {
            EraseLevel::Quick => "Reusing the drive yourself, or inside your own organisation.",
            EraseLevel::Standard => {
                "Selling, donating, recycling, or returning the drive. \
                 Meets the current US government standard."
            }
            EraseLevel::Thorough => {
                "Your policy or customer specifically requires multi-pass \
                 overwriting. Takes several times longer for no additional \
                 protection on modern drives."
            }
        }
    }

    /// Whether to mark this as the default choice.
    pub fn is_recommended(&self) -> bool {
        matches!(self, EraseLevel::Standard)
    }

    /// Roughly how much longer than a single pass this takes, for estimating.
    pub fn pass_multiplier(&self, drive: &DriveInfo) -> f64 {
        match self {
            EraseLevel::Quick => 1.0,
            // A verified pass reads the whole surface back.
            EraseLevel::Standard => {
                if uses_firmware_erase(drive) {
                    0.05
                } else {
                    2.0
                }
            }
            // Three passes, each verified.
            EraseLevel::Thorough => 6.0,
        }
    }

    /// The actual wipe method this maps to for a given drive.
    ///
    /// The mapping is drive-aware because the honest answer differs by medium:
    /// overwriting an SSD cannot reach its overprovisioned cells, so the
    /// hybrid methods that also issue a controller sanitize are the correct
    /// "standard" there, while a spinning disk is fully covered by overwrites.
    pub fn method_id(&self, drive: &DriveInfo) -> &'static str {
        match self {
            EraseLevel::Quick => "zero",
            EraseLevel::Standard => match (drive.drive_type, drive.transport) {
                (DriveType::Nvme, _) => "drivewipe-secure-nvme",
                (DriveType::Ssd, Transport::Usb) => "drivewipe-secure-usb",
                (DriveType::Ssd, _) => "drivewipe-secure-sata-ssd",
                _ => "nist-800-88-clear",
            },
            EraseLevel::Thorough => "dod-short",
        }
    }

    /// The standard behind the choice, so Basic mode never obscures what was
    /// actually run.
    pub fn standard_name(&self, drive: &DriveInfo) -> &'static str {
        match self {
            EraseLevel::Quick => "Single-pass zero overwrite",
            EraseLevel::Standard => match drive.drive_type {
                DriveType::Nvme | DriveType::Ssd => {
                    "NIST SP 800-88 Purge (controller sanitize + overwrite)"
                }
                _ => "NIST SP 800-88 Clear",
            },
            EraseLevel::Thorough => "DoD 5220.22-M (3-pass)",
        }
    }
}

/// Whether the standard level will attempt a firmware erase on this drive,
/// which is near-instant compared with overwriting it.
fn uses_firmware_erase(drive: &DriveInfo) -> bool {
    matches!(drive.drive_type, DriveType::Nvme | DriveType::Ssd)
        && drive.transport != Transport::Usb
}

/// A human estimate of how long a level will take on a drive.
///
/// Deliberately coarse. Someone deciding between "Standard" and "Thorough"
/// needs to know whether it is minutes or hours, not a false-precision figure
/// that will be wrong anyway.
pub fn estimate(level: EraseLevel, drive: &DriveInfo, observed_mbps: Option<f64>) -> String {
    // Conservative defaults when nothing has been measured yet.
    let mbps = observed_mbps.unwrap_or(match drive.transport {
        Transport::Usb => 35.0,
        Transport::Nvme => 900.0,
        _ => 140.0,
    });

    let mb = drive.capacity as f64 / (1024.0 * 1024.0);
    let secs = (mb / mbps.max(1.0)) * level.pass_multiplier(drive);

    if secs < 90.0 {
        "under 2 minutes".to_string()
    } else if secs < 3600.0 {
        format!("about {} minutes", ((secs / 60.0).round() as u64).max(2))
    } else if secs < 7200.0 {
        "about an hour".to_string()
    } else {
        let hours = secs / 3600.0;
        format!("about {} hours", hours.round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AtaSecurityState, HiddenAreaInfo};
    use std::path::PathBuf;

    fn drive(t: DriveType, tr: Transport, capacity_gb: u64) -> DriveInfo {
        DriveInfo {
            path: PathBuf::from("/dev/sda"),
            model: "Test".into(),
            serial: "S".into(),
            firmware_rev: "1".into(),
            capacity: capacity_gb * 1024 * 1024 * 1024,
            block_size: 512,
            physical_block_size: None,
            drive_type: t,
            transport: tr,
            is_boot_drive: false,
            is_removable: false,
            ata_security: AtaSecurityState::NotSupported,
            hidden_areas: HiddenAreaInfo::default(),
            supports_trim: false,
            is_sed: false,
            smart_healthy: Some(true),
            partition_table: None,
            partition_count: 0,
        }
    }

    #[test]
    fn standard_is_the_recommended_level() {
        assert!(EraseLevel::Standard.is_recommended());
        assert!(!EraseLevel::Quick.is_recommended());
        assert!(!EraseLevel::Thorough.is_recommended());
        // Standard is listed first so the recommendation is also the default
        // cursor position.
        assert_eq!(EraseLevel::ALL[0], EraseLevel::Standard);
    }

    #[test]
    fn every_level_maps_to_a_method_that_exists() {
        let registry = crate::wipe::WipeMethodRegistry::new();
        for d in [
            drive(DriveType::Hdd, Transport::Sata, 500),
            drive(DriveType::Ssd, Transport::Sata, 500),
            drive(DriveType::Nvme, Transport::Nvme, 500),
            drive(DriveType::Ssd, Transport::Usb, 64),
        ] {
            for level in EraseLevel::ALL {
                let id = level.method_id(&d);
                assert!(
                    registry.get(id).is_some(),
                    "{} maps to '{id}', which is not a registered method",
                    level.title()
                );
            }
        }
    }

    #[test]
    fn flash_media_gets_a_method_that_reaches_the_controller() {
        // Overwriting alone cannot reach an SSD's overprovisioned cells, so the
        // guided "Standard" choice must not silently be a plain overwrite.
        for d in [
            drive(DriveType::Ssd, Transport::Sata, 500),
            drive(DriveType::Nvme, Transport::Nvme, 500),
        ] {
            let id = EraseLevel::Standard.method_id(&d);
            assert!(
                id.starts_with("drivewipe-secure"),
                "flash media should use a hybrid method, got '{id}'"
            );
        }
    }

    #[test]
    fn spinning_disks_use_the_plain_standard() {
        let d = drive(DriveType::Hdd, Transport::Sata, 2000);
        assert_eq!(EraseLevel::Standard.method_id(&d), "nist-800-88-clear");
    }

    #[test]
    fn estimates_are_ordered_and_human() {
        let d = drive(DriveType::Hdd, Transport::Sata, 1000);
        let quick = estimate(EraseLevel::Quick, &d, Some(150.0));
        let thorough = estimate(EraseLevel::Thorough, &d, Some(150.0));
        assert!(quick.contains("hour") || quick.contains("minute"));
        assert!(thorough.contains("hour"));
        // No false precision.
        assert!(!quick.contains('.'));
    }

    #[test]
    fn a_small_usb_stick_reads_as_minutes_not_seconds() {
        let d = drive(DriveType::Ssd, Transport::Usb, 8);
        let s = estimate(EraseLevel::Quick, &d, None);
        assert!(s.contains("minute"), "got {s}");
    }

    #[test]
    fn experience_toggles_both_ways() {
        assert_eq!(Experience::Basic.toggled(), Experience::Expert);
        assert_eq!(Experience::Expert.toggled(), Experience::Basic);
        assert_eq!(Experience::default(), Experience::Basic);
    }
}

//! Hidden drive areas (HPA/DCO) and the transports that reach them.
//!
//! Public entry points exist on every platform; only Linux can actually reach a
//! hidden area, so elsewhere they report that nothing was hidden.

#[cfg(target_os = "linux")]
pub mod dco;
#[cfg(target_os = "linux")]
pub mod dma_io;
#[cfg(target_os = "linux")]
pub mod hpa;
#[cfg(target_os = "linux")]
pub mod kernel_module;

use crate::error::Result;
use crate::types::{DriveInfo, HiddenAreaInfo};

/// What a pre-wipe hidden-area sweep found and did.
#[derive(Debug, Clone, Default)]
pub struct HiddenAreaOutcome {
    /// An HPA was present before the wipe.
    pub hpa_found: bool,
    /// The HPA was successfully removed, exposing the hidden sectors.
    pub hpa_removed: bool,
    /// A DCO was present before the wipe.
    pub dco_found: bool,
    /// The DCO was successfully restored, exposing the factory capacity.
    pub dco_removed: bool,
    /// Capacity in bytes before removal, as reported by the drive.
    pub capacity_before: u64,
    /// Capacity in bytes after removal.
    pub capacity_after: u64,
    /// Human-readable notes for the audit trail and wipe report.
    pub notes: Vec<String>,
}

impl HiddenAreaOutcome {
    /// Whether anything was hidden on this drive to begin with.
    pub fn found_anything(&self) -> bool {
        self.hpa_found || self.dco_found
    }

    /// Whether a hidden area was found but could not be removed, meaning the
    /// wipe cannot reach every sector.
    pub fn has_unremoved_area(&self) -> bool {
        (self.hpa_found && !self.hpa_removed) || (self.dco_found && !self.dco_removed)
    }

    /// How many bytes the removal made newly addressable.
    pub fn bytes_recovered(&self) -> u64 {
        self.capacity_after.saturating_sub(self.capacity_before)
    }
}

/// Clear hidden areas on `drive` and update its capacity to match.
///
/// Call after inspecting the drive but before opening it for I/O — the device
/// handle caches capacity at open time. `enabled == false` still reports what
/// is hidden without removing it.
pub fn prepare_for_wipe(drive: &mut DriveInfo, enabled: bool) -> HiddenAreaOutcome {
    let path = drive.path.display().to_string();

    if !enabled {
        let info = detect(&path);
        let mut outcome = HiddenAreaOutcome {
            hpa_found: info.hpa_enabled,
            dco_found: info.dco_enabled,
            capacity_before: drive.capacity,
            capacity_after: drive.capacity,
            ..Default::default()
        };
        if outcome.found_anything() {
            outcome.notes.push(
                "Hidden areas were detected but removal is disabled; the wipe will not \
                 cover the hidden sectors"
                    .to_string(),
            );
        }
        return outcome;
    }

    let mut outcome = match remove_all(&path) {
        Ok(o) => o,
        Err(e) => {
            let mut o = HiddenAreaOutcome {
                capacity_before: drive.capacity,
                capacity_after: drive.capacity,
                ..Default::default()
            };
            o.notes.push(format!("Hidden-area sweep failed: {e}"));
            return o;
        }
    };

    if outcome.capacity_before == 0 {
        outcome.capacity_before = drive.capacity;
    }

    if outcome.capacity_after > drive.capacity {
        drive.capacity = outcome.capacity_after;
    } else {
        outcome.capacity_after = drive.capacity;
    }

    drive.hidden_areas = detect(&path);
    outcome
}

/// Detect HPA and DCO on a device without modifying anything.
#[cfg(not(target_os = "linux"))]
pub fn detect(_device_path: &str) -> HiddenAreaInfo {
    HiddenAreaInfo::default()
}

/// Remove any HPA and DCO so the whole physical surface becomes addressable.
#[cfg(not(target_os = "linux"))]
pub fn remove_all(_device_path: &str) -> Result<HiddenAreaOutcome> {
    Ok(HiddenAreaOutcome::default())
}

/// Detect HPA and DCO on a device without modifying anything.
#[cfg(target_os = "linux")]
pub fn detect(device_path: &str) -> HiddenAreaInfo {
    let mut info = HiddenAreaInfo::default();

    match hpa::detect_hpa(device_path) {
        Ok(status) => {
            info.hpa_enabled = status.hpa_present;
            if status.hpa_present {
                info.hpa_size = Some(status.hpa_bytes);
            }
            info.hpa_native_max_lba = Some(status.native_max_lba);
            info.hpa_current_max_lba = Some(status.current_max_lba);
        }
        Err(e) => log::debug!("HPA detection unavailable on {device_path}: {e}"),
    }

    match dco::detect_dco(device_path) {
        Ok(status) => {
            info.dco_enabled = status.dco_present;
            if status.dco_present {
                info.dco_size = Some(status.dco_hidden_bytes);
            }
            info.dco_features_restricted = status.restricted_features;
            info.dco_factory_max_lba = Some(status.factory_max_lba);
        }
        Err(e) => log::debug!("DCO detection unavailable on {device_path}: {e}"),
    }

    info
}

/// Remove any HPA and DCO so the whole physical surface becomes addressable.
///
/// Irreversible. A failure on one area does not abort the sweep; the outcome
/// records what was and was not cleared.
#[cfg(target_os = "linux")]
pub fn remove_all(device_path: &str) -> Result<HiddenAreaOutcome> {
    let mut outcome = HiddenAreaOutcome::default();

    // DCO first: it constrains the reported native max that the HPA step reads.
    match dco::detect_dco(device_path) {
        Ok(status) if status.dco_present => {
            outcome.dco_found = true;
            outcome.capacity_before = status.current_max_lba.saturating_mul(512);
            match dco::restore_dco(device_path) {
                Ok(_) => {
                    outcome.dco_removed = true;
                    outcome.notes.push(format!(
                        "DCO removed: {} hidden sectors ({} bytes) restored",
                        status.dco_hidden_sectors, status.dco_hidden_bytes
                    ));
                }
                Err(e) => {
                    outcome
                        .notes
                        .push(format!("DCO present but could not be removed: {e}"));
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::debug!("DCO detection unavailable on {device_path}: {e}"),
    }

    match hpa::detect_hpa(device_path) {
        Ok(status) if status.hpa_present => {
            outcome.hpa_found = true;
            if outcome.capacity_before == 0 {
                outcome.capacity_before = status.current_max_lba.saturating_mul(512);
            }
            match hpa::remove_hpa(device_path) {
                Ok(after) => {
                    outcome.hpa_removed = !after.hpa_present;
                    outcome.capacity_after = after.current_max_lba.saturating_mul(512);
                    if outcome.hpa_removed {
                        outcome.notes.push(format!(
                            "HPA removed: {} hidden sectors ({} bytes) restored",
                            status.hpa_sectors, status.hpa_bytes
                        ));
                    } else {
                        outcome.notes.push(
                            "HPA removal reported success but the area is still present"
                                .to_string(),
                        );
                    }
                }
                Err(e) => {
                    outcome
                        .notes
                        .push(format!("HPA present but could not be removed: {e}"));
                }
            }
        }
        Ok(_) => {}
        Err(e) => log::debug!("HPA detection unavailable on {device_path}: {e}"),
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_reports_nothing_hidden_by_default() {
        let outcome = HiddenAreaOutcome::default();
        assert!(!outcome.found_anything());
        assert!(!outcome.has_unremoved_area());
        assert_eq!(outcome.bytes_recovered(), 0);
    }

    #[test]
    fn an_unremoved_hpa_is_flagged() {
        // The wipe must not be able to claim completeness when sectors were
        // hidden and stayed hidden.
        let outcome = HiddenAreaOutcome {
            hpa_found: true,
            hpa_removed: false,
            ..Default::default()
        };
        assert!(outcome.found_anything());
        assert!(outcome.has_unremoved_area());
    }

    #[test]
    fn a_removed_hpa_is_not_flagged() {
        let outcome = HiddenAreaOutcome {
            hpa_found: true,
            hpa_removed: true,
            capacity_before: 500_000,
            capacity_after: 600_000,
            ..Default::default()
        };
        assert!(outcome.found_anything());
        assert!(!outcome.has_unremoved_area());
        assert_eq!(outcome.bytes_recovered(), 100_000);
    }

    #[test]
    fn bytes_recovered_never_underflows() {
        // Capacity should never shrink, but the report must not panic if a
        // drive reports something odd.
        let outcome = HiddenAreaOutcome {
            capacity_before: 900,
            capacity_after: 100,
            ..Default::default()
        };
        assert_eq!(outcome.bytes_recovered(), 0);
    }
}

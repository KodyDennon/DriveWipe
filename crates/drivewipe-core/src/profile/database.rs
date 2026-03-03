use std::path::Path;

use super::DriveProfile;
use crate::error::Result;

/// Database of drive profiles loaded from TOML files and built-in defaults.
pub struct ProfileDatabase {
    profiles: Vec<DriveProfile>,
}

// Built-in profile definitions compiled into the binary.
const BUILTIN_PROFILES: &[&str] = &[
    include_str!("../../profiles/samsung_evo.toml"),
    include_str!("../../profiles/samsung_pro.toml"),
    include_str!("../../profiles/wd_blue.toml"),
    include_str!("../../profiles/seagate_barracuda.toml"),
    include_str!("../../profiles/crucial_mx.toml"),
    include_str!("../../profiles/intel_ssd.toml"),
    include_str!("../../profiles/kingston.toml"),
    include_str!("../../profiles/generic_hdd.toml"),
    include_str!("../../profiles/generic_ssd.toml"),
    include_str!("../../profiles/generic_nvme.toml"),
];

impl ProfileDatabase {
    /// Load all profiles: built-in first, then user profiles from directory.
    pub fn load(profiles_dir: &Path) -> Result<Self> {
        let mut profiles = Vec::new();

        // Load built-in profiles
        for toml_str in BUILTIN_PROFILES {
            match toml::from_str::<DriveProfile>(toml_str) {
                Ok(profile) => profiles.push(profile),
                Err(e) => {
                    log::warn!("Failed to parse built-in profile: {}", e);
                }
            }
        }

        // Load user profiles from directory
        if profiles_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(profiles_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|ext| ext == "toml") {
                        match std::fs::read_to_string(&path) {
                            Ok(contents) => match toml::from_str::<DriveProfile>(&contents) {
                                Ok(profile) => {
                                    log::info!("Loaded user profile: {}", profile.name);
                                    profiles.push(profile);
                                }
                                Err(e) => {
                                    log::warn!("Failed to parse profile {}: {}", path.display(), e);
                                }
                            },
                            Err(e) => {
                                log::warn!("Failed to read profile {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
        }

        Ok(Self { profiles })
    }

    /// Get all loaded profiles.
    pub fn profiles(&self) -> &[DriveProfile] {
        &self.profiles
    }

    /// Convert into a `ProfileMatcher` for efficient lookups.
    pub fn into_matcher(self) -> super::ProfileMatcher {
        super::ProfileMatcher::new(self.profiles)
    }
}

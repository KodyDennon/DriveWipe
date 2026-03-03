use regex::Regex;

use super::DriveProfile;
use crate::types::DriveInfo;

/// Matches drives against profiles using model string regex patterns.
pub struct ProfileMatcher {
    compiled: Vec<(Vec<Regex>, DriveProfile)>,
}

impl ProfileMatcher {
    pub fn new(profiles: Vec<DriveProfile>) -> Self {
        let compiled = profiles
            .into_iter()
            .map(|p| {
                let regexes: Vec<Regex> = p
                    .model_patterns
                    .iter()
                    .filter_map(|pattern| {
                        Regex::new(pattern)
                            .map_err(|e| {
                                log::warn!(
                                    "Invalid regex pattern '{}' in profile '{}': {}",
                                    pattern,
                                    p.name,
                                    e
                                );
                                e
                            })
                            .ok()
                    })
                    .collect();
                (regexes, p)
            })
            .collect();

        Self { compiled }
    }

    /// Match a drive against all loaded profiles. Returns the first match.
    pub fn match_drive(&self, drive_info: &DriveInfo) -> Option<&DriveProfile> {
        let model = drive_info.model.trim();
        self.compiled.iter().find_map(|(regexes, profile)| {
            if regexes.iter().any(|re| re.is_match(model)) {
                Some(profile)
            } else {
                None
            }
        })
    }
}

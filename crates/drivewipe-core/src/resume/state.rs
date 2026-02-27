use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{DriveWipeError, Result};

/// Serializable snapshot of an in-progress wipe session, persisted to disk
/// so that an interrupted wipe can be resumed from the last checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeState {
    pub session_id: Uuid,
    pub device_path: PathBuf,
    pub device_serial: String,
    pub device_model: String,
    pub device_capacity: u64,
    pub method_id: String,
    pub current_pass: u32,
    pub total_passes: u32,
    pub bytes_written_this_pass: u64,
    pub total_bytes_written: u64,
    pub started_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub verify_after: bool,
}

impl WipeState {
    pub fn new(
        session_id: Uuid,
        device_path: PathBuf,
        device_serial: String,
        device_model: String,
        device_capacity: u64,
        method_id: String,
        total_passes: u32,
        verify_after: bool,
    ) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            device_path,
            device_serial,
            device_model,
            device_capacity,
            method_id,
            current_pass: 1,
            total_passes,
            bytes_written_this_pass: 0,
            total_bytes_written: 0,
            started_at: now,
            last_updated: now,
            verify_after,
        }
    }

    /// Save state to a file in the sessions directory as JSON.
    pub fn save(&self, sessions_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(sessions_dir).map_err(|e| DriveWipeError::Io {
            path: sessions_dir.to_path_buf(),
            source: e,
        })?;

        // Set restrictive permissions on the sessions directory (Unix only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                sessions_dir,
                std::fs::Permissions::from_mode(0o700),
            );
        }

        let path = Self::state_path(sessions_dir, self.session_id);
        let json = serde_json::to_string_pretty(self)?;

        std::fs::write(&path, json).map_err(|e| DriveWipeError::Io {
            path: path.clone(),
            source: e,
        })?;

        // Set restrictive permissions on the state file (Unix only).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(0o600),
            );
        }

        Ok(())
    }

    /// Load state from a file.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path).map_err(|e| DriveWipeError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let state: Self = serde_json::from_str(&contents)?;
        Ok(state)
    }

    /// Find all incomplete sessions in the sessions directory.
    pub fn find_incomplete(sessions_dir: &Path) -> Result<Vec<Self>> {
        let mut results = Vec::new();

        if !sessions_dir.exists() {
            return Ok(results);
        }

        let entries = std::fs::read_dir(sessions_dir).map_err(|e| DriveWipeError::Io {
            path: sessions_dir.to_path_buf(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| DriveWipeError::Io {
                path: sessions_dir.to_path_buf(),
                source: e,
            })?;

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("state") {
                match Self::load(&path) {
                    Ok(state) => results.push(state),
                    Err(e) => {
                        log::warn!("Failed to load state file {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(results)
    }

    /// Find a resumable session for a specific device serial.
    pub fn find_for_device(sessions_dir: &Path, serial: &str) -> Result<Option<Self>> {
        let all = Self::find_incomplete(sessions_dir)?;
        Ok(all.into_iter().find(|s| s.device_serial == serial))
    }

    /// Delete the state file (called on completion).
    pub fn cleanup(&self, sessions_dir: &Path) -> Result<()> {
        let path = Self::state_path(sessions_dir, self.session_id);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| DriveWipeError::Io {
                path: path.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Update the state with new progress.
    pub fn update_progress(
        &mut self,
        pass: u32,
        bytes_written_this_pass: u64,
        total_written: u64,
    ) {
        self.current_pass = pass;
        self.bytes_written_this_pass = bytes_written_this_pass;
        self.total_bytes_written = total_written;
        self.last_updated = Utc::now();
    }

    /// Compute the state file path for a given session.
    pub fn state_path(sessions_dir: &Path, session_id: Uuid) -> PathBuf {
        sessions_dir.join(format!("{session_id}.state"))
    }
}

use std::path::PathBuf;

use serde::Deserialize;

use crate::error::{DriveWipeError, Result};

// ── Main configuration ──────────────────────────────────────────────────

/// Top-level configuration for DriveWipe, loaded from
/// `~/.config/drivewipe/config.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DriveWipeConfig {
    /// Default wipe method id (e.g. "zero", "random", "dod-short", "nist-800-88").
    pub default_method: String,

    /// Maximum number of drives to wipe in parallel.
    pub parallel_drives: usize,

    /// Automatically run a verification pass after each wipe.
    pub auto_verify: bool,

    /// Automatically generate a JSON report after each wipe completes.
    pub auto_report_json: bool,

    /// Directory where resumable session state files are stored.
    pub sessions_dir: PathBuf,

    /// Log level filter (e.g. "info", "debug", "warn").
    pub log_level: String,

    /// User-defined wipe methods.
    #[serde(default)]
    pub custom_methods: Vec<CustomMethodConfig>,

    /// How often (in seconds) session state is persisted to disk for resume
    /// support.
    #[serde(default = "default_state_save_interval")]
    pub state_save_interval_secs: u64,

    /// Optional operator name recorded in reports and session metadata.
    pub operator_name: Option<String>,

    // ── New overhaul fields ──────────────────────────────────────────

    /// Directory containing drive profile TOML files.
    pub profiles_dir: PathBuf,

    /// Whether desktop notifications are enabled.
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,

    /// Whether sleep prevention is enabled during operations.
    #[serde(default = "default_true")]
    pub sleep_prevention_enabled: bool,

    /// Key sequence to unlock keyboard lock mode (e.g. "UNLOCK").
    #[serde(default = "default_keyboard_lock_sequence")]
    pub keyboard_lock_sequence: String,

    /// Automatically run a health check before each wipe.
    #[serde(default)]
    pub auto_health_pre_wipe: bool,

    /// Directory for audit log output.
    pub audit_dir: PathBuf,

    /// Directory for historical performance data.
    pub performance_history_dir: PathBuf,
}

/// A user-defined wipe method declared in the configuration file.
#[derive(Debug, Clone, Deserialize)]
pub struct CustomMethodConfig {
    /// Unique identifier used on the command line (e.g. "my-3pass").
    pub id: String,

    /// Human-readable name shown in UI and reports.
    pub name: String,

    /// Longer description of the method.
    pub description: String,

    /// Ordered list of wipe passes.
    pub passes: Vec<CustomPassConfig>,

    /// Whether to run a verification pass after the last wipe pass.
    #[serde(default)]
    pub verify_after: bool,
}

/// A single pass within a custom wipe method.
#[derive(Debug, Clone, Deserialize)]
pub struct CustomPassConfig {
    /// The fill-pattern kind: `"zero"`, `"one"`, `"random"`, `"constant"`,
    /// or `"repeating"`.
    pub pattern_type: String,

    /// Byte value used when `pattern_type` is `"constant"`.
    pub constant_value: Option<u8>,

    /// Byte sequence used when `pattern_type` is `"repeating"`.
    pub repeating_pattern: Option<Vec<u8>>,
}

// ── Defaults ─────────────────────────────────────────────────────────────

fn default_state_save_interval() -> u64 {
    10
}

fn default_true() -> bool {
    true
}

fn default_keyboard_lock_sequence() -> String {
    "UNLOCK".to_string()
}

/// Return the default sessions directory:
/// `~/.local/share/drivewipe/sessions/`
fn default_sessions_dir() -> PathBuf {
    drivewipe_data_dir().join("sessions")
}

fn default_profiles_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("drivewipe")
        .join("profiles")
}

fn default_audit_dir() -> PathBuf {
    drivewipe_data_dir().join("audit")
}

fn default_performance_history_dir() -> PathBuf {
    drivewipe_data_dir().join("performance")
}

fn drivewipe_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| {
            log::warn!(
                "Could not determine local data directory, falling back to /tmp/drivewipe"
            );
            PathBuf::from("/tmp")
        })
        .join("drivewipe")
}

impl Default for DriveWipeConfig {
    fn default() -> Self {
        Self {
            default_method: "zero".to_string(),
            parallel_drives: 1,
            auto_verify: true,
            auto_report_json: true,
            sessions_dir: default_sessions_dir(),
            log_level: "info".to_string(),
            custom_methods: Vec::new(),
            state_save_interval_secs: default_state_save_interval(),
            operator_name: None,
            profiles_dir: default_profiles_dir(),
            notifications_enabled: true,
            sleep_prevention_enabled: true,
            keyboard_lock_sequence: default_keyboard_lock_sequence(),
            auto_health_pre_wipe: false,
            audit_dir: default_audit_dir(),
            performance_history_dir: default_performance_history_dir(),
        }
    }
}

impl DriveWipeConfig {
    // ── Well-known paths ─────────────────────────────────────────────

    /// Return the canonical configuration file path:
    /// `~/.config/drivewipe/config.toml`
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                log::warn!("Could not determine config directory, falling back to /tmp");
                PathBuf::from("/tmp")
            })
            .join("drivewipe")
            .join("config.toml")
    }

    /// Return the directory used to store resumable session state.
    pub fn sessions_dir(&self) -> &PathBuf {
        &self.sessions_dir
    }

    // ── Loading ──────────────────────────────────────────────────────

    /// Load configuration from `~/.config/drivewipe/config.toml`.
    ///
    /// If the file does not exist, sensible defaults are returned.  If the
    /// file exists but cannot be parsed, a [`DriveWipeError::ConfigParse`]
    /// error is returned.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if !path.exists() {
            log::debug!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| DriveWipeError::Io {
            path: path.clone(),
            source: e,
        })?;

        let config: DriveWipeConfig =
            toml::from_str(&contents).map_err(|e| DriveWipeError::ConfigParse {
                path: path.clone(),
                source: e,
            })?;

        log::info!("Loaded configuration from {}", path.display());
        Ok(config)
    }
}

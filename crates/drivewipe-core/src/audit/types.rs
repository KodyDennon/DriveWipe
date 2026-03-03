use serde::{Deserialize, Serialize};

/// Category of an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditCategory {
    Wipe,
    Clone,
    Partition,
    Forensic,
    Health,
    Config,
    KeyboardLock,
}

/// Severity level of an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

use drivewipe_core::audit::{AuditEvent, AuditLogger};

#[test]
fn test_audit_logger_creates_entries() {
    let dir = tempfile::tempdir().unwrap();
    let mut logger = AuditLogger::new(dir.path().to_path_buf(), None);

    logger
        .log(
            AuditEvent::WipeStarted {
                device: "/dev/sda".to_string(),
                method: "zero-fill".to_string(),
            },
            Some("/dev/sda"),
            Some("SERIAL1"),
            None,
        )
        .unwrap();

    logger
        .log(
            AuditEvent::WipeCompleted {
                outcome: "Success".to_string(),
                duration_secs: 42.5,
            },
            Some("/dev/sda"),
            Some("SERIAL1"),
            None,
        )
        .unwrap();

    // Read today's entries
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let entries = AuditLogger::read_entries(dir.path(), &date).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(matches!(entries[0].event, AuditEvent::WipeStarted { .. }));
    assert!(matches!(entries[1].event, AuditEvent::WipeCompleted { .. }));
}

#[test]
fn test_audit_entry_serialization() {
    let event = AuditEvent::WipeStarted {
        device: "/dev/sda".to_string(),
        method: "dod-3pass".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: AuditEvent = serde_json::from_str(&json).unwrap();

    match deserialized {
        AuditEvent::WipeStarted { device, method } => {
            assert_eq!(device, "/dev/sda");
            assert_eq!(method, "dod-3pass");
        }
        _ => panic!("Unexpected variant"),
    }
}

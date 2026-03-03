use drivewipe_core::health::snapshot::DriveHealthSnapshot;

#[test]
fn test_health_snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.json");

    let snapshot = DriveHealthSnapshot {
        device_path: "/dev/sda".to_string(),
        device_model: "Test SSD 500GB".to_string(),
        device_serial: "SERIAL123".to_string(),
        timestamp: chrono::Utc::now(),
        smart_data: None,
        nvme_health: None,
        temperature_celsius: Some(42),
        benchmark: None,
    };

    snapshot.save(&path).unwrap();
    let loaded = DriveHealthSnapshot::load(&path).unwrap();

    assert_eq!(loaded.device_path, "/dev/sda");
    assert_eq!(loaded.device_model, "Test SSD 500GB");
    assert_eq!(loaded.device_serial, "SERIAL123");
    assert_eq!(loaded.temperature_celsius, Some(42));
}

#[test]
fn test_health_diff_identical_snapshots() {
    use drivewipe_core::health::diff::{HealthDiff, HealthVerdict};

    let snap1 = DriveHealthSnapshot {
        device_path: "/dev/sda".to_string(),
        device_model: "Test SSD".to_string(),
        device_serial: "SN001".to_string(),
        timestamp: chrono::Utc::now(),
        smart_data: None,
        nvme_health: None,
        temperature_celsius: None,
        benchmark: None,
    };

    let snap2 = snap1.clone();
    let comparison = HealthDiff::compare(&snap1, &snap2);
    assert_eq!(comparison.verdict, HealthVerdict::Pass);
}

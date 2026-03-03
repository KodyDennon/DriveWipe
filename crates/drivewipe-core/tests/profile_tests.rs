use std::path::PathBuf;

use drivewipe_core::profile::database::ProfileDatabase;
use drivewipe_core::profile::matcher::ProfileMatcher;
use drivewipe_core::types::*;

fn test_drive(model: &str) -> DriveInfo {
    DriveInfo {
        path: PathBuf::from("/dev/test0"),
        model: model.to_string(),
        serial: "TEST-001".to_string(),
        firmware_rev: "1.0".to_string(),
        capacity: 500_000_000_000,
        block_size: 512,
        physical_block_size: None,
        drive_type: DriveType::Ssd,
        transport: Transport::Sata,
        is_boot_drive: false,
        is_removable: false,
        ata_security: AtaSecurityState::NotSupported,
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim: true,
        is_sed: false,
        smart_healthy: Some(true),
        partition_table: None,
        partition_count: 0,
    }
}

#[test]
fn test_profile_database_loads_builtins() {
    // Use a non-existent path for user profiles (only builtins)
    let db = ProfileDatabase::load(&PathBuf::from("/tmp/nonexistent_profiles_dir")).unwrap();
    let profiles = db.profiles();
    assert!(
        profiles.len() >= 8,
        "Expected at least 8 built-in profiles, got {}",
        profiles.len()
    );
}

#[test]
fn test_profile_matcher_finds_samsung() {
    let db = ProfileDatabase::load(&PathBuf::from("/tmp/nonexistent_profiles_dir")).unwrap();
    let matcher = ProfileMatcher::new(db.profiles().to_vec());

    let drive = test_drive("Samsung SSD 860 EVO 500GB");
    let result = matcher.match_drive(&drive);
    assert!(result.is_some(), "Should match Samsung EVO profile");
    let profile = result.unwrap();
    assert_eq!(profile.manufacturer, "Samsung");
}

#[test]
fn test_profile_matcher_unknown_model() {
    let db = ProfileDatabase::load(&PathBuf::from("/tmp/nonexistent_profiles_dir")).unwrap();
    let matcher = ProfileMatcher::new(db.profiles().to_vec());

    let drive = test_drive("Unknown Brand XYZ 1TB");
    // Unknown model may or may not match a generic profile
    let _ = matcher.match_drive(&drive);
}

#[test]
fn test_profile_matcher_nvme() {
    let db = ProfileDatabase::load(&PathBuf::from("/tmp/nonexistent_profiles_dir")).unwrap();
    let matcher = ProfileMatcher::new(db.profiles().to_vec());

    let drive = test_drive("Samsung SSD 970 EVO Plus 1TB");
    let result = matcher.match_drive(&drive);
    assert!(result.is_some());
}

use drivewipe_core::types::*;
use std::path::PathBuf;

#[test]
fn format_bytes_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1023), "1023 B");
}

#[test]
fn format_bytes_kib() {
    assert_eq!(format_bytes(1024), "1.00 KiB");
    assert_eq!(format_bytes(2048), "2.00 KiB");
}

#[test]
fn format_bytes_mib() {
    assert_eq!(format_bytes(1024 * 1024), "1.00 MiB");
    assert_eq!(format_bytes(500 * 1024 * 1024), "500.00 MiB");
}

#[test]
fn format_bytes_gib() {
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GiB");
}

#[test]
fn format_bytes_tib() {
    assert_eq!(format_bytes(1024u64 * 1024 * 1024 * 1024), "1.00 TiB");
}

#[test]
fn format_throughput_mib_range() {
    let result = format_throughput(100.0 * 1024.0 * 1024.0);
    assert!(result.contains("MiB/s"));
}

#[test]
fn format_throughput_gib_range() {
    let result = format_throughput(2000.0 * 1024.0 * 1024.0);
    assert!(result.contains("GiB/s"));
}

#[test]
fn transport_display() {
    assert_eq!(format!("{}", Transport::Sata), "SATA");
    assert_eq!(format!("{}", Transport::Nvme), "NVMe");
    assert_eq!(format!("{}", Transport::Usb), "USB");
    assert_eq!(format!("{}", Transport::Scsi), "SCSI");
    assert_eq!(format!("{}", Transport::Sas), "SAS");
    assert_eq!(format!("{}", Transport::Unknown), "Unknown");
}

#[test]
fn drive_type_display() {
    assert_eq!(format!("{}", DriveType::Hdd), "HDD");
    assert_eq!(format!("{}", DriveType::Ssd), "SSD");
    assert_eq!(format!("{}", DriveType::Nvme), "NVMe");
    assert_eq!(format!("{}", DriveType::Unknown), "Unknown");
}

#[test]
fn ata_security_display() {
    assert_eq!(format!("{}", AtaSecurityState::Frozen), "Frozen");
    assert_eq!(format!("{}", AtaSecurityState::Locked), "Locked");
    assert_eq!(format!("{}", AtaSecurityState::Disabled), "Disabled");
}

#[test]
fn wipe_outcome_display() {
    assert_eq!(format!("{}", WipeOutcome::Success), "Success");
    assert_eq!(format!("{}", WipeOutcome::Cancelled), "Cancelled");
    assert_eq!(format!("{}", WipeOutcome::Interrupted), "Interrupted");
    assert_eq!(format!("{}", WipeOutcome::Failed), "Failed");
    assert_eq!(
        format!("{}", WipeOutcome::SuccessWithWarnings),
        "Success (with warnings)"
    );
}

#[test]
fn drive_info_suggested_method_nvme() {
    let drive = make_drive(DriveType::Nvme, Transport::Nvme);
    assert_eq!(drive.suggested_method(), "nvme-format-crypto");
}

#[test]
fn drive_info_suggested_method_ssd_sata() {
    let drive = make_drive(DriveType::Ssd, Transport::Sata);
    assert_eq!(drive.suggested_method(), "ata-erase-enhanced");
}

#[test]
fn drive_info_suggested_method_ssd_usb() {
    let drive = make_drive(DriveType::Ssd, Transport::Usb);
    assert_eq!(drive.suggested_method(), "dod-short");
}

#[test]
fn drive_info_suggested_method_hdd() {
    let drive = make_drive(DriveType::Hdd, Transport::Sata);
    assert_eq!(drive.suggested_method(), "dod-short");
}

#[test]
fn drive_info_firmware_erase_supported() {
    let nvme = make_drive(DriveType::Nvme, Transport::Nvme);
    assert!(nvme.firmware_erase_likely_supported());

    let usb = make_drive(DriveType::Ssd, Transport::Usb);
    assert!(!usb.firmware_erase_likely_supported());
}

#[test]
fn drive_info_capacity_display() {
    let mut d = make_drive(DriveType::Hdd, Transport::Sata);
    d.capacity = 500_000_000_000;
    assert!(d.capacity_display().contains("GiB"));
}

#[test]
fn wipe_result_serialization_roundtrip() {
    let result = make_wipe_result();
    let json = serde_json::to_string(&result).unwrap();
    let parsed: WipeResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.session_id, result.session_id);
    assert_eq!(parsed.outcome, WipeOutcome::Success);
    assert_eq!(parsed.method_id, "zero");
}

#[test]
fn drive_info_serialization_roundtrip() {
    let drive = make_drive(DriveType::Ssd, Transport::Nvme);
    let json = serde_json::to_string(&drive).unwrap();
    let parsed: DriveInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.serial, drive.serial);
    assert_eq!(parsed.drive_type, DriveType::Ssd);
    assert_eq!(parsed.transport, Transport::Nvme);
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn make_drive(drive_type: DriveType, transport: Transport) -> DriveInfo {
    DriveInfo {
        path: PathBuf::from("/dev/sda"),
        model: "Test Model".to_string(),
        serial: "TEST-SERIAL-001".to_string(),
        firmware_rev: "1.0".to_string(),
        capacity: 500_000_000_000,
        block_size: 512,
        physical_block_size: None,
        drive_type,
        transport,
        is_boot_drive: false,
        is_removable: false,
        ata_security: AtaSecurityState::NotSupported,
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim: false,
        is_sed: false,
        smart_healthy: Some(true),
        partition_table: Some("gpt".to_string()),
        partition_count: 2,
    }
}

fn make_wipe_result() -> WipeResult {
    WipeResult {
        session_id: uuid::Uuid::new_v4(),
        device_path: PathBuf::from("/dev/sda"),
        device_serial: "TEST-SERIAL".to_string(),
        device_model: "Test Model".to_string(),
        device_capacity: 1_000_000_000,
        method_id: "zero".to_string(),
        method_name: "Zero Fill".to_string(),
        outcome: WipeOutcome::Success,
        passes: vec![PassResult {
            pass_number: 1,
            pattern_name: "ZeroFill".to_string(),
            bytes_written: 1_000_000_000,
            duration_secs: 10.0,
            throughput_mbps: 100.0,
            verified: false,
            verification_passed: None,
        }],
        total_bytes_written: 1_000_000_000,
        total_duration_secs: 10.0,
        average_throughput_mbps: 100.0,
        verification_passed: None,
        started_at: chrono::Utc::now(),
        completed_at: chrono::Utc::now(),
        hostname: "test-host".to_string(),
        operator: None,
        warnings: vec![],
        errors: vec![],
    }
}

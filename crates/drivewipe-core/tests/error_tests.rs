use std::path::PathBuf;

use drivewipe_core::error::DriveWipeError;

#[test]
fn device_not_found_display() {
    let err = DriveWipeError::DeviceNotFound(PathBuf::from("/dev/sda"));
    let msg = format!("{err}");
    assert!(msg.contains("Device not found"));
    assert!(msg.contains("/dev/sda"));
}

#[test]
fn boot_drive_refused_display() {
    let err = DriveWipeError::BootDriveRefused(PathBuf::from("/dev/sda"));
    let msg = format!("{err}");
    assert!(msg.contains("boot drive"));
}

#[test]
fn cancelled_display() {
    let err = DriveWipeError::Cancelled;
    assert_eq!(format!("{err}"), "Wipe cancelled by user");
}

#[test]
fn unknown_method_display() {
    let err = DriveWipeError::UnknownMethod("foo".to_string());
    assert!(format!("{err}").contains("foo"));
}

#[test]
fn verification_failed_display() {
    let err = DriveWipeError::VerificationFailed {
        offset: 0x1000,
        expected: 0x00,
        actual: 0xFF,
    };
    let msg = format!("{err}");
    assert!(msg.contains("0x1000"));
    assert!(msg.contains("0x00"));
    assert!(msg.contains("0xff"));
}

#[test]
fn firmware_not_supported_display() {
    let err = DriveWipeError::FirmwareNotSupported {
        reason: "USB bridge".to_string(),
    };
    assert!(format!("{err}").contains("USB bridge"));
}

#[test]
fn ata_security_frozen_display() {
    let err = DriveWipeError::AtaSecurityFrozen;
    assert!(format!("{err}").contains("frozen"));
}

#[test]
fn json_error_from_serde() {
    let bad_json = "not json";
    let result: Result<serde_json::Value, _> = serde_json::from_str(bad_json);
    let serde_err = result.unwrap_err();
    let dw_err: DriveWipeError = serde_err.into();
    assert!(format!("{dw_err}").contains("JSON"));
}

#[test]
fn device_error_display() {
    let err = DriveWipeError::DeviceError("not a block device".to_string());
    assert!(format!("{err}").contains("not a block device"));
}

#[test]
fn platform_not_supported_display() {
    let err = DriveWipeError::PlatformNotSupported("NVMe on macOS".to_string());
    assert!(format!("{err}").contains("NVMe on macOS"));
}

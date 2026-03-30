use drivewipe_core::wipe::WipeMethod;
use drivewipe_core::wipe::drivewipe_secure::*;

#[test]
fn test_secure_hdd_4_passes() {
    let m = DriveWipeSecureHdd;
    assert_eq!(m.pass_count(), 4);
    assert!(m.includes_verification());
}

#[test]
fn test_secure_sata_ssd_4_passes() {
    let m = DriveWipeSecureSataSsd;
    assert_eq!(m.pass_count(), 4);
    assert!(m.includes_verification());
}

#[test]
fn test_secure_nvme_4_passes() {
    let m = DriveWipeSecureNvme;
    assert_eq!(m.pass_count(), 4);
    assert!(m.includes_verification());
}

#[test]
fn test_secure_usb_4_passes() {
    let m = DriveWipeSecureUsb;
    assert_eq!(m.pass_count(), 4);
    assert!(m.includes_verification());
}

#[test]
fn test_secure_hdd_final_pass_is_zero() {
    let m = DriveWipeSecureHdd;
    let p = m.pattern_for_pass(3);
    assert!(
        p.name().contains("Zero"),
        "Final HDD pass should be zero for clean verification, got: {}",
        p.name()
    );
}

#[test]
fn test_secure_hdd_patterns() {
    let m = DriveWipeSecureHdd;
    assert!(m.pattern_for_pass(0).name().contains("Zero"));
    assert!(m.pattern_for_pass(1).name().contains("Random"));
    assert!(m.pattern_for_pass(2).name().contains("Random"));
    assert!(m.pattern_for_pass(3).name().contains("Zero"));
}

#[test]
fn test_secure_sata_patterns() {
    let m = DriveWipeSecureSataSsd;
    assert!(m.pattern_for_pass(0).name().contains("Random"));
    assert!(m.pattern_for_pass(1).name().contains("Zero"));
    assert!(m.pattern_for_pass(2).name().contains("Random"));
    assert!(m.pattern_for_pass(3).name().contains("Zero"));
}

#[test]
fn test_secure_nvme_patterns() {
    let m = DriveWipeSecureNvme;
    assert!(m.pattern_for_pass(0).name().contains("Random"));
    assert!(m.pattern_for_pass(1).name().contains("Zero"));
    assert!(m.pattern_for_pass(2).name().contains("Random"));
    assert!(m.pattern_for_pass(3).name().contains("Zero"));
}

#[test]
fn test_secure_usb_patterns() {
    let m = DriveWipeSecureUsb;
    assert!(m.pattern_for_pass(0).name().contains("Random"));
    assert!(m.pattern_for_pass(1).name().contains("Zero"));
    assert!(m.pattern_for_pass(2).name().contains("Random"));
    assert!(m.pattern_for_pass(3).name().contains("Zero"));
}

#[test]
fn test_secure_methods_are_not_firmware() {
    assert!(!DriveWipeSecureHdd.is_firmware());
    assert!(!DriveWipeSecureSataSsd.is_firmware());
    assert!(!DriveWipeSecureNvme.is_firmware());
    assert!(!DriveWipeSecureUsb.is_firmware());
}

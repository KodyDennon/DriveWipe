use drivewipe_core::wipe::WipeMethodRegistry;

#[test]
fn registry_contains_software_methods() {
    let reg = WipeMethodRegistry::new();
    assert!(reg.get("zero").is_some());
    assert!(reg.get("one").is_some());
    assert!(reg.get("random").is_some());
    assert!(reg.get("dod-short").is_some());
    assert!(reg.get("dod-ece").is_some());
    assert!(reg.get("gutmann").is_some());
    assert!(reg.get("hmg-baseline").is_some());
    assert!(reg.get("hmg-enhanced").is_some());
    assert!(reg.get("rcmp").is_some());
}

#[test]
fn registry_contains_firmware_methods() {
    let reg = WipeMethodRegistry::new();
    assert!(reg.get("ata-erase").is_some());
    assert!(reg.get("ata-erase-enhanced").is_some());
    assert!(reg.get("nvme-format-user").is_some());
    assert!(reg.get("nvme-format-crypto").is_some());
    assert!(reg.get("nvme-sanitize-block").is_some());
    assert!(reg.get("nvme-sanitize-crypto").is_some());
    assert!(reg.get("nvme-sanitize-overwrite").is_some());
    assert!(reg.get("tcg-opal").is_some());
}

#[test]
fn registry_total_count() {
    let reg = WipeMethodRegistry::new();
    // 9 software + 8 firmware = 17
    assert_eq!(reg.list().len(), 17);
}

#[test]
fn registry_nonexistent_returns_none() {
    let reg = WipeMethodRegistry::new();
    assert!(reg.get("nonexistent-method").is_none());
}

#[test]
fn registry_firmware_methods_are_firmware() {
    let reg = WipeMethodRegistry::new();
    let firmware_ids = [
        "ata-erase",
        "ata-erase-enhanced",
        "nvme-format-user",
        "nvme-format-crypto",
        "nvme-sanitize-block",
        "nvme-sanitize-crypto",
        "nvme-sanitize-overwrite",
        "tcg-opal",
    ];
    for id in firmware_ids {
        let m = reg.get(id).unwrap();
        assert!(m.is_firmware(), "{id} should be firmware");
    }
}

#[test]
fn registry_register_custom_method() {
    use drivewipe_core::config::{CustomMethodConfig, CustomPassConfig};
    use drivewipe_core::wipe::custom::CustomWipeMethod;

    let mut reg = WipeMethodRegistry::new();
    let custom = CustomWipeMethod::from_config(CustomMethodConfig {
        id: "test-custom".to_string(),
        name: "Test Custom".to_string(),
        description: "A test method".to_string(),
        passes: vec![CustomPassConfig {
            pattern_type: "zero".to_string(),
            constant_value: None,
            repeating_pattern: None,
        }],
        verify_after: true,
    });
    reg.register(Box::new(custom));

    let m = reg.get("test-custom").unwrap();
    assert_eq!(m.name(), "Test Custom");
    assert_eq!(m.pass_count(), 1);
    assert!(m.includes_verification());
}

#[test]
fn registry_default_is_same_as_new() {
    let new_reg = WipeMethodRegistry::new();
    let default_reg = WipeMethodRegistry::default();
    assert_eq!(new_reg.list().len(), default_reg.list().len());
}

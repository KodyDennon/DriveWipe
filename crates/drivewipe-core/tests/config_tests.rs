use drivewipe_core::config::DriveWipeConfig;

#[test]
fn default_config_has_sensible_values() {
    let config = DriveWipeConfig::default();
    assert_eq!(config.default_method, "zero");
    assert_eq!(config.parallel_drives, 1);
    assert!(config.auto_verify);
    assert!(config.auto_report_json);
    assert_eq!(config.log_level, "info");
    assert!(config.custom_methods.is_empty());
    assert_eq!(config.state_save_interval_secs, 10);
    assert!(config.operator_name.is_none());
}

#[test]
fn default_sessions_dir_exists_in_path() {
    let config = DriveWipeConfig::default();
    let path = config.sessions_dir();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("drivewipe") && path_str.contains("sessions"));
}

#[test]
fn config_path_contains_drivewipe() {
    let path = DriveWipeConfig::config_path();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("drivewipe"));
    assert!(path_str.ends_with("config.toml"));
}

#[test]
fn config_toml_parse_minimal() {
    let toml_str = r#"
default_method = "dod-short"
parallel_drives = 4
auto_verify = false
"#;
    let config: DriveWipeConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.default_method, "dod-short");
    assert_eq!(config.parallel_drives, 4);
    assert!(!config.auto_verify);
    // Defaults still apply for unspecified fields.
    assert!(config.auto_report_json);
}

#[test]
fn config_toml_parse_custom_methods() {
    let toml_str = r#"
[[custom_methods]]
id = "my-3pass"
name = "My Custom 3-Pass"
description = "A test custom method"
verify_after = true

[[custom_methods.passes]]
pattern_type = "zero"

[[custom_methods.passes]]
pattern_type = "one"

[[custom_methods.passes]]
pattern_type = "random"
"#;
    let config: DriveWipeConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.custom_methods.len(), 1);
    let m = &config.custom_methods[0];
    assert_eq!(m.id, "my-3pass");
    assert_eq!(m.passes.len(), 3);
    assert!(m.verify_after);
}

#[test]
fn config_toml_parse_with_operator() {
    let toml_str = r#"
operator_name = "John Doe"
"#;
    let config: DriveWipeConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.operator_name.as_deref(), Some("John Doe"));
}

#[test]
fn config_toml_parse_empty_is_default() {
    let config: DriveWipeConfig = toml::from_str("").unwrap();
    let default_config = DriveWipeConfig::default();
    assert_eq!(config.default_method, default_config.default_method);
    assert_eq!(config.parallel_drives, default_config.parallel_drives);
}

use std::path::PathBuf;

use drivewipe_core::report::data::WipeReport;
use drivewipe_core::report::json::JsonReportGenerator;
use drivewipe_core::report::ReportGenerator;
use drivewipe_core::types::*;

#[test]
fn json_report_roundtrip() {
    let result = make_wipe_result();
    let report = WipeReport::from_result(result.clone());

    let json = report.to_json().unwrap();
    let parsed = WipeReport::from_json(&json).unwrap();

    assert_eq!(parsed.version, "1.0");
    assert_eq!(parsed.result.session_id, result.session_id);
    assert_eq!(parsed.result.outcome, WipeOutcome::Success);
    assert_eq!(parsed.result.method_id, "zero");
    assert_eq!(parsed.result.passes.len(), 1);
}

#[test]
fn json_generator_produces_valid_json() {
    let result = make_wipe_result();
    let generator = JsonReportGenerator;
    let bytes = generator.generate(&result).unwrap();
    let json_str = std::str::from_utf8(&bytes).unwrap();

    // Should be valid JSON.
    let _: serde_json::Value = serde_json::from_str(json_str).unwrap();
}

#[test]
fn json_generator_file_extension() {
    let generator = JsonReportGenerator;
    assert_eq!(generator.file_extension(), "json");
}

#[test]
fn report_contains_all_fields() {
    let result = make_wipe_result();
    let generator = JsonReportGenerator;
    let bytes = generator.generate(&result).unwrap();
    let json_str = std::str::from_utf8(&bytes).unwrap();

    // Check for expected fields in the JSON output.
    assert!(json_str.contains("session_id"));
    assert!(json_str.contains("device_path"));
    assert!(json_str.contains("method_id"));
    assert!(json_str.contains("outcome"));
    assert!(json_str.contains("passes"));
    assert!(json_str.contains("total_bytes_written"));
    assert!(json_str.contains("version"));
    assert!(json_str.contains("generated_at"));
}

#[test]
fn report_with_warnings() {
    let mut result = make_wipe_result();
    result.outcome = WipeOutcome::SuccessWithWarnings;
    result.warnings = vec!["SSD detected".to_string(), "USB bridge".to_string()];

    let report = WipeReport::from_result(result);
    let json = report.to_json().unwrap();
    let parsed = WipeReport::from_json(&json).unwrap();

    assert_eq!(parsed.result.outcome, WipeOutcome::SuccessWithWarnings);
    assert_eq!(parsed.result.warnings.len(), 2);
}

#[test]
fn report_with_operator() {
    let mut result = make_wipe_result();
    result.operator = Some("Jane Doe".to_string());

    let report = WipeReport::from_result(result);
    let json = report.to_json().unwrap();
    let parsed = WipeReport::from_json(&json).unwrap();

    assert_eq!(parsed.result.operator.as_deref(), Some("Jane Doe"));
}

// ── Helper ──────────────────────────────────────────────────────────

fn make_wipe_result() -> WipeResult {
    WipeResult {
        session_id: uuid::Uuid::new_v4(),
        device_path: PathBuf::from("/dev/sda"),
        device_serial: "SER-001".to_string(),
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
            throughput_mbps: 95.37,
            verified: false,
            verification_passed: None,
        }],
        total_bytes_written: 1_000_000_000,
        total_duration_secs: 10.0,
        average_throughput_mbps: 95.37,
        verification_passed: None,
        started_at: chrono::Utc::now(),
        completed_at: chrono::Utc::now(),
        hostname: "testhost".to_string(),
        operator: None,
        warnings: vec![],
        errors: vec![],
    }
}

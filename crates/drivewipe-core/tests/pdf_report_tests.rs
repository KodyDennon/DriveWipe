#![cfg(feature = "pdf-report")]

use std::path::PathBuf;

use chrono::Utc;
use drivewipe_core::report::ReportGenerator;
use drivewipe_core::report::pdf::PdfReportGenerator;
use drivewipe_core::types::*;
use uuid::Uuid;

fn sample_result(passes: usize, warnings: Vec<String>) -> WipeResult {
    WipeResult {
        session_id: Uuid::new_v4(),
        device_path: PathBuf::from("/dev/sda"),
        device_serial: "SN-TEST-0001".to_string(),
        device_model: "ACME SuperDisk 2TB".to_string(),
        device_capacity: 2_000_398_934_016,
        method_id: "dod-short".to_string(),
        method_name: "DoD 5220.22-M (3-pass)".to_string(),
        outcome: WipeOutcome::Success,
        passes: (1..=passes)
            .map(|i| PassResult {
                pass_number: i as u32,
                pattern_name: "ZeroFill (0x00)".to_string(),
                bytes_written: 2_000_398_934_016,
                duration_secs: 1234.5,
                throughput_mbps: 180.2,
                verified: true,
                verification_passed: Some(true),
            })
            .collect(),
        total_bytes_written: 2_000_398_934_016,
        total_duration_secs: 3703.5,
        average_throughput_mbps: 180.2,
        verification_passed: Some(true),
        started_at: Utc::now(),
        completed_at: Utc::now(),
        hostname: "wipe-station-01".to_string(),
        operator: Some("A. Technician".to_string()),
        warnings,
        errors: vec![],
    }
}

/// A PDF must begin with the %PDF- header and end with the EOF marker;
/// anything else will not open in a reader.
fn assert_structurally_valid(bytes: &[u8]) {
    assert!(
        bytes.starts_with(b"%PDF-"),
        "missing %PDF- header, got: {:?}",
        &bytes[..bytes.len().min(16)]
    );

    let tail = &bytes[bytes.len().saturating_sub(1024)..];
    let tail = String::from_utf8_lossy(tail);
    assert!(tail.contains("%%EOF"), "missing %%EOF trailer");
    assert!(tail.contains("startxref"), "missing startxref");

    // The cross-reference offset must point inside the file.
    let idx = tail.rfind("startxref").expect("startxref present");
    let offset: usize = tail[idx + "startxref".len()..]
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .expect("startxref followed by an offset");
    assert!(
        offset < bytes.len(),
        "startxref points past the end of the file ({offset} >= {})",
        bytes.len()
    );
}

#[test]
fn certificate_is_a_structurally_valid_pdf() {
    let bytes = PdfReportGenerator
        .generate(&sample_result(3, vec![]))
        .expect("certificate should generate");
    assert_structurally_valid(&bytes);
    assert!(bytes.len() > 500, "suspiciously small PDF: {}", bytes.len());
}

#[test]
fn certificate_generates_without_any_font_files_present() {
    // The previous implementation searched four Linux font directories and
    // failed outright when none contained LiberationSans, so PDF reports could
    // not be produced on macOS or Windows. Base-14 fonts need no files at all.
    let bytes = PdfReportGenerator
        .generate(&sample_result(1, vec![]))
        .expect("must not depend on system font files");
    assert_structurally_valid(&bytes);
}

#[test]
fn long_warnings_do_not_break_generation() {
    let warnings = vec![
        "HPA present but could not be removed: the drive rejected SET MAX ADDRESS, so \
         the wipe did not reach the hidden sectors and this is not a complete sanitisation"
            .to_string(),
        "Pass 2 verification mismatch at offset 0x4c000: expected 0xff, got 0x00".to_string(),
    ];
    let bytes = PdfReportGenerator
        .generate(&sample_result(3, warnings))
        .expect("warnings should render");
    assert_structurally_valid(&bytes);
}

#[test]
fn a_long_run_spills_onto_further_pages() {
    // Gutmann produces 35 passes, which cannot fit on one page.
    let short = PdfReportGenerator
        .generate(&sample_result(1, vec![]))
        .unwrap();
    let long = PdfReportGenerator
        .generate(&sample_result(35, vec![]))
        .unwrap();

    assert_structurally_valid(&long);
    assert!(
        long.len() > short.len(),
        "a 35-pass certificate should be larger than a 1-pass one"
    );
}

#[test]
fn a_firmware_erase_with_no_passes_still_produces_a_certificate() {
    let bytes = PdfReportGenerator
        .generate(&sample_result(0, vec![]))
        .expect("firmware erases have no software passes");
    assert_structurally_valid(&bytes);
}

/// Render the certificate with an external PDF reader and return its text.
///
/// Returns `None` when `pdftotext` is unavailable, so the test degrades to a
/// skip on machines without poppler rather than failing.
fn extract_text(bytes: &[u8]) -> Option<String> {
    use std::process::Command;

    if Command::new("pdftotext").arg("-v").output().is_err() {
        return None;
    }

    let dir = tempfile::tempdir().ok()?;
    let pdf = dir.path().join("certificate.pdf");
    std::fs::write(&pdf, bytes).ok()?;

    let out = Command::new("pdftotext").arg(&pdf).arg("-").output().ok()?;
    if !out.status.success() {
        panic!(
            "pdftotext refused the certificate: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn a_real_pdf_reader_can_extract_the_certificate_contents() {
    let result = sample_result(3, vec!["HPA present but could not be removed".to_string()]);
    let bytes = PdfReportGenerator.generate(&result).unwrap();

    let Some(text) = extract_text(&bytes) else {
        eprintln!("skipping: pdftotext not installed");
        return;
    };

    // Every field an auditor would look for must survive rendering.
    for expected in [
        "DATA SANITIZATION CERTIFICATE",
        "ACME SuperDisk 2TB",
        "SN-TEST-0001",
        "DoD 5220.22-M (3-pass)",
        "wipe-station-01",
        "A. Technician",
        "PASSED",
        "WARNINGS",
        "HPA present but could not be removed",
    ] {
        assert!(
            text.contains(expected),
            "certificate text is missing {expected:?}.\nExtracted:\n{text}"
        );
    }

    assert!(
        text.contains(&result.session_id.to_string()),
        "certificate must carry its session id"
    );
}

#[test]
fn a_failed_wipe_says_so_on_the_certificate() {
    // A certificate that does not clearly state failure is worse than none.
    let mut result = sample_result(3, vec![]);
    result.outcome = WipeOutcome::Failed;
    result.verification_passed = Some(false);
    result.passes[1].verification_passed = Some(false);

    let bytes = PdfReportGenerator.generate(&result).unwrap();
    let Some(text) = extract_text(&bytes) else {
        eprintln!("skipping: pdftotext not installed");
        return;
    };

    assert!(text.contains("FAILED"), "outcome must be legible:\n{text}");
    assert!(
        text.contains("VERIFICATION FAILED"),
        "the failing pass must be identified:\n{text}"
    );
}

#[test]
#[ignore = "writes a sample certificate for manual inspection"]
fn dump_sample_certificate() {
    let result = sample_result(
        3,
        vec![
            "HPA present but could not be removed: the drive rejected SET MAX ADDRESS".to_string(),
        ],
    );
    let bytes = PdfReportGenerator.generate(&result).unwrap();
    let path = std::env::var("DW_PDF_OUT").unwrap_or_else(|_| "/tmp/certificate.pdf".to_string());
    std::fs::write(&path, bytes).unwrap();
    eprintln!("wrote {path}");
}

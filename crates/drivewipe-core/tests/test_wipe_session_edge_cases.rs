mod common;

use common::{MockDevice, test_drive_info, test_hdd_info};
use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::session::{CancellationToken, WipeSession};
use drivewipe_core::types::*;
use drivewipe_core::wipe::WipeMethod;
use drivewipe_core::wipe::software::{DodShortMethod, ZeroFillMethod};

#[tokio::test]
async fn test_wipe_zero_capacity() {
    let mut device = MockDevice::new(0);
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };
    let method: Box<dyn WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(test_drive_info(0), method, config);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();
    assert_eq!(result.total_bytes_written, 0);
    assert_eq!(result.outcome, WipeOutcome::Success);
}

#[tokio::test]
async fn test_wipe_immediate_cancel() {
    let capacity: u64 = 10 * 1024 * 1024;
    let mut device = MockDevice::new(capacity);
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };
    let method: Box<dyn WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(test_drive_info(capacity), method, config);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = session.execute(&mut device, &tx, &cancel, None).await;
    // The session should either return Cancelled or an error.
    match result {
        Ok(r) => {
            assert!(
                r.outcome == WipeOutcome::Cancelled || r.outcome == WipeOutcome::Interrupted,
                "Expected Cancelled or Interrupted, got {:?}",
                r.outcome
            );
        }
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.contains("cancel") || msg.contains("interrupt"),
                "unexpected error: {msg}"
            );
        }
    }
}

#[tokio::test]
async fn test_multipass_wipe() {
    let capacity: u64 = 1024 * 1024; // 1 MiB
    let mut device = MockDevice::new(capacity);
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };
    let method: Box<dyn WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(test_hdd_info(capacity), method, config);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();
    assert_eq!(result.passes.len(), 3);
    assert_eq!(result.outcome, WipeOutcome::Success);
}

#[tokio::test]
async fn test_wipe_write_error_propagates() {
    let capacity: u64 = 1024 * 1024; // 1 MiB
    // Inject a write error at offset 0 so the very first write fails.
    let mut device = MockDevice::new(capacity).inject_write_error(0);
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };
    let method: Box<dyn WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(test_drive_info(capacity), method, config);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    let result = session.execute(&mut device, &tx, &cancel, None).await;
    assert!(result.is_err(), "Expected write error to propagate");
}

#[tokio::test]
async fn test_wipe_with_verification() {
    let capacity: u64 = 512 * 1024; // 512 KiB
    let mut device = MockDevice::new(capacity);
    let config = DriveWipeConfig {
        auto_verify: true,
        ..DriveWipeConfig::default()
    };
    // Zero-fill so verification can confirm all-zeros.
    let method: Box<dyn WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(test_drive_info(capacity), method, config);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();
    assert_eq!(result.verification_passed, Some(true));
    assert_eq!(result.outcome, WipeOutcome::Success);
}

#[tokio::test]
async fn test_progress_events_emitted() {
    let capacity: u64 = 512 * 1024; // 512 KiB
    let mut device = MockDevice::new(capacity);
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };
    let method: Box<dyn WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(test_drive_info(capacity), method, config);
    let (tx, rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();
    session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();
    drop(tx);

    let events: Vec<ProgressEvent> = rx.iter().collect();
    // Must have SessionStarted, at least one PassStarted, PassCompleted, Completed.
    assert!(
        events.len() >= 4,
        "Expected at least 4 events, got {}",
        events.len()
    );
    assert!(matches!(events[0], ProgressEvent::SessionStarted { .. }));
    assert!(matches!(
        events.last().unwrap(),
        ProgressEvent::Completed { .. }
    ));
}

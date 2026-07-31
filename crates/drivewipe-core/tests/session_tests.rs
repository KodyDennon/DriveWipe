use std::path::PathBuf;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::error::Result;
use drivewipe_core::io::RawDeviceIo;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::session::{CancellationToken, WipeSession};
use drivewipe_core::types::*;
use drivewipe_core::wipe::software::ZeroFillMethod;

/// How a faulty byte behaves when written to.
#[derive(Clone, Copy)]
enum BadByte {
    /// Stores the complement of whatever was written, so it never matches the
    /// intended pattern. Use when a pass must be seen to fail: pinning to a
    /// fixed value would match a random pass roughly once every 256 runs.
    Inverts,
    /// Always holds this value, so it matches some patterns and not others.
    StuckAt(u8),
}

/// A mock device backed by an in-memory buffer.
struct MockDevice {
    data: Vec<u8>,
    block_size: u32,
    /// Offset of a byte that silently refuses writes, simulating a drive that
    /// reports success but does not store what it was given.
    bad_byte: Option<(u64, BadByte)>,
}

impl MockDevice {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0xFFu8; size],
            block_size: 512,
            bad_byte: None,
        }
    }

    /// A device whose byte at `offset` never holds what was written to it.
    fn with_bad_sector(size: usize, offset: u64) -> Self {
        Self {
            bad_byte: Some((offset, BadByte::Inverts)),
            ..Self::new(size)
        }
    }

    /// A device whose byte at `offset` is pinned to `value` regardless of what
    /// is written, so it satisfies passes writing `value` and fails the rest.
    fn with_stuck_byte(size: usize, offset: u64, value: u8) -> Self {
        let mut dev = Self::new(size);
        dev.bad_byte = Some((offset, BadByte::StuckAt(value)));
        dev.data[offset as usize] = value;
        dev
    }
}

impl RawDeviceIo for MockDevice {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let start = offset as usize;
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        self.data[start..end].copy_from_slice(&buf[..n]);

        // The faulty byte swallows the write but still reports success.
        if let Some((bad, behaviour)) = self.bad_byte
            && (start..end).contains(&(bad as usize))
        {
            let written = buf[bad as usize - start];
            self.data[bad as usize] = match behaviour {
                BadByte::Inverts => !written,
                BadByte::StuckAt(v) => v,
            };
        }
        Ok(n)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let start = offset as usize;
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        buf[..n].copy_from_slice(&self.data[start..end]);
        Ok(n)
    }

    fn capacity(&self) -> u64 {
        self.data.len() as u64
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        Ok(())
    }
}

fn make_drive_info(capacity: u64) -> DriveInfo {
    DriveInfo {
        path: PathBuf::from("/dev/test"),
        model: "MockDrive".to_string(),
        serial: "MOCK-SERIAL-001".to_string(),
        firmware_rev: "1.0".to_string(),
        capacity,
        block_size: 512,
        physical_block_size: None,
        drive_type: DriveType::Hdd,
        transport: Transport::Sata,
        is_boot_drive: false,
        is_removable: false,
        ata_security: AtaSecurityState::NotSupported,
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim: false,
        is_sed: false,
        smart_healthy: Some(true),
        partition_table: None,
        partition_count: 0,
    }
}

#[tokio::test]
async fn wipe_session_zero_fill_10mib() {
    let size = 10 * 1024 * 1024; // 10 MiB
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();

    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();

    assert_eq!(result.outcome, WipeOutcome::Success);
    assert_eq!(result.total_bytes_written, size as u64);
    assert_eq!(result.passes.len(), 1);
    assert!(result.total_duration_secs > 0.0);

    // Verify the device is all zeros.
    assert!(device.data.iter().all(|&b| b == 0x00));
}

#[tokio::test]
async fn wipe_session_with_verification() {
    let size = 1024 * 1024; // 1 MiB
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        auto_verify: true,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();

    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();

    assert_eq!(result.outcome, WipeOutcome::Success);
    assert_eq!(result.verification_passed, Some(true));
}

#[tokio::test]
async fn wipe_session_cancellation() {
    let size = 10 * 1024 * 1024; // 10 MiB
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig::default();
    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();

    // Cancel immediately before execution.
    cancel.cancel();

    let result = session.execute(&mut device, &tx, &cancel, None).await;
    // The session should either return Cancelled or Interrupted.
    match result {
        Ok(r) => {
            assert!(r.outcome == WipeOutcome::Cancelled || r.outcome == WipeOutcome::Interrupted);
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
async fn wipe_session_progress_events() {
    let size = 2 * 1024 * 1024; // 2 MiB
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(ZeroFillMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();

    session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();
    drop(tx);

    let events: Vec<ProgressEvent> = rx.iter().collect();

    // Should have SessionStarted, PassStarted, BlockWritten(s), PassCompleted, Completed.
    assert!(events.len() >= 4);
    assert!(matches!(events[0], ProgressEvent::SessionStarted { .. }));
    assert!(matches!(
        events.last().unwrap(),
        ProgressEvent::Completed { .. }
    ));
}

#[tokio::test]
async fn dod_short_is_verified_even_when_auto_verify_is_off() {
    use drivewipe_core::wipe::software::DodShortMethod;

    let size = 1024 * 1024;
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    // DoD 5220.22-M mandates verification, so it must run regardless of the
    // operator's default preference.
    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let result = session
        .execute(&mut device, &tx, &CancellationToken::new(), None)
        .await
        .unwrap();

    assert_eq!(
        result.verification_passed,
        Some(true),
        "DoD 5220.22-M must be verified even with auto_verify disabled"
    );
    assert_eq!(result.outcome, WipeOutcome::Success);
}

#[tokio::test]
async fn random_final_pass_is_verified_byte_for_byte() {
    use drivewipe_core::wipe::software::DodShortMethod;

    // A single sector that silently fails to take the write. The old sampling
    // heuristic only checked that blocks were non-zero, so a stale 0xFF sector
    // like this passed unnoticed; replaying the keystream catches it.
    let size = 1024 * 1024;
    let bad_offset = 700 * 1024;
    let mut device = MockDevice::with_bad_sector(size, bad_offset);
    let drive_info = make_drive_info(size as u64);

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, DriveWipeConfig::default());

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let result = session
        .execute(&mut device, &tx, &CancellationToken::new(), None)
        .await
        .unwrap();

    assert_eq!(
        result.verification_passed,
        Some(false),
        "a sector that did not take the random pass must fail verification"
    );
    assert_eq!(result.outcome, WipeOutcome::Failed);
    assert!(
        result.warnings.iter().any(|w| w.contains("mismatch")),
        "expected a mismatch warning, got: {:?}",
        result.warnings
    );
}

#[tokio::test]
async fn verify_each_pass_records_a_result_for_every_pass() {
    use drivewipe_core::wipe::software::DodShortMethod;

    let size = 512 * 1024;
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        verify_each_pass: true,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let result = session
        .execute(&mut device, &tx, &CancellationToken::new(), None)
        .await
        .unwrap();

    assert_eq!(result.passes.len(), 3);
    for pass in &result.passes {
        assert!(
            pass.verified,
            "pass {} was not verified under verify_each_pass",
            pass.pass_number
        );
        assert_eq!(pass.verification_passed, Some(true));
    }
    assert_eq!(result.verification_passed, Some(true));
}

#[tokio::test]
async fn verify_each_pass_catches_a_failure_on_an_intermediate_pass() {
    use drivewipe_core::wipe::software::DodShortMethod;

    // Verifying only the final pass cannot tell you whether pass 1 landed.
    // Per-pass verification is what makes each pass individually evidenced.
    let size = 512 * 1024;
    let mut device = MockDevice::with_bad_sector(size, 300 * 1024);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        verify_each_pass: true,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let result = session
        .execute(&mut device, &tx, &CancellationToken::new(), None)
        .await
        .unwrap();

    // Pass 1 writes zeros; the stuck 0xFF byte must be caught right there.
    assert_eq!(result.passes[0].verification_passed, Some(false));
    assert!(
        result
            .warnings
            .iter()
            .any(|w| w.contains("Pass 1 verification mismatch")),
        "expected pass 1 to be blamed, got: {:?}",
        result.warnings
    );
    assert_eq!(result.outcome, WipeOutcome::Failed);
}

#[tokio::test]
async fn an_earlier_failed_pass_fails_the_whole_session() {
    use drivewipe_core::wipe::software::DodShortMethod;

    // A sector stuck at 0x00 takes the zero pass fine but not the 0xFF pass,
    // so pass 2 fails while pass 3 (random) is unaffected at that byte only if
    // the final pass happens to match. Regardless of which passes fail, the
    // session verdict must not be decided by the last pass alone.
    let size = 512 * 1024;
    let mut device = MockDevice::with_stuck_byte(size, 200 * 1024, 0x00);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        verify_each_pass: true,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let result = session
        .execute(&mut device, &tx, &CancellationToken::new(), None)
        .await
        .unwrap();

    // Pass 1 writes zeros, so the stuck 0x00 byte matches and pass 1 passes.
    assert_eq!(result.passes[0].verification_passed, Some(true));
    // Pass 2 writes 0xFF, which the stuck byte refuses.
    assert_eq!(result.passes[1].verification_passed, Some(false));

    assert_eq!(
        result.verification_passed,
        Some(false),
        "a failure on any pass must fail the session, not just a failure on the last"
    );
    assert_eq!(result.outcome, WipeOutcome::Failed);
}

#[tokio::test]
async fn wipe_session_multiple_passes() {
    use drivewipe_core::wipe::software::DodShortMethod;

    let size = 1024 * 1024; // 1 MiB
    let mut device = MockDevice::new(size);
    let drive_info = make_drive_info(size as u64);

    let config = DriveWipeConfig {
        auto_verify: false,
        ..DriveWipeConfig::default()
    };

    let method: Box<dyn drivewipe_core::wipe::WipeMethod> = Box::new(DodShortMethod);
    let session = WipeSession::new(drive_info, method, config);

    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let cancel = CancellationToken::new();

    let result = session
        .execute(&mut device, &tx, &cancel, None)
        .await
        .unwrap();

    assert_eq!(result.outcome, WipeOutcome::Success);
    assert_eq!(result.passes.len(), 3);
    // Last pass is random, so total bytes should be 3x the device size.
    assert_eq!(result.total_bytes_written, 3 * size as u64);
}

use crossbeam_channel;
use uuid::Uuid;

use drivewipe_core::error::Result;
use drivewipe_core::io::RawDeviceIo;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::verify::Verifier;
use drivewipe_core::verify::zero_verify::ZeroVerifier;

/// A mock device backed by an in-memory buffer.
struct MockDevice {
    data: Vec<u8>,
    block_size: u32,
}

impl MockDevice {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            block_size: 512,
        }
    }

    fn all_zeros(size: usize) -> Self {
        Self::new(vec![0u8; size])
    }
}

impl RawDeviceIo for MockDevice {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let start = offset as usize;
        let end = (start + buf.len()).min(self.data.len());
        let n = end - start;
        self.data[start..end].copy_from_slice(&buf[..n]);
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

#[test]
fn zero_verify_passes_on_all_zeros() {
    let mut device = MockDevice::all_zeros(4096);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    let result = verifier.verify(&mut device, Uuid::new_v4(), &tx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), true);
}

#[test]
fn zero_verify_fails_on_non_zero_data() {
    let mut data = vec![0u8; 4096];
    data[2048] = 0xFF;
    let mut device = MockDevice::new(data);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    let result = verifier.verify(&mut device, Uuid::new_v4(), &tx);
    assert!(result.is_err());
}

#[test]
fn zero_verify_progress_events() {
    let mut device = MockDevice::all_zeros(2 * 1024 * 1024); // 2 MiB
    let (tx, rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    verifier.verify(&mut device, Uuid::new_v4(), &tx).unwrap();
    drop(tx);

    let events: Vec<ProgressEvent> = rx.iter().collect();
    // Should have at least: VerificationStarted, one or more VerificationProgress, VerificationCompleted.
    assert!(events.len() >= 3);

    // First event should be VerificationStarted.
    assert!(matches!(
        events[0],
        ProgressEvent::VerificationStarted { .. }
    ));

    // Last event should be VerificationCompleted.
    assert!(matches!(
        events.last().unwrap(),
        ProgressEvent::VerificationCompleted { passed: true, .. }
    ));
}

#[test]
fn zero_verify_empty_device() {
    let mut device = MockDevice::all_zeros(0);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    let result = verifier.verify(&mut device, Uuid::new_v4(), &tx);
    assert!(result.is_ok());
}

#[test]
fn zero_verify_fails_at_first_byte() {
    let mut data = vec![0u8; 1024];
    data[0] = 0x42;
    let mut device = MockDevice::new(data);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    let result = verifier.verify(&mut device, Uuid::new_v4(), &tx);
    match result {
        Err(drivewipe_core::DriveWipeError::VerificationFailed {
            offset,
            expected,
            actual,
        }) => {
            assert_eq!(offset, 0);
            assert_eq!(expected, 0x00);
            assert_eq!(actual, 0x42);
        }
        other => panic!("Expected VerificationFailed, got: {:?}", other),
    }
}

#[test]
fn zero_verify_fails_at_last_byte() {
    let mut data = vec![0u8; 1024];
    data[1023] = 0x01;
    let mut device = MockDevice::new(data);
    let (tx, _rx) = crossbeam_channel::unbounded::<ProgressEvent>();
    let verifier = ZeroVerifier;
    let result = verifier.verify(&mut device, Uuid::new_v4(), &tx);
    match result {
        Err(drivewipe_core::DriveWipeError::VerificationFailed {
            offset,
            expected: _,
            actual,
        }) => {
            assert_eq!(offset, 1023);
            assert_eq!(actual, 0x01);
        }
        other => panic!("Expected VerificationFailed, got: {:?}", other),
    }
}

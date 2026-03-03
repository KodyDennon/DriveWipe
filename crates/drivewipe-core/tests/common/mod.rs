//! Shared test utilities for drivewipe-core integration tests.

use std::path::PathBuf;

use drivewipe_core::error::Result;
use drivewipe_core::io::RawDeviceIo;
use drivewipe_core::types::*;

/// A mock device backed by an in-memory buffer for testing.
///
/// Tracks all writes for verification and supports injecting errors.
pub struct MockDevice {
    data: Vec<u8>,
    capacity: u64,
    block_size: u32,
    inject_write_error_at: Option<u64>,
    inject_read_error_at: Option<u64>,
    write_count: u64,
    read_count: u64,
}

impl MockDevice {
    /// Create a new mock device of the given size (in bytes).
    pub fn new(capacity: u64) -> Self {
        Self {
            data: vec![0u8; capacity as usize],
            capacity,
            block_size: 512,
            inject_write_error_at: None,
            inject_read_error_at: None,
            write_count: 0,
            read_count: 0,
        }
    }

    /// Create a mock device with a custom block size.
    pub fn with_block_size(capacity: u64, block_size: u32) -> Self {
        Self {
            data: vec![0u8; capacity as usize],
            capacity,
            block_size,
            inject_write_error_at: None,
            inject_read_error_at: None,
            write_count: 0,
            read_count: 0,
        }
    }

    /// Configure the device to return an error at a specific write offset.
    pub fn inject_write_error(mut self, at_offset: u64) -> Self {
        self.inject_write_error_at = Some(at_offset);
        self
    }

    /// Configure the device to return an error at a specific read offset.
    pub fn inject_read_error(mut self, at_offset: u64) -> Self {
        self.inject_read_error_at = Some(at_offset);
        self
    }

    /// Return the underlying data buffer for verification.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Check if a byte range is entirely filled with the given value.
    pub fn is_filled_with(&self, value: u8) -> bool {
        self.data.iter().all(|&b| b == value)
    }

    /// Check if a byte range is entirely zeros.
    pub fn is_zeroed(&self) -> bool {
        self.is_filled_with(0)
    }

    /// Get the total number of write operations.
    pub fn write_count(&self) -> u64 {
        self.write_count
    }

    /// Get the total number of read operations.
    pub fn read_count(&self) -> u64 {
        self.read_count
    }
}

impl RawDeviceIo for MockDevice {
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        if let Some(err_offset) = self.inject_write_error_at {
            if offset == err_offset {
                return Err(drivewipe_core::error::DriveWipeError::IoGeneric(
                    std::io::Error::new(std::io::ErrorKind::Other, "injected write error"),
                ));
            }
        }

        let start = offset as usize;
        let end = (start + buf.len()).min(self.data.len());
        let bytes_to_write = end - start;
        self.data[start..end].copy_from_slice(&buf[..bytes_to_write]);
        self.write_count += 1;
        Ok(bytes_to_write)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if let Some(err_offset) = self.inject_read_error_at {
            if offset == err_offset {
                return Err(drivewipe_core::error::DriveWipeError::IoGeneric(
                    std::io::Error::new(std::io::ErrorKind::Other, "injected read error"),
                ));
            }
        }

        let start = offset as usize;
        let end = (start + buf.len()).min(self.data.len());
        let bytes_to_read = end - start;
        buf[..bytes_to_read].copy_from_slice(&self.data[start..end]);
        self.read_count += 1;
        Ok(bytes_to_read)
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn block_size(&self) -> u32 {
        self.block_size
    }

    fn sync(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Create a test DriveInfo with sensible defaults.
pub fn test_drive_info(capacity: u64) -> DriveInfo {
    DriveInfo {
        path: PathBuf::from("/dev/test0"),
        model: "Test Drive Model".to_string(),
        serial: "TEST-SERIAL-001".to_string(),
        firmware_rev: "1.0".to_string(),
        capacity,
        block_size: 512,
        physical_block_size: None,
        drive_type: DriveType::Ssd,
        transport: Transport::Sata,
        is_boot_drive: false,
        is_removable: false,
        ata_security: AtaSecurityState::NotSupported,
        hidden_areas: HiddenAreaInfo::default(),
        supports_trim: true,
        is_sed: false,
        smart_healthy: Some(true),
        partition_table: None,
        partition_count: 0,
    }
}

/// Create a test DriveInfo that resembles an HDD.
pub fn test_hdd_info(capacity: u64) -> DriveInfo {
    let mut info = test_drive_info(capacity);
    info.drive_type = DriveType::Hdd;
    info.transport = Transport::Sata;
    info.supports_trim = false;
    info.model = "Test HDD Model".to_string();
    info
}

/// Create a test DriveInfo that resembles an NVMe drive.
pub fn test_nvme_info(capacity: u64) -> DriveInfo {
    let mut info = test_drive_info(capacity);
    info.drive_type = DriveType::Nvme;
    info.transport = Transport::Nvme;
    info.model = "Test NVMe Model".to_string();
    info
}

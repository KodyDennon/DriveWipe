use serde::{Deserialize, Serialize};

/// NVMe health log page data (Log Page 02h).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmeHealthLog {
    /// Critical warning bitmap.
    pub critical_warning: u8,
    /// Composite temperature in Kelvin.
    pub temperature_kelvin: u16,
    /// Available spare percentage (0-100).
    pub available_spare: u8,
    /// Available spare threshold.
    pub available_spare_threshold: u8,
    /// Percentage of drive life used (can exceed 100).
    pub percentage_used: u8,
    /// Data units read (in 512-byte units * 1000).
    pub data_units_read: u128,
    /// Data units written (in 512-byte units * 1000).
    pub data_units_written: u128,
    /// Host read commands.
    pub host_read_commands: u128,
    /// Host write commands.
    pub host_write_commands: u128,
    /// Controller busy time in minutes.
    pub controller_busy_time: u128,
    /// Power cycles.
    pub power_cycles: u128,
    /// Power-on hours.
    pub power_on_hours: u128,
    /// Unsafe shutdowns.
    pub unsafe_shutdowns: u128,
    /// Media and data integrity errors.
    pub media_errors: u128,
    /// Number of error log entries.
    pub error_log_entries: u128,
}

impl NvmeHealthLog {
    /// Parse NVMe health log from a raw buffer (512 bytes, Log Page 02h).
    pub fn from_buffer(buf: &[u8]) -> crate::error::Result<Self> {
        if buf.len() < 512 {
            return Err(crate::error::DriveWipeError::Health(
                "NVMe health log buffer too small".to_string(),
            ));
        }

        fn read_u16_le(buf: &[u8], offset: usize) -> u16 {
            u16::from_le_bytes([buf[offset], buf[offset + 1]])
        }

        fn read_u128_le(buf: &[u8], offset: usize) -> u128 {
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&buf[offset..offset + 16]);
            u128::from_le_bytes(bytes)
        }

        Ok(Self {
            critical_warning: buf[0],
            temperature_kelvin: read_u16_le(buf, 1),
            available_spare: buf[3],
            available_spare_threshold: buf[4],
            percentage_used: buf[5],
            data_units_read: read_u128_le(buf, 32),
            data_units_written: read_u128_le(buf, 48),
            host_read_commands: read_u128_le(buf, 64),
            host_write_commands: read_u128_le(buf, 80),
            controller_busy_time: read_u128_le(buf, 96),
            power_cycles: read_u128_le(buf, 112),
            power_on_hours: read_u128_le(buf, 128),
            unsafe_shutdowns: read_u128_le(buf, 144),
            media_errors: read_u128_le(buf, 160),
            error_log_entries: read_u128_le(buf, 176),
        })
    }

    /// Temperature in Celsius.
    pub fn temperature_celsius(&self) -> i16 {
        let raw = self.temperature_kelvin as i16 - 273;
        if !(-40..=200).contains(&raw) {
            log::warn!(
                "NVMe temperature {}°C out of expected range (raw {}K)",
                raw,
                self.temperature_kelvin
            );
        }
        raw
    }

    /// Whether the drive is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        self.critical_warning == 0
            && self.available_spare >= self.available_spare_threshold
            && self.percentage_used < 100
            && self.media_errors == 0
    }
}

use serde::{Deserialize, Serialize};

/// A single SMART attribute with its ID, name, and values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAttribute {
    /// SMART attribute ID (e.g. 5 = Reallocated Sectors Count).
    pub id: u8,
    /// Human-readable name of the attribute.
    pub name: String,
    /// Current normalized value (0-253).
    pub value: u8,
    /// Worst recorded normalized value.
    pub worst: u8,
    /// Threshold below which the attribute indicates failure.
    pub threshold: u8,
    /// Raw value (interpretation is vendor-specific).
    pub raw_value: u64,
    /// Whether this attribute has exceeded its threshold.
    pub failing: bool,
}

/// Parsed ATA SMART data from a drive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartData {
    /// Whether SMART indicates the drive is healthy overall.
    pub healthy: bool,
    /// All parsed SMART attributes.
    pub attributes: Vec<SmartAttribute>,
    /// Drive temperature in Celsius, if available.
    pub temperature_celsius: Option<i16>,
    /// Power-on hours, if available.
    pub power_on_hours: Option<u64>,
    /// Power cycle count, if available.
    pub power_cycle_count: Option<u64>,
    /// Reallocated sector count, if available.
    pub reallocated_sectors: Option<u64>,
    /// Pending sector count, if available.
    pub pending_sectors: Option<u64>,
    /// Uncorrectable sector count, if available.
    pub uncorrectable_sectors: Option<u64>,
}

impl SmartData {
    /// Parse SMART data from a raw ATA SMART data buffer (512 bytes).
    ///
    /// This parses the SMART attribute table from the ATA SMART READ DATA
    /// response. On real hardware this would come from an ioctl; for testing
    /// it can be supplied as a byte buffer.
    pub fn from_ata_buffer(buf: &[u8]) -> crate::error::Result<Self> {
        if buf.len() < 362 {
            return Err(crate::error::DriveWipeError::SmartUnavailable(
                "SMART data buffer too small".to_string(),
            ));
        }

        let mut attributes = Vec::new();
        let mut temperature_celsius = None;
        let mut power_on_hours = None;
        let mut power_cycle_count = None;
        let mut reallocated_sectors = None;
        let mut pending_sectors = None;
        let mut uncorrectable_sectors = None;

        // SMART attributes start at offset 2, each is 12 bytes, 30 attributes max
        for i in 0..30 {
            let offset = 2 + i * 12;
            if offset + 12 > buf.len() {
                break;
            }

            let id = buf[offset];
            if id == 0 {
                continue;
            }

            let value = buf[offset + 3];
            let worst = buf[offset + 4];
            let threshold = 0; // Threshold table is separate
            let raw_value = u64::from(buf[offset + 5])
                | (u64::from(buf[offset + 6]) << 8)
                | (u64::from(buf[offset + 7]) << 16)
                | (u64::from(buf[offset + 8]) << 24)
                | (u64::from(buf[offset + 9]) << 32)
                | (u64::from(buf[offset + 10]) << 40);

            let name = smart_attribute_name(id);

            match id {
                5 => reallocated_sectors = Some(raw_value),
                9 => power_on_hours = Some(raw_value),
                12 => power_cycle_count = Some(raw_value),
                194 | 190 => temperature_celsius = Some((raw_value & 0xFF) as i16),
                197 => pending_sectors = Some(raw_value),
                198 => uncorrectable_sectors = Some(raw_value),
                _ => {}
            }

            attributes.push(SmartAttribute {
                id,
                name,
                value,
                worst,
                threshold,
                raw_value,
                failing: false,
            });
        }

        // Overall health: check for critical attribute failures
        let healthy = reallocated_sectors.unwrap_or(0) < 100
            && pending_sectors.unwrap_or(0) < 10
            && uncorrectable_sectors.unwrap_or(0) < 10;

        Ok(Self {
            healthy,
            attributes,
            temperature_celsius,
            power_on_hours,
            power_cycle_count,
            reallocated_sectors,
            pending_sectors,
            uncorrectable_sectors,
        })
    }
}

/// Return a human-readable name for a SMART attribute ID.
fn smart_attribute_name(id: u8) -> String {
    match id {
        1 => "Raw Read Error Rate".to_string(),
        3 => "Spin-Up Time".to_string(),
        4 => "Start/Stop Count".to_string(),
        5 => "Reallocated Sectors Count".to_string(),
        7 => "Seek Error Rate".to_string(),
        9 => "Power-On Hours".to_string(),
        10 => "Spin Retry Count".to_string(),
        12 => "Power Cycle Count".to_string(),
        187 => "Reported Uncorrectable Errors".to_string(),
        188 => "Command Timeout".to_string(),
        190 => "Airflow Temperature".to_string(),
        194 => "Temperature".to_string(),
        196 => "Reallocated Event Count".to_string(),
        197 => "Current Pending Sector Count".to_string(),
        198 => "Offline Uncorrectable Sector Count".to_string(),
        199 => "Ultra DMA CRC Error Count".to_string(),
        200 => "Multi-Zone Error Rate".to_string(),
        241 => "Total LBAs Written".to_string(),
        242 => "Total LBAs Read".to_string(),
        _ => format!("Attribute {id}"),
    }
}

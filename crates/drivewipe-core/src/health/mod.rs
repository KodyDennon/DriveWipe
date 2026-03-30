pub mod benchmark;
pub mod diff;
pub mod nvme;
pub mod smart;
pub mod snapshot;

pub use benchmark::BenchmarkResult;
pub use diff::{HealthComparison, HealthDiff, HealthVerdict};
pub use nvme::NvmeHealthLog;
pub use smart::{SmartAttribute, SmartData};
pub use snapshot::DriveHealthSnapshot;

/// Get current health data for a drive.
pub async fn get_health(path: &std::path::Path) -> crate::error::Result<DriveHealthSnapshot> {
    let enumerator = crate::drive::create_enumerator();
    let drive_info = enumerator.inspect(path).await?;

    let mut snapshot = DriveHealthSnapshot {
        timestamp: chrono::Utc::now(),
        device_path: path.to_string_lossy().to_string(),
        device_serial: drive_info.serial.clone(),
        device_model: drive_info.model.clone(),
        smart_data: None,
        nvme_health: None,
        temperature_celsius: None,
        benchmark: None,
    };

    // Populate SMART data from DriveInfo's existing smart_healthy field.
    if drive_info.smart_healthy.is_some() {
        snapshot.smart_data = Some(SmartData {
            healthy: drive_info.smart_healthy.unwrap_or(true),
            attributes: Vec::new(),
            temperature_celsius: None,
            power_on_hours: None,
            power_cycle_count: None,
            reallocated_sectors: None,
            pending_sectors: None,
            uncorrectable_sectors: None,
        });
    }

    // On Linux, try reading the hwmon temperature from sysfs.
    #[cfg(target_os = "linux")]
    {
        if let Some(temp) = read_hwmon_temperature(path).await {
            snapshot.temperature_celsius = Some(temp);
            // Also populate into SMART data if we have it.
            if let Some(ref mut smart) = snapshot.smart_data {
                if smart.temperature_celsius.is_none() {
                    smart.temperature_celsius = Some(temp);
                }
            }
        }
    }

    Ok(snapshot)
}

/// Read temperature from the Linux hwmon sysfs interface for a block device.
///
/// Walks `/sys/block/<dev>/device/hwmon/hwmon*/temp1_input` which reports
/// the drive temperature in millidegrees Celsius.
#[cfg(target_os = "linux")]
async fn read_hwmon_temperature(dev_path: &std::path::Path) -> Option<i16> {
    let dev_name = dev_path.file_name()?.to_str()?;
    let hwmon_dir = std::path::PathBuf::from(format!("/sys/block/{dev_name}/device/hwmon"));

    let mut entries = tokio::fs::read_dir(&hwmon_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let temp_path = entry.path().join("temp1_input");
        if let Ok(contents) = tokio::fs::read_to_string(&temp_path).await {
            if let Ok(millidegrees) = contents.trim().parse::<i64>() {
                let celsius = (millidegrees / 1000) as i16;
                if celsius >= -40 && celsius <= 200 {
                    return Some(celsius);
                }
                log::warn!(
                    "hwmon temperature {}°C out of expected range for {}",
                    celsius,
                    dev_name,
                );
            }
        }
    }
    None
}

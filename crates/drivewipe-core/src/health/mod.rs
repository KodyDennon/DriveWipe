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

    // In a real implementation, we'd open the device and call ioctls.
    // For now, we'll return a basic snapshot based on DriveInfo.
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

    Ok(snapshot)
}

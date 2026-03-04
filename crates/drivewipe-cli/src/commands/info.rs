use std::path::Path;

use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive::create_enumerator;

use crate::display;

/// Execute `drivewipe info`.
pub async fn run(_config: &DriveWipeConfig, device: &str) -> Result<()> {
    let enumerator = create_enumerator();
    let device_path = Path::new(device);
    let drive_info = enumerator
        .inspect(device_path)
        .await
        .with_context(|| format!("Failed to inspect device {device}"))?;

    display::print_drive_info(&drive_info);
    Ok(())
}

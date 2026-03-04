use anyhow::{Context, Result};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive::create_enumerator;

use crate::display;

/// Execute `drivewipe list`.
///
/// Enumerates all detected drives and displays them in the requested format.
pub async fn run(_config: &DriveWipeConfig, format: &str) -> Result<()> {
    let enumerator = create_enumerator();
    let drives = enumerator
        .enumerate()
        .await
        .context("Failed to enumerate drives")?;

    if drives.is_empty() {
        eprintln!(
            "{} No drives detected. Are you running with sufficient privileges?",
            console::style("warning:").yellow().bold(),
        );
        return Ok(());
    }

    match format {
        "json" => {
            let json =
                serde_json::to_string_pretty(&drives).context("Failed to serialise drive list")?;
            println!("{json}");
        }
        "table" => {
            display::print_drive_table(&drives);
        }
        other => {
            anyhow::bail!("Unknown output format: {other}. Supported formats: table, json");
        }
    }

    Ok(())
}

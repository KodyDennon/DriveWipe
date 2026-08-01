//! `drivewipe methods` — list the available sanitization methods.

use anyhow::Result;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::wipe::WipeMethodRegistry;

/// Execute `drivewipe methods`.
pub async fn run(config: &DriveWipeConfig, format: &str) -> Result<()> {
    let mut registry = WipeMethodRegistry::new();
    registry.register_custom_methods(config);

    match format {
        "json" => print_json(&registry),
        "ids" => {
            for m in registry.list() {
                println!("{}", m.id());
            }
        }
        _ => print_table(&registry),
    }

    Ok(())
}

fn kind(m: &dyn drivewipe_core::wipe::WipeMethod) -> &'static str {
    if m.is_firmware() {
        "firmware"
    } else {
        "software"
    }
}

fn print_table(registry: &WipeMethodRegistry) {
    println!(
        "{:<26} {:<34} {:>6}  {:<9} VERIFIED",
        "ID", "NAME", "PASSES", "TYPE"
    );
    println!("{}", "-".repeat(90));

    for m in registry.list() {
        let passes = if m.is_firmware() {
            "-".to_string()
        } else {
            m.pass_count().to_string()
        };
        println!(
            "{:<26} {:<34} {:>6}  {:<9} {}",
            m.id(),
            m.name(),
            passes,
            kind(m.as_ref()),
            if m.includes_verification() {
                "always"
            } else {
                "optional"
            },
        );
    }

    println!();
    println!("Methods marked \"always\" are verified regardless of the auto_verify setting,");
    println!("because the standard they implement requires a read-back.");
    println!();
    println!("  drivewipe wipe --device /dev/sdX --method <ID>");
}

fn print_json(registry: &WipeMethodRegistry) {
    let methods: Vec<_> = registry
        .list()
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id(),
                "name": m.name(),
                "description": m.description(),
                "passes": m.pass_count(),
                "firmware": m.is_firmware(),
                "verification_required": m.includes_verification(),
            })
        })
        .collect();

    match serde_json::to_string_pretty(&methods) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("error: could not serialise methods: {e}"),
    }
}

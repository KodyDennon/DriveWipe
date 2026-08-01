//! `drivewipe live` — create DriveWipe Live boot media.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use console::style;

use drivewipe_core::drive::create_enumerator;
use drivewipe_core::live_media::{
    BurnProgress, BurnTarget, TargetSafety, burn, classify_targets, verify_image_checksum,
};
use drivewipe_core::types::format_bytes;

/// List devices that could receive the live image.
pub async fn list() -> Result<()> {
    let drives = create_enumerator()
        .enumerate()
        .await
        .context("Failed to enumerate drives")?;
    let targets = classify_targets(&drives);

    if targets.is_empty() {
        println!("No drives detected.");
        return Ok(());
    }

    println!(
        "{:<16} {:<30} {:>10}  {}",
        "DEVICE", "MODEL", "SIZE", "STATUS"
    );
    println!("{}", "-".repeat(78));

    for t in &targets {
        let (marker, colour) = match t.safety {
            TargetSafety::Removable => ("*", style(t.safety.reason()).green()),
            TargetSafety::Fixed => (" ", style(t.safety.reason()).yellow()),
            TargetSafety::BootDrive => (" ", style(t.safety.reason()).red()),
        };
        println!(
            "{marker}{:<15} {:<30} {:>10}  {}",
            t.path.display(),
            truncate(&t.model, 30),
            format_bytes(t.capacity),
            colour,
        );
    }

    println!();
    println!("  {} = removable, offered by default", style("*").green());
    println!();
    println!("  sudo drivewipe live burn --device /dev/sdX --iso drivewipe-live.iso");

    Ok(())
}

/// Write the live image to a device.
pub async fn burn_cmd(
    device: &str,
    iso: &str,
    checksum: Option<&str>,
    allow_fixed: bool,
    force: bool,
    yes_i_know: bool,
) -> Result<()> {
    if force && !yes_i_know {
        bail!(
            "--force requires --yes-i-know-what-im-doing to confirm you understand \
             this erases the target device"
        );
    }

    let iso_path = Path::new(iso);
    if !iso_path.exists() {
        bail!("no image at {iso}");
    }

    // Verify before writing, not after. An image that fails its checksum must
    // never reach the media.
    if let Some(sum) = checksum {
        print!("Verifying image checksum... ");
        std::io::stdout().flush().ok();
        verify_image_checksum(iso_path, sum)?;
        println!("{}", style("ok").green());
    }

    // Inspect the path the operator actually named rather than requiring it to
    // appear in the enumeration. Enumeration filters out device classes on
    // purpose, and a target given explicitly should still be usable.
    let enumerator = create_enumerator();
    let target = match enumerator.inspect(Path::new(device)).await {
        Ok(info) => BurnTarget::from_drive(&info),
        Err(e) => {
            let drives = enumerator
                .enumerate()
                .await
                .context("Failed to enumerate drives")?;
            classify_targets(&drives)
                .into_iter()
                .find(|t| t.path.to_string_lossy() == device)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "cannot use {device} as a target: {e}\n\
                         Run `drivewipe live list` to see available devices."
                    )
                })?
        }
    };

    drivewipe_core::live_media::check_target(&target, allow_fixed)?;

    let image_size = std::fs::metadata(iso_path)?.len();

    println!();
    println!("{}", style("  Write DriveWipe Live boot media").bold());
    println!();
    println!("    Image   {iso}  ({})", format_bytes(image_size));
    println!(
        "    Target  {}  {}  ({})",
        target.path.display(),
        truncate(&target.model, 30),
        format_bytes(target.capacity),
    );
    if !target.serial.is_empty() {
        println!("    Serial  {}", target.serial);
    }
    println!();
    println!(
        "  {} Everything on {} will be destroyed.",
        style("!").red().bold(),
        target.path.display()
    );
    println!();

    if !force && !confirm(&target)? {
        println!("Cancelled.");
        return Ok(());
    }

    let started = Instant::now();
    let mut phase = String::new();
    let mut last = Instant::now();

    burn(iso_path, &target, allow_fixed, |p| {
        // Throttle redraws; a 4 MiB chunk on USB 3 arrives faster than a
        // terminal can usefully repaint.
        let (label, done, total) = match p {
            BurnProgress::Writing { written, total } => ("Writing  ", written, total),
            BurnProgress::Syncing => ("Flushing ", 0, 0),
            BurnProgress::Verifying { checked, total } => ("Verifying", checked, total),
        };

        if label != phase {
            if !phase.is_empty() {
                println!();
            }
            phase = label.to_string();
        }
        if total == 0 {
            print!("\r  {label} ...");
            std::io::stdout().flush().ok();
            return;
        }
        if last.elapsed().as_millis() < 120 && done < total {
            return;
        }
        last = Instant::now();

        let pct = (done as f64 / total as f64 * 100.0).min(100.0);
        let filled = (pct / 2.5) as usize;
        print!(
            "\r  {label} [{}{}] {:>5.1}%  {} / {}",
            "#".repeat(filled),
            " ".repeat(40usize.saturating_sub(filled)),
            pct,
            format_bytes(done),
            format_bytes(total),
        );
        std::io::stdout().flush().ok();
    })?;

    println!();
    println!();
    println!(
        "  {} Boot media ready on {} ({:.0}s)",
        style("+").green().bold(),
        target.path.display(),
        started.elapsed().as_secs_f64(),
    );
    println!("    The image was read back and matches what was written.");
    println!();
    println!("  Boot the target machine from this device to run DriveWipe Live.");

    Ok(())
}

fn confirm(target: &BurnTarget) -> Result<bool> {
    let expect = target
        .path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "yes".to_string());

    print!("  Type {} to continue: ", style(&expect).bold());
    std::io::stdout().flush().ok();

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim() == expect)
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n.saturating_sub(1)).collect();
        out.push('~');
        out
    }
}

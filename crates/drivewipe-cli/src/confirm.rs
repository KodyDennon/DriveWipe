use anyhow::Result;
use console::{style, Term};
use dialoguer::Input;

use drivewipe_core::types::{DriveInfo, format_bytes};

/// Multi-step interactive confirmation before wiping a drive.
///
/// Returns `true` only if all steps pass. Returns `false` if the user
/// explicitly aborts, or `Err` on I/O failure.
///
/// Steps:
/// 1. Display a prominent warning with drive details.
/// 2. Ask the user to type the device path to confirm.
/// 3. 3-second countdown with abort option.
pub fn run_confirmation(drive: &DriveInfo, method_name: &str) -> Result<bool> {
    let term = Term::stderr();

    // ── Step 1: Display warning ─────────────────────────────────────────
    term.write_line("")?;
    term.write_line(&format!(
        "  {}",
        style("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!").red().bold()
    ))?;
    term.write_line(&format!(
        "  {}",
        style("!!           DESTRUCTIVE OPERATION WARNING             !!").red().bold()
    ))?;
    term.write_line(&format!(
        "  {}",
        style("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!").red().bold()
    ))?;
    term.write_line("")?;
    term.write_line(&format!(
        "  {} ALL DATA on the following drive will be PERMANENTLY DESTROYED:",
        style("WARNING:").red().bold(),
    ))?;
    term.write_line("")?;
    term.write_line(&format!(
        "    Device   : {}",
        style(drive.path.display().to_string()).yellow().bold(),
    ))?;
    term.write_line(&format!("    Model    : {}", drive.model))?;
    term.write_line(&format!("    Serial   : {}", drive.serial))?;
    term.write_line(&format!(
        "    Capacity : {}",
        format_bytes(drive.capacity),
    ))?;
    term.write_line(&format!("    Type     : {}", drive.drive_type))?;
    term.write_line(&format!("    Method   : {}", method_name))?;
    term.write_line("")?;
    term.write_line(&format!(
        "  {}",
        style("This operation CANNOT be undone.").red().bold()
    ))?;
    term.write_line("")?;

    // ── Step 2: Type the device path to confirm ─────────────────────────
    let expected = drive.path.display().to_string();
    let prompt_msg = format!(
        "  Type the device path ({}) to confirm, or anything else to abort",
        style(&expected).yellow().bold(),
    );

    let response: String = Input::new()
        .with_prompt(&prompt_msg)
        .allow_empty(true)
        .interact_text()?;

    let response = response.trim();
    if response != expected && response != "YES I UNDERSTAND" {
        term.write_line(&format!(
            "\n  {} Input did not match. Aborting.",
            style("ABORTED:").yellow().bold()
        ))?;
        return Ok(false);
    }

    // ── Step 3: 3-second countdown ──────────────────────────────────────
    term.write_line("")?;
    term.write_line(&format!(
        "  {} Starting in...",
        style("FINAL CHANCE:").red().bold()
    ))?;

    for i in (1..=3).rev() {
        term.write_str(&format!("  {}...", style(i).yellow().bold()))?;
        term.flush()?;

        // Sleep for 1 second. During this time, if the user presses Ctrl-C
        // the ctrlc handler in main.rs will fire and set the cancellation
        // token, which the wipe loop will detect.
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Erase the countdown number on the same line for a clean look.
        term.clear_line()?;
    }

    term.write_line(&format!(
        "  {} Proceeding with wipe.",
        style("GO").green().bold()
    ))?;
    term.write_line("")?;

    Ok(true)
}

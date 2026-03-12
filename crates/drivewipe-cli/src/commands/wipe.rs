use std::path::Path;
use std::sync::Arc;


use anyhow::{Context, Result, bail};
use crossbeam_channel;

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::drive::create_enumerator;
use drivewipe_core::progress::ProgressEvent;
use drivewipe_core::report::ReportGenerator;
use drivewipe_core::report::json::JsonReportGenerator;
use drivewipe_core::resume::WipeState;
use drivewipe_core::session::{CancellationToken, WipeSession};
use drivewipe_core::types::*;
use drivewipe_core::wipe::WipeMethodRegistry;

use crate::confirm;
use crate::progress::WipeProgressDisplay;

/// Execute `drivewipe wipe`.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    device: &str,
    method_id: &str,
    force: bool,
    yes_i_know: bool,
    verify_override: Option<bool>,
    report_pdf: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    // ── Argument validation ─────────────────────────────────────────────
    if force && !yes_i_know {
        bail!(
            "--force requires --yes-i-know-what-im-doing to confirm \
             you understand the consequences"
        );
    }

    // ── Resolve wipe method ─────────────────────────────────────────────
    let registry = WipeMethodRegistry::new();
    let method_ref = registry
        .get(method_id)
        .ok_or_else(|| anyhow::anyhow!("Unknown wipe method: {method_id}"))?;

    let method_name = method_ref.name().to_string();
    let pass_count = method_ref.pass_count();
    let is_firmware = method_ref.is_firmware();

    // ── Inspect the device ──────────────────────────────────────────────
    let enumerator = create_enumerator();
    let device_path = Path::new(device);
    let drive_info = enumerator
        .inspect(device_path)
        .await
        .with_context(|| format!("Failed to inspect device {device}"))?;

    // ── Safety checks ───────────────────────────────────────────────────
    if drive_info.is_boot_drive {
        bail!(
            "Refusing to wipe boot/system drive {}. \
             Remove the drive from the running system first.",
            drive_info.path.display()
        );
    }

    if drive_info.drive_type == DriveType::Ssd && !is_firmware {
        eprintln!(
            "{} {} is an SSD. Software overwrite methods may not sanitise \
             all flash cells. Consider using a firmware-based method \
             (e.g., ata-erase-enhanced, nvme-format-crypto) for full sanitisation.",
            console::style("warning:").yellow().bold(),
            drive_info.path.display(),
        );
    }

    if drive_info.transport == Transport::Usb {
        eprintln!(
            "{} {} is connected via USB. Performance may be limited and \
             firmware erase commands may not be passed through the USB bridge.",
            console::style("warning:").yellow().bold(),
            drive_info.path.display(),
        );
    }

    if drive_info.ata_security == AtaSecurityState::Frozen {
        eprintln!(
            "{} ATA security is FROZEN on {}. Firmware erase commands will \
             fail. Suspend and resume the system to unfreeze, or use a \
             software wipe method.",
            console::style("warning:").yellow().bold(),
            drive_info.path.display(),
        );
        if is_firmware {
            bail!(
                "Cannot execute firmware erase on a frozen drive. \
                 Suspend/resume the machine or use a software method."
            );
        }
    }

    // ── Confirmation ────────────────────────────────────────────────────
    if dry_run {
        println!(
            "{} Dry run mode -- no data will be written.",
            console::style("[DRY RUN]").cyan().bold(),
        );
    }

    if !force {
        let confirmed = confirm::run_confirmation(&drive_info, &method_name)?;
        if !confirmed {
            println!("Wipe cancelled.");
            return Ok(());
        }
    }

    if dry_run {
        println!();
        println!(
            "Would wipe {} ({}) with method \"{}\" ({} pass(es)).",
            drive_info.path.display(),
            drive_info.capacity_display(),
            method_name,
            pass_count,
        );
        println!(
            "Verification after wipe: {}",
            match verify_override {
                Some(true) => "yes (override)",
                Some(false) => "no (override)",
                None if config.auto_verify => "yes (config default)",
                None => "no (config default)",
            }
        );
        println!("Dry run complete. No data was modified.");
        return Ok(());
    }

    // ── Open the device ─────────────────────────────────────────────────
    #[cfg(target_os = "linux")]
    let mut device_io = drivewipe_core::io::linux::LinuxDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    #[cfg(target_os = "macos")]
    let mut device_io = drivewipe_core::io::macos::MacosDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    #[cfg(target_os = "windows")]
    let mut device_io = drivewipe_core::io::windows::WindowsDeviceIo::open(device_path)
        .with_context(|| format!("Failed to open device {device}"))?;

    // ── Build the session ───────────────────────────────────────────────
    let mut session_config = config.clone();
    if let Some(v) = verify_override {
        session_config.auto_verify = v;
    }

    let session = {
        let method_box = find_and_clone_method_by_id(method_id).await
            .ok_or_else(|| anyhow::anyhow!("Internal error: method {method_id} not found"))?;
        WipeSession::new(drive_info.clone(), method_box, session_config)
    };

    // ── Check for resumable state ───────────────────────────────────────
    let resume_state = WipeState::find_for_device(config.sessions_dir(), &drive_info.serial)
        .ok()
        .flatten();

    if let Some(ref state) = resume_state {
        println!(
            "{} Found resumable session {} (pass {}/{}, {:.1}% complete)",
            console::style("resume:").green().bold(),
            state.session_id,
            state.current_pass,
            state.total_passes,
            (state.total_bytes_written as f64 / state.device_capacity as f64) * 100.0,
        );
    }

    // ── Progress display ────────────────────────────────────────────────
    let progress_display = WipeProgressDisplay::new(drive_info.capacity, pass_count);

    let (progress_tx, progress_rx) = crossbeam_channel::unbounded::<ProgressEvent>();

    // Spawn a thread to consume progress events and update the display.
    let display_handle = {
        let pd = progress_display.clone();
        tokio::task::spawn_blocking(move || {
            while let Ok(event) = progress_rx.recv() {
                pd.update(&event);
            }
        })
    };

    // ── Execute the wipe ────────────────────────────────────────────────
    println!(
        "\n{} Wiping {} with \"{}\" ({} pass(es))...\n",
        console::style("==>").green().bold(),
        drive_info.path.display(),
        method_name,
        pass_count,
    );

    let wipe_result = session
        .execute(&mut device_io, &progress_tx, cancel_token, resume_state)
        .await
        .context("Wipe operation failed")?;

    // Drop the sender so the display thread terminates.
    drop(progress_tx);
    let _ = display_handle.await;

    // ── Summary ─────────────────────────────────────────────────────────
    progress_display.finish();

    println!();
    print_wipe_summary(&wipe_result);

    // ── JSON report (auto or always) ────────────────────────────────────
    if config.auto_report_json || wipe_result.outcome == WipeOutcome::Success {
        let report_dir = config.sessions_dir();
        std::fs::create_dir_all(report_dir).context("Failed to create report directory")?;

        let json_path = report_dir.join(format!("{}.json", wipe_result.session_id));
        let generator = JsonReportGenerator;
        let report_bytes = generator
            .generate(&wipe_result)
            .context("Failed to generate JSON report")?;

        std::fs::write(&json_path, &report_bytes)
            .with_context(|| format!("Failed to write report to {}", json_path.display()))?;

        println!(
            "{} JSON report saved to {}",
            console::style("report:").blue().bold(),
            json_path.display(),
        );
    }

    // ── PDF report (optional) ───────────────────────────────────────────
    if let Some(pdf_path) = report_pdf {
        generate_pdf_report(&wipe_result, pdf_path)?;
    }

    // ── Exit code ───────────────────────────────────────────────────────
    match wipe_result.outcome {
        WipeOutcome::Success | WipeOutcome::SuccessWithWarnings => Ok(()),
        WipeOutcome::Cancelled => {
            bail!("Wipe was cancelled by user")
        }
        WipeOutcome::Failed => {
            bail!("Wipe failed -- see warnings above")
        }
        WipeOutcome::Interrupted => {
            bail!("Wipe was interrupted and can be resumed with `drivewipe resume`")
        }
    }
}

// ── Public helpers (used by resume.rs and queue.rs) ─────────────────────────

/// Create an owned `Box<dyn WipeMethod>` for the given method id.
///
/// Because `WipeMethodRegistry` only exposes `&dyn WipeMethod` references and
/// does not support removing or cloning entries, we use a thin proxy struct
/// that delegates calls back to a fresh registry.  All built-in software
/// methods are cheap, stateless unit structs so re-creating the registry on
/// each delegated call is essentially free.
pub async fn find_and_clone_method_by_id(
    method_id: &str,
) -> Option<Box<dyn drivewipe_core::wipe::WipeMethod>> {
    let proxy = MethodProxy::new(method_id.to_string())?;
    Some(Box::new(proxy))
}

// ── Private helpers ─────────────────────────────────────────────────────────

/// Thin proxy that delegates to a fresh `WipeMethodRegistry` lookup.
///
/// This exists because `WipeMethodRegistry::get` returns a reference and we
/// need an owned `Box<dyn WipeMethod>` for `WipeSession::new`.  The `name`
/// and `description` fields are cached at construction time so that `name()`
/// and `description()` can return `&str` without leaking memory.
struct MethodProxy {
    id: String,
    cached_name: String,
    cached_description: String,
    cached_pass_count: u32,
    cached_includes_verification: bool,
    cached_is_firmware: bool,
}

impl MethodProxy {
    fn new(id: String) -> Option<Self> {
        let registry = WipeMethodRegistry::new();
        let method = registry.get(&id)?;
        Some(Self {
            cached_name: method.name().to_string(),
            cached_description: method.description().to_string(),
            cached_pass_count: method.pass_count(),
            cached_includes_verification: method.includes_verification(),
            cached_is_firmware: method.is_firmware(),
            id,
        })
    }

    fn with_inner<R>(&self, f: impl FnOnce(&dyn drivewipe_core::wipe::WipeMethod) -> R) -> R {
        let registry = WipeMethodRegistry::new();
        let method = registry
            .get(&self.id)
            .expect("MethodProxy: method must exist in registry");
        f(method)
    }
}

impl drivewipe_core::wipe::WipeMethod for MethodProxy {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.cached_name
    }

    fn description(&self) -> &str {
        &self.cached_description
    }

    fn pass_count(&self) -> u32 {
        self.cached_pass_count
    }

    fn pattern_for_pass(
        &self,
        pass: u32,
    ) -> Box<dyn drivewipe_core::wipe::patterns::PatternGenerator + Send> {
        self.with_inner(|m| m.pattern_for_pass(pass))
    }

    fn includes_verification(&self) -> bool {
        self.cached_includes_verification
    }

    fn is_firmware(&self) -> bool {
        self.cached_is_firmware
    }
}

fn print_wipe_summary(result: &WipeResult) {
    let style = match result.outcome {
        WipeOutcome::Success => console::style(format!("{}", result.outcome)).green().bold(),
        WipeOutcome::SuccessWithWarnings => console::style(format!("{}", result.outcome))
            .yellow()
            .bold(),
        _ => console::style(format!("{}", result.outcome)).red().bold(),
    };

    println!("{}", console::style("=== Wipe Summary ===").bold());
    println!("  Session ID : {}", result.session_id);
    println!("  Device     : {}", result.device_path.display());
    println!("  Model      : {}", result.device_model);
    println!("  Serial     : {}", result.device_serial);
    println!("  Capacity   : {}", format_bytes(result.device_capacity));
    println!(
        "  Method     : {} ({})",
        result.method_name, result.method_id
    );
    println!("  Outcome    : {}", style);
    println!("  Passes     : {}", result.passes.len());
    println!("  Duration   : {:.1}s", result.total_duration_secs,);
    println!(
        "  Throughput : {:.1} MiB/s (avg)",
        result.average_throughput_mbps,
    );
    println!(
        "  Written    : {}",
        format_bytes(result.total_bytes_written),
    );

    match result.verification_passed {
        Some(true) => println!(
            "  Verification: {}",
            console::style("PASSED").green().bold()
        ),
        Some(false) => println!("  Verification: {}", console::style("FAILED").red().bold()),
        None => println!("  Verification: not performed"),
    }

    if !result.warnings.is_empty() {
        println!();
        println!("  Warnings:");
        for w in &result.warnings {
            println!("    {} {}", console::style("!").yellow().bold(), w);
        }
    }
}

#[cfg(feature = "pdf-report")]
fn generate_pdf_report(result: &WipeResult, pdf_path: &str) -> Result<()> {
    use drivewipe_core::report::pdf::PdfReportGenerator;

    let generator = PdfReportGenerator;
    let pdf_bytes = generator
        .generate(result)
        .context("Failed to generate PDF report")?;
    std::fs::write(pdf_path, &pdf_bytes)
        .with_context(|| format!("Failed to write PDF report to {pdf_path}"))?;
    println!(
        "{} PDF report saved to {pdf_path}",
        console::style("report:").blue().bold(),
    );
    Ok(())
}

#[cfg(not(feature = "pdf-report"))]
fn generate_pdf_report(_result: &WipeResult, _pdf_path: &str) -> Result<()> {
    bail!(
        "PDF report generation is not available. \
         Rebuild with the `pdf-report` feature enabled."
    );
}

//! DriveWipe Command-Line Interface (CLI)
//!
//! The `drivewipe` binary provides a traditional POSIX-compatible interface
//! for secure sanitization and drive management. It is designed for both
//! interactive terminal use and integration into automation scripts.
//!
//! Use `drivewipe --help` for a full list of commands and flags.

use std::process;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::platform::privilege;
use drivewipe_core::session::CancellationToken;

mod commands;
mod confirm;
mod display;
mod progress;

// ── CLI definition ──────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "drivewipe")]
#[command(about = "Secure data sanitization tool \u{2014} NIST SP 800-88 / IEEE 2883 compliant")]
#[command(version)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Config file path override
    #[arg(long, global = true)]
    config: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// List detected drives
    List {
        /// Output format (table, json)
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Wipe a drive
    Wipe {
        /// Device path (e.g., /dev/sda)
        #[arg(short, long)]
        device: String,
        /// Wipe method ID
        #[arg(short, long)]
        method: String,
        /// Skip interactive confirmation (requires --yes-i-know-what-im-doing)
        #[arg(long)]
        force: bool,
        /// Required with --force
        #[arg(long)]
        yes_i_know_what_im_doing: bool,
        /// Run verification after wipe. Methods that mandate verification
        /// (DoD, NIST 800-88, HMG IS5, AFSSI/AR/NAVSO) are always verified.
        #[arg(long)]
        verify: Option<bool>,
        /// Verify the full surface after every pass, not just the last.
        /// Roughly doubles wipe time; produces per-pass evidence.
        #[arg(long)]
        verify_each_pass: bool,
        /// Generate PDF report to this path
        #[arg(long)]
        report_pdf: Option<String>,
        /// Dry run mode (no actual writes)
        #[arg(long)]
        dry_run: bool,
    },
    /// Verify a previously wiped drive
    Verify {
        /// Device path (e.g., /dev/sda)
        #[arg(short, long)]
        device: String,
        /// Expected pattern (zero, one, random)
        #[arg(long, default_value = "zero")]
        pattern: String,
    },
    /// Show detailed drive information
    Info {
        /// Device path (e.g., /dev/sda)
        #[arg(short, long)]
        device: String,
    },
    /// Generate or convert reports
    Report {
        /// Input JSON report file
        #[arg(short, long)]
        input: String,
        /// Output format (json, pdf)
        #[arg(long, default_value = "json")]
        format: String,
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Manage the wipe queue
    Queue {
        #[command(subcommand)]
        action: QueueAction,
    },
    /// Resume interrupted wipe sessions
    Resume {
        /// List all incomplete sessions
        #[arg(long)]
        list: bool,
        /// Resume a specific session by ID
        #[arg(long)]
        session: Option<String>,
        /// Auto-resume matching sessions
        #[arg(long)]
        auto: bool,
    },
    /// Check drive health (SMART/NVMe data)
    Health {
        /// Device path
        #[arg(short, long)]
        device: String,
        /// Save a health snapshot
        #[arg(long)]
        save: bool,
        /// Compare with a previous snapshot file
        #[arg(long)]
        compare: Option<String>,
    },
    /// Show matched drive profile
    Profile {
        /// Device path
        #[arg(short, long)]
        device: String,
    },
    /// Clone a drive
    Clone {
        /// Source device path
        #[arg(short, long)]
        source: String,
        /// Target device path or image file
        #[arg(short, long)]
        target: String,
        /// Clone mode (block, partition)
        #[arg(long, default_value = "block")]
        mode: String,
        /// Enable compression
        #[arg(long)]
        compress: bool,
        /// Enable encryption
        #[arg(long)]
        encrypt: bool,
    },
    /// Manage partitions
    Partition {
        #[command(subcommand)]
        action: PartitionAction,
    },
    /// Forensic analysis
    Forensic {
        #[command(subcommand)]
        action: ForensicAction,
    },
}

#[derive(Subcommand)]
enum QueueAction {
    /// Add a drive to the queue
    Add {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        method: String,
    },
    /// Start processing the queue
    Start {
        /// Number of drives to wipe in parallel
        #[arg(long)]
        parallel: Option<usize>,
    },
    /// Show queue status
    Status,
    /// Cancel all queued operations
    Cancel,
}

#[derive(Subcommand)]
enum PartitionAction {
    /// List partitions on a device
    List {
        #[arg(short, long)]
        device: String,
    },
    /// Create a new partition
    Create {
        #[arg(short, long)]
        device: String,
        #[arg(long)]
        start: u64,
        #[arg(long)]
        end: u64,
        #[arg(long)]
        type_id: String,
        #[arg(long)]
        name: String,
    },
    /// Delete a partition
    Delete {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        index: u32,
    },
    /// Resize a partition
    Resize {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        index: u32,
        #[arg(long)]
        new_end: u64,
    },
    /// Move a partition to a new start LBA
    Move {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        index: u32,
        #[arg(long)]
        new_start: u64,
    },
}

#[derive(Subcommand)]
enum ForensicAction {
    /// Run a forensic scan
    Scan {
        #[arg(short, long)]
        device: String,
    },
    /// Generate a forensic report
    Report {
        #[arg(short, long)]
        device: String,
        #[arg(short, long)]
        output: Option<String>,
    },
}

// ── Entry point ─────────────────────────────────────────────────────────────

/// Which interface this invocation should open.
#[derive(Debug, PartialEq)]
enum Mode {
    Cli,
    Tui,
    Gui,
}

/// Decide the interface from how the program was invoked.
///
/// In order of precedence: the name it was called as (so `drivewipe-tui` and
/// `drivewipe-gui` symlinks work), an explicit `--tui`/`--gui` flag, and
/// finally a bare invocation, which opens the TUI on a terminal and otherwise
/// falls through to the CLI so pipelines and `--help` still behave.
fn detect_mode(args: &[String]) -> Mode {
    use std::io::IsTerminal;
    let interactive = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    detect_mode_with(args, interactive)
}

/// The decision itself, with terminal detection passed in so it can be tested.
fn detect_mode_with(args: &[String], interactive: bool) -> Mode {
    let invoked_as = args
        .first()
        .map(std::path::Path::new)
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if invoked_as.ends_with("-tui") {
        return Mode::Tui;
    }
    if invoked_as.ends_with("-gui") {
        return Mode::Gui;
    }

    let rest = &args[args.len().min(1)..];
    if rest.iter().any(|a| a == "--gui") {
        return Mode::Gui;
    }
    if rest.iter().any(|a| a == "--tui") {
        return Mode::Tui;
    }

    if rest.is_empty() && interactive {
        return Mode::Tui;
    }

    Mode::Cli
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();

    match detect_mode(&args) {
        // iced drives a winit event loop, which must own the main thread and
        // must not be started from inside a tokio runtime — so the GUI is
        // dispatched before any runtime exists.
        #[cfg(feature = "gui")]
        Mode::Gui => {
            env_logger::init();
            if let Err(msg) = check_display_available() {
                eprintln!("error: {msg}");
                return std::process::ExitCode::FAILURE;
            }
            match drivewipe_gui::run() {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: could not open the DriveWipe window: {e}");
                    eprintln!("       The terminal interface works anywhere: drivewipe --tui");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        #[cfg(not(feature = "gui"))]
        Mode::Gui => {
            eprintln!(
                "error: this build of DriveWipe was compiled without the graphical \
                 interface.\n       Use the terminal interface instead: drivewipe --tui"
            );
            std::process::ExitCode::FAILURE
        }
        Mode::Tui => {
            env_logger::init();
            match tokio_runtime().block_on(drivewipe_tui::run()) {
                Ok(()) => std::process::ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        Mode::Cli => {
            tokio_runtime().block_on(cli_main());
            std::process::ExitCode::SUCCESS
        }
    }
}

/// Refuse to start the desktop interface when there is no display server.
///
/// Without this the windowing layer panics deep inside winit, which reads as a
/// crash rather than the ordinary situation it is — someone ran `--gui` over
/// SSH or on a headless box.
#[cfg(all(feature = "gui", unix, not(target_os = "macos")))]
fn check_display_available() -> Result<(), String> {
    let has_display = std::env::var_os("DISPLAY")
        .filter(|v| !v.is_empty())
        .is_some();
    let has_wayland = std::env::var_os("WAYLAND_DISPLAY")
        .filter(|v| !v.is_empty())
        .is_some();

    if has_display || has_wayland {
        Ok(())
    } else {
        Err(
            "no graphical display was found (neither DISPLAY nor WAYLAND_DISPLAY is set).\n       \
             If you are connected over SSH, use the terminal interface instead:\n       \
             drivewipe --tui"
                .to_string(),
        )
    }
}

#[cfg(all(feature = "gui", not(all(unix, not(target_os = "macos")))))]
fn check_display_available() -> Result<(), String> {
    Ok(())
}

fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to start the async runtime")
}

async fn cli_main() {
    let cli = Cli::parse_from(std::env::args().filter(|a| a != "--tui" && a != "--gui"));

    // Initialise logging. With --verbose we use debug level; otherwise honour
    // the existing RUST_LOG value or default to "info".
    {
        let default_level = if cli.verbose { "debug" } else { "info" };
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_level))
            .init();
    }

    if let Err(e) = run(cli).await {
        let console = console::Term::stderr();
        let _ = console.write_line(&format!("{} {}", console::style("error:").red().bold(), e));
        // Print the full error chain with --verbose / RUST_LOG=debug.
        for cause in e.chain().skip(1) {
            let _ = console.write_line(&format!(
                "  {} {}",
                console::style("caused by:").yellow(),
                cause,
            ));
        }
        process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    // Load configuration.
    let config = if let Some(ref path) = cli.config {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {path}"))?;
        toml::from_str::<DriveWipeConfig>(&contents)
            .with_context(|| format!("Failed to parse config file: {path}"))?
    } else {
        DriveWipeConfig::load().context("Failed to load configuration")?
    };

    // Privilege check -- warn for read-only commands, hard-fail for destructive ones.
    let needs_privilege = matches!(
        &cli.command,
        Commands::Wipe { .. }
            | Commands::Queue { .. }
            | Commands::Resume { .. }
            | Commands::Clone { .. }
            | Commands::Partition { .. }
    );
    if let Err(e) = privilege::check_privileges() {
        if needs_privilege {
            anyhow::bail!("Elevated privileges are required for this operation. {}", e);
        }
        log::warn!("{}", e);
        eprintln!("{} {}", console::style("warning:").yellow().bold(), e,);
    }

    // Global cancellation token shared with the Ctrl-C handler.
    let cancel_token = Arc::new(CancellationToken::new());
    {
        let ct = cancel_token.clone();
        ctrlc::set_handler(move || {
            eprintln!(
                "\n{} Interrupt received -- shutting down gracefully...",
                console::style("^C").red().bold(),
            );
            ct.cancel();
        })
        .context("Failed to install Ctrl-C handler")?;
    }

    match cli.command {
        Commands::List { format } => commands::list::run(&config, &format).await,
        Commands::Wipe {
            device,
            method,
            force,
            yes_i_know_what_im_doing,
            verify,
            verify_each_pass,
            report_pdf,
            dry_run,
        } => {
            commands::wipe::run(
                &config,
                &cancel_token,
                &device,
                &method,
                force,
                yes_i_know_what_im_doing,
                verify,
                verify_each_pass,
                report_pdf.as_deref(),
                dry_run,
            )
            .await
        }
        Commands::Verify { device, pattern } => {
            commands::verify::run(&config, &cancel_token, &device, &pattern).await
        }
        Commands::Info { device } => commands::info::run(&config, &device).await,
        Commands::Report {
            input,
            format,
            output,
        } => commands::report::run(&config, &input, &format, output.as_deref()).await,
        Commands::Queue { action } => match action {
            QueueAction::Add { device, method } => {
                commands::queue::add(&config, &device, &method).await
            }
            QueueAction::Start { parallel } => {
                commands::queue::start(&config, &cancel_token, parallel).await
            }
            QueueAction::Status => commands::queue::status(&config).await,
            QueueAction::Cancel => commands::queue::cancel(&config).await,
        },
        Commands::Resume {
            list,
            session,
            auto,
        } => commands::resume::run(&config, &cancel_token, list, session.as_deref(), auto).await,
        Commands::Health {
            device,
            save,
            compare,
        } => commands::health::run(&config, &device, save, compare.as_deref()).await,
        Commands::Profile { device } => commands::profile::run(&config, &device).await,
        Commands::Clone {
            source,
            target,
            mode,
            compress,
            encrypt,
        } => {
            commands::clone::run(
                &config,
                &cancel_token,
                &source,
                &target,
                &mode,
                compress,
                encrypt,
            )
            .await
        }
        Commands::Partition { action } => match action {
            PartitionAction::List { device } => commands::partition::list(&config, &device).await,
            PartitionAction::Create {
                device,
                start,
                end,
                type_id,
                name,
            } => commands::partition::create(&config, &device, start, end, &type_id, &name).await,
            PartitionAction::Delete { device, index } => {
                commands::partition::delete(&config, &device, index).await
            }
            PartitionAction::Resize {
                device,
                index,
                new_end,
            } => commands::partition::resize(&config, &device, index, new_end).await,
            PartitionAction::Move {
                device,
                index,
                new_start,
            } => commands::partition::move_partition(&config, &device, index, new_start).await,
        },
        Commands::Forensic { action } => match action {
            ForensicAction::Scan { device } => {
                commands::forensic::scan(&config, &cancel_token, &device).await
            }
            ForensicAction::Report { device, output } => {
                commands::forensic::report(&config, &cancel_token, &device, output.as_deref()).await
            }
        },
    }
}

#[cfg(test)]
mod mode_tests {
    use super::{Mode, detect_mode_with};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bare_invocation_on_a_terminal_opens_the_tui() {
        assert_eq!(detect_mode_with(&args(&["drivewipe"]), true), Mode::Tui);
    }

    #[test]
    fn bare_invocation_without_a_terminal_stays_on_the_cli() {
        // `drivewipe | cat`, cron, and CI must not try to paint a dashboard.
        assert_eq!(detect_mode_with(&args(&["drivewipe"]), false), Mode::Cli);
    }

    #[test]
    fn a_subcommand_always_means_the_cli() {
        assert_eq!(
            detect_mode_with(&args(&["drivewipe", "list"]), true),
            Mode::Cli
        );
        assert_eq!(
            detect_mode_with(&args(&["drivewipe", "wipe", "--device", "/dev/sda"]), true),
            Mode::Cli
        );
    }

    #[test]
    fn help_and_version_reach_the_cli_even_on_a_terminal() {
        for flag in ["--help", "-h", "--version", "-V"] {
            assert_eq!(
                detect_mode_with(&args(&["drivewipe", flag]), true),
                Mode::Cli,
                "{flag} should print CLI output, not open the TUI"
            );
        }
    }

    #[test]
    fn explicit_flags_win_over_terminal_detection() {
        assert_eq!(
            detect_mode_with(&args(&["drivewipe", "--gui"]), false),
            Mode::Gui
        );
        assert_eq!(
            detect_mode_with(&args(&["drivewipe", "--tui"]), false),
            Mode::Tui
        );
    }

    #[test]
    fn the_name_it_was_invoked_as_selects_the_interface() {
        // The installer ships these as symlinks to the one binary.
        assert_eq!(
            detect_mode_with(&args(&["drivewipe-tui"]), false),
            Mode::Tui
        );
        assert_eq!(
            detect_mode_with(&args(&["drivewipe-gui"]), false),
            Mode::Gui
        );
        assert_eq!(
            detect_mode_with(&args(&["/usr/local/bin/drivewipe-gui"]), false),
            Mode::Gui
        );
    }

    #[test]
    fn a_windows_exe_suffix_still_dispatches() {
        assert_eq!(
            detect_mode_with(
                &args(&["C:\\Program Files\\DriveWipe\\drivewipe-gui.exe"]),
                false
            ),
            Mode::Gui
        );
    }

    #[test]
    fn the_invoked_name_outranks_a_conflicting_flag() {
        assert_eq!(
            detect_mode_with(&args(&["drivewipe-tui", "--gui"]), false),
            Mode::Tui
        );
    }

    #[test]
    fn an_empty_argv_does_not_panic() {
        assert_eq!(detect_mode_with(&[], false), Mode::Cli);
        assert_eq!(detect_mode_with(&[], true), Mode::Tui);
    }
}

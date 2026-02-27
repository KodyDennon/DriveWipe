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
        /// Run verification after wipe
        #[arg(long)]
        verify: Option<bool>,
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

// ── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    // Initialise logging. With --verbose we use debug level; otherwise honour
    // the existing RUST_LOG value or default to "info".
    {
        let default_level = if cli.verbose { "debug" } else { "info" };
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or(default_level),
        )
        .init();
    }

    if let Err(e) = run(cli) {
        let console = console::Term::stderr();
        let _ = console.write_line(&format!(
            "{} {}",
            console::style("error:").red().bold(),
            e
        ));
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

fn run(cli: Cli) -> Result<()> {
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
        Commands::Wipe { .. } | Commands::Queue { .. } | Commands::Resume { .. }
    );
    if let Err(e) = privilege::check_privileges() {
        if needs_privilege {
            anyhow::bail!(
                "Elevated privileges are required for this operation. {}",
                e
            );
        }
        log::warn!("{}", e);
        eprintln!(
            "{} {}",
            console::style("warning:").yellow().bold(),
            e,
        );
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
        Commands::List { format } => {
            commands::list::run(&config, &format)
        }
        Commands::Wipe {
            device,
            method,
            force,
            yes_i_know_what_im_doing,
            verify,
            report_pdf,
            dry_run,
        } => commands::wipe::run(
            &config,
            &cancel_token,
            &device,
            &method,
            force,
            yes_i_know_what_im_doing,
            verify,
            report_pdf.as_deref(),
            dry_run,
        ),
        Commands::Verify { device, pattern } => {
            commands::verify::run(&config, &cancel_token, &device, &pattern)
        }
        Commands::Info { device } => {
            commands::info::run(&config, &device)
        }
        Commands::Report {
            input,
            format,
            output,
        } => commands::report::run(&config, &input, &format, output.as_deref()),
        Commands::Queue { action } => match action {
            QueueAction::Add { device, method } => {
                commands::queue::add(&config, &device, &method)
            }
            QueueAction::Start { parallel } => {
                commands::queue::start(&config, &cancel_token, parallel)
            }
            QueueAction::Status => commands::queue::status(&config),
            QueueAction::Cancel => commands::queue::cancel(&config),
        },
        Commands::Resume {
            list,
            session,
            auto,
        } => commands::resume::run(&config, &cancel_token, list, session.as_deref(), auto),
    }
}

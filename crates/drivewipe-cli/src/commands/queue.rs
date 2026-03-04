use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use drivewipe_core::config::DriveWipeConfig;
use drivewipe_core::session::CancellationToken;

// ── Queue data model ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub device: String,
    pub method: String,
    pub status: QueueEntryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueEntryStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for QueueEntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::InProgress => write!(f, "In Progress"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WipeQueue {
    pub entries: Vec<QueueEntry>,
}

// ── Queue file helpers ──────────────────────────────────────────────────────

fn queue_path(config: &DriveWipeConfig) -> PathBuf {
    config.sessions_dir().join("queue.json")
}

fn load_queue(config: &DriveWipeConfig) -> Result<WipeQueue> {
    let path = queue_path(config);
    if !path.exists() {
        return Ok(WipeQueue::default());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read queue file: {}", path.display()))?;
    let queue: WipeQueue = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse queue file: {}", path.display()))?;
    Ok(queue)
}

fn save_queue(config: &DriveWipeConfig, queue: &WipeQueue) -> Result<()> {
    let path = queue_path(config);
    std::fs::create_dir_all(
        path.parent()
            .expect("queue path must have a parent directory"),
    )
    .context("Failed to create sessions directory")?;

    let json = serde_json::to_string_pretty(queue).context("Failed to serialise queue")?;
    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write queue file: {}", path.display()))?;
    Ok(())
}

// ── Commands ────────────────────────────────────────────────────────────────

/// `drivewipe queue add`
pub async fn add(config: &DriveWipeConfig, device: &str, method: &str) -> Result<()> {
    // Validate that the method exists.
    let registry = drivewipe_core::wipe::WipeMethodRegistry::new();
    if registry.get(method).is_none() {
        bail!("Unknown wipe method: {method}");
    }

    let mut queue = load_queue(config)?;

    // Check for duplicates.
    if queue
        .entries
        .iter()
        .any(|e| e.device == device && e.status == QueueEntryStatus::Pending)
    {
        bail!("Device {device} is already in the queue with status Pending");
    }

    queue.entries.push(QueueEntry {
        device: device.to_string(),
        method: method.to_string(),
        status: QueueEntryStatus::Pending,
    });

    save_queue(config, &queue)?;

    println!(
        "{} Added {} (method: {}) to the wipe queue.",
        console::style("+").green().bold(),
        device,
        method,
    );

    Ok(())
}

/// `drivewipe queue start`
pub async fn start(
    config: &DriveWipeConfig,
    cancel_token: &Arc<CancellationToken>,
    parallel: Option<usize>,
) -> Result<()> {
    let mut queue = load_queue(config)?;

    let pending: Vec<usize> = queue
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.status == QueueEntryStatus::Pending)
        .map(|(i, _)| i)
        .collect();

    if pending.is_empty() {
        println!("No pending entries in the queue.");
        return Ok(());
    }

    let max_parallel = parallel.unwrap_or(config.parallel_drives).max(1);

    println!(
        "{} Processing {} queued wipe(s) (parallelism: {})...",
        console::style("==>").green().bold(),
        pending.len(),
        max_parallel,
    );

    // Process entries in parallel using tokio tasks and JoinSet.
    // For simplicity we chunk the pending indices and process each chunk
    // concurrently using JoinSet.
    use tokio::task::JoinSet;

    for chunk in pending.chunks(max_parallel) {
        if cancel_token.is_cancelled() {
            // Mark remaining as cancelled.
            for &idx in chunk {
                queue.entries[idx].status = QueueEntryStatus::Cancelled;
            }
            save_queue(config, &queue)?;
            bail!("Queue processing cancelled by user");
        }

        // Mark entries as in-progress.
        for &idx in chunk {
            queue.entries[idx].status = QueueEntryStatus::InProgress;
        }
        save_queue(config, &queue)?;

        // Run each wipe in a tokio task using JoinSet.
        let mut set = JoinSet::new();
        for &idx in chunk {
            let entry = &queue.entries[idx];
            let ct = cancel_token.clone();
            let cfg = config.clone();
            let dev = entry.device.clone();
            let mth = entry.method.clone();
            
            set.spawn(async move {
                let res = super::wipe::run(
                    &cfg, &ct, &dev, &mth, true,  // force
                    true,  // yes_i_know
                    None,  // verify override
                    None,  // pdf report
                    false, // dry_run
                ).await;
                (idx, res)
            });
        }

        // Collect results as they finish.
        while let Some(res) = set.join_next().await {
            match res {
                Ok((idx, result)) => {
                    queue.entries[idx].status = match result {
                        Ok(()) => QueueEntryStatus::Completed,
                        Err(ref e) => {
                            eprintln!(
                                "{} Queue entry {} failed: {e}",
                                console::style("error:").red().bold(),
                                queue.entries[idx].device,
                            );
                            QueueEntryStatus::Failed
                        }
                    };
                }
                Err(e) => {
                    log::error!("Queue task panicked or failed: {e}");
                }
            }
        }
        
        // Save state after each chunk.
        save_queue(config, &queue)?;
    }

    // Print final summary.
    println!();
    println!("{}", console::style("=== Queue Summary ===").bold());
    let completed = queue
        .entries
        .iter()
        .filter(|e| e.status == QueueEntryStatus::Completed)
        .count();
    let failed = queue
        .entries
        .iter()
        .filter(|e| e.status == QueueEntryStatus::Failed)
        .count();
    let cancelled = queue
        .entries
        .iter()
        .filter(|e| e.status == QueueEntryStatus::Cancelled)
        .count();
    println!("  Completed : {completed}");
    println!("  Failed    : {failed}");
    println!("  Cancelled : {cancelled}");

    Ok(())
}

/// `drivewipe queue status`
pub async fn status(config: &DriveWipeConfig) -> Result<()> {
    let queue = load_queue(config)?;

    if queue.entries.is_empty() {
        println!("Queue is empty.");
        return Ok(());
    }

    println!("{}", console::style("=== Wipe Queue ===").bold());
    println!(
        "  {:<20} {:<16} {}",
        console::style("DEVICE").bold(),
        console::style("METHOD").bold(),
        console::style("STATUS").bold(),
    );
    println!("  {}", "-".repeat(56));

    for entry in &queue.entries {
        let status_style = match entry.status {
            QueueEntryStatus::Pending => console::style(entry.status.to_string()).cyan(),
            QueueEntryStatus::InProgress => console::style(entry.status.to_string()).yellow(),
            QueueEntryStatus::Completed => console::style(entry.status.to_string()).green(),
            QueueEntryStatus::Failed => console::style(entry.status.to_string()).red(),
            QueueEntryStatus::Cancelled => console::style(entry.status.to_string()).dim(),
        };

        println!(
            "  {:<20} {:<16} {}",
            entry.device, entry.method, status_style
        );
    }

    let pending = queue
        .entries
        .iter()
        .filter(|e| e.status == QueueEntryStatus::Pending)
        .count();
    println!();
    println!("  {pending} pending, {} total", queue.entries.len());

    Ok(())
}

/// `drivewipe queue cancel`
pub async fn cancel(config: &DriveWipeConfig) -> Result<()> {
    let mut queue = load_queue(config)?;

    let mut cancelled = 0;
    for entry in &mut queue.entries {
        if entry.status == QueueEntryStatus::Pending || entry.status == QueueEntryStatus::InProgress
        {
            entry.status = QueueEntryStatus::Cancelled;
            cancelled += 1;
        }
    }

    save_queue(config, &queue)?;

    if cancelled == 0 {
        println!("No active entries to cancel.");
    } else {
        println!(
            "{} Cancelled {cancelled} queue entry/entries.",
            console::style("cancelled:").yellow().bold(),
        );
    }

    Ok(())
}

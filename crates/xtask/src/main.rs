mod release;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use toml_edit::{DocumentMut, value};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Project automation tasks", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bumps crate versions based on git history and LOC
    Bump,
    /// Interactive release wizard — build, tag, and publish releases
    Release,
    /// Build the DriveWipe Live environment (ISO + PXE artifacts)
    LiveBuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BumpLevel {
    None,
    Patch,
    Minor,
    Major,
}

impl BumpLevel {
    fn to_string(self) -> &'static str {
        match self {
            BumpLevel::None => "none",
            BumpLevel::Patch => "patch",
            BumpLevel::Minor => "minor",
            BumpLevel::Major => "major",
        }
    }
}

const PATCH_LOC_THRESHOLD: u32 = 250;
const MINOR_LOC_THRESHOLD: u32 = 1000;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bump => bump_versions()?,
        Commands::Release => release::run()?,
        Commands::LiveBuild => live_build()?,
    }

    Ok(())
}

fn bump_versions() -> Result<()> {
    println!("🔍 Analyzing git history and LOC for version bumps...");

    let crates = vec![
        "drivewipe-core",
        "drivewipe-cli",
        "drivewipe-tui",
        "drivewipe-gui",
        "drivewipe-live",
    ];
    let mut bumps = HashMap::new();

    for &krate in &crates {
        let loc = get_loc_changes(krate)?;
        let commit_bump = get_commit_bump_level(krate)?;

        let mut final_bump = commit_bump;

        // LOC Overrides (Safety Triggers)
        if loc > MINOR_LOC_THRESHOLD && final_bump < BumpLevel::Minor {
            println!(
                "🚀 {} crossed {} LOC threshold. Promoting to Minor.",
                krate, MINOR_LOC_THRESHOLD
            );
            final_bump = BumpLevel::Minor;
        } else if loc > PATCH_LOC_THRESHOLD && final_bump < BumpLevel::Patch {
            println!(
                "📦 {} crossed {} LOC threshold. Promoting to Patch.",
                krate, PATCH_LOC_THRESHOLD
            );
            final_bump = BumpLevel::Patch;
        }

        if final_bump != BumpLevel::None {
            bumps.insert(krate, final_bump);
        }
    }

    if bumps.is_empty() {
        println!("✅ No version bumps required.");
        return Ok(());
    }

    for (krate, level) in bumps {
        apply_bump(krate, level)?;
    }

    Ok(())
}

fn get_loc_changes(krate: &str) -> Result<u32> {
    let path = format!("crates/{}", krate);
    let output = Command::new("git")
        .args(["diff", "--numstat", "origin/main..HEAD", "--", &path])
        .output()
        .context("Failed to run git diff")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut total = 0;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let additions: u32 = parts[0].parse().unwrap_or(0);
            let deletions: u32 = parts[1].parse().unwrap_or(0);
            total += additions + deletions;
        }
    }

    Ok(total)
}

fn get_commit_bump_level(krate: &str) -> Result<BumpLevel> {
    let scope = krate.replace("drivewipe-", "");
    let output = Command::new("git")
        .args(["log", "origin/main..HEAD", "--format=%s"])
        .output()
        .context("Failed to run git log")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut max_level = BumpLevel::None;

    for line in stdout.lines() {
        let line = line.to_lowercase();

        // Check for major manual triggers (Global or Scoped)
        if (line.contains("major-release") || line.contains("breaking change"))
            && (line.contains(&format!("({})", scope)) || !line.contains('('))
        {
            return Ok(BumpLevel::Major);
        }

        // Check for scoped commits
        if line.contains(&format!("({})", scope)) {
            if line.starts_with("feat") {
                max_level = max_level.max(BumpLevel::Minor);
            } else if line.starts_with("fix")
                || line.starts_with("chore")
                || line.starts_with("refactor")
            {
                max_level = max_level.max(BumpLevel::Patch);
            }
        }
    }

    Ok(max_level)
}

fn apply_bump(krate: &str, level: BumpLevel) -> Result<()> {
    let path = format!("crates/{}/Cargo.toml", krate);
    let content = fs::read_to_string(&path)?;
    let mut doc = content
        .parse::<DocumentMut>()
        .context("Failed to parse Cargo.toml")?;

    let current_version = doc["package"]["version"]
        .as_str()
        .context("Missing version in Cargo.toml")?;

    let new_version = bump_semver(current_version, level)?;

    println!(
        "🆙 Bumping {} from {} to {} ({})",
        krate,
        current_version,
        new_version,
        level.to_string()
    );

    doc["package"]["version"] = value(new_version);
    fs::write(path, doc.to_string())?;

    Ok(())
}

fn bump_semver(version: &str, level: BumpLevel) -> Result<String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid version format: {}", version));
    }

    let mut major: u32 = parts[0].parse()?;
    let mut minor: u32 = parts[1].parse()?;
    let mut patch: u32 = parts[2].parse()?;

    match level {
        BumpLevel::Major => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        BumpLevel::Minor => {
            minor += 1;
            patch = 0;
        }
        BumpLevel::Patch => {
            patch += 1;
        }
        BumpLevel::None => {}
    }

    Ok(format!("{}.{}.{}", major, minor, patch))
}

fn live_build() -> Result<()> {
    println!("🔨 Building DriveWipe Live environment...");
    println!();

    // Locate the repository root (two levels up from crates/xtask/).
    let workspace_root = std::env::current_dir().context("Failed to get current directory")?;

    let build_script = workspace_root.join("scripts/build-live.sh");
    if !build_script.exists() {
        anyhow::bail!(
            "Build script not found at {}\nRun this command from the repository root.",
            build_script.display()
        );
    }

    // Read the live crate version to stamp the ISO.
    let live_cargo_toml = workspace_root.join("crates/drivewipe-live/Cargo.toml");
    let live_version = if live_cargo_toml.exists() {
        let content = fs::read_to_string(&live_cargo_toml)?;
        let doc = content
            .parse::<DocumentMut>()
            .context("Failed to parse drivewipe-live Cargo.toml")?;
        doc["package"]["version"]
            .as_str()
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    };

    println!("  Live version: {}", live_version);
    println!("  Script: {}", build_script.display());
    println!();

    let status = Command::new("bash")
        .arg(&build_script)
        .env("DRIVEWIPE_LIVE_VERSION", &live_version)
        .current_dir(&workspace_root)
        .status()
        .context("Failed to execute build-live.sh")?;

    if !status.success() {
        let code = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        anyhow::bail!("build-live.sh exited with code {}", code);
    }

    println!();
    println!("✅ DriveWipe Live v{} built successfully!", live_version);
    println!("   Output: output/drivewipe-live-{}.iso", live_version);
    Ok(())
}

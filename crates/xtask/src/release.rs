use anyhow::{Context, Result, bail};
use console::{Style, style};
use dialoguer::{Confirm, Input, Select};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use toml_edit::{DocumentMut, value};

// ── Styles ──────────────────────────────────────────────────────────────────

fn header(text: &str) {
    let s = Style::new().bold().cyan();
    println!();
    println!("{}", s.apply_to(format!("=== {text} ===")));
    println!();
}

fn step(num: u8, text: &str) {
    let s = Style::new().bold().white();
    println!("{}", s.apply_to(format!("[{num}] {text}")));
}

fn success(text: &str) {
    println!("  {} {text}", style("✔").green().bold());
}

fn skipped(text: &str) {
    println!("  {} {text}", style("⊘").yellow().bold());
}

fn error_line(text: &str) {
    println!("  {} {text}", style("✘").red().bold());
}

// ── Platform detection ──────────────────────────────────────────────────────

struct Platform {
    os: &'static str,
    arch: &'static str,
    target: String,
    exe_suffix: &'static str,
    archive_ext: &'static str,
}

fn detect_platform() -> Result<Platform> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        other => bail!("Unsupported OS: {other}"),
    };

    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => bail!("Unsupported architecture: {other}"),
    };

    let target = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        _ => bail!("Unsupported platform: {os}-{arch}"),
    };

    let exe_suffix = if os == "windows" { ".exe" } else { "" };
    let archive_ext = if os == "windows" { "zip" } else { "tar.gz" };

    Ok(Platform {
        os,
        arch,
        target: target.to_string(),
        exe_suffix,
        archive_ext,
    })
}

// ── Shell helpers ───────────────────────────────────────────────────────────

fn cmd_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("Failed to run: {program} {}", args.join(" ")))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn cmd_ok(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn cmd_run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("Failed to run: {program} {}", args.join(" ")))?;
    if !status.success() {
        bail!("{program} {} exited with {status}", args.join(" "));
    }
    Ok(())
}

fn cmd_run_quiet(program: &str, args: &[&str]) -> Result<bool> {
    let status = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("Failed to run: {program} {}", args.join(" ")))?;
    Ok(status.success())
}

// ── Spinner helper ──────────────────────────────────────────────────────────

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

// ── Version helpers ─────────────────────────────────────────────────────────

fn get_current_version() -> Result<String> {
    let content = fs::read_to_string("crates/drivewipe-core/Cargo.toml")
        .context("Cannot read crates/drivewipe-core/Cargo.toml — are you in the repo root?")?;
    let doc: DocumentMut = content.parse().context("Failed to parse Cargo.toml")?;
    let version = doc["package"]["version"]
        .as_str()
        .context("Missing version field")?;
    Ok(version.to_string())
}

fn bump_semver(version: &str, kind: &str) -> Result<String> {
    let parts: Vec<u32> = version
        .split('.')
        .map(|p| p.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .context("Invalid semver")?;
    if parts.len() != 3 {
        bail!("Invalid version format: {version}");
    }
    let (major, minor, patch) = (parts[0], parts[1], parts[2]);
    Ok(match kind {
        "major" => format!("{}.0.0", major + 1),
        "minor" => format!("{}.{}.0", major, minor + 1),
        "patch" => format!("{}.{}.{}", major, minor, patch + 1),
        _ => bail!("Unknown bump kind: {kind}"),
    })
}

fn auto_detect_bump() -> Result<String> {
    let last_tag = cmd_output("git", &["describe", "--tags", "--abbrev=0"]).unwrap_or_default();
    let range = if last_tag.is_empty() {
        "HEAD".to_string()
    } else {
        format!("{last_tag}..HEAD")
    };
    let log = cmd_output("git", &["log", &range, "--pretty=%s"])?;

    if log
        .lines()
        .any(|l| l.to_lowercase().contains("feat!") || l.to_lowercase().contains("breaking"))
    {
        Ok("major".into())
    } else if log.lines().any(|l| l.to_lowercase().starts_with("feat")) {
        Ok("minor".into())
    } else {
        Ok("patch".into())
    }
}

fn set_workspace_version(ver: &str) -> Result<()> {
    for entry in fs::read_dir("crates")? {
        let entry = entry?;
        let toml_path = entry.path().join("Cargo.toml");
        if !toml_path.exists() {
            continue;
        }
        let content = fs::read_to_string(&toml_path)?;
        let mut doc: DocumentMut = content.parse()?;
        if doc.get("package").and_then(|p| p.get("version")).is_some() {
            doc["package"]["version"] = value(ver);
            fs::write(&toml_path, doc.to_string())?;
        }
    }
    Ok(())
}

// ── Build & Package ─────────────────────────────────────────────────────────

struct BuildResult {
    archive_path: PathBuf,
    checksum_path: PathBuf,
    gui_built: bool,
}

fn build_and_package(tag: &str, platform: &Platform) -> Result<BuildResult> {
    step(4, "Building release binaries");

    // Ensure target is installed
    let installed = cmd_output("rustup", &["target", "list", "--installed"])?;
    if !installed.lines().any(|l| l.trim() == platform.target) {
        let sp = spinner(&format!("Adding Rust target: {}", platform.target));
        cmd_run("rustup", &["target", "add", &platform.target])?;
        sp.finish_and_clear();
        success(&format!("Added target {}", platform.target));
    }

    // Build CLI
    let sp = spinner("Building drivewipe-cli...");
    cmd_run(
        "cargo",
        &[
            "build",
            "--release",
            "--target",
            &platform.target,
            "--package",
            "drivewipe-cli",
        ],
    )?;
    sp.finish_and_clear();
    success("drivewipe-cli");

    // Build TUI
    let sp = spinner("Building drivewipe-tui...");
    cmd_run(
        "cargo",
        &[
            "build",
            "--release",
            "--target",
            &platform.target,
            "--package",
            "drivewipe-tui",
        ],
    )?;
    sp.finish_and_clear();
    success("drivewipe-tui");

    // Build GUI (non-fatal)
    let sp = spinner("Building drivewipe-gui...");
    let gui_ok = cmd_run_quiet(
        "cargo",
        &[
            "build",
            "--release",
            "--target",
            &platform.target,
            "--package",
            "drivewipe-gui",
        ],
    )?;
    sp.finish_and_clear();
    if gui_ok {
        success("drivewipe-gui");
    } else {
        skipped("drivewipe-gui (build failed — may need platform GUI libs)");
    }

    // ── Package ─────────────────────────────────────────────────────────────

    step(5, "Packaging artifacts");

    let dist_dir = PathBuf::from("target/dist");
    let archive_name = format!("drivewipe-{tag}-{}", platform.target);

    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir)?;
    }
    fs::create_dir_all(&dist_dir)?;

    let bin_dir = PathBuf::from(format!("target/{}/release", platform.target));

    // Copy CLI binary
    let cli_bin = format!("drivewipe{}", platform.exe_suffix);
    fs::copy(bin_dir.join(&cli_bin), dist_dir.join(&cli_bin))
        .context("CLI binary not found — build may have failed")?;

    // Copy TUI binary
    let tui_bin = format!("drivewipe-tui{}", platform.exe_suffix);
    fs::copy(bin_dir.join(&tui_bin), dist_dir.join(&tui_bin))
        .context("TUI binary not found — build may have failed")?;

    // Copy GUI binary (optional)
    let gui_bin = format!("drivewipe-gui{}", platform.exe_suffix);
    if gui_ok {
        let _ = fs::copy(bin_dir.join(&gui_bin), dist_dir.join(&gui_bin));
    }

    // Copy docs
    if Path::new("LICENSE.md").exists() {
        let _ = fs::copy("LICENSE.md", dist_dir.join("LICENSE.md"));
    }
    if Path::new("README.md").exists() {
        let _ = fs::copy("README.md", dist_dir.join("README.md"));
    }

    // Create archive
    let archive_filename = format!("{archive_name}.{}", platform.archive_ext);
    let archive_path = std::env::current_dir()?.join(&archive_filename);

    let sp = spinner(&format!("Creating {archive_filename}..."));
    if platform.archive_ext == "zip" {
        cmd_run("zip", &["-qj", archive_path.to_str().unwrap(), &format!("{}/", dist_dir.display())])?;
    } else {
        cmd_run(
            "tar",
            &[
                "czf",
                archive_path.to_str().unwrap(),
                "-C",
                dist_dir.to_str().unwrap(),
                ".",
            ],
        )?;
    }
    sp.finish_and_clear();
    success(&archive_filename);

    // Generate checksum
    let checksum_filename = format!("{archive_name}.sha256");
    let checksum_path = std::env::current_dir()?.join(&checksum_filename);

    let hash = if cmd_ok("sha256sum", &["--version"]) {
        cmd_output("sha256sum", &[archive_path.to_str().unwrap()])?
    } else {
        cmd_output("shasum", &["-a", "256", archive_path.to_str().unwrap()])?
    };

    // Write checksum with just the filename (not full path)
    let hash_value = hash.split_whitespace().next().unwrap_or("");
    fs::write(
        &checksum_path,
        format!("{hash_value}  {archive_filename}\n"),
    )?;
    success(&checksum_filename);

    Ok(BuildResult {
        archive_path,
        checksum_path,
        gui_built: gui_ok,
    })
}

// ── Flow A: New Release ─────────────────────────────────────────────────────

fn flow_new_release(platform: &Platform) -> Result<()> {
    header("New Release");

    // ── Step 1: Pre-flight checks ───────────────────────────────────────────
    step(1, "Pre-flight checks");

    // Clean working tree (excluding .claude directory)
    let status = cmd_output("git", &["status", "--porcelain"])?;
    let dirty: Vec<&str> = status
        .lines()
        .filter(|l| !l.contains(".claude"))
        .collect();
    if !dirty.is_empty() {
        error_line("Working tree is dirty:");
        for line in &dirty {
            println!("    {line}");
        }
        bail!("Commit or stash your changes first.");
    }
    success("Clean working tree");

    // On main branch
    let branch = cmd_output("git", &["branch", "--show-current"])?;
    if branch != "main" {
        bail!("Must be on 'main' branch (currently on '{branch}').");
    }
    success("On main branch");

    // Required tools
    for tool in &["cargo", "git", "gh"] {
        if !cmd_ok(tool, &["--version"]) {
            bail!("Required tool not found: {tool}");
        }
    }
    success("Required tools available (cargo, git, gh)");

    // gh auth
    if !cmd_ok("gh", &["auth", "status"]) {
        error_line("GitHub CLI not authenticated");
        println!("    Run: gh auth login");
        bail!("GitHub CLI authentication required.");
    }
    success("GitHub CLI authenticated");

    println!(
        "  {} Platform: {} ({}) → {}",
        style("ℹ").blue().bold(),
        platform.os,
        platform.arch,
        platform.target
    );

    // ── Step 2: Version selection ───────────────────────────────────────────
    step(2, "Version selection");

    let current = get_current_version()?;
    let auto = auto_detect_bump()?;
    println!("  Current version: {}", style(&current).yellow());
    println!(
        "  Auto-detected bump: {}",
        style(&auto).cyan()
    );
    println!();

    let choices = &[
        format!("Auto ({auto}) → {}", bump_semver(&current, &auto)?),
        format!("patch → {}", bump_semver(&current, "patch")?),
        format!("minor → {}", bump_semver(&current, "minor")?),
        format!("major → {}", bump_semver(&current, "major")?),
        "Enter exact version".to_string(),
    ];

    let selection = Select::new()
        .with_prompt("How should the version be bumped?")
        .items(choices)
        .default(0)
        .interact()?;

    let new_version = match selection {
        0 => bump_semver(&current, &auto)?,
        1 => bump_semver(&current, "patch")?,
        2 => bump_semver(&current, "minor")?,
        3 => bump_semver(&current, "major")?,
        4 => {
            let v: String = Input::new()
                .with_prompt("Enter version (e.g. 2.0.0)")
                .interact_text()?;
            // Validate format
            let parts: Vec<&str> = v.split('.').collect();
            if parts.len() != 3 || parts.iter().any(|p| p.parse::<u32>().is_err()) {
                bail!("Invalid version format: {v} (expected X.Y.Z)");
            }
            v
        }
        _ => unreachable!(),
    };

    let tag = format!("v{new_version}");

    // Check tag doesn't already exist
    if cmd_ok("git", &["rev-parse", &tag]) {
        bail!(
            "Tag '{tag}' already exists. Use \"Attach to existing release\" to add builds."
        );
    }

    // ── Step 3: Confirm ─────────────────────────────────────────────────────
    step(3, "Confirm release");

    println!("  Version: {} → {}", style(&current).red(), style(&new_version).green());
    println!("  Tag:     {}", style(&tag).green());
    println!();

    if !Confirm::new()
        .with_prompt("Proceed with build?")
        .default(true)
        .interact()?
    {
        println!("Aborted.");
        return Ok(());
    }

    // ── Steps 4–5: Build & Package ──────────────────────────────────────────
    let build = build_and_package(&tag, platform)?;

    // ── Step 6: Version bump + tag + push ───────────────────────────────────
    step(6, "Version bump, tag, and push");

    println!("  Updating all Cargo.toml versions to {new_version}...");
    set_workspace_version(&new_version)?;
    success("Cargo.toml versions updated");

    println!();
    if !Confirm::new()
        .with_prompt("Commit version bump, create tag, and push to origin?")
        .default(true)
        .interact()?
    {
        println!("  Rolling back version changes...");
        cmd_run("git", &["checkout", "--", "crates/"])?;
        println!("Aborted. Archive files remain in the repo root if you want them.");
        return Ok(());
    }

    let sp = spinner("Committing and tagging...");
    cmd_run("git", &["add", "crates/"])?;
    // Also stage Cargo.lock if it exists and changed
    let _ = cmd_run_quiet("git", &["add", "Cargo.lock"]);
    cmd_run(
        "git",
        &[
            "commit",
            "-m",
            &format!("chore(release): bump version to {new_version}"),
        ],
    )?;
    cmd_run("git", &["tag", "-a", &tag, "-m", &format!("Release {tag}")])?;
    sp.finish_and_clear();
    success(&format!("Committed and tagged {tag}"));

    let sp = spinner("Pushing to origin...");
    cmd_run("git", &["push", "origin", "main"])?;
    cmd_run("git", &["push", "origin", &tag])?;
    sp.finish_and_clear();
    success("Pushed to origin");

    // ── Step 7: GitHub release ──────────────────────────────────────────────
    step(7, "Create GitHub release");
    println!();

    if !Confirm::new()
        .with_prompt("Create GitHub release and upload artifacts?")
        .default(true)
        .interact()?
    {
        println!("Skipped. Tag {tag} is pushed — create the release manually:");
        println!("  gh release create {tag} {} {}",
            build.archive_path.display(),
            build.checksum_path.display());
        return Ok(());
    }

    let release_notes = build_release_notes(&tag, platform, &build);

    let sp = spinner("Creating GitHub release...");
    cmd_run(
        "gh",
        &[
            "release",
            "create",
            &tag,
            build.archive_path.to_str().unwrap(),
            build.checksum_path.to_str().unwrap(),
            "--title",
            &format!("DriveWipe {tag}"),
            "--notes",
            &release_notes,
        ],
    )?;
    sp.finish_and_clear();
    success("GitHub release created");

    // ── Summary ─────────────────────────────────────────────────────────────
    let url = cmd_output("gh", &["release", "view", &tag, "--json", "url", "-q", ".url"])
        .unwrap_or_else(|_| "https://github.com/KodyDennon/DriveWipe/releases".into());

    // Clean up dist
    let _ = fs::remove_dir_all("target/dist");

    header("Release Published");
    println!("  Tag:     {}", style(&tag).green().bold());
    println!("  URL:     {}", style(&url).underlined());
    println!();
    println!("  To add builds from other platforms:");
    println!(
        "    {}",
        style(format!("cargo xtask release   (choose \"Attach to existing release\")")).dim()
    );
    println!();

    Ok(())
}

// ── Flow B: Attach to existing release ──────────────────────────────────────

fn flow_attach(platform: &Platform) -> Result<()> {
    header("Attach to Existing Release");

    // ── Step 1: Select release ──────────────────────────────────────────────
    step(1, "Select release");

    // gh auth check
    if !cmd_ok("gh", &["auth", "status"]) {
        error_line("GitHub CLI not authenticated");
        println!("    Run: gh auth login");
        bail!("GitHub CLI authentication required.");
    }

    let sp = spinner("Fetching releases from GitHub...");
    let json = cmd_output(
        "gh",
        &[
            "release",
            "list",
            "--json",
            "tagName,name,publishedAt",
            "--limit",
            "15",
        ],
    )?;
    sp.finish_and_clear();

    let releases: Vec<serde_json::Value> = serde_json::from_str(&json)
        .context("Failed to parse release list from GitHub")?;

    if releases.is_empty() {
        bail!("No releases found on GitHub. Create one first with a new release.");
    }

    let release_items: Vec<String> = releases
        .iter()
        .map(|r| {
            let tag = r["tagName"].as_str().unwrap_or("?");
            let name = r["name"].as_str().unwrap_or("");
            let date = r["publishedAt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();
            format!("{tag}  {name}  ({date})")
        })
        .collect();

    let mut items = release_items.clone();
    items.push("Enter tag manually".to_string());

    let selection = Select::new()
        .with_prompt("Which release?")
        .items(&items)
        .default(0)
        .interact()?;

    let tag = if selection == items.len() - 1 {
        let t: String = Input::new()
            .with_prompt("Tag (e.g. v1.0.0)")
            .interact_text()?;
        let t = if t.starts_with('v') {
            t
        } else {
            format!("v{t}")
        };
        // Verify release exists
        if !cmd_ok("gh", &["release", "view", &t]) {
            bail!("Release '{t}' not found on GitHub.");
        }
        t
    } else {
        releases[selection]["tagName"]
            .as_str()
            .unwrap()
            .to_string()
    };

    println!(
        "  {} Platform: {} ({}) → {}",
        style("ℹ").blue().bold(),
        platform.os,
        platform.arch,
        platform.target
    );
    println!();

    // ── Step 2: Confirm ─────────────────────────────────────────────────────
    step(2, "Confirm");

    println!("  Release: {}", style(&tag).green());
    println!(
        "  Will build and upload {} artifacts",
        style(&platform.target).cyan()
    );
    println!();

    if !Confirm::new()
        .with_prompt("Proceed with build?")
        .default(true)
        .interact()?
    {
        println!("Aborted.");
        return Ok(());
    }

    // ── Steps 3–4: Build & Package (reuses step numbers 4–5 internally) ──────
    // Override step numbering for attach flow
    let build = build_and_package(&tag, platform)?;

    // ── Step 5: Upload ──────────────────────────────────────────────────────
    step(6, "Upload to GitHub release");
    println!();

    if !Confirm::new()
        .with_prompt("Upload artifacts to the release?")
        .default(true)
        .interact()?
    {
        println!("Aborted. Built artifacts:");
        println!("  {}", build.archive_path.display());
        println!("  {}", build.checksum_path.display());
        return Ok(());
    }

    let sp = spinner("Uploading artifacts...");
    cmd_run(
        "gh",
        &[
            "release",
            "upload",
            &tag,
            build.archive_path.to_str().unwrap(),
            build.checksum_path.to_str().unwrap(),
            "--clobber",
        ],
    )?;
    sp.finish_and_clear();
    success("Artifacts uploaded");

    // ── Summary ─────────────────────────────────────────────────────────────
    let url = cmd_output("gh", &["release", "view", &tag, "--json", "url", "-q", ".url"])
        .unwrap_or_else(|_| "https://github.com/KodyDennon/DriveWipe/releases".into());

    // Clean up dist
    let _ = fs::remove_dir_all("target/dist");

    header("Artifacts Attached");
    println!("  Release: {}", style(&tag).green().bold());
    println!("  Target:  {}", style(&platform.target).cyan());
    println!("  URL:     {}", style(&url).underlined());
    println!();

    Ok(())
}

// ── Release notes builder ───────────────────────────────────────────────────

fn build_release_notes(tag: &str, platform: &Platform, build: &BuildResult) -> String {
    let archive_name = format!("drivewipe-{tag}-{}", platform.target);
    let mut notes = format!(
        "## DriveWipe {tag}\n\
         \n\
         ### Downloads\n\
         | Platform | Architecture | Archive |\n\
         |---|---|---|\n\
         | {} | {} | `{}.{}` |\n\
         \n\
         *Run `cargo xtask release` on other platforms to add their builds.*\n\
         \n\
         ### Contents\n\
         - `drivewipe{}` — CLI tool\n\
         - `drivewipe-tui{}` — Terminal UI",
        platform.os,
        platform.arch,
        archive_name,
        platform.archive_ext,
        platform.exe_suffix,
        platform.exe_suffix,
    );

    if build.gui_built {
        notes.push_str(&format!(
            "\n- `drivewipe-gui{}` — Graphical UI",
            platform.exe_suffix
        ));
    }

    notes.push_str(&format!(
        "\n\n### Verify\n```\nshasum -a 256 -c {archive_name}.sha256\n```\n"
    ));

    notes
}

// ── Entry point ─────────────────────────────────────────────────────────────

pub fn run() -> Result<()> {
    header("DriveWipe Release Wizard");

    let platform = detect_platform()?;
    println!(
        "  Platform: {} ({}) → {}",
        style(platform.os).cyan(),
        style(platform.arch).cyan(),
        style(&platform.target).cyan().bold()
    );
    println!();

    let choices = &["Create a new release", "Attach to an existing release"];
    let mode = Select::new()
        .with_prompt("What would you like to do?")
        .items(choices)
        .default(0)
        .interact()?;

    match mode {
        0 => flow_new_release(&platform),
        1 => flow_attach(&platform),
        _ => unreachable!(),
    }
}

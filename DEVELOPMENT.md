# DriveWipe Development Guide

## Workspace Structure

DriveWipe is a Cargo workspace with six crates:

| Crate | Purpose |
|---|---|
| `drivewipe-core` | Library — all business logic, no UI |
| `drivewipe-cli` | CLI binary (`drivewipe`) |
| `drivewipe-tui` | Terminal UI binary (`drivewipe-tui`) |
| `drivewipe-gui` | Graphical UI binary (`drivewipe-gui`) |
| `drivewipe-live` | Live environment — HPA/DCO, ATA security, kernel module, DMA I/O |
| `xtask` | Build automation (bump, release, live-build) |

## Automated Versioning System

DriveWipe uses a **"Safety First"** automated versioning system. Versioning is handled locally by a Git `pre-push` hook and the `xtask` utility.

### The Versioning Contract

| Level | Commit Message Trigger | LOC Safety Trigger (per crate) |
| :--- | :--- | :--- |
| **Patch** (`0.0.x`) | `fix:`, `chore:`, `refactor:`, `test:`, `style:` | **> 250 lines** changed |
| **Minor** (`0.x.0`) | `feat:` | **> 1000 lines** changed |
| **Major** (`x.0.0`) | `Major-Release`, `BREAKING CHANGE` | **Manual Only** |

### How to use Scoped Commits

To ensure only the relevant crate gets a version bump, use **scoped commit messages**:

- `feat(core): add NVMe secure erase support` → Bumps `drivewipe-core` to **Minor**.
- `fix(cli): resolve progress bar flicker` → Bumps `drivewipe-cli` to **Patch**.
- `feat(tui): new dashboard view` → Bumps `drivewipe-tui` to **Minor**.
- `feat(live): add DCO freeze support` → Bumps `drivewipe-live` to **Minor**.
- `fix(gui): correct window resize` → Bumps `drivewipe-gui` to **Patch**.

### LOC Safety Triggers

If you write a `fix(core): ...` commit but change **1,200 lines** of code, the system will **automatically promote** the bump to a **Minor** version because it crossed the 1,000-line safety threshold. This prevents large refactors from being hidden in patch releases.

### Triggering a Major Release

Major releases are 100% manual and have no LOC trigger. To trigger one, include `Major-Release` or `BREAKING CHANGE` in your commit message. If you want it scoped to a specific crate, use `Major-Release(core): ...`.

### The Workflow

1.  Work on your changes and commit using the formats above.
2.  Run `git push`.
3.  The `pre-push` hook runs. If version bumps are needed, it will:
    - Update the relevant `Cargo.toml` files.
    - Create a local commit: `chore(version): automated version bump`.
    - **Abort the push.**
4.  Run `git push` again. The push will now include your changes + the version bump commit.

### Manual Verification

You can manually run the versioning check at any time:
```bash
cargo run --package xtask -- bump
```

## xtask Commands

```bash
cargo xtask bump          # Automated version bumps based on git history + LOC
cargo xtask release       # Interactive release wizard — build, tag, publish
cargo xtask live-build    # Build the live USB image + PXE artifacts
```

## Building

```bash
# Build all crates
cargo build --workspace

# Build with PDF support
cargo build --workspace --features drivewipe-core/pdf-report

# Build TUI with live environment features (Linux only)
cargo build --package drivewipe-tui --features live

# Build the live USB image (requires Docker)
cargo xtask live-build

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-features -- -D warnings

# Format code
cargo fmt --all
```

## CI / CD

### CI (`ci.yml`)
Runs on every push to `main` and all PRs:
- Cargo check (stable + nightly, Linux/macOS/Windows)
- Test suite (all platforms)
- Clippy lints (all platforms)
- `cargo fmt` check
- Security audit (`rustsec/audit-check`)
- Documentation build

### Release (`release.yml`)
Triggered by pushing a `v*` tag:
- CI gate (check + test + clippy on all platforms)
- Build desktop binaries for 6 targets (Linux/macOS/Windows × x86_64/ARM64)
- Build live ISO (musl static TUI with `--features live`)
- Package PXE boot artifacts
- Create GitHub Release with categorized notes (Desktop / Live Environment)
- SHA-256 checksums for all artifacts

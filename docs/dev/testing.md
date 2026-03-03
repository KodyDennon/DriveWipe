# Testing Guide

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p drivewipe-core

# Specific test file
cargo test -p drivewipe-core --test integration_tests

# With output
cargo test --workspace -- --nocapture

# Single test
cargo test --workspace test_audit_logger_creates_entries
```

## Test Organization

### Unit tests
Located alongside source code in `#[cfg(test)]` modules.

### Integration tests
Located in `crates/drivewipe-core/tests/`:

| File | Tests |
|---|---|
| `integration_tests.rs` | Wipe methods, sessions, verification, cancellation, registry |
| `audit_tests.rs` | Audit logger creation, entry writing and reading |
| `profile_tests.rs` | Profile database loading, drive matching |
| `health_tests.rs` | Health snapshot save/load, diff comparison |
| `time_estimate_tests.rs` | EMA smoothing, calibration, multi-pass estimates |
| `keyboard_lock_tests.rs` | Lock/unlock sequence detection |

### Test Infrastructure

`tests/common/mod.rs` provides:

- **`MockDevice`** — In-memory device implementing `RawDeviceIo` with:
  - Configurable size
  - Error injection (`inject_error_at_offset`)
  - Write/read counting
  - Alignment validation
- **`test_drive_info()`** — Generic SATA SSD DriveInfo
- **`test_hdd_info()`** — Generic HDD DriveInfo
- **`test_nvme_info()`** — Generic NVMe DriveInfo

## Writing Tests

### Pattern for new module tests

```rust
mod common;
use common::MockDevice;

#[test]
fn test_my_feature() {
    let device = MockDevice::new(1024 * 1024); // 1 MiB
    // ... test logic
}
```

### Testing with temp directories

```rust
use tempfile::tempdir;

#[test]
fn test_file_output() {
    let dir = tempdir().unwrap();
    // use dir.path() for file operations
    // dir is cleaned up on drop
}
```

## CI

Tests run on every PR via GitHub Actions across Linux, macOS, and Windows.

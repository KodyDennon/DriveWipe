use std::path::PathBuf;

use drivewipe_core::resume::WipeState;
use uuid::Uuid;

#[test]
fn new_state_has_defaults() {
    let id = Uuid::new_v4();
    let state = WipeState::new(
        id,
        PathBuf::from("/dev/sda"),
        "SERIAL-001".to_string(),
        "Model X".to_string(),
        500_000_000_000,
        "dod-short".to_string(),
        3,
        true,
    );

    assert_eq!(state.session_id, id);
    assert_eq!(state.current_pass, 1);
    assert_eq!(state.total_passes, 3);
    assert_eq!(state.bytes_written_this_pass, 0);
    assert_eq!(state.total_bytes_written, 0);
    assert!(state.verify_after);
    assert_eq!(state.device_serial, "SERIAL-001");
    assert_eq!(state.method_id, "dod-short");
}

#[test]
fn update_progress() {
    let mut state = make_state();
    state.update_progress(2, 1_000_000, 501_000_000);
    assert_eq!(state.current_pass, 2);
    assert_eq!(state.bytes_written_this_pass, 1_000_000);
    assert_eq!(state.total_bytes_written, 501_000_000);
}

#[test]
fn save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state();
    state.save(dir.path()).unwrap();

    let loaded = WipeState::load(&WipeState::state_path(dir.path(), state.session_id)).unwrap();
    assert_eq!(loaded.session_id, state.session_id);
    assert_eq!(loaded.device_serial, state.device_serial);
    assert_eq!(loaded.method_id, state.method_id);
    assert_eq!(loaded.current_pass, state.current_pass);
    assert_eq!(loaded.total_passes, state.total_passes);
}

#[test]
fn save_creates_directory() {
    let dir = tempfile::tempdir().unwrap();
    let sessions_dir = dir.path().join("sub").join("sessions");
    let state = make_state();
    state.save(&sessions_dir).unwrap();
    assert!(sessions_dir.exists());
}

#[test]
fn find_incomplete_empty_dir() {
    let dir = tempfile::tempdir().unwrap();
    let results = WipeState::find_incomplete(dir.path()).unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_incomplete_nonexistent_dir() {
    let results =
        WipeState::find_incomplete(&PathBuf::from("/tmp/nonexistent_drivewipe_test_dir")).unwrap();
    assert!(results.is_empty());
}

#[test]
fn find_incomplete_finds_state_files() {
    let dir = tempfile::tempdir().unwrap();

    let s1 = make_state();
    s1.save(dir.path()).unwrap();

    let s2 = WipeState::new(
        Uuid::new_v4(),
        PathBuf::from("/dev/sdb"),
        "SERIAL-002".to_string(),
        "Model Y".to_string(),
        1_000_000_000_000,
        "random".to_string(),
        1,
        false,
    );
    s2.save(dir.path()).unwrap();

    let results = WipeState::find_incomplete(dir.path()).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn find_for_device() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state();
    state.save(dir.path()).unwrap();

    let found = WipeState::find_for_device(dir.path(), "SERIAL-001").unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().device_serial, "SERIAL-001");

    let not_found = WipeState::find_for_device(dir.path(), "SERIAL-999").unwrap();
    assert!(not_found.is_none());
}

#[test]
fn cleanup_removes_state_file() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state();
    state.save(dir.path()).unwrap();

    let path = WipeState::state_path(dir.path(), state.session_id);
    assert!(path.exists());

    state.cleanup(dir.path()).unwrap();
    assert!(!path.exists());
}

#[test]
fn cleanup_nonexistent_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state();
    // Never saved — cleanup should still succeed.
    state.cleanup(dir.path()).unwrap();
}

#[test]
fn state_path_format() {
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let path = WipeState::state_path(&PathBuf::from("/tmp"), id);
    assert_eq!(
        path.to_string_lossy(),
        "/tmp/550e8400-e29b-41d4-a716-446655440000.state"
    );
}

// ── Helper ─────────────────────────────────────────────────────────

fn make_state() -> WipeState {
    WipeState::new(
        Uuid::new_v4(),
        PathBuf::from("/dev/sda"),
        "SERIAL-001".to_string(),
        "Model X".to_string(),
        500_000_000_000,
        "dod-short".to_string(),
        3,
        true,
    )
}

# DriveWipe Production Implementation Plan

> **Status**: Ready for Execution
> **Goal**: Bridge the gap between the current "advanced prototype" state and a fully production-ready v1.1.0 release.

This plan details the specific engineering tasks required to complete the remaining features identified in the codebase audit. It focuses on **persistence**, **integration**, and **user interfaces**.

---

## Phase 1: Core Reliability & Persistence

### 1.1 Partition Table Persistence (`drivewipe-core`)
**Current State**: `GptTable` and `MbrTable` can be parsed (read), but there is no logic to serialize them back to bytes and write them to disk. `write_table()` is missing.
**Task**: Implement full serialization and write logic with CRC32 recalculation.

*   **`crates/drivewipe-core/src/partition/gpt.rs`**:
    *   Implement `GptTable::to_bytes(&self) -> Vec<u8>`.
    *   Recalculate header CRC32 (offset 16) and partition entry array CRC32 (offset 88).
    *   Implement `GptTable::write(&self, device: &mut dyn RawDeviceIo) -> Result<()>`.
    *   **Critical**: Write both the **Primary** (LBA 1 + entries) and **Backup** (Last LBA + entries) tables.
*   **`crates/drivewipe-core/src/partition/mbr.rs`**:
    *   Implement `MbrTable::to_bytes(&self) -> Vec<u8>`.
    *   Implement `MbrTable::write(&self, device: &mut dyn RawDeviceIo) -> Result<()>`.
    *   Ensure the 0x55AA signature is preserved.
*   **`crates/drivewipe-core/src/partition/ops.rs`**:
    *   Expose a public `write_table(device, table)` function that dispatches to the correct implementation.

### 1.2 Image-Based Cloning (`drivewipe-core`)
**Current State**: `clone_block` performs raw sector-copying. The `CloneImage` format (header + chunks) exists in `image.rs` but is not used by the clone engine. Users cannot actually create compressed/encrypted images yet.
**Task**: Implement the image-based clone engine.

*   **`crates/drivewipe-core/src/clone/ops.rs`** (New File):
    *   Implement `clone_device_to_image(...)`:
        *   Writes `CloneImageHeader`.
        *   Reads source blocks, compresses/encrypts (if configured), writes chunks via `CloneImage::write_chunk`.
    *   Implement `restore_image_to_device(...)`:
        *   Reads `CloneImageHeader`.
        *   Reads chunks, decompresses/decrypts, writes to target device.
*   **`crates/drivewipe-cli/src/commands/clone.rs`**:
    *   Update logic: If `target` is a file AND (`compress` OR `encrypt` is set), use `clone_device_to_image`.

### 1.3 Forensic Integration (`drivewipe-core`)
**Current State**: `ForensicSession` has placeholders for hidden area detection. The logic exists in `drivewipe-live` but isn't connected.
**Task**: Conditionally enable `drivewipe-live` features in core.

*   **`crates/drivewipe-core/Cargo.toml`**:
    *   Add `drivewipe-live = { path = "../drivewipe-live", optional = true }`.
    *   Add `live` feature flag.
*   **`crates/drivewipe-core/src/forensic/mod.rs`**:
    *   In `ForensicSession::execute`:
        *   `#[cfg(feature = "live")]`: Call `drivewipe_live::detect_hidden_areas()`.
        *   `#[cfg(not(feature = "live"))]`: Return `None` or a "not supported" result for hidden areas.

---

## Phase 2: GUI Implementation (`drivewipe-gui`)

**Current State**: The GUI is a visual shell ("mockup") with no underlying logic.
**Task**: Wire the GUI to the Core engine using `iced`'s async capabilities.

*   **State Management**:
    *   Create `AppState` struct to hold `WipeSession`, `ForensicSession`, etc.
    *   Use `iced::Subscription` to listen for `ProgressEvent` channels from `drivewipe-core`.
*   **Wipe Screen**:
    *   On "Start Wipe", spawn a `tokio` task running `session.execute()`.
    *   Map `ProgressEvent::WipeProgress` to the GUI progress bar and throughput label.
*   **Health Screen**:
    *   Call `drivewipe_core::health::get_health(path)` asynchronously.
    *   Display the result in the `health_info` text widget.
*   **Partition Screen**:
    *   Call `drivewipe_core::partition::read_table(path)`.
    *   Render the table visually (currently just a text dump).

---

## Phase 3: CLI & TUI Completion

### 3.1 CLI Partition Commands
**Current State**: `drivewipe partition list` works. `create`, `delete`, etc. are defined in `main.rs` but likely missing implementation logic in `commands/partition.rs`.
**Task**: Implement CRUD commands.

*   **`crates/drivewipe-cli/src/commands/partition.rs`**:
    *   Implement `create(device, size, type, name)`.
    *   Implement `delete(device, index)`.
    *   Implement `resize(device, index, new_end)`.
    *   All must call `read_table` -> modify -> `write_table`.

### 3.2 TUI Interactive Partition Manager
**Current State**: The Partition screen (`AppScreen::PartitionManager`) is read-only.
**Task**: Add interactive keys.

*   **`crates/drivewipe-tui/src/ui/partition.rs`**:
    *   Handle keys: `n` (New), `d` (Delete), `r` (Resize).
    *   Show a popup dialog for input (Size/Name).
    *   Execute the operation and refresh the table.

---

## Execution Order

1.  **Core Persistence** (High Priority) - Enables all other partition features.
2.  **Image Cloning** (High Priority) - Delivers promised compression/encryption features.
3.  **CLI Partition** (Medium) - Exposes persistence to users.
4.  **Forensic/Live** (Medium) - Completes the "Pro" feature set.
5.  **GUI Wiring** (Low/Parallel) - Can be done independently once Core is stable.

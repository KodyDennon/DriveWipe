//! DriveWipe Core Library
//!
//! `drivewipe-core` is the engine behind the DriveWipe sanitization suite. It provides
//! a cross-platform (Linux, macOS, Windows) API for secure data destruction,
//! drive management, forensics, and health monitoring.
//!
//! ## Key Features
//!
//! - **Sanitization**: Implements 21 wipe methods compliant with NIST SP 800-88 and IEEE 2883.
//! - **Dual-Layer I/O**: High-performance software overwrites combined with firmware-level erase commands.
//! - **Safety First**: Integrated boot drive detection, multi-step confirmation logic, and hardware warnings.
//! - **Extensible Architecture**: Trait-based design for I/O, patterns, and wipe methods.
//! - **Platform Native**: Direct hardware access via platform-specific IOCTLs and raw I/O flags.
//!
//! ## Crate Structure
//!
//! The library is organized into specialized modules:
//! - `wipe`: The main orchestrator for data sanitization.
//! - `drive`: Hardware discovery and identification.
//! - `health`: SMART and NVMe monitoring.
//! - `forensic`: Data remnant analysis and entropy mapping.
//! - `clone`: Block and partition-level drive duplication.
//! - `partition`: Native GPT and MBR manipulation.
//!
//! // DriveWipeError is intentionally large (contains platform-specific error data).
//! // Boxing would add indirection cost to every error path.
#![allow(clippy::result_large_err)]
// Cross-platform code uses explicit casts (e.g. `as i32`, `.into()`) for portability.
// These are redundant on some platforms but required on others.
#![allow(clippy::unnecessary_cast, clippy::useless_conversion)]

pub mod audit;
pub mod clone;
pub mod config;
pub mod crypto;
pub mod drive;
pub mod error;
pub mod forensic;
pub mod health;
pub mod io;
pub mod keyboard_lock;
pub mod notify;
pub mod partition;
pub mod platform;
pub mod profile;
pub mod progress;
pub mod report;
pub mod resume;
pub mod session;
pub mod sleep_inhibit;
pub mod time_estimate;
pub mod types;
pub mod verify;
pub mod wipe;

pub use error::DriveWipeError;
pub use types::*;

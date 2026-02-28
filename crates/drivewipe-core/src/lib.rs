// DriveWipeError is intentionally large (contains platform-specific error data).
// Boxing would add indirection cost to every error path.
#![allow(clippy::result_large_err)]
// Cross-platform code uses explicit casts (e.g. `as i32`, `.into()`) for portability.
// These are redundant on some platforms but required on others.
#![allow(clippy::unnecessary_cast, clippy::useless_conversion)]

pub mod config;
pub mod crypto;
pub mod drive;
pub mod error;
pub mod io;
pub mod platform;
pub mod progress;
pub mod report;
pub mod resume;
pub mod session;
pub mod types;
pub mod verify;
pub mod wipe;

pub use error::DriveWipeError;
pub use types::*;

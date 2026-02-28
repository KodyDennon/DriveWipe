//! Integration tests for drivewipe-core.
//!
//! These tests exercise the public API surface of the library crate without
//! touching real block devices.

mod aes_ctr_tests;
mod aligned_buffer_tests;
mod cancellation_tests;
mod config_tests;
mod error_tests;
mod registry_tests;
mod report_tests;
mod resume_state_tests;
mod session_tests;
mod types_tests;
mod verify_tests;
mod wipe_method_tests;

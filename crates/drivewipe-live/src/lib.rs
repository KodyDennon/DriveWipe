//! DriveWipe Live Environment Capability Crate
//!
//! This crate provides enhanced hardware interaction capabilities specifically
//! designed for the DriveWipe Live boot environment. It bridges the gap between
//! standard userspace tools and low-level hardware control.
//!
//! ## Core Capabilities
//!
//! - **Kernel Integration**: Direct communication with the `drivewipe` Linux kernel module.
//! - **Hidden Areas**: Detection and removal of HPA (Host Protected Area) and DCO (Device Configuration Overlay).
//! - **Security Management**: ATA security state querying and one-click unfreezing via suspend/resume cycles.
//! - **Performance**: Support for zero-copy DMA I/O paths when the kernel module is loaded.
//!
//! ## Fallback Strategy
//!
//! All hardware commands attempt to use the custom kernel module for maximum control.
//! If the module is missing, they gracefully fall back to standard `SG_IO` or
//! vendor-specific IOCTLs available in standard Linux kernels.

pub mod ata_security;
pub mod capabilities;
pub mod dco;
pub mod detect;
pub mod dma_io;
pub mod hpa;
pub mod kernel_module;
pub mod unfreeze;

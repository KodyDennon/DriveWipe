//! Wipe methods, pattern generators, and the method registry.

pub mod crypto_erase;
pub mod custom;
pub mod firmware;
pub mod patterns;
pub mod software;

use crossbeam_channel::Sender;
use uuid::Uuid;

use patterns::PatternGenerator;

use self::custom::CustomWipeMethod;
use self::firmware::FirmwareWipe;
use crate::error::Result;
use crate::progress::ProgressEvent;
use crate::types::DriveInfo;

// ── WipeMethod trait ─────────────────────────────────────────────────────────

/// Describes a data-destruction method composed of one or more overwrite passes.
///
/// Each pass writes a specific pattern (zeros, ones, random bytes, or a
/// repeating sequence) across the entire device surface. Software methods
/// implement this trait directly; firmware methods set [`is_firmware`](WipeMethod::is_firmware) to
/// `true` and delegate to the drive controller.
pub trait WipeMethod: Send + Sync {
    /// Machine-readable identifier (e.g. `"dod-short"`, `"gutmann"`).
    fn id(&self) -> &str;

    /// Human-readable name shown in the UI and reports.
    fn name(&self) -> &str;

    /// Longer description of the method and its provenance.
    fn description(&self) -> &str;

    /// Total number of overwrite passes.
    fn pass_count(&self) -> u32;

    /// Returns a [`PatternGenerator`] for the given zero-indexed pass number.
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send>;

    /// Whether the method specification calls for a verification read-back
    /// after all passes complete.
    fn includes_verification(&self) -> bool;

    /// Returns `true` if this method delegates to a firmware command rather
    /// than performing software overwrites.
    fn is_firmware(&self) -> bool {
        false
    }

    /// Execute a firmware-level erase, if this method is firmware-backed.
    ///
    /// Software methods return `None` (the default). Firmware methods return
    /// `Some(Ok(()))` on success or `Some(Err(...))` on failure, causing
    /// [`WipeSession::execute()`](crate::session::WipeSession::execute) to skip the software write loop entirely.
    fn execute_firmware(
        &self,
        _drive: &DriveInfo,
        _session_id: Uuid,
        _progress_tx: &Sender<ProgressEvent>,
    ) -> Option<Result<()>> {
        None
    }
}

// ── FirmwareMethodAdapter ────────────────────────────────────────────────────

/// Wraps a [`FirmwareWipe`] implementor so it can be stored in the
/// [`WipeMethodRegistry`] alongside software methods.
///
/// The adapter delegates metadata (`id`, `name`, `description`) to the inner
/// firmware method.  Because firmware erases are a single atomic operation
/// handled by the drive controller, `pass_count()` returns 1 and
/// `pattern_for_pass()` returns a placeholder `ZeroFill` (it is never used
/// for actual I/O).
pub struct FirmwareMethodAdapter {
    inner: Box<dyn FirmwareWipe>,
}

impl FirmwareMethodAdapter {
    /// Create a new adapter wrapping the given firmware wipe method.
    pub fn new(inner: Box<dyn FirmwareWipe>) -> Self {
        Self { inner }
    }

    /// Borrow the underlying [`FirmwareWipe`] implementation.
    pub fn inner(&self) -> &dyn FirmwareWipe {
        self.inner.as_ref()
    }
}

impl WipeMethod for FirmwareMethodAdapter {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn pass_count(&self) -> u32 {
        1
    }

    fn pattern_for_pass(&self, _pass: u32) -> Box<dyn PatternGenerator + Send> {
        // Placeholder -- firmware erases never use host-side pattern generation.
        Box::new(patterns::ZeroFill)
    }

    fn includes_verification(&self) -> bool {
        false
    }

    fn is_firmware(&self) -> bool {
        true
    }

    fn execute_firmware(
        &self,
        drive: &DriveInfo,
        session_id: Uuid,
        progress_tx: &Sender<ProgressEvent>,
    ) -> Option<Result<()>> {
        Some(self.inner.execute(drive, session_id, progress_tx))
    }
}

// ── WipeMethodRegistry ───────────────────────────────────────────────────────

/// Central registry of available wipe methods.
///
/// Created with all built-in software methods pre-registered. Custom and
/// firmware methods can be added at runtime via [`register`](Self::register).
pub struct WipeMethodRegistry {
    methods: Vec<Box<dyn WipeMethod>>,
}

impl WipeMethodRegistry {
    /// Create a new registry with all built-in software *and* firmware methods
    /// registered.
    pub fn new() -> Self {
        let mut registry = Self {
            methods: software::all_software_methods(),
        };
        registry.register_firmware_methods();
        registry
    }

    /// Register an additional wipe method (custom or firmware-backed).
    pub fn register(&mut self, method: Box<dyn WipeMethod>) {
        self.methods.push(method);
    }

    /// Register all built-in firmware wipe methods via [`FirmwareMethodAdapter`].
    pub fn register_firmware_methods(&mut self) {
        let firmware_methods: Vec<Box<dyn FirmwareWipe>> = vec![
            Box::new(firmware::ata::AtaSecureErase),
            Box::new(firmware::ata::AtaEnhancedSecureErase),
            Box::new(firmware::nvme::NvmeFormatUserData),
            Box::new(firmware::nvme::NvmeFormatCrypto),
            Box::new(firmware::nvme::NvmeSanitizeBlock),
            Box::new(firmware::nvme::NvmeSanitizeCrypto),
            Box::new(firmware::nvme::NvmeSanitizeOverwrite),
            Box::new(crypto_erase::TcgOpalCryptoErase),
        ];

        for fw in firmware_methods {
            self.methods.push(Box::new(FirmwareMethodAdapter::new(fw)));
        }
    }

    /// Register user-defined custom wipe methods from the application
    /// configuration.
    pub fn register_custom_methods(&mut self, config: &crate::config::DriveWipeConfig) {
        for method_cfg in &config.custom_methods {
            self.methods
                .push(Box::new(CustomWipeMethod::from_config(method_cfg.clone())));
        }
    }

    /// Look up a method by its identifier string.
    pub fn get(&self, id: &str) -> Option<&dyn WipeMethod> {
        self.methods
            .iter()
            .find(|m| m.id() == id)
            .map(|m| m.as_ref())
    }

    /// Return a slice of all registered methods.
    pub fn list(&self) -> &[Box<dyn WipeMethod>] {
        &self.methods
    }
}

impl Default for WipeMethodRegistry {
    fn default() -> Self {
        Self::new()
    }
}

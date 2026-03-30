//! Built-in software overwrite wipe methods.
//!
//! Each struct is a unit type implementing [`WipeMethod`] that describes a
//! well-known secure-erase standard. The actual byte patterns are provided by
//! the generators in [`super::patterns`].

use super::WipeMethod;
use super::patterns::{
    ConstantFill, OneFill, PatternGenerator, RandomFill, RepeatingPattern, ZeroFill,
};
use async_trait::async_trait;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Shorthand for boxing a pattern generator.
fn boxed<P: PatternGenerator + Send + 'static>(p: P) -> Box<dyn PatternGenerator + Send> {
    Box::new(p)
}

// ── Zero Fill ────────────────────────────────────────────────────────────────

/// Single-pass zero (0x00) overwrite.
pub struct ZeroFillMethod;

#[async_trait]
impl WipeMethod for ZeroFillMethod {
    fn id(&self) -> &str {
        "zero"
    }
    fn name(&self) -> &str {
        "Zero Fill"
    }
    fn description(&self) -> &str {
        "Single pass of all-zero bytes (0x00)"
    }
    fn pass_count(&self) -> u32 {
        1
    }
    fn pattern_for_pass(&self, _pass: u32) -> Box<dyn PatternGenerator + Send> {
        boxed(ZeroFill)
    }
    fn includes_verification(&self) -> bool {
        false
    }
}

// ── One Fill ─────────────────────────────────────────────────────────────────

/// Single-pass one (0xFF) overwrite.
pub struct OneFillMethod;

#[async_trait]
impl WipeMethod for OneFillMethod {
    fn id(&self) -> &str {
        "one"
    }
    fn name(&self) -> &str {
        "One Fill"
    }
    fn description(&self) -> &str {
        "Single pass of all-one bytes (0xFF)"
    }
    fn pass_count(&self) -> u32 {
        1
    }
    fn pattern_for_pass(&self, _pass: u32) -> Box<dyn PatternGenerator + Send> {
        boxed(OneFill)
    }
    fn includes_verification(&self) -> bool {
        false
    }
}

// ── Random Fill ──────────────────────────────────────────────────────────────

/// Single-pass cryptographic random overwrite.
pub struct RandomFillMethod;

#[async_trait]
impl WipeMethod for RandomFillMethod {
    fn id(&self) -> &str {
        "random"
    }
    fn name(&self) -> &str {
        "Random Fill"
    }
    fn description(&self) -> &str {
        "Single pass of cryptographically secure random data (AES-256-CTR)"
    }
    fn pass_count(&self) -> u32 {
        1
    }
    fn pattern_for_pass(&self, _pass: u32) -> Box<dyn PatternGenerator + Send> {
        boxed(RandomFill::new())
    }
    fn includes_verification(&self) -> bool {
        false
    }
}

// ── DoD 5220.22-M (Short / 3-pass) ──────────────────────────────────────────

/// DoD 5220.22-M short: 3 passes (0x00, 0xFF, random) with verification.
pub struct DodShortMethod;

#[async_trait]
impl WipeMethod for DodShortMethod {
    fn id(&self) -> &str {
        "dod-short"
    }
    fn name(&self) -> &str {
        "DoD 5220.22-M (3-pass)"
    }
    fn description(&self) -> &str {
        "U.S. DoD 5220.22-M short: zero, one, random — with verification"
    }
    fn pass_count(&self) -> u32 {
        3
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ZeroFill),
            1 => boxed(OneFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── DoD 5220.22-M ECE (7-pass) ──────────────────────────────────────────────

/// DoD 5220.22-M ECE: 7 passes with verification.
pub struct DodEceMethod;

#[async_trait]
impl WipeMethod for DodEceMethod {
    fn id(&self) -> &str {
        "dod-ece"
    }
    fn name(&self) -> &str {
        "DoD 5220.22-M ECE (7-pass)"
    }
    fn description(&self) -> &str {
        "U.S. DoD 5220.22-M ECE: 7-pass overwrite with verification"
    }
    fn pass_count(&self) -> u32 {
        7
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ZeroFill),
            1 => boxed(OneFill),
            2 => boxed(RandomFill::new()),
            3 => boxed(RandomFill::new()),
            4 => boxed(ZeroFill),
            5 => boxed(OneFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── Gutmann (35-pass) ────────────────────────────────────────────────────────

/// Peter Gutmann's 35-pass method (1996 paper).
///
/// Passes 1-4 and 32-35 are random. Passes 5-31 use specific fixed or
/// repeating patterns designed to defeat magnetic-force microscopy on older
/// recording technologies.
pub struct GutmannMethod;

#[async_trait]
impl WipeMethod for GutmannMethod {
    fn id(&self) -> &str {
        "gutmann"
    }
    fn name(&self) -> &str {
        "Gutmann (35-pass)"
    }
    fn description(&self) -> &str {
        "Peter Gutmann 35-pass method with encoding-specific patterns"
    }
    fn pass_count(&self) -> u32 {
        35
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            // Passes 1-4: random (0-indexed: 0..4)
            0..4 => boxed(RandomFill::new()),

            // Pass 5: 0x55
            4 => boxed(ConstantFill(0x55)),
            // Pass 6: 0xAA
            5 => boxed(ConstantFill(0xAA)),
            // Pass 7: repeating 0x92 0x49 0x24
            6 => boxed(RepeatingPattern(vec![0x92, 0x49, 0x24])),
            // Pass 8: repeating 0x49 0x24 0x92
            7 => boxed(RepeatingPattern(vec![0x49, 0x24, 0x92])),
            // Pass 9: repeating 0x24 0x92 0x49 (per Gutmann paper)
            8 => boxed(RepeatingPattern(vec![0x24, 0x92, 0x49])),

            // Passes 10-16: single-byte fills 0x00..0x66 (step 0x11)
            9 => boxed(ConstantFill(0x00)),
            10 => boxed(ConstantFill(0x11)),
            11 => boxed(ConstantFill(0x22)),
            12 => boxed(ConstantFill(0x33)),
            13 => boxed(ConstantFill(0x44)),
            14 => boxed(ConstantFill(0x55)),
            15 => boxed(ConstantFill(0x66)),

            // Passes 17-19: repeating 3-byte fills
            16 => boxed(RepeatingPattern(vec![0x88, 0x88, 0x88])),
            17 => boxed(RepeatingPattern(vec![0x99, 0x99, 0x99])),
            18 => boxed(RepeatingPattern(vec![0xAA, 0xAA, 0xAA])),

            // Passes 20-24: repeating 3-byte fills
            19 => boxed(RepeatingPattern(vec![0xBB, 0xBB, 0xBB])),
            20 => boxed(RepeatingPattern(vec![0xCC, 0xCC, 0xCC])),
            21 => boxed(RepeatingPattern(vec![0xDD, 0xDD, 0xDD])),
            22 => boxed(RepeatingPattern(vec![0xEE, 0xEE, 0xEE])),
            23 => boxed(RepeatingPattern(vec![0xFF, 0xFF, 0xFF])),

            // Passes 25-27: same MFM/RLL patterns repeated
            24 => boxed(RepeatingPattern(vec![0x92, 0x49, 0x24])),
            25 => boxed(RepeatingPattern(vec![0x49, 0x24, 0x92])),
            26 => boxed(RepeatingPattern(vec![0x24, 0x92, 0x49])),

            // Passes 28-31: single-byte fills
            27 => boxed(ConstantFill(0x77)),
            28 => boxed(ConstantFill(0x88)),
            29 => boxed(ConstantFill(0x99)),
            30 => boxed(ConstantFill(0xAA)),

            // Passes 32-35: random (0-indexed: 31..35)
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        false
    }
}

// ── HMG IS5 Baseline ────────────────────────────────────────────────────────

/// UK HMG Infosec Standard 5, Baseline: single zero pass with verification.
pub struct HmgBaselineMethod;

#[async_trait]
impl WipeMethod for HmgBaselineMethod {
    fn id(&self) -> &str {
        "hmg-baseline"
    }
    fn name(&self) -> &str {
        "HMG IS5 Baseline"
    }
    fn description(&self) -> &str {
        "UK HMG Infosec Standard 5 Baseline: single zero pass with verification"
    }
    fn pass_count(&self) -> u32 {
        1
    }
    fn pattern_for_pass(&self, _pass: u32) -> Box<dyn PatternGenerator + Send> {
        boxed(ZeroFill)
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── HMG IS5 Enhanced ─────────────────────────────────────────────────────────

/// UK HMG Infosec Standard 5, Enhanced: 3 passes (0x00, 0xFF, random) with
/// verification.
pub struct HmgEnhancedMethod;

#[async_trait]
impl WipeMethod for HmgEnhancedMethod {
    fn id(&self) -> &str {
        "hmg-enhanced"
    }
    fn name(&self) -> &str {
        "HMG IS5 Enhanced"
    }
    fn description(&self) -> &str {
        "UK HMG Infosec Standard 5 Enhanced: zero, one, random — with verification"
    }
    fn pass_count(&self) -> u32 {
        3
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ZeroFill),
            1 => boxed(OneFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── RCMP TSSIT OPS-II ───────────────────────────────────────────────────────

/// Royal Canadian Mounted Police TSSIT OPS-II: 7 passes — alternating
/// 0x00/0xFF for 6 passes, then a final random pass.
pub struct RcmpMethod;

#[async_trait]
impl WipeMethod for RcmpMethod {
    fn id(&self) -> &str {
        "rcmp"
    }
    fn name(&self) -> &str {
        "RCMP TSSIT OPS-II"
    }
    fn description(&self) -> &str {
        "RCMP TSSIT OPS-II: 6 alternating zero/one passes followed by random"
    }
    fn pass_count(&self) -> u32 {
        7
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            // Alternating: even passes = 0x00, odd passes = 0xFF
            p if p < 6 && p % 2 == 0 => boxed(ZeroFill),
            p if p < 6 => boxed(OneFill),
            // Final pass: random
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        false
    }
}

// ── Registry helper ──────────────────────────────────────────────────────────

/// Returns all built-in software wipe methods.
pub fn all_software_methods() -> Vec<Box<dyn WipeMethod>> {
    vec![
        Box::new(ZeroFillMethod),
        Box::new(OneFillMethod),
        Box::new(RandomFillMethod),
        Box::new(DodShortMethod),
        Box::new(DodEceMethod),
        Box::new(GutmannMethod),
        Box::new(HmgBaselineMethod),
        Box::new(HmgEnhancedMethod),
        Box::new(RcmpMethod),
    ]
}

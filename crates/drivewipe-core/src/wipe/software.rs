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
        // Pass numbers in comments are 1-indexed to match the paper; `pass` is
        // 0-indexed.
        match pass {
            // Passes 1-4: random.
            0..4 => boxed(RandomFill::new()),

            // Pass 5: 0x55, pass 6: 0xAA.
            4 => boxed(ConstantFill(0x55)),
            5 => boxed(ConstantFill(0xAA)),

            // Passes 7-9: the MFM/RLL 3-byte sequence and its rotations.
            6 => boxed(RepeatingPattern(vec![0x92, 0x49, 0x24])),
            7 => boxed(RepeatingPattern(vec![0x49, 0x24, 0x92])),
            8 => boxed(RepeatingPattern(vec![0x24, 0x92, 0x49])),

            // Passes 10-25: the sixteen single-nibble-repeated constants,
            // 0x00, 0x11, 0x22 ... 0xFF.
            9..25 => boxed(ConstantFill(((pass - 9) * 0x11) as u8)),

            // Passes 26-28: the MFM/RLL sequence again.
            25 => boxed(RepeatingPattern(vec![0x92, 0x49, 0x24])),
            26 => boxed(RepeatingPattern(vec![0x49, 0x24, 0x92])),
            27 => boxed(RepeatingPattern(vec![0x24, 0x92, 0x49])),

            // Passes 29-31: the complementary RLL sequence and its rotations.
            28 => boxed(RepeatingPattern(vec![0x6D, 0xB6, 0xDB])),
            29 => boxed(RepeatingPattern(vec![0xB6, 0xDB, 0x6D])),
            30 => boxed(RepeatingPattern(vec![0xDB, 0x6D, 0xB6])),

            // Passes 32-35: random.
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

// ── NIST SP 800-88 Clear ────────────────────────────────────────────────────

/// NIST SP 800-88 Rev. 1, *Clear*: a single overwrite of the addressable
/// surface with a fixed pattern, verified.
///
/// Clear protects against recovery using standard read commands — it is the
/// appropriate level when the media stays within the organisation. It does not
/// address data in reallocated sectors or overprovisioned flash; that is what
/// Purge is for.
pub struct Nist80088ClearMethod;

#[async_trait]
impl WipeMethod for Nist80088ClearMethod {
    fn id(&self) -> &str {
        "nist-800-88-clear"
    }
    fn name(&self) -> &str {
        "NIST SP 800-88 Clear"
    }
    fn description(&self) -> &str {
        "NIST SP 800-88 Rev. 1 Clear: single zero-fill overwrite of all addressable locations, \
         with full read-back verification"
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

// ── NIST SP 800-88 Purge (overwrite) ────────────────────────────────────────

/// NIST SP 800-88 Rev. 1, *Purge* by overwrite: three passes, verified.
///
/// Note that 800-88 prefers a firmware sanitize for Purge — ATA Secure Erase,
/// NVMe Sanitize, or a cryptographic erase — because host-side overwrites
/// cannot reach reallocated sectors or flash overprovisioning. This method is
/// the overwrite-based fallback the standard permits for magnetic media whose
/// controller does not offer a sanitize command. On an SSD, prefer one of the
/// firmware methods.
pub struct Nist80088PurgeMethod;

#[async_trait]
impl WipeMethod for Nist80088PurgeMethod {
    fn id(&self) -> &str {
        "nist-800-88-purge"
    }
    fn name(&self) -> &str {
        "NIST SP 800-88 Purge (overwrite)"
    }
    fn description(&self) -> &str {
        "NIST SP 800-88 Rev. 1 Purge by overwrite: random, zero, random with full read-back \
         verification. For flash media a firmware sanitize or cryptographic erase is preferred."
    }
    fn pass_count(&self) -> u32 {
        3
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ZeroFill),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── AFSSI-5020 (U.S. Air Force) ─────────────────────────────────────────────

/// U.S. Air Force System Security Instruction 5020: zero, one, random, verified.
pub struct Afssi5020Method;

#[async_trait]
impl WipeMethod for Afssi5020Method {
    fn id(&self) -> &str {
        "afssi-5020"
    }
    fn name(&self) -> &str {
        "AFSSI-5020 (U.S. Air Force)"
    }
    fn description(&self) -> &str {
        "U.S. Air Force AFSSI-5020: zero, one, random — with verification"
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

// ── AR 380-19 (U.S. Army) ───────────────────────────────────────────────────

/// U.S. Army Regulation 380-19: random, a fixed character, then its
/// complement — verified.
///
/// The regulation specifies "a random character, then a character, then its
/// complement". The fixed pair is realised here as `0x00` / `0xFF`; a literal
/// per-run random character would make the pass unreproducible and therefore
/// unverifiable, since verification replays the pattern to compare it against
/// the device.
pub struct Ar380_19Method;

#[async_trait]
impl WipeMethod for Ar380_19Method {
    fn id(&self) -> &str {
        "ar-380-19"
    }
    fn name(&self) -> &str {
        "AR 380-19 (U.S. Army)"
    }
    fn description(&self) -> &str {
        "U.S. Army AR 380-19: random, then 0x00, then its complement 0xFF — with verification"
    }
    fn pass_count(&self) -> u32 {
        3
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(RandomFill::new()),
            1 => boxed(ConstantFill(0x00)),
            _ => boxed(ConstantFill(0xFF)),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── NAVSO P-5239-26 (U.S. Navy) ─────────────────────────────────────────────

/// U.S. Navy NAVSO P-5239-26: a character, its complement, then random —
/// verified.
///
/// The publication names `0x01` and its complement `0xFE` for MFM-encoded
/// media, which is the pairing used here.
pub struct NavsoP5239_26Method;

#[async_trait]
impl WipeMethod for NavsoP5239_26Method {
    fn id(&self) -> &str {
        "navso-p-5239-26"
    }
    fn name(&self) -> &str {
        "NAVSO P-5239-26 (U.S. Navy)"
    }
    fn description(&self) -> &str {
        "U.S. Navy NAVSO P-5239-26: 0x01, its complement 0xFE, then random — with verification"
    }
    fn pass_count(&self) -> u32 {
        3
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            0 => boxed(ConstantFill(0x01)),
            1 => boxed(ConstantFill(0xFE)),
            _ => boxed(RandomFill::new()),
        }
    }
    fn includes_verification(&self) -> bool {
        true
    }
}

// ── VSITR (German BSI) ──────────────────────────────────────────────────────

/// German BSI VSITR: seven passes — three alternating zero/one pairs followed
/// by a final `0xAA` pass.
pub struct VsitrMethod;

#[async_trait]
impl WipeMethod for VsitrMethod {
    fn id(&self) -> &str {
        "vsitr"
    }
    fn name(&self) -> &str {
        "VSITR (German BSI, 7-pass)"
    }
    fn description(&self) -> &str {
        "German BSI VSITR: three alternating 0x00/0xFF pass pairs, then a final 0xAA pass — \
         with verification"
    }
    fn pass_count(&self) -> u32 {
        7
    }
    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        match pass {
            // Passes 1-6 alternate 0x00 and 0xFF.
            p if p < 6 && p % 2 == 0 => boxed(ZeroFill),
            p if p < 6 => boxed(OneFill),
            // Pass 7 is 0xAA.
            _ => boxed(ConstantFill(0xAA)),
        }
    }
    fn includes_verification(&self) -> bool {
        true
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
        Box::new(Nist80088ClearMethod),
        Box::new(Nist80088PurgeMethod),
        Box::new(Afssi5020Method),
        Box::new(Ar380_19Method),
        Box::new(NavsoP5239_26Method),
        Box::new(VsitrMethod),
    ]
}

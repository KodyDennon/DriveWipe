use crate::crypto::AesCtrRng;

/// Trait for pattern generators used during drive wipe operations.
///
/// Each implementor fills a buffer with a specific byte pattern and provides
/// a human-readable name for logging and reporting.
pub trait PatternGenerator {
    /// Fills `buf` entirely with this generator's pattern.
    fn fill(&mut self, buf: &mut [u8]);

    /// Returns a human-readable name describing this pattern.
    fn name(&self) -> &str;
}

/// Fills the buffer with all zero bytes (`0x00`).
pub struct ZeroFill;

impl PatternGenerator for ZeroFill {
    fn fill(&mut self, buf: &mut [u8]) {
        buf.fill(0x00);
    }

    fn name(&self) -> &str {
        "ZeroFill (0x00)"
    }
}

/// Fills the buffer with all one bytes (`0xFF`).
pub struct OneFill;

impl PatternGenerator for OneFill {
    fn fill(&mut self, buf: &mut [u8]) {
        buf.fill(0xFF);
    }

    fn name(&self) -> &str {
        "OneFill (0xFF)"
    }
}

/// Fills the buffer with a single constant byte value.
pub struct ConstantFill(pub u8);

impl PatternGenerator for ConstantFill {
    fn fill(&mut self, buf: &mut [u8]) {
        buf.fill(self.0);
    }

    fn name(&self) -> &str {
        "ConstantFill"
    }
}

/// Fills the buffer with cryptographically secure random data from an AES-256-CTR PRNG.
pub struct RandomFill {
    rng: AesCtrRng,
}

impl RandomFill {
    /// Creates a new `RandomFill` backed by a freshly-seeded `AesCtrRng`.
    pub fn new() -> Self {
        Self {
            rng: AesCtrRng::new(),
        }
    }
}

impl Default for RandomFill {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternGenerator for RandomFill {
    fn fill(&mut self, buf: &mut [u8]) {
        self.rng.fill_bytes(buf);
    }

    fn name(&self) -> &str {
        "RandomFill (AES-256-CTR)"
    }
}

/// Fills the buffer by repeating a byte sequence across its entire length.
///
/// If the buffer length is not an exact multiple of the pattern length, the final
/// repetition is truncated to fit.
pub struct RepeatingPattern(pub Vec<u8>);

impl PatternGenerator for RepeatingPattern {
    fn fill(&mut self, buf: &mut [u8]) {
        if self.0.is_empty() {
            return;
        }
        let pattern = &self.0;

        // Use efficient chunk copying instead of modulo for each byte
        let mut remaining = buf;
        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(pattern.len());
            remaining[..chunk_len].copy_from_slice(&pattern[..chunk_len]);
            remaining = &mut remaining[chunk_len..];
        }
    }

    fn name(&self) -> &str {
        "RepeatingPattern"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_fill_writes_all_zeros() {
        let mut buf = [0xAA_u8; 64];
        ZeroFill.fill(&mut buf);
        assert!(buf.iter().all(|&b| b == 0x00));
    }

    #[test]
    fn one_fill_writes_all_ones() {
        let mut buf = [0x00_u8; 64];
        OneFill.fill(&mut buf);
        assert!(buf.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn constant_fill_writes_given_byte() {
        let mut buf = [0x00_u8; 64];
        ConstantFill(0x55).fill(&mut buf);
        assert!(buf.iter().all(|&b| b == 0x55));
    }

    #[test]
    fn random_fill_produces_non_zero_output() {
        let mut buf = [0x00_u8; 256];
        RandomFill::new().fill(&mut buf);
        // A 256-byte buffer of AES-CTR output should not be all zeroes.
        assert!(!buf.iter().all(|&b| b == 0x00));
    }

    #[test]
    fn random_fill_produces_different_output_each_call() {
        let mut rng = RandomFill::new();
        let mut buf1 = [0u8; 64];
        let mut buf2 = [0u8; 64];
        rng.fill(&mut buf1);
        rng.fill(&mut buf2);
        // Two successive fills from the same generator should differ.
        assert_ne!(buf1, buf2);
    }

    #[test]
    fn repeating_pattern_exact_multiple() {
        let mut buf = [0u8; 6];
        RepeatingPattern(vec![0xAA, 0xBB, 0xCC]).fill(&mut buf);
        assert_eq!(buf, [0xAA, 0xBB, 0xCC, 0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn repeating_pattern_partial_tail() {
        let mut buf = [0u8; 5];
        RepeatingPattern(vec![0x01, 0x02, 0x03]).fill(&mut buf);
        assert_eq!(buf, [0x01, 0x02, 0x03, 0x01, 0x02]);
    }

    #[test]
    fn repeating_pattern_single_byte() {
        let mut buf = [0u8; 4];
        RepeatingPattern(vec![0x42]).fill(&mut buf);
        assert!(buf.iter().all(|&b| b == 0x42));
    }

    #[test]
    fn repeating_pattern_empty_is_noop() {
        let mut buf = [0xAA_u8; 4];
        RepeatingPattern(vec![]).fill(&mut buf);
        // Buffer should remain untouched.
        assert!(buf.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn names_are_nonempty() {
        assert!(!ZeroFill.name().is_empty());
        assert!(!OneFill.name().is_empty());
        assert!(!ConstantFill(0).name().is_empty());
        assert!(!RandomFill::new().name().is_empty());
        assert!(!RepeatingPattern(vec![1]).name().is_empty());
    }
}

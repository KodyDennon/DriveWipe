use crate::crypto::AesCtrRng;

/// Trait for pattern generators used during drive wipe operations.
///
/// Each implementor fills a buffer with a specific byte pattern and provides
/// a human-readable name for logging and reporting.
///
/// Generators are offset-addressable: [`fill_at`](Self::fill_at) produces the
/// bytes belonging at an absolute device offset, which is what makes a pass
/// verifiable and keeps a resumed pass byte-identical to an uninterrupted one.
///
/// For [`RandomFill`] this holds per generator instance — each new instance
/// seeds itself afresh, so verification must reuse the instance that wrote.
pub trait PatternGenerator {
    /// Fills `buf` with the pattern bytes belonging at absolute device `offset`.
    fn fill_at(&mut self, offset: u64, buf: &mut [u8]);

    /// Equivalent to `fill_at(0, buf)`; for callers that only care about shape.
    fn fill(&mut self, buf: &mut [u8]) {
        self.fill_at(0, buf);
    }

    /// Returns a human-readable name describing this pattern.
    fn name(&self) -> &str;
}

/// Fills the buffer with all zero bytes (`0x00`).
pub struct ZeroFill;

impl PatternGenerator for ZeroFill {
    fn fill_at(&mut self, _offset: u64, buf: &mut [u8]) {
        buf.fill(0x00);
    }

    fn name(&self) -> &str {
        "ZeroFill (0x00)"
    }
}

/// Fills the buffer with all one bytes (`0xFF`).
pub struct OneFill;

impl PatternGenerator for OneFill {
    fn fill_at(&mut self, _offset: u64, buf: &mut [u8]) {
        buf.fill(0xFF);
    }

    fn name(&self) -> &str {
        "OneFill (0xFF)"
    }
}

/// Fills the buffer with a single constant byte value.
pub struct ConstantFill(pub u8);

impl PatternGenerator for ConstantFill {
    fn fill_at(&mut self, _offset: u64, buf: &mut [u8]) {
        buf.fill(self.0);
    }

    fn name(&self) -> &str {
        "ConstantFill"
    }
}

/// Fills the buffer with cryptographically secure random data from an AES-256-CTR PRNG.
///
/// Keyed to the absolute device offset, so a given instance always produces the
/// same bytes for the same offset — which makes a random pass verifiable
/// byte-for-byte rather than sampled.
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

    /// Recreates a `RandomFill` from a previously recorded seed, reproducing
    /// the exact keystream of an earlier pass.
    pub fn from_seed(key: [u8; 32], nonce: [u8; 16]) -> Self {
        Self {
            rng: AesCtrRng::from_seed(key, nonce),
        }
    }

    /// Returns the key and nonce backing this generator, so the pass can be
    /// reproduced later for verification or resumption.
    pub fn seed(&self) -> ([u8; 32], [u8; 16]) {
        self.rng.seed()
    }
}

impl Default for RandomFill {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternGenerator for RandomFill {
    fn fill_at(&mut self, offset: u64, buf: &mut [u8]) {
        self.rng.fill_bytes_at(offset, buf);
    }

    fn name(&self) -> &str {
        "RandomFill (AES-256-CTR)"
    }
}

/// Fills the buffer by repeating a byte sequence across its entire length.
///
/// Phased to the absolute device offset so the sequence repeats continuously
/// across the device rather than restarting at every buffer boundary.
pub struct RepeatingPattern(pub Vec<u8>);

impl PatternGenerator for RepeatingPattern {
    fn fill_at(&mut self, offset: u64, buf: &mut [u8]) {
        if self.0.is_empty() {
            return;
        }
        let pattern = &self.0;
        let mut phase = (offset % pattern.len() as u64) as usize;

        let mut remaining = &mut buf[..];
        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(pattern.len() - phase);
            remaining[..chunk_len].copy_from_slice(&pattern[phase..phase + chunk_len]);
            remaining = &mut remaining[chunk_len..];
            phase = 0;
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
    fn random_fill_differs_across_offsets() {
        let mut rng = RandomFill::new();
        let mut buf1 = [0u8; 64];
        let mut buf2 = [0u8; 64];
        rng.fill_at(0, &mut buf1);
        rng.fill_at(64, &mut buf2);
        // Distinct regions of the device must receive distinct keystream.
        assert_ne!(buf1, buf2);
    }

    #[test]
    fn random_fill_is_reproducible_at_a_given_offset() {
        let mut rng = RandomFill::new();
        let mut first = [0u8; 128];
        let mut second = [0u8; 128];
        rng.fill_at(4096, &mut first);
        rng.fill_at(0, &mut [0u8; 512]); // move the stream position elsewhere
        rng.fill_at(4096, &mut second);
        assert_eq!(first, second);
    }

    #[test]
    fn random_fill_reproducible_from_recorded_seed() {
        let original = RandomFill::new();
        let (key, nonce) = original.seed();
        let mut original = original;
        let mut replayed = RandomFill::from_seed(key, nonce);

        let mut a = [0u8; 96];
        let mut b = [0u8; 96];
        original.fill_at(1 << 20, &mut a);
        replayed.fill_at(1 << 20, &mut b);
        assert_eq!(a, b);
    }

    #[test]
    fn repeating_pattern_exact_multiple() {
        let mut buf = [0u8; 6];
        RepeatingPattern(vec![0xAA, 0xBB, 0xCC]).fill(&mut buf);
        assert_eq!(buf, [0xAA, 0xBB, 0xCC, 0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn repeating_pattern_is_phased_by_offset() {
        // Starting one byte into a 3-byte sequence must continue the sequence,
        // not restart it.
        let mut buf = [0u8; 5];
        RepeatingPattern(vec![0xAA, 0xBB, 0xCC]).fill_at(1, &mut buf);
        assert_eq!(buf, [0xBB, 0xCC, 0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn repeating_pattern_is_continuous_across_chunk_boundaries() {
        // Filling in two chunks must produce the same bytes as one contiguous
        // fill, even when the chunk size is not a multiple of the pattern.
        let pattern = vec![0x92, 0x49, 0x24];
        let mut whole = [0u8; 16];
        RepeatingPattern(pattern.clone()).fill_at(0, &mut whole);

        let mut split = [0u8; 16];
        let (head, tail) = split.split_at_mut(7);
        RepeatingPattern(pattern.clone()).fill_at(0, head);
        RepeatingPattern(pattern).fill_at(7, tail);

        assert_eq!(whole, split);
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

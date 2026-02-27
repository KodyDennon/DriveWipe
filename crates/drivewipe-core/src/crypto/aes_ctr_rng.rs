use aes::Aes256;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use zeroize::Zeroize;

/// AES-256-CTR based cryptographic type alias.
type Aes256Ctr = Ctr128BE<Aes256>;

/// A cryptographically secure pseudo-random number generator built on AES-256-CTR.
///
/// Generates a keystream by encrypting a stream of zeroes using AES-256 in CTR mode.
/// The key and nonce are sourced from the system CSPRNG on construction, and all
/// sensitive material is zeroized on drop.
pub struct AesCtrRng {
    cipher: Aes256Ctr,
    /// Retained only so we can zeroize on drop.
    key: [u8; 32],
    /// Retained only so we can zeroize on drop.
    nonce: [u8; 16],
}

impl AesCtrRng {
    /// Creates a new `AesCtrRng` seeded from the operating system's CSPRNG
    /// via `rand::rng()`.
    pub fn new() -> Self {
        let mut rng = rand::rng();
        let key: [u8; 32] = rand::Rng::random(&mut rng);
        let nonce: [u8; 16] = rand::Rng::random(&mut rng);
        Self::from_seed(key, nonce)
    }

    /// Creates a new `AesCtrRng` from an explicit 256-bit key and 128-bit nonce.
    pub fn from_seed(key: [u8; 32], nonce: [u8; 16]) -> Self {
        let cipher = Aes256Ctr::new(&key.into(), &nonce.into());
        Self { cipher, key, nonce }
    }

    /// Fills `buf` with pseudorandom keystream bytes.
    ///
    /// The buffer is first zeroed, then the AES-256-CTR keystream is XORed over it,
    /// which effectively writes the raw keystream into the buffer.
    pub fn fill_bytes(&mut self, buf: &mut [u8]) {
        buf.fill(0);
        self.cipher.apply_keystream(buf);
    }
}

impl Drop for AesCtrRng {
    fn drop(&mut self) {
        self.key.zeroize();
        self.nonce.zeroize();
    }
}

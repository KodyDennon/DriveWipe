//! AES-256-CTR stream encryption for clone image chunks.

use aes::Aes256;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use sha2::{Digest, Sha256};

type Aes256Ctr = Ctr128BE<Aes256>;

/// Derives a 256-bit encryption key from a password and salt using SHA-256 iterated hashing.
/// This is a simplified KDF — for production with user-facing passwords, use Argon2 or PBKDF2.
pub fn derive_key(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    let mut hasher = Sha256::new();
    hasher.update(password);
    hasher.update(salt);
    let mut hash = hasher.finalize_reset();

    for _ in 1..iterations {
        hasher.update(hash);
        hasher.update(salt);
        hash = hasher.finalize_reset();
    }

    key.copy_from_slice(&hash);
    key
}

/// Generate a random salt.
pub fn generate_salt() -> [u8; 16] {
    use rand::RngExt;
    let mut rng = rand::rng();
    rng.random()
}

/// Generate a random nonce/IV for AES-CTR.
pub fn generate_nonce() -> [u8; 16] {
    use rand::RngExt;
    let mut rng = rand::rng();
    rng.random()
}

/// Encrypt data in-place using AES-256-CTR.
pub fn encrypt_chunk(data: &mut [u8], key: &[u8; 32], nonce: &[u8; 16]) {
    let mut cipher = Aes256Ctr::new(key.into(), nonce.into());
    cipher.apply_keystream(data);
}

/// Decrypt data in-place using AES-256-CTR (same as encrypt for CTR mode).
pub fn decrypt_chunk(data: &mut [u8], key: &[u8; 32], nonce: &[u8; 16]) {
    encrypt_chunk(data, key, nonce);
}

/// Increment a 128-bit nonce (big-endian) for per-chunk unique IVs.
pub fn increment_nonce(nonce: &mut [u8; 16]) {
    for byte in nonce.iter_mut().rev() {
        let (val, overflow) = byte.overflowing_add(1);
        *byte = val;
        if !overflow {
            break;
        }
    }
}

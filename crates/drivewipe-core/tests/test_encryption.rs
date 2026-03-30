use drivewipe_core::crypto::encrypt::*;

#[test]
fn test_encrypt_decrypt_roundtrip() {
    let key = derive_key(b"test-password", b"salt1234salt1234", 1000);
    let nonce = generate_nonce();
    let original = vec![0x42u8; 4096];
    let mut data = original.clone();
    encrypt_chunk(&mut data, &key, &nonce);
    assert_ne!(data, original);
    decrypt_chunk(&mut data, &key, &nonce);
    assert_eq!(data, original);
}

#[test]
fn test_different_salts_different_keys() {
    let k1 = derive_key(b"pw", b"salt_one________", 100);
    let k2 = derive_key(b"pw", b"salt_two________", 100);
    assert_ne!(k1, k2);
}

#[test]
fn test_nonce_increment_single_block() {
    let mut nonce = [0u8; 16];
    nonce[15] = 1;
    increment_nonce_by_data_len(&mut nonce, 16);
    assert_eq!(u128::from_be_bytes(nonce), 2);
}

#[test]
fn test_nonce_increment_4mib() {
    let mut nonce = [0u8; 16];
    increment_nonce_by_data_len(&mut nonce, 4 * 1024 * 1024);
    assert_eq!(u128::from_be_bytes(nonce), 262144);
}

#[test]
fn test_nonce_increment_non_aligned() {
    let mut nonce = [0u8; 16];
    increment_nonce_by_data_len(&mut nonce, 15);
    assert_eq!(u128::from_be_bytes(nonce), 1);
}

#[test]
fn test_no_keystream_reuse() {
    let key = derive_key(b"pw", b"salt1234salt1234", 100);
    let mut nonce = generate_nonce();
    let mut c1 = vec![0u8; 64];
    let mut c2 = vec![0u8; 64];
    encrypt_chunk(&mut c1, &key, &nonce);
    increment_nonce_by_data_len(&mut nonce, 64);
    encrypt_chunk(&mut c2, &key, &nonce);
    assert_ne!(c1, c2);
}

#[test]
fn test_empty_password() {
    let key = derive_key(b"", b"salt1234salt1234", 100);
    assert_ne!(key, [0u8; 32]);
}

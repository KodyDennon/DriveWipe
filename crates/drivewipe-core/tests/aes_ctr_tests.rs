use drivewipe_core::crypto::aes_ctr_rng::AesCtrRng;

#[test]
fn fills_non_zero_bytes() {
    let mut rng = AesCtrRng::new();
    let mut buf = vec![0u8; 4096];
    rng.fill_bytes(&mut buf);
    // An all-zero output from a random generator is astronomically unlikely.
    assert!(buf.iter().any(|&b| b != 0), "PRNG produced all zeros");
}

#[test]
fn produces_different_output_each_call() {
    let mut rng = AesCtrRng::new();
    let mut buf1 = vec![0u8; 1024];
    let mut buf2 = vec![0u8; 1024];
    rng.fill_bytes(&mut buf1);
    rng.fill_bytes(&mut buf2);
    // The CTR keystream advances, so consecutive outputs should differ.
    assert_ne!(buf1, buf2);
}

#[test]
fn from_seed_is_deterministic() {
    let key = [0x42u8; 32];
    let nonce = [0x13u8; 16];

    let mut rng1 = AesCtrRng::from_seed(key, nonce);
    let mut rng2 = AesCtrRng::from_seed(key, nonce);

    let mut buf1 = vec![0u8; 512];
    let mut buf2 = vec![0u8; 512];
    rng1.fill_bytes(&mut buf1);
    rng2.fill_bytes(&mut buf2);

    assert_eq!(buf1, buf2, "same seed should produce same output");
}

#[test]
fn different_seeds_produce_different_output() {
    let key1 = [0x01u8; 32];
    let key2 = [0x02u8; 32];
    let nonce = [0x00u8; 16];

    let mut rng1 = AesCtrRng::from_seed(key1, nonce);
    let mut rng2 = AesCtrRng::from_seed(key2, nonce);

    let mut buf1 = vec![0u8; 512];
    let mut buf2 = vec![0u8; 512];
    rng1.fill_bytes(&mut buf1);
    rng2.fill_bytes(&mut buf2);

    assert_ne!(buf1, buf2, "different seeds should produce different output");
}

#[test]
fn large_fill() {
    let mut rng = AesCtrRng::new();
    let mut buf = vec![0u8; 1024 * 1024]; // 1 MiB
    rng.fill_bytes(&mut buf);
    // Verify at least some randomness — count distinct bytes.
    let mut seen = [false; 256];
    for &b in &buf {
        seen[b as usize] = true;
    }
    let distinct = seen.iter().filter(|&&s| s).count();
    assert!(distinct >= 200, "expected diverse byte distribution, got {distinct}");
}

#[test]
fn drop_cleans_up_without_panic() {
    let rng = AesCtrRng::new();
    drop(rng);
    // If we get here without a panic/segfault, the Drop impl is safe.
}

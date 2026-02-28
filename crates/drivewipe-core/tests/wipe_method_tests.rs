use drivewipe_core::wipe::WipeMethod;
use drivewipe_core::wipe::software::*;

#[test]
fn zero_fill_method_metadata() {
    let m = ZeroFillMethod;
    assert_eq!(m.id(), "zero");
    assert_eq!(m.pass_count(), 1);
    assert!(!m.includes_verification());
    assert!(!m.is_firmware());
}

#[test]
fn one_fill_method_metadata() {
    let m = OneFillMethod;
    assert_eq!(m.id(), "one");
    assert_eq!(m.pass_count(), 1);
}

#[test]
fn random_fill_method_metadata() {
    let m = RandomFillMethod;
    assert_eq!(m.id(), "random");
    assert_eq!(m.pass_count(), 1);
}

#[test]
fn dod_short_method_metadata() {
    let m = DodShortMethod;
    assert_eq!(m.id(), "dod-short");
    assert_eq!(m.pass_count(), 3);
    assert!(m.includes_verification());
}

#[test]
fn dod_ece_method_metadata() {
    let m = DodEceMethod;
    assert_eq!(m.id(), "dod-ece");
    assert_eq!(m.pass_count(), 7);
    assert!(m.includes_verification());
}

#[test]
fn gutmann_method_metadata() {
    let m = GutmannMethod;
    assert_eq!(m.id(), "gutmann");
    assert_eq!(m.pass_count(), 35);
    assert!(!m.includes_verification());
}

#[test]
fn hmg_baseline_method_metadata() {
    let m = HmgBaselineMethod;
    assert_eq!(m.id(), "hmg-baseline");
    assert_eq!(m.pass_count(), 1);
    assert!(m.includes_verification());
}

#[test]
fn hmg_enhanced_method_metadata() {
    let m = HmgEnhancedMethod;
    assert_eq!(m.id(), "hmg-enhanced");
    assert_eq!(m.pass_count(), 3);
    assert!(m.includes_verification());
}

#[test]
fn rcmp_method_metadata() {
    let m = RcmpMethod;
    assert_eq!(m.id(), "rcmp");
    assert_eq!(m.pass_count(), 7);
    assert!(!m.includes_verification());
}

#[test]
fn zero_fill_generates_zeros() {
    let m = ZeroFillMethod;
    let mut pat = m.pattern_for_pass(0);
    let mut buf = vec![0xFF; 1024];
    pat.fill(&mut buf);
    assert!(buf.iter().all(|&b| b == 0x00));
}

#[test]
fn one_fill_generates_ones() {
    let m = OneFillMethod;
    let mut pat = m.pattern_for_pass(0);
    let mut buf = vec![0x00; 1024];
    pat.fill(&mut buf);
    assert!(buf.iter().all(|&b| b == 0xFF));
}

#[test]
fn dod_short_pass_patterns() {
    let m = DodShortMethod;

    // Pass 0: zeros
    let mut pat = m.pattern_for_pass(0);
    let mut buf = vec![0xFF; 512];
    pat.fill(&mut buf);
    assert!(buf.iter().all(|&b| b == 0x00));

    // Pass 1: ones
    let mut pat = m.pattern_for_pass(1);
    buf.fill(0x00);
    pat.fill(&mut buf);
    assert!(buf.iter().all(|&b| b == 0xFF));

    // Pass 2: random (non-deterministic — just check it's not all-zero)
    let mut pat = m.pattern_for_pass(2);
    buf.fill(0x00);
    pat.fill(&mut buf);
    assert!(buf.iter().any(|&b| b != 0x00));
}

#[test]
fn rcmp_alternating_pattern() {
    let m = RcmpMethod;

    // Even passes (0, 2, 4) should produce zeros.
    for pass in [0, 2, 4] {
        let mut pat = m.pattern_for_pass(pass);
        let mut buf = vec![0xFF; 256];
        pat.fill(&mut buf);
        assert!(
            buf.iter().all(|&b| b == 0x00),
            "pass {pass} should be zeros"
        );
    }

    // Odd passes (1, 3, 5) should produce ones.
    for pass in [1, 3, 5] {
        let mut pat = m.pattern_for_pass(pass);
        let mut buf = vec![0x00; 256];
        pat.fill(&mut buf);
        assert!(buf.iter().all(|&b| b == 0xFF), "pass {pass} should be ones");
    }
}

#[test]
fn gutmann_pass_5_is_0x55() {
    let m = GutmannMethod;
    let mut pat = m.pattern_for_pass(4); // 0-indexed pass 5
    let mut buf = vec![0x00; 256];
    pat.fill(&mut buf);
    assert!(buf.iter().all(|&b| b == 0x55));
}

#[test]
fn all_software_methods_returns_nine() {
    let methods = all_software_methods();
    assert_eq!(methods.len(), 9);
}

#[test]
fn all_software_methods_unique_ids() {
    let methods = all_software_methods();
    let ids: Vec<&str> = methods.iter().map(|m| m.id()).collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "duplicate method IDs found");
}

#[test]
fn all_software_methods_non_firmware() {
    let methods = all_software_methods();
    for m in &methods {
        assert!(!m.is_firmware(), "software method {} is firmware", m.id());
    }
}

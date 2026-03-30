use drivewipe_core::wipe::WipeMethod;
use drivewipe_core::wipe::software::GutmannMethod;

#[test]
fn test_gutmann_35_passes() {
    assert_eq!(GutmannMethod.pass_count(), 35);
}

#[test]
fn test_gutmann_random_passes() {
    let m = GutmannMethod;
    for pass in 0..4 {
        assert!(
            m.pattern_for_pass(pass).name().contains("Random"),
            "Pass {} should be Random, got: {}",
            pass,
            m.pattern_for_pass(pass).name()
        );
    }
    for pass in 31..35 {
        assert!(
            m.pattern_for_pass(pass).name().contains("Random"),
            "Pass {} should be Random, got: {}",
            pass,
            m.pattern_for_pass(pass).name()
        );
    }
}

#[test]
fn test_gutmann_pass5_0x55() {
    let mut buf = [0u8; 3];
    GutmannMethod.pattern_for_pass(4).fill(&mut buf);
    assert_eq!(buf, [0x55, 0x55, 0x55]);
}

#[test]
fn test_gutmann_pass9_corrected() {
    let mut buf = [0u8; 6];
    GutmannMethod.pattern_for_pass(8).fill(&mut buf);
    assert_eq!(buf, [0x24, 0x92, 0x49, 0x24, 0x92, 0x49]);
}

#[test]
fn test_gutmann_pass7_mfm() {
    let mut buf = [0u8; 6];
    GutmannMethod.pattern_for_pass(6).fill(&mut buf);
    assert_eq!(buf, [0x92, 0x49, 0x24, 0x92, 0x49, 0x24]);
}

#[test]
fn test_gutmann_pass8_mfm() {
    let mut buf = [0u8; 6];
    GutmannMethod.pattern_for_pass(7).fill(&mut buf);
    assert_eq!(buf, [0x49, 0x24, 0x92, 0x49, 0x24, 0x92]);
}

#[test]
fn test_gutmann_constant_fills() {
    // Passes 10-16 (0-indexed: 9..16) are constant fills: 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66
    let expected = [0x00u8, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
    for (i, &exp) in expected.iter().enumerate() {
        let pass_0idx = 9 + i as u32;
        let mut buf = [0u8; 1];
        GutmannMethod.pattern_for_pass(pass_0idx).fill(&mut buf);
        assert_eq!(
            buf[0],
            exp,
            "Pass {} (0-indexed {}) expected {:#04x}, got {:#04x}",
            pass_0idx + 1,
            pass_0idx,
            exp,
            buf[0]
        );
    }
}

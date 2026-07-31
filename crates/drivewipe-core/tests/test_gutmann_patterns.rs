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
    // Passes 10-25 (0-indexed 9..25) are the sixteen constants 0x00..0xFF.
    let expected: Vec<u8> = (0..16).map(|i| i * 0x11).collect();
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

#[test]
fn test_gutmann_second_mfm_group() {
    // Passes 26-28 repeat the MFM/RLL sequence from passes 7-9.
    let expected: [[u8; 3]; 3] = [[0x92, 0x49, 0x24], [0x49, 0x24, 0x92], [0x24, 0x92, 0x49]];
    for (i, exp) in expected.iter().enumerate() {
        let mut buf = [0u8; 3];
        GutmannMethod.pattern_for_pass(25 + i as u32).fill(&mut buf);
        assert_eq!(&buf, exp, "pass {} (0-indexed {})", 26 + i, 25 + i);
    }
}

#[test]
fn test_gutmann_complementary_rll_group() {
    // Passes 29-31 are the complementary RLL sequence 0x6D 0xB6 0xDB and its
    // two rotations. These were absent entirely before.
    let expected: [[u8; 3]; 3] = [[0x6D, 0xB6, 0xDB], [0xB6, 0xDB, 0x6D], [0xDB, 0x6D, 0xB6]];
    for (i, exp) in expected.iter().enumerate() {
        let mut buf = [0u8; 3];
        GutmannMethod.pattern_for_pass(28 + i as u32).fill(&mut buf);
        assert_eq!(&buf, exp, "pass {} (0-indexed {})", 29 + i, 28 + i);
    }
}

#[test]
fn test_gutmann_covers_every_constant_exactly_once() {
    // Each of the sixteen 0xNN constants must appear exactly once across the
    // whole 35-pass sequence; 0x77 in particular used to be missing.
    let mut seen = std::collections::BTreeSet::new();
    for pass in 9..25 {
        let mut buf = [0u8; 1];
        GutmannMethod.pattern_for_pass(pass).fill(&mut buf);
        assert!(
            seen.insert(buf[0]),
            "constant {:#04x} appeared twice",
            buf[0]
        );
    }
    let expected: std::collections::BTreeSet<u8> = (0..16).map(|i| i * 0x11).collect();
    assert_eq!(seen, expected);
}

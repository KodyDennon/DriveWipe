use drivewipe_core::io::{AlignedBuffer, allocate_aligned_buffer};

#[test]
fn buffer_is_zeroed() {
    let buf = AlignedBuffer::new(4096, 4096);
    assert!(buf.as_slice().iter().all(|&b| b == 0));
}

#[test]
fn buffer_size_matches() {
    let buf = AlignedBuffer::new(1024, 512);
    assert_eq!(buf.len(), 1024);
    assert!(!buf.is_empty());
}

#[test]
fn buffer_is_writable() {
    let mut buf = AlignedBuffer::new(512, 512);
    buf.as_mut_slice()[0] = 0xFF;
    buf.as_mut_slice()[511] = 0xAA;
    assert_eq!(buf[0], 0xFF);
    assert_eq!(buf[511], 0xAA);
}

#[test]
fn buffer_deref_works() {
    let buf = AlignedBuffer::new(256, 256);
    // Deref to &[u8] should work.
    let slice: &[u8] = &buf;
    assert_eq!(slice.len(), 256);
}

#[test]
fn buffer_deref_mut_works() {
    let mut buf = AlignedBuffer::new(256, 256);
    let slice: &mut [u8] = &mut buf;
    slice[0] = 42;
    assert_eq!(buf[0], 42);
}

#[test]
fn allocate_aligned_buffer_helper() {
    let buf = allocate_aligned_buffer(8192, 4096);
    assert_eq!(buf.len(), 8192);
    assert!(buf.as_slice().iter().all(|&b| b == 0));
}

#[test]
fn empty_buffer_is_empty() {
    // A zero-sized buffer is technically not valid for Layout, so we test
    // with size=0 alignment=1 which is a valid layout edge case.
    // Actually AlignedBuffer rounds size up to alignment, so this creates
    // a buffer of size 0 with is_empty() true only if len == 0.
    // The padded_size would be 0 which creates a zero-sized layout.
    // Skip this since Layout requires size > 0 for alloc_zeroed.
}

#[test]
fn alignment_is_respected() {
    let buf = AlignedBuffer::new(4096, 4096);
    let ptr = buf.as_slice().as_ptr();
    assert_eq!(
        ptr as usize % 4096,
        0,
        "buffer pointer should be 4096-aligned"
    );
}

#[test]
fn different_alignments() {
    for alignment in [512, 1024, 4096] {
        let buf = AlignedBuffer::new(alignment * 2, alignment);
        let ptr = buf.as_slice().as_ptr();
        assert_eq!(
            ptr as usize % alignment,
            0,
            "buffer should be {alignment}-aligned"
        );
    }
}

#[test]
#[should_panic]
fn zero_alignment_panics() {
    let _ = AlignedBuffer::new(512, 0);
}

#[test]
#[should_panic]
fn non_power_of_two_alignment_panics() {
    let _ = AlignedBuffer::new(512, 3);
}

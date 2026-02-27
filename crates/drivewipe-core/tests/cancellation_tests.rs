use drivewipe_core::session::CancellationToken;

#[test]
fn new_token_is_not_cancelled() {
    let ct = CancellationToken::new();
    assert!(!ct.is_cancelled());
}

#[test]
fn cancel_sets_flag() {
    let ct = CancellationToken::new();
    ct.cancel();
    assert!(ct.is_cancelled());
}

#[test]
fn clone_token_shares_state() {
    let ct = CancellationToken::new();
    let ct2 = ct.clone_token();

    assert!(!ct2.is_cancelled());
    ct.cancel();
    assert!(ct2.is_cancelled());
}

#[test]
fn reset_clears_cancellation() {
    let ct = CancellationToken::new();
    ct.cancel();
    assert!(ct.is_cancelled());

    ct.reset();
    assert!(!ct.is_cancelled());
}

#[test]
fn reset_affects_clones() {
    let ct = CancellationToken::new();
    let ct2 = ct.clone_token();

    ct.cancel();
    assert!(ct2.is_cancelled());

    ct.reset();
    assert!(!ct2.is_cancelled());
}

#[test]
fn multiple_cancels_idempotent() {
    let ct = CancellationToken::new();
    ct.cancel();
    ct.cancel();
    ct.cancel();
    assert!(ct.is_cancelled());
}

#[test]
fn cross_thread_cancellation() {
    use std::sync::Arc;
    use std::thread;

    let ct = Arc::new(CancellationToken::new());
    let ct2 = ct.clone();

    let handle = thread::spawn(move || {
        // Wait a tiny bit then cancel.
        thread::sleep(std::time::Duration::from_millis(10));
        ct2.cancel();
    });

    // Poll until cancelled (with timeout).
    let start = std::time::Instant::now();
    while !ct.is_cancelled() {
        assert!(
            start.elapsed().as_millis() < 5000,
            "Timed out waiting for cross-thread cancellation"
        );
        thread::sleep(std::time::Duration::from_millis(1));
    }

    handle.join().unwrap();
}

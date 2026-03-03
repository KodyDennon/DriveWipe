use drivewipe_core::keyboard_lock::{KeySequenceDetector, KeyboardLockState};

#[test]
fn test_keyboard_lock_state_transitions() {
    let state = KeyboardLockState::Unlocked;
    assert_eq!(state, KeyboardLockState::Unlocked);

    let state = KeyboardLockState::Locked;
    assert_eq!(state, KeyboardLockState::Locked);
}

#[test]
fn test_key_sequence_detector_default_sequence() {
    let mut detector = KeySequenceDetector::new("qwerty");
    detector.lock();
    assert!(detector.is_locked());

    // Feed partial sequence
    assert!(!detector.process_key('q'));
    assert!(!detector.process_key('w'));
    assert!(!detector.process_key('e'));
    assert!(!detector.process_key('r'));
    assert!(!detector.process_key('t'));
    assert!(detector.process_key('y')); // Complete! Should unlock
    assert!(!detector.is_locked());
}

#[test]
fn test_key_sequence_detector_wrong_keys() {
    let mut detector = KeySequenceDetector::new("abc");
    detector.lock();

    // Feed partial + wrong key
    detector.process_key('a');
    detector.process_key('x'); // wrong
    // Now type correct sequence
    assert!(!detector.process_key('a'));
    assert!(!detector.process_key('b'));
    assert!(detector.process_key('c'));
    assert!(!detector.is_locked());
}

#[test]
fn test_key_sequence_detector_not_locked() {
    let mut detector = KeySequenceDetector::new("abc");

    // process_key should return false when not locked
    assert!(!detector.process_key('a'));
    assert!(!detector.process_key('b'));
    assert!(!detector.process_key('c'));
}

#[test]
fn test_key_sequence_detector_empty_sequence() {
    let mut detector = KeySequenceDetector::new("");
    detector.lock();
    // With empty sequence, keys shouldn't unlock (ring buffer won't match)
    assert!(!detector.process_key('a'));
    // Still locked
    assert!(detector.is_locked());
}

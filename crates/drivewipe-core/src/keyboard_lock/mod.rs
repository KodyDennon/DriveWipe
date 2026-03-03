use std::collections::VecDeque;

/// Current state of keyboard lock mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLockState {
    Unlocked,
    Locked,
}

/// Detects a configurable unlock key sequence using a ring buffer.
pub struct KeySequenceDetector {
    /// The unlock sequence to match against.
    unlock_sequence: Vec<char>,
    /// Ring buffer of recent key presses.
    buffer: VecDeque<char>,
    /// Current lock state.
    state: KeyboardLockState,
}

impl KeySequenceDetector {
    /// Create a new detector with the given unlock sequence.
    pub fn new(unlock_sequence: &str) -> Self {
        let chars: Vec<char> = unlock_sequence.chars().collect();
        let capacity = chars.len();
        Self {
            unlock_sequence: chars,
            buffer: VecDeque::with_capacity(capacity),
            state: KeyboardLockState::Unlocked,
        }
    }

    /// Lock the keyboard.
    pub fn lock(&mut self) {
        self.state = KeyboardLockState::Locked;
        self.buffer.clear();
        log::info!("Keyboard locked");
    }

    /// Get the current lock state.
    pub fn state(&self) -> KeyboardLockState {
        self.state
    }

    /// Check if the keyboard is locked.
    pub fn is_locked(&self) -> bool {
        self.state == KeyboardLockState::Locked
    }

    /// Process a key press. Returns true if the unlock sequence was matched
    /// (and the keyboard is now unlocked).
    pub fn process_key(&mut self, ch: char) -> bool {
        if self.state == KeyboardLockState::Unlocked {
            return false;
        }

        let max_len = self.unlock_sequence.len();
        if self.buffer.len() >= max_len {
            self.buffer.pop_front();
        }
        self.buffer.push_back(ch);

        if self.buffer.len() == max_len {
            let matches = self
                .buffer
                .iter()
                .zip(self.unlock_sequence.iter())
                .all(|(a, b)| a == b);

            if matches {
                self.state = KeyboardLockState::Unlocked;
                self.buffer.clear();
                log::info!("Keyboard unlocked");
                return true;
            }
        }

        false
    }
}

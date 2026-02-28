use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, bounded, select, tick};
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};

use drivewipe_core::progress::ProgressEvent;

/// All events the application loop can receive.
pub enum AppEvent {
    /// A keyboard event from the terminal.
    Key(KeyEvent),
    /// A progress event from the wipe engine.
    Progress(ProgressEvent),
    /// A periodic tick for UI refresh.
    Tick,
    /// Terminal resize event.
    #[allow(dead_code)]
    Resize(u16, u16),
}

/// Multiplexed event source that merges terminal input, progress updates,
/// and periodic ticks into a single channel.
pub struct EventHandler {
    rx: Receiver<AppEvent>,
    _input_thread: std::thread::JoinHandle<()>,
}

impl EventHandler {
    /// Create a new event handler.
    ///
    /// `tick_rate` controls how often a `Tick` event is generated.
    /// `progress_rx` receives wipe engine progress updates.
    pub fn new(tick_rate: Duration, progress_rx: Option<Receiver<ProgressEvent>>) -> Self {
        let (tx, rx) = bounded::<AppEvent>(256);

        // Spawn a thread that reads crossterm events and forwards them.
        let input_tx = tx.clone();
        let _input_thread = std::thread::spawn(move || {
            let ticker = tick(tick_rate);
            loop {
                select! {
                    recv(ticker) -> _ => {
                        if input_tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                    default(Duration::from_millis(50)) => {
                        // Poll crossterm for input events
                        if event::poll(Duration::from_millis(0)).unwrap_or(false) {
                            if let Ok(evt) = event::read() {
                                match evt {
                                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                                        if input_tx.send(AppEvent::Key(key)).is_err() {
                                            break;
                                        }
                                    }
                                    Event::Resize(w, h) => {
                                        if input_tx.send(AppEvent::Resize(w, h)).is_err() {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        });

        // If we have a progress channel, spawn a forwarder thread.
        if let Some(prx) = progress_rx {
            let progress_tx = tx.clone();
            std::thread::spawn(move || {
                while let Ok(evt) = prx.recv() {
                    if progress_tx.send(AppEvent::Progress(evt)).is_err() {
                        break;
                    }
                }
            });
        }

        Self { rx, _input_thread }
    }

    /// Receive the next event, blocking until one is available.
    pub fn next(&self) -> Result<AppEvent, crossbeam_channel::RecvError> {
        self.rx.recv()
    }

    /// Try to receive an event without blocking.
    #[allow(dead_code)]
    pub fn try_next(&self) -> Option<AppEvent> {
        self.rx.try_recv().ok()
    }

    /// Get the underlying receiver for use with `select!`.
    #[allow(dead_code)]
    pub fn receiver(&self) -> &Receiver<AppEvent> {
        &self.rx
    }
}

/// Create a progress channel pair (sender for wipe threads, receiver for the event handler).
pub fn progress_channel() -> (Sender<ProgressEvent>, Receiver<ProgressEvent>) {
    bounded::<ProgressEvent>(512)
}

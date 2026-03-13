use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use drivewipe_core::progress::ProgressEvent;
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver};

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
}

impl EventHandler {
    /// Create a new event handler.
    pub fn new(
        tick_rate: Duration,
        progress_rx: Option<crossbeam_channel::Receiver<ProgressEvent>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<AppEvent>(256);

        // Task for terminal input and ticks
        let input_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if input_tx.send(AppEvent::Tick).await.is_err() {
                            break;
                        }
                    }
                    else => {
                        // Poll crossterm for input events using spawn_blocking
                        let input_tx_inner = input_tx.clone();
                        let evt = tokio::task::spawn_blocking(move || {
                            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                                event::read().ok()
                            } else {
                                None
                            }
                        }).await;

                        if let Ok(Some(evt)) = evt {
                            match evt {
                                Event::Key(key) if key.kind == KeyEventKind::Press => {
                                    if input_tx_inner.send(AppEvent::Key(key)).await.is_err() {
                                        break;
                                    }
                                }
                                Event::Resize(w, h) => {
                                    if input_tx_inner.send(AppEvent::Resize(w, h)).await.is_err() {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        });

        // Task for progress events
        if let Some(prx) = progress_rx {
            let progress_tx = tx.clone();
            tokio::spawn(async move {
                while let Ok(evt) = tokio::task::spawn_blocking({
                    let prx = prx.clone();
                    move || prx.recv()
                })
                .await
                .unwrap_or(Err(crossbeam_channel::RecvError))
                {
                    if progress_tx.send(AppEvent::Progress(evt)).await.is_err() {
                        break;
                    }
                }
            });
        }

        Self { rx }
    }

    /// Receive the next event, blocking until one is available.
    pub async fn next(&mut self) -> Result<AppEvent, ()> {
        self.rx.recv().await.ok_or(())
    }
}

/// Create a progress channel pair (sender for wipe threads, receiver for the event handler).
pub fn progress_channel() -> (
    crossbeam_channel::Sender<ProgressEvent>,
    crossbeam_channel::Receiver<ProgressEvent>,
) {
    crossbeam_channel::bounded::<ProgressEvent>(512)
}

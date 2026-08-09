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

        // Terminal reads are blocking, so keep them on a dedicated blocking
        // task.  This must be separate from the tick timer: `else` in
        // `tokio::select!` only runs when every branch is disabled, not while
        // an enabled timer is pending, so using it for input polling makes the
        // keyboard branch permanently unreachable.
        let input_tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            while !input_tx.is_closed() {
                let app_event = if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                            Some(AppEvent::Key(key))
                        }
                        Ok(Event::Resize(w, h)) => Some(AppEvent::Resize(w, h)),
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(app_event) = app_event
                    && input_tx.blocking_send(app_event).is_err()
                {
                    break;
                }
            }
        });

        // Periodic redraws run independently from keyboard input.
        let tick_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).await.is_err() {
                    break;
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

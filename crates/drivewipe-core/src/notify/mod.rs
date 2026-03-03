use crate::error::Result;

/// Urgency level for desktop notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

/// Trait for platform-specific notification delivery.
pub trait Notifier: Send + Sync {
    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()>;
}

/// Send a desktop notification using the best available platform method.
pub fn send_notification(title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
    let notifier = create_notifier();
    notifier.send(title, body, urgency)
}

/// Create the platform-appropriate notifier.
fn create_notifier() -> Box<dyn Notifier> {
    Box::new(DefaultNotifier)
}

/// Default notifier using the `notify-rust` crate.
struct DefaultNotifier;

impl Notifier for DefaultNotifier {
    fn send(&self, title: &str, body: &str, urgency: NotificationUrgency) -> Result<()> {
        use notify_rust::Notification;

        let mut notification = Notification::new();
        notification.summary(title).body(body);

        // Set timeout based on urgency
        match urgency {
            NotificationUrgency::Low => { notification.timeout(5000); }
            NotificationUrgency::Normal => { notification.timeout(10000); }
            NotificationUrgency::Critical => { notification.timeout(0); } // persistent
        }

        notification.show().map_err(|e| {
            crate::error::DriveWipeError::Notification(format!(
                "Failed to send notification: {e}"
            ))
        })?;

        Ok(())
    }
}

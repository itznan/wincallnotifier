use tracing::{error, info};
use winrt_notification::{Duration, Scenario, Sound, Toast};
use crate::call::state::CallEvent;
use crate::config::AppConfig;
use crate::error::{AppError, Result};

pub const APP_ID: &str = Toast::POWERSHELL_APP_ID;

#[derive(Debug, Clone)]
pub struct NotificationManager {}

impl NotificationManager {
    pub fn new() -> Self {
        Self {}
    }

    /// Display Windows Toast notification showing strictly the phone number
    pub fn show_call_event(&self, event: &CallEvent, config: &AppConfig) -> Result<()> {
        let (title, message, is_incoming, enabled) = match event {
            CallEvent::IncomingCall { metadata } => {
                let number_display = metadata
                    .caller_number
                    .as_deref()
                    .unwrap_or("Unknown Number");
                (
                    "📞 Incoming Call".to_string(),
                    format!("Incoming call from {number_display}"),
                    true,
                    config.notify_incoming,
                )
            }
            CallEvent::OutgoingCall { metadata } => {
                let number_display = metadata
                    .caller_number
                    .as_deref()
                    .unwrap_or("Dialing...");
                (
                    "📞 Outgoing Call".to_string(),
                    format!("Call started to {number_display}"),
                    false,
                    config.notify_outgoing,
                )
            }
            CallEvent::CallConnected { metadata } => {
                let desc = if let Some(ref num) = metadata.caller_number {
                    format!("Call connected with {num}")
                } else {
                    "The call is now active".to_string()
                };
                (
                    "📞 Call Connected".to_string(),
                    desc,
                    false,
                    config.notify_connected,
                )
            }
            CallEvent::CallEnded { duration_secs } => {
                let desc = if let Some(d) = duration_secs {
                    format!("The call has ended (duration: {d}s)")
                } else {
                    "The call has ended".to_string()
                };
                (
                    "📞 Call Ended".to_string(),
                    desc,
                    false,
                    config.notify_ended,
                )
            }
        };

        if !enabled {
            info!("Notification disabled for this event type: {}", title);
            return Ok(());
        }

        self.send_toast_internal(&title, &message, is_incoming, config.notification_sound)
    }

    /// Sends an arbitrary informational notification
    pub fn send_toast(&self, title: &str, message: &str, sound: bool) -> Result<()> {
        self.send_toast_internal(title, message, false, sound)
    }

    fn send_toast_internal(&self, title: &str, message: &str, is_incoming: bool, sound: bool) -> Result<()> {
        let toast = Toast::new(APP_ID);
        let mut toast = toast.title(title).text1(message);

        if is_incoming {
            // IncomingCall scenario causes toast to stay prominent on screen
            toast = toast.scenario(Scenario::IncomingCall);
        } else {
            toast = toast.duration(Duration::Short);
        }

        let toast = if sound {
            toast.sound(Some(Sound::Default))
        } else {
            toast.sound(None)
        };

        toast.show().map_err(|e| {
            error!("Failed to show toast notification: {:?}", e);
            AppError::NotificationError(format!("{:?}", e))
        })?;

        info!("Toast notification displayed: {} - {}", title, message);
        Ok(())
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

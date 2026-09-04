use std::fmt;
use serde::{Deserialize, Serialize};

/// Normalized telephony call states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallState {
    Idle,
    Ringing,
    Offhook,
    Unknown,
}

impl Default for CallState {
    fn default() -> Self {
        CallState::Unknown
    }
}

impl fmt::Display for CallState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallState::Idle => write!(f, "IDLE"),
            CallState::Ringing => write!(f, "RINGING"),
            CallState::Offhook => write!(f, "OFFHOOK"),
            CallState::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Metadata captured during call detection containing exclusively the phone number
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallMetadata {
    pub caller_number: Option<String>,
}

/// High-level logical call event derived from transitions in CallState
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallEvent {
    IncomingCall {
        metadata: CallMetadata,
    },
    OutgoingCall {
        metadata: CallMetadata,
    },
    CallConnected {
        metadata: CallMetadata,
    },
    CallEnded {
        duration_secs: Option<u64>,
    },
}

impl fmt::Display for CallEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallEvent::IncomingCall { metadata } => {
                if let Some(num) = &metadata.caller_number {
                    write!(f, "Incoming Call from {}", num)
                } else {
                    write!(f, "Incoming Call")
                }
            }
            CallEvent::OutgoingCall { metadata } => {
                if let Some(num) = &metadata.caller_number {
                    write!(f, "Outgoing Call to {}", num)
                } else {
                    write!(f, "Outgoing Call")
                }
            }
            CallEvent::CallConnected { metadata } => {
                if let Some(num) = &metadata.caller_number {
                    write!(f, "Call Connected with {}", num)
                } else {
                    write!(f, "Call Connected")
                }
            }
            CallEvent::CallEnded { duration_secs } => {
                if let Some(d) = duration_secs {
                    write!(f, "Call Ended (duration: {}s)", d)
                } else {
                    write!(f, "Call Ended")
                }
            }
        }
    }
}

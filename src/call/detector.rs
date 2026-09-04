use std::time::Instant;
use tracing::debug;
use crate::call::state::{CallEvent, CallMetadata, CallState};

/// Dedicated Call State Machine to prevent duplicate notifications
/// and accurately detect Call transitions (Incoming, Outgoing, Connected, Ended).
#[derive(Debug)]
pub struct CallStateMachine {
    current_state: CallState,
    previous_state: CallState,
    call_start_time: Option<Instant>,
    active_metadata: CallMetadata,
    is_incoming: bool,
}

impl CallStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: CallState::Idle,
            previous_state: CallState::Idle,
            call_start_time: None,
            active_metadata: CallMetadata::default(),
            is_incoming: false,
        }
    }

    pub fn current_state(&self) -> CallState {
        self.current_state
    }

    /// Feeds a newly polled CallState and optional metadata.
    /// Returns Some(CallEvent) if and only if a meaningful state transition occurs.
    /// Returns None for identical consecutive states (debouncing/deduplication).
    pub fn transition(&mut self, new_state: CallState, metadata: Option<CallMetadata>) -> Option<CallEvent> {
        // Unknown states do not change existing logical call tracking
        if new_state == CallState::Unknown {
            return None;
        }

        let meta = metadata.unwrap_or_default();
        if meta.caller_number.is_some() {
            self.active_metadata = meta.clone();
        }

        let old_state = self.current_state;

        // Debounce identical states
        if old_state == new_state {
            return None;
        }

        debug!("Call state transition: {} -> {}", old_state, new_state);
        self.previous_state = old_state;
        self.current_state = new_state;

        match (old_state, new_state) {
            // IDLE -> RINGING: New incoming call detected
            (CallState::Idle, CallState::Ringing) => {
                self.is_incoming = true;
                self.call_start_time = Some(Instant::now());
                Some(CallEvent::IncomingCall {
                    metadata: self.active_metadata.clone(),
                })
            }

            // IDLE -> OFFHOOK: Outgoing call initiated
            (CallState::Idle, CallState::Offhook) => {
                self.is_incoming = false;
                self.call_start_time = Some(Instant::now());
                Some(CallEvent::OutgoingCall {
                    metadata: self.active_metadata.clone(),
                })
            }

            // RINGING -> OFFHOOK: Incoming call was answered (Call connected)
            (CallState::Ringing, CallState::Offhook) => {
                Some(CallEvent::CallConnected {
                    metadata: self.active_metadata.clone(),
                })
            }

            // OFFHOOK -> IDLE: Call ended normally
            (CallState::Offhook, CallState::Idle) => {
                let duration = self.call_start_time.take().map(|t| t.elapsed().as_secs());
                self.is_incoming = false;
                self.active_metadata = CallMetadata::default();
                Some(CallEvent::CallEnded {
                    duration_secs: duration,
                })
            }

            // RINGING -> IDLE: Call missed or rejected
            (CallState::Ringing, CallState::Idle) => {
                let duration = self.call_start_time.take().map(|t| t.elapsed().as_secs());
                self.is_incoming = false;
                self.active_metadata = CallMetadata::default();
                Some(CallEvent::CallEnded {
                    duration_secs: duration,
                })
            }

            // OFFHOOK -> RINGING: Call waiting or conference transition
            (CallState::Offhook, CallState::Ringing) => {
                Some(CallEvent::IncomingCall {
                    metadata: self.active_metadata.clone(),
                })
            }

            _ => None,
        }
    }

    /// Resets the state machine, e.g. when a device disconnects
    pub fn reset(&mut self) {
        self.current_state = CallState::Idle;
        self.previous_state = CallState::Idle;
        self.call_start_time = None;
        self.active_metadata = CallMetadata::default();
        self.is_incoming = false;
    }
}

impl Default for CallStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

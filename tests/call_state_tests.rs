use wincallnotifier::call::detector::CallStateMachine;
use wincallnotifier::call::state::{CallEvent, CallMetadata, CallState};

#[test]
fn test_call_transitions_incoming() {
    let mut sm = CallStateMachine::new();
    assert_eq!(sm.current_state(), CallState::Idle);

    let meta = CallMetadata {
        caller_number: Some("+1234567890".to_string()),
    };

    // IDLE -> RINGING: Incoming call
    let ev = sm.transition(CallState::Ringing, Some(meta.clone()));
    assert!(matches!(ev, Some(CallEvent::IncomingCall { .. })));
    assert_eq!(sm.current_state(), CallState::Ringing);

    // Repeated RINGING -> RINGING: Must debounce/deduplicate
    let ev_dup1 = sm.transition(CallState::Ringing, Some(meta.clone()));
    assert_eq!(ev_dup1, None);
    let ev_dup2 = sm.transition(CallState::Ringing, Some(meta.clone()));
    assert_eq!(ev_dup2, None);

    // RINGING -> OFFHOOK: Call connected/answered
    let ev_connected = sm.transition(CallState::Offhook, Some(meta.clone()));
    assert!(matches!(ev_connected, Some(CallEvent::CallConnected { .. })));
    assert_eq!(sm.current_state(), CallState::Offhook);

    // Repeated OFFHOOK -> OFFHOOK: Must debounce
    let ev_offhook_dup = sm.transition(CallState::Offhook, None);
    assert_eq!(ev_offhook_dup, None);

    // OFFHOOK -> IDLE: Call ended
    let ev_ended = sm.transition(CallState::Idle, None);
    assert!(matches!(ev_ended, Some(CallEvent::CallEnded { .. })));
    assert_eq!(sm.current_state(), CallState::Idle);

    // Repeated IDLE -> IDLE: Must debounce
    let ev_idle_dup = sm.transition(CallState::Idle, None);
    assert_eq!(ev_idle_dup, None);
}

#[test]
fn test_call_transitions_outgoing() {
    let mut sm = CallStateMachine::new();

    // IDLE -> OFFHOOK: Outgoing call initiated
    let ev_outgoing = sm.transition(CallState::Offhook, None);
    assert!(matches!(ev_outgoing, Some(CallEvent::OutgoingCall { .. })));
    assert_eq!(sm.current_state(), CallState::Offhook);

    // OFFHOOK -> IDLE: Outgoing call ended
    let ev_ended = sm.transition(CallState::Idle, None);
    assert!(matches!(ev_ended, Some(CallEvent::CallEnded { .. })));
    assert_eq!(sm.current_state(), CallState::Idle);
}

#[test]
fn test_call_rejected_or_missed() {
    let mut sm = CallStateMachine::new();

    // IDLE -> RINGING: Incoming
    let ev_incoming = sm.transition(CallState::Ringing, None);
    assert!(matches!(ev_incoming, Some(CallEvent::IncomingCall { .. })));

    // RINGING -> IDLE: Rejected or missed before answering
    let ev_ended = sm.transition(CallState::Idle, None);
    assert!(matches!(ev_ended, Some(CallEvent::CallEnded { .. })));
    assert_eq!(sm.current_state(), CallState::Idle);
}

#[test]
fn test_unknown_state_ignored() {
    let mut sm = CallStateMachine::new();

    sm.transition(CallState::Ringing, None);
    assert_eq!(sm.current_state(), CallState::Ringing);

    let ev_unk = sm.transition(CallState::Unknown, None);
    assert_eq!(ev_unk, None);
    assert_eq!(sm.current_state(), CallState::Ringing);
}

#[test]
fn test_state_machine_reset_on_usb_disconnect() {
    let mut sm = CallStateMachine::new();
    sm.transition(CallState::Offhook, None);
    assert_eq!(sm.current_state(), CallState::Offhook);

    sm.reset();
    assert_eq!(sm.current_state(), CallState::Idle);
}

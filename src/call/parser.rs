use regex::Regex;
use std::sync::OnceLock;
use crate::call::state::{CallMetadata, CallState};

pub trait TelephonyParser: Send + Sync {
    fn parse(&self, raw: &str) -> (CallState, CallMetadata);
}

pub struct AndroidTelephonyParser;

impl AndroidTelephonyParser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidTelephonyParser {
    fn default() -> Self {
        Self::new()
    }
}

static RE_RINGING_CALL_STATE: OnceLock<Regex> = OnceLock::new();
static RE_FOREGROUND_CALL_STATE: OnceLock<Regex> = OnceLock::new();
static RE_CALL_STATE: OnceLock<Regex> = OnceLock::new();
static RE_CALL_STATE_SUB: OnceLock<Regex> = OnceLock::new();
static RE_CALL_STATE_VAL: OnceLock<Regex> = OnceLock::new();
static RE_INCOMING_NUMBER: OnceLock<Regex> = OnceLock::new();
static RE_TELECOM_ACTIVE_CALL: OnceLock<Regex> = OnceLock::new();
static RE_TELECOM_STATE: OnceLock<Regex> = OnceLock::new();

fn get_re_ringing_call_state() -> &'static Regex {
    RE_RINGING_CALL_STATE.get_or_init(|| {
        Regex::new(r"(?i)\bmRingingCallState\s*=\s*([0-9A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_foreground_call_state() -> &'static Regex {
    RE_FOREGROUND_CALL_STATE.get_or_init(|| {
        Regex::new(r"(?i)\bmForegroundCallState\s*=\s*([0-9A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_call_state() -> &'static Regex {
    RE_CALL_STATE.get_or_init(|| {
        Regex::new(r"(?i)\bmCallState\s*=\s*([0-9A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_call_state_sub() -> &'static Regex {
    RE_CALL_STATE_SUB.get_or_init(|| {
        Regex::new(r"(?i)\bmCallState(?:\[\d+\]|\s*\(subId=\d+\))?\s*[:=]\s*([0-9A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_call_state_val() -> &'static Regex {
    RE_CALL_STATE_VAL.get_or_init(|| {
        Regex::new(r"(?i)\bcall_?state\s*[:=]\s*([0-9A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_incoming_number() -> &'static Regex {
    RE_INCOMING_NUMBER.get_or_init(|| {
        Regex::new(r"(?i)\bm?CallIncomingNumber\s*=\s*([+0-9A-Za-z\-_]+)").expect("Invalid regex")
    })
}

fn get_re_telecom_active_call() -> &'static Regex {
    RE_TELECOM_ACTIVE_CALL.get_or_init(|| {
        Regex::new(r"(?i)\[Call\s+id=[^,]+,\s*state=([A-Z_]+)").expect("Invalid regex")
    })
}

fn get_re_telecom_state() -> &'static Regex {
    RE_TELECOM_STATE.get_or_init(|| {
        Regex::new(r"(?i)\bState:\s*([A-Z_]+)").expect("Invalid regex")
    })
}

fn parse_state_token(token: &str) -> CallState {
    let t = token.trim().to_uppercase();
    match t.as_str() {
        "0" | "IDLE" | "CALL_STATE_IDLE" | "DISCONNECTED" => CallState::Idle,
        "1" | "RINGING" | "CALL_STATE_RINGING" => CallState::Ringing,
        "2" | "OFFHOOK" | "CALL_STATE_OFFHOOK" | "ACTIVE" | "DIALING" | "CONNECTING" | "HOLDING" => CallState::Offhook,
        _ => CallState::Unknown,
    }
}

fn is_valid_phone_number(token: &str) -> bool {
    let s = token.trim();
    if s.is_empty() || s.len() < 7 || s.len() > 20 {
        return false;
    }
    let digits_count = s.chars().filter(|c| c.is_ascii_digit()).count();
    digits_count >= 7
}

impl TelephonyParser for AndroidTelephonyParser {
    fn parse(&self, raw: &str) -> (CallState, CallMetadata) {
        let mut metadata = CallMetadata::default();
        let mut best_state = CallState::Unknown;

        // 1. Check precise sub-states first (mRingingCallState > mForegroundCallState)
        for cap in get_re_ringing_call_state().captures_iter(raw) {
            if let Some(matched) = cap.get(1) {
                if matched.as_str().trim() != "0" {
                    best_state = pick_highest_priority_state(best_state, CallState::Ringing);
                }
            }
        }

        for cap in get_re_foreground_call_state().captures_iter(raw) {
            if let Some(matched) = cap.get(1) {
                if matched.as_str().trim() != "0" {
                    best_state = pick_highest_priority_state(best_state, CallState::Offhook);
                }
            }
        }

        // 2. Standard mCallState parser
        if best_state == CallState::Unknown {
            for cap in get_re_call_state().captures_iter(raw) {
                if let Some(matched) = cap.get(1) {
                    let state = parse_state_token(matched.as_str());
                    best_state = pick_highest_priority_state(best_state, state);
                }
            }
        }

        if best_state == CallState::Unknown {
            for cap in get_re_call_state_sub().captures_iter(raw) {
                if let Some(matched) = cap.get(1) {
                    let state = parse_state_token(matched.as_str());
                    best_state = pick_highest_priority_state(best_state, state);
                }
            }
        }

        if best_state == CallState::Unknown {
            for cap in get_re_call_state_val().captures_iter(raw) {
                if let Some(matched) = cap.get(1) {
                    let state = parse_state_token(matched.as_str());
                    best_state = pick_highest_priority_state(best_state, state);
                }
            }
        }

        // 3. Telecom dumpsys fallbacks
        if best_state == CallState::Unknown {
            for cap in get_re_telecom_active_call().captures_iter(raw) {
                if let Some(matched) = cap.get(1) {
                    let state = parse_state_token(matched.as_str());
                    best_state = pick_highest_priority_state(best_state, state);
                }
            }
        }

        if best_state == CallState::Unknown {
            for cap in get_re_telecom_state().captures_iter(raw) {
                if let Some(matched) = cap.get(1) {
                    let state = parse_state_token(matched.as_str());
                    best_state = pick_highest_priority_state(best_state, state);
                }
            }
        }

        // 4. Extract incoming phone number if valid
        if let Some(cap) = get_re_incoming_number().captures(raw) {
            if let Some(matched) = cap.get(1) {
                let token = matched.as_str().trim();
                if is_valid_phone_number(token) {
                    metadata.caller_number = Some(token.to_string());
                }
            }
        }

        (best_state, metadata)
    }
}

fn pick_highest_priority_state(current: CallState, next: CallState) -> CallState {
    match (current, next) {
        (_, CallState::Ringing) => CallState::Ringing,
        (CallState::Ringing, _) => CallState::Ringing,
        (_, CallState::Offhook) => CallState::Offhook,
        (CallState::Offhook, _) => CallState::Offhook,
        (_, CallState::Idle) => CallState::Idle,
        (CallState::Idle, _) => CallState::Idle,
        _ => CallState::Unknown,
    }
}

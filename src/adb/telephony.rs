use std::sync::Arc;
use crate::adb::client::AdbClient;
use crate::call::parser::{AndroidTelephonyParser, TelephonyParser};
use crate::call::state::{CallMetadata, CallState};
use crate::error::Result;

pub struct AdbCallStateProvider {
    adb: Arc<AdbClient>,
    parser: Arc<dyn TelephonyParser>,
}

impl AdbCallStateProvider {
    pub fn new(adb: Arc<AdbClient>) -> Self {
        Self {
            adb,
            parser: Arc::new(AndroidTelephonyParser::new()),
        }
    }

    pub fn with_parser(adb: Arc<AdbClient>, parser: Arc<dyn TelephonyParser>) -> Self {
        Self { adb, parser }
    }

    /// Query Android telephony diagnostic status via ADB (extracts phone number only)
    pub async fn query_call_state(&self, serial: &str) -> Result<(CallState, CallMetadata)> {
        // 1. Fast query dumpsys telephony.registry
        let output = match self.adb.shell_exec(serial, "dumpsys telephony.registry").await {
            Ok(out) => out,
            Err(e) => return Err(e),
        };

        let (mut state, mut metadata) = self.parser.parse(&output);

        // 2. Query telecom to capture active / VoIP calls
        if let Ok(telecom_out) = self.adb.shell_exec(serial, "dumpsys telecom").await {
            let (telecom_state, telecom_meta) = self.parser.parse(&telecom_out);
            if state == CallState::Unknown || state == CallState::Idle {
                if telecom_state != CallState::Unknown && telecom_state != CallState::Idle {
                    state = telecom_state;
                }
            }
            if metadata.caller_number.is_none() {
                metadata.caller_number = telecom_meta.caller_number;
            }
        }

        // 3. When call is ringing or active and phone number is missing, extract the phone number directly
        if (state == CallState::Ringing || state == CallState::Offhook) && metadata.caller_number.is_none() {
            // Strategy A: Check active ongoing notifications from dialer / whatsapp
            if let Ok(notif_keys) = self.adb.shell_exec(serial, "cmd notification list").await {
                for line in notif_keys.lines() {
                    let line = line.trim();
                    if line.contains("MissedCall") || line.contains("missed_call") {
                        continue;
                    }

                    if (line.contains("dialer") || line.contains("whatsapp") || line.contains("w4b") || line.contains("telecom") || line.contains("phone"))
                        && (line.contains("Call") || line.contains("call") || line.contains("Incoming") || line.contains("Ongoing"))
                    {
                        let cmd = format!("cmd notification get '{}'", line);
                        if let Ok(detail) = self.adb.shell_exec(serial, &cmd).await {
                            if let Some(num) = extract_phone_number(&detail) {
                                metadata.caller_number = Some(num);
                                break;
                            }
                        }
                    }
                }
            }

            // Strategy B: Query content://call_log/calls for the most recent phone number
            if metadata.caller_number.is_none() {
                let query_cmd = "content query --uri content://call_log/calls --projection number:date:type --sort 'date DESC'";
                if let Ok(call_log_out) = self.adb.shell_exec(serial, query_cmd).await {
                    if let Some(first_row) = call_log_out.lines().find(|l| l.starts_with("Row: 0")) {
                        for part in first_row.split(',') {
                            let part = part.trim();
                            if let Some(num_str) = part.strip_prefix("number=") {
                                let num = num_str.trim();
                                if !num.is_empty() && num != "null" {
                                    metadata.caller_number = Some(num.to_string());
                                    break;
                                }
                            } else if let Some(first_part) = part.split_once("number=") {
                                let num = first_part.1.trim();
                                if !num.is_empty() && num != "null" {
                                    metadata.caller_number = Some(num.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((state, metadata))
    }
}

/// Helper function to scan a text block for telephone numbers (+XX... or 10-digit numbers)
fn extract_phone_number(text: &str) -> Option<String> {
    for word in text.split(|c: char| !c.is_ascii_digit() && c != '+') {
        let trimmed = word.trim();
        let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 10 && digits.len() <= 15 {
            return Some(trimmed.to_string());
        }
    }
    None
}

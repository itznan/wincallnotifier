use wincallnotifier::adb::client::parse_adb_devices_output;
use wincallnotifier::adb::device::DeviceStatus;
use wincallnotifier::call::parser::{AndroidTelephonyParser, TelephonyParser};
use wincallnotifier::call::state::CallState;

#[test]
fn test_parse_android15_stock_telephony_idle() {
    let raw = r#"
Telephony Registry:
  mCallState=0
  mDataActivity=0
  mDataConnectionState=2
  mVoiceActivationState=0
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, meta) = parser.parse(raw);
    assert_eq!(state, CallState::Idle);
    assert_eq!(meta.caller_number, None);
}

#[test]
fn test_parse_android15_stock_telephony_ringing() {
    let raw = r#"
Telephony Registry:
  mCallState=1
  mCallIncomingNumber=+1987654321
  mDataActivity=0
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, meta) = parser.parse(raw);
    assert_eq!(state, CallState::Ringing);
    assert_eq!(meta.caller_number, Some("+1987654321".to_string()));
}

#[test]
fn test_parse_android15_stock_telephony_offhook() {
    let raw = r#"
Telephony Registry:
  mCallState=2
  mDataActivity=1
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, _meta) = parser.parse(raw);
    assert_eq!(state, CallState::Offhook);
}

#[test]
fn test_parse_samsung_oneui_telephony() {
    let raw = r#"
Telephony Registry:
  mCallState=CALL_STATE_RINGING
  mCallIncomingNumber=01012345678
  mServiceState=0
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, meta) = parser.parse(raw);
    assert_eq!(state, CallState::Ringing);
    assert_eq!(meta.caller_number, Some("01012345678".to_string()));
}

#[test]
fn test_parse_multisim_telephony() {
    // Multi-SIM: SIM 1 is idle, SIM 2 is ringing
    let raw = r#"
Telephony Registry:
  mCallState (subId=1) = 0
  mCallState (subId=2) = 1
  mCallIncomingNumber=+447123456789
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, meta) = parser.parse(raw);
    assert_eq!(state, CallState::Ringing);
    assert_eq!(meta.caller_number, Some("+447123456789".to_string()));
}

#[test]
fn test_parse_telecom_dumpsys_fallback() {
    let raw = r#"
Telecom Services:
  Call 1: [State: ACTIVE, id: 101, handles: tel:5551234]
"#;
    let parser = AndroidTelephonyParser::new();
    let (state, _) = parser.parse(raw);
    assert_eq!(state, CallState::Offhook);
}

#[test]
fn test_parse_adb_devices() {
    let output = r#"
List of devices attached
9A281FFAZ003TR         device product:shiba model:Pixel_8 device:shiba transport_id:1
RFCW102V98E            unauthorized transport_id:2
192.168.1.50:5555      device product:husky model:Pixel_8_Pro device:husky transport_id:3
"#;
    let devices = parse_adb_devices_output(output);
    assert_eq!(devices.len(), 3);

    // Device 1: USB authorized Pixel 8
    assert_eq!(devices[0].serial, "9A281FFAZ003TR");
    assert_eq!(devices[0].status, DeviceStatus::Device);
    assert_eq!(devices[0].model, Some("Pixel 8".to_string()));
    assert_eq!(devices[0].usb_connection, true);

    // Device 2: Unauthorized USB
    assert_eq!(devices[1].serial, "RFCW102V98E");
    assert_eq!(devices[1].status, DeviceStatus::Unauthorized);
    assert_eq!(devices[1].usb_connection, true);

    // Device 3: TCP/IP connected
    assert_eq!(devices[2].serial, "192.168.1.50:5555");
    assert_eq!(devices[2].status, DeviceStatus::Device);
    assert_eq!(devices[2].usb_connection, false);
}

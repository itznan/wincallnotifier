# WinCallNotifier: Android 15 Windows Call Monitor

A lightweight, robust, native Windows desktop and system-tray application written entirely in **Rust** that monitors Android 15 phone call states via **USB + ADB** and delivers native Windows 10/11 toast notifications.

---

## Key Highlights & Constraints Adherence

- **Windows Side Only (Rust)**: Written 100% in Rust (`tokio`, `winrt-notification`, `tray-icon`, `windows-sys`).
- **No Android App**: Does **NOT** require any Android APK, Kotlin/Java code, background service, or root access on the phone.
- **Pure USB + ADB Transport**: Uses ADB shell commands over USB; operates independently of USB tethering or network IP configurations.
- **Zero Cloud / Local & Private**: All state detection, logging, and notifications run entirely on your local machine. No data ever leaves your computer.
- **Ultra-Low Battery & CPU Impact**: Asynchronous event-driven polling (default: 750 ms) without keeping the phone display awake or blocking Windows UI threads.
- **Resilient & Fault-Tolerant**: Recovers cleanly from USB cable disconnects, device reboots, ADB restarts, and unauthorized debugging states without crashing.

---

## 1. Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                 Android 15 Device                           │
│  (Telephony Registry / Telecom Framework / Diagnostics)     │
└──────────────────────────────┬──────────────────────────────┘
                               │  USB Connection (ADB)
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                     adb.exe server                          │
└──────────────────────────────┬──────────────────────────────┘
                               │  Async I/O / Process Spawner
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                    WinCallNotifier                          │
│                                                             │
│   ┌─────────────────────┐       ┌───────────────────────┐   │
│   │   AdbDeviceManager  │──────▶│ AdbCallStateProvider  │   │
│   └─────────────────────┘       └───────────┬───────────┘   │
│                                             │               │
│                                             ▼               │
│   ┌─────────────────────┐       ┌───────────────────────┐   │
│   │ CallStateMachine    │◀──────│ TelephonyParser       │   │
│   │ (Debounce & Filter) │       │ (Multi-vendor Parser) │   │
│   └──────────┬──────────┘       └───────────────────────┘   │
│              │                                              │
│              ▼                                              │
│   ┌─────────────────────┐       ┌───────────────────────┐   │
│   │ NotificationManager │       │  System Tray & UI     │   │
│   │ (Windows Toasts)    │       │  (Status & Settings)  │   │
│   └─────────────────────┘       └───────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Requirements

- **Operating System**: Windows 10 or Windows 11 (64-bit).
- **Phone**: Android 15 (also backwards-compatible with Android 10-14, including Samsung One UI, Google Pixel, Xiaomi HyperOS, Motorola, OnePlus).
- **Android Platform Tools**: `adb.exe` (part of the Android SDK Platform-Tools).
- **Hardware**: USB data cable connected from PC to phone.

---

## 3. Setup Guide

### Step 1: Install Android Platform Tools (`adb.exe`)
If you do not have ADB installed on your Windows machine:
1. Download [Android SDK Platform-Tools for Windows](https://developer.android.com/tools/releases/platform-tools).
2. Extract the ZIP to `C:\platform-tools` or any directory of your choice.
3. (Optional) Add `C:\platform-tools` to your system `PATH` environment variable.

### Step 2: Enable USB Debugging on Android 15
1. Open phone **Settings** → **About Phone**.
2. Tap **Build Number** 7 times until you see *"You are now a developer!"*.
3. Go to **Settings** → **System** → **Developer Options**.
4. Enable **USB Debugging**.

### Step 3: Connect and Authorize
1. Connect the phone to your Windows PC using a USB data cable.
2. Set USB mode to **File Transfer** or **No Data Transfer** (both work with ADB). USB Tethering can also be active.
3. On your phone, a prompt will appear: *"Allow USB debugging?"*.
4. Check **"Always allow from this computer"** and tap **Allow**.

---

## 4. Building & Running

### Build from Source
Ensure Rust 1.75+ is installed:
```powershell
# Check dependencies and syntax
cargo check

# Run automated tests
cargo test

# Build optimized release binary
cargo build --release
```

The compiled binary will be generated at:
```text
target\release\wincallnotifier.exe
```

### Run the Application
```powershell
cargo run --release
```
Or double-click `wincallnotifier.exe`.

---

## 5. System Tray & Status UI

When started, `WinCallNotifier` minimizes to the Windows System Tray:

### System Tray Menu Options
- **Phone Call Monitor (Header)**
- **● Connected (Device Status & Call State)**
- **Device: [Model Name]**
- `Enable Monitoring`: Checkbox to toggle polling on/off instantly.
- `Test Notification`: Triggers an immediate test Windows toast notification.
- `Show Status Window`: Opens the live diagnostics window.
- `Restart ADB`: Safely kills and restarts the ADB server if in an inconsistent state.
- `About`: Displays application and version information.
- `Exit`: Cleanly stops background services and terminates the application.

### Status Window
Opening **Show Status Window** displays real-time connection diagnostics:
- **ADB Connection**: Status of ADB server connection.
- **Device**: Model, Manufacturer, Android Version, Serial, and Transport (`USB`).
- **Monitoring**: Active status and current normalized call state (`IDLE`, `RINGING`, `OFFHOOK`).
- **Last Event**: Timestamp and description of the most recent call or device event.

---

## 6. Call Detection Engine

### Normalized States
Android diagnostics are parsed into a normalized state machine:
- `IDLE`: Phone has no active or incoming calls.
- `RINGING`: Incoming call is actively ringing.
- `OFFHOOK`: Call is active, dialing, or connected.
- `UNKNOWN`: Diagnostic information unavailable or indeterminate.

### Supported Call Transitions & Toasts
| Transition | Derived Event | Toast Notification |
|---|---|---|
| `IDLE` → `RINGING` | **Incoming Call** | 📞 Incoming Call (Caller info if available) |
| `IDLE` → `OFFHOOK` | **Outgoing Call** | 📞 Outgoing Call (Call started) |
| `RINGING` → `OFFHOOK` | **Call Connected** | 📞 Call Connected (The call is now active) |
| `OFFHOOK` → `IDLE` | **Call Ended** | 📞 Call Ended (Displays call duration in seconds) |
| `RINGING` → `IDLE` | **Call Ended (Rejected/Missed)** | 📞 Call Ended |

### Deduplication & Debounce Guarantee
Repeated diagnostic reads (e.g. `RINGING` → `RINGING` during a 30-second ring) are filtered by `CallStateMachine`. Exactly **one** notification is emitted for each state change.

---

## 7. Configuration & Settings

Configuration is automatically persisted to:
```text
%APPDATA%\wincallnotifier\WinCallNotifier\config.json
```

### Default Settings
```json
{
  "adb_path": null,
  "target_device_serial": null,
  "polling_interval_ms": 750,
  "enable_monitoring": true,
  "notify_incoming": true,
  "notify_outgoing": true,
  "notify_connected": true,
  "notify_ended": true,
  "notification_sound": true,
  "log_level": "info",
  "start_with_windows": false
}
```

- `adb_path`: Custom path to `adb.exe` if not located in PATH or standard Android SDK directories.
- `target_device_serial`: Optional serial number to bind to if multiple devices are attached.
- `polling_interval_ms`: Polling frequency in milliseconds (default: `750`, recommended range `500` to `2000`).

---

## 8. Battery & Performance Considerations

- **No Screen Wakeup**: Polling occurs over the USB ADB debugging bridge without waking up the phone display.
- **Low Overhead**: Each poll executes a targeted diagnostic read (`dumpsys telephony.registry`).
- **Low Windows Resource Usage**: Release build binary is ~3 MB, consumes < 15 MB RAM, and near 0% CPU.
- **Power Suspension Safe**: If the USB cable is unplugged, the polling loop enters a dormant discovery cycle and resets the call state machine.

---

## 9. Known Android 15 & ADB Limitations

1. **Caller ID Availability via ADB**:
   - In modern Android versions (including Android 15), Google restricts caller phone numbers in unprivileged diagnostic registries (`dumpsys telephony.registry`) for privacy reasons unless `READ_CALL_LOG` / `READ_PHONE_STATE` permissions are held by a system/carrier app.
   - When caller information (`mCallIncomingNumber`) is populated by OEM ROMs (e.g. certain Samsung, Motorola, or carrier builds), `WinCallNotifier` parses and displays it.
   - When redacted by Android 15 security, `WinCallNotifier` gracefully falls back to:
     ```text
     📞 Incoming Call
     Your phone is ringing
     ```
   - **Privacy Policy Guarantee**: No exploits, no root elevation, and no non-standard APIs are used.

2. **USB Tethering Coexistence**:
   - USB Tethering creates an RNDIS/NCM network adapter between Windows and Android. ADB communicates over the separate USB bulk endpoints (`transport_id`). Both coexist without conflicts.

---

## 10. Verification & Test Results

Run all automated unit and integration tests:
```powershell
cargo test
```

### Test Coverage
- `tests\call_state_tests.rs`:
  - `test_call_transitions_incoming`: Verifies `IDLE` → `RINGING` → `OFFHOOK` → `IDLE` sequence and duration calculation.
  - `test_call_transitions_outgoing`: Verifies `IDLE` → `OFFHOOK` → `IDLE`.
  - `test_call_rejected_or_missed`: Verifies rejected calls.
  - `test_state_machine_reset_on_usb_disconnect`: Verifies state reset when device disconnects.
  - `test_unknown_state_ignored`: Verifies parser resilience.
- `tests\parser_tests.rs`:
  - `test_parse_android15_stock_telephony_idle`: Tests Android 15 stock format.
  - `test_parse_android15_stock_telephony_ringing`: Tests Android 15 incoming call capture.
  - `test_parse_android15_stock_telephony_offhook`: Tests Android 15 active call capture.
  - `test_parse_samsung_oneui_telephony`: Tests Samsung One UI token variants.
  - `test_parse_multisim_telephony`: Tests multi-SIM device priority handling.
  - `test_parse_telecom_dumpsys_fallback`: Tests fallback dumpsys parser.
  - `test_parse_adb_devices`: Tests device discovery parser (`adb devices -l`).

**Result**: 12 passed; 0 failed.

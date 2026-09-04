use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint,
    InvalidateRect, SelectObject, SetBkMode, SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use crate::app::AppStatus;

static WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_STATUS: Option<Arc<RwLock<AppStatus>>> = None;
static mut CURRENT_HWND: HWND = 0;

pub struct StatusWindow;

impl StatusWindow {
    pub fn show(status: Arc<RwLock<AppStatus>>) {
        unsafe {
            if CURRENT_HWND != 0 && IsWindow(CURRENT_HWND) != 0 {
                // If already created, restore and bring to front
                ShowWindow(CURRENT_HWND, SW_RESTORE);
                SetForegroundWindow(CURRENT_HWND);
                return;
            }
        }

        if WINDOW_OPEN.swap(true, Ordering::SeqCst) {
            return;
        }

        unsafe {
            GLOBAL_STATUS = Some(status);
        }

        std::thread::spawn(move || {
            unsafe {
                let class_name = encode_wide("WinCallNotifierStatusClass");
                let window_title = encode_wide("WinCallNotifier - Status & Settings");

                let h_instance = GetModuleHandleW(std::ptr::null());

                let wnd_class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: h_instance,
                    hIcon: LoadIconW(0, IDI_APPLICATION),
                    hCursor: LoadCursorW(0, IDC_ARROW),
                    hbrBackground: CreateSolidBrush(0x00F8F8F8),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: LoadIconW(0, IDI_APPLICATION),
                };

                RegisterClassExW(&wnd_class);

                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    window_title.as_ptr(),
                    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    480,
                    440,
                    0,
                    0,
                    h_instance,
                    std::ptr::null(),
                );

                if hwnd == 0 {
                    WINDOW_OPEN.store(false, Ordering::SeqCst);
                    return;
                }

                CURRENT_HWND = hwnd;

                // Setup timer to repaint window every 500ms
                SetTimer(hwnd, 1, 500, None);

                ShowWindow(hwnd, SW_SHOW);

                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                CURRENT_HWND = 0;
                WINDOW_OPEN.store(false, Ordering::SeqCst);
            }
        });
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_SYSCOMMAND => {
            // When user clicks the Minimize button (_), hide the window completely so it recedes into the system tray
            if (wparam & 0xFFF0) == SC_MINIMIZE as usize {
                ShowWindow(hwnd, SW_HIDE);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            // When user clicks the Close button (X), hide to system tray instead of exiting the monitor
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Fetch current status snapshot
            let (status_text_lines, error_text) = if let Some(ref st_lock) = GLOBAL_STATUS {
                if let Ok(st) = st_lock.try_read() {
                    let mut lines = Vec::new();
                    lines.push("PHONE CALL MONITOR (ANDROID 15)".to_string());
                    lines.push("──────────────────────────────────────────".to_string());
                    lines.push("Connection:".to_string());
                    if st.adb_connected {
                        lines.push("  ● ADB: Connected".to_string());
                    } else {
                        lines.push("  ○ ADB: Waiting for connection...".to_string());
                    }

                    if let Some(ref dev) = st.device_info {
                        lines.push(format!("  Device: {}", dev.display_name()));
                        lines.push(format!("  Android: {}", dev.android_version.as_deref().unwrap_or("15")));
                        lines.push(format!("  Serial: {}", dev.serial));
                        lines.push(format!("  Status: {}", dev.status));
                        lines.push(format!("  Transport: {}", if dev.usb_connection { "USB" } else { "TCP/IP" }));
                    } else {
                        lines.push("  Device: None detected (connect USB cable)".to_string());
                    }

                    lines.push("".to_string());
                    lines.push("Monitoring:".to_string());
                    lines.push(format!("  Status: {}", if st.monitoring_active { "Active" } else { "Paused" }));
                    lines.push(format!("  Call State: {}", st.current_call_state));

                    lines.push("".to_string());
                    lines.push("Last Event:".to_string());
                    lines.push(format!("  {} - {}", st.last_event_time, st.last_event));

                    (lines, st.error_message.clone())
                } else {
                    (vec!["Loading status...".to_string()], None)
                }
            } else {
                (vec!["Initializing...".to_string()], None)
            };

            SetBkMode(hdc, TRANSPARENT as i32);

            let font = CreateFontW(
                18, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0,
                encode_wide("Segoe UI").as_ptr(),
            );
            let bold_font = CreateFontW(
                20, 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 0, 0,
                encode_wide("Segoe UI").as_ptr(),
            );

            let old_font = SelectObject(hdc, font);

            let mut y = 20;
            for (idx, line) in status_text_lines.iter().enumerate() {
                if idx == 0 {
                    SelectObject(hdc, bold_font);
                    SetTextColor(hdc, 0x00803000); // Navy Blue
                } else if line.starts_with("Connection:") || line.starts_with("Monitoring:") || line.starts_with("Last Event:") {
                    SelectObject(hdc, bold_font);
                    SetTextColor(hdc, 0x00333333); // Dark Gray
                } else {
                    SelectObject(hdc, font);
                    if line.contains("●") {
                        SetTextColor(hdc, 0x00008800); // Green
                    } else {
                        SetTextColor(hdc, 0x00222222);
                    }
                }

                let wide_str = encode_wide(line);
                TextOutW(hdc, 24, y, wide_str.as_ptr(), (wide_str.len() - 1) as i32);
                y += 24;
            }

            if let Some(err) = error_text {
                SelectObject(hdc, font);
                SetTextColor(hdc, 0x000000CC); // Red
                let wide_err = encode_wide(&format!("Notice: {err}"));
                TextOutW(hdc, 24, y + 10, wide_err.as_ptr(), (wide_err.len() - 1) as i32);
            }

            SelectObject(hdc, old_font);
            DeleteObject(font);
            DeleteObject(bold_font);

            EndPaint(hwnd, &ps);
            0
        }
        WM_TIMER => {
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            WINDOW_OPEN.store(false, Ordering::SeqCst);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

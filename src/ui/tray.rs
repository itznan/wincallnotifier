use std::sync::Arc;
use tokio::sync::RwLock;
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};
use crate::app::AppStatus;

pub struct TrayMenuIds {
    pub toggle_monitoring: MenuId,
    pub test_notification: MenuId,
    pub show_status: MenuId,
    pub restart_adb: MenuId,
    pub about: MenuId,
    pub exit: MenuId,
}

pub struct SystemTrayManager {
    pub tray_icon: TrayIcon,
    pub menu_ids: TrayMenuIds,
    status: Arc<RwLock<AppStatus>>,
    menu_enable_monitoring: CheckMenuItem,
    status_item: MenuItem,
    device_item: MenuItem,
}

impl SystemTrayManager {
    pub fn new(
        status: Arc<RwLock<AppStatus>>,
        initial_monitoring: bool,
    ) -> anyhow::Result<Self> {
        let menu = Menu::new();

        let title_item = MenuItem::new("Phone Call Monitor", false, None);
        let status_item = MenuItem::new("● Initializing...", false, None);
        let device_item = MenuItem::new("Device: Waiting...", false, None);

        let menu_enable_monitoring = CheckMenuItem::new("Enable Monitoring", true, initial_monitoring, None);
        let test_notification = MenuItem::new("Test Notification", true, None);
        let show_status = MenuItem::new("Show Status Window", true, None);
        let restart_adb = MenuItem::new("Restart ADB", true, None);
        let about_item = MenuItem::new("About", true, None);
        let exit_item = MenuItem::new("Exit", true, None);

        let menu_ids = TrayMenuIds {
            toggle_monitoring: menu_enable_monitoring.id().clone(),
            test_notification: test_notification.id().clone(),
            show_status: show_status.id().clone(),
            restart_adb: restart_adb.id().clone(),
            about: about_item.id().clone(),
            exit: exit_item.id().clone(),
        };

        menu.append(&title_item)?;
        menu.append(&status_item)?;
        menu.append(&device_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&menu_enable_monitoring)?;
        menu.append(&test_notification)?;
        menu.append(&show_status)?;
        menu.append(&restart_adb)?;
        menu.append(&about_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&exit_item)?;

        let icon = create_default_icon();

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("WinCallNotifier - Android 15 Call Monitor")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            tray_icon,
            menu_ids,
            status,
            menu_enable_monitoring,
            status_item,
            device_item,
        })
    }

    pub fn is_monitoring_checked(&self) -> bool {
        self.menu_enable_monitoring.is_checked()
    }

    pub fn set_monitoring_checked(&self, checked: bool) {
        self.menu_enable_monitoring.set_checked(checked);
    }

    /// Update tray menu items reflecting latest connection/device/call state
    pub async fn update_status(&self) {
        let st = self.status.read().await;
        let conn_text = if st.adb_connected {
            if let Some(dev) = &st.device_info {
                format!("● Connected ({}) - Call: {}", dev.status, st.current_call_state)
            } else {
                "● ADB Connected (Waiting for phone...)".to_string()
            }
        } else {
            "○ Waiting for ADB...".to_string()
        };

        self.status_item.set_text(conn_text);

        let dev_text = if let Some(dev) = &st.device_info {
            format!("Device: {}", dev.display_name())
        } else {
            "Device: None".to_string()
        };

        self.device_item.set_text(dev_text);
    }
}

/// Creates a 16x16 RGBA phone icon programmatically for the system tray
fn create_default_icon() -> Icon {
    let width = 16u32;
    let height = 16u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let is_phone_shape = (x >= 4 && x <= 11 && (y == 2 || y == 13))
                || ((x == 4 || x == 5 || x == 10 || x == 11) && y >= 3 && y <= 12)
                || (x >= 6 && x <= 9 && (y == 3 || y == 12));

            if is_phone_shape {
                rgba.extend_from_slice(&[30, 200, 100, 255]); // Green
            } else if x >= 2 && x <= 13 && y >= 2 && y <= 13 {
                rgba.extend_from_slice(&[20, 30, 50, 220]); // Navy
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]); // Transparent
            }
        }
    }

    Icon::from_rgba(rgba, width, height).expect("Failed to create tray icon")
}

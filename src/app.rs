use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use crate::adb::device::{DeviceInfo, DeviceStatus};
use crate::adb::manager::AdbDeviceManager;
use crate::adb::telephony::AdbCallStateProvider;
use crate::call::detector::CallStateMachine;
use crate::call::state::CallState;
use crate::config::AppConfig;
use crate::notifications::NotificationManager;

/// Real-time application monitoring status snapshot
#[derive(Debug, Clone)]
pub struct AppStatus {
    pub adb_connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub monitoring_active: bool,
    pub current_call_state: CallState,
    pub last_event: String,
    pub last_event_time: String,
    pub error_message: Option<String>,
}

impl Default for AppStatus {
    fn default() -> Self {
        Self {
            adb_connected: false,
            device_info: None,
            monitoring_active: false,
            current_call_state: CallState::Idle,
            last_event: "Application initialized".to_string(),
            last_event_time: chrono::Local::now().format("%H:%M:%S").to_string(),
            error_message: None,
        }
    }
}

pub enum AppCommand {
    ToggleMonitoring(bool),
    SetPollingInterval(u64),
    RestartAdb,
    TestNotification,
    UpdateConfig(AppConfig),
    Quit,
}

pub struct AppService {
    config: Arc<RwLock<AppConfig>>,
    status: Arc<RwLock<AppStatus>>,
    cmd_tx: mpsc::Sender<AppCommand>,
    cmd_rx: mpsc::Receiver<AppCommand>,
    notifier: NotificationManager,
}

impl AppService {
    pub fn new(initial_config: AppConfig) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        Self {
            config: Arc::new(RwLock::new(initial_config)),
            status: Arc::new(RwLock::new(AppStatus::default())),
            cmd_tx,
            cmd_rx,
            notifier: NotificationManager::new(),
        }
    }

    pub fn sender(&self) -> mpsc::Sender<AppCommand> {
        self.cmd_tx.clone()
    }

    pub fn status(&self) -> Arc<RwLock<AppStatus>> {
        self.status.clone()
    }

    pub fn config(&self) -> Arc<RwLock<AppConfig>> {
        self.config.clone()
    }

    /// Run the main background monitoring loop
    pub async fn run(mut self) {
        info!("Starting background call monitoring service...");

        let current_cfg = self.config.read().await.clone();
        let mut adb_client = match crate::adb::AdbClient::find_adb(current_cfg.adb_path.as_deref()) {
            Ok(c) => Some(c),
            Err(e) => {
                warn!("Initial ADB discovery: {}. Will retry...", e);
                let mut st = self.status.write().await;
                st.error_message = Some(format!("ADB not found: {e}"));
                None
            }
        };

        let mut device_manager = adb_client.clone().map(|c| {
            AdbDeviceManager::new(c, current_cfg.target_device_serial.clone())
        });

        let mut telephony_provider = adb_client.as_ref().map(|c| {
            Arc::new(AdbCallStateProvider::new(Arc::new(c.clone())))
        });

        let mut call_sm = CallStateMachine::new();
        let mut last_device_status: Option<DeviceStatus> = None;

        loop {
            // Check for incoming UI commands with timeout matching polling interval
            let poll_duration = {
                let cfg = self.config.read().await;
                Duration::from_millis(cfg.polling_interval_ms.max(250))
            };

            tokio::select! {
                Some(cmd) = self.cmd_rx.recv() => {
                    match cmd {
                        AppCommand::ToggleMonitoring(enable) => {
                            let mut cfg = self.config.write().await;
                            cfg.enable_monitoring = enable;
                            let _ = cfg.save();
                            let mut st = self.status.write().await;
                            st.monitoring_active = enable;
                            info!("Call monitoring toggled: {}", enable);
                        }
                        AppCommand::SetPollingInterval(ms) => {
                            let mut cfg = self.config.write().await;
                            cfg.polling_interval_ms = ms;
                            let _ = cfg.save();
                            info!("Polling interval updated to {} ms", ms);
                        }
                        AppCommand::RestartAdb => {
                            info!("Restarting ADB requested by user...");
                            if let Some(c) = &adb_client {
                                let _ = c.restart_server().await;
                            }
                        }
                        AppCommand::TestNotification => {
                            info!("Sending test notification...");
                            let _ = self.notifier.send_toast("📞 Test Notification", "WinCallNotifier is working properly!", true);
                        }
                        AppCommand::UpdateConfig(new_cfg) => {
                            *self.config.write().await = new_cfg.clone();
                            let _ = new_cfg.save();
                            // Re-init adb client if path changed
                            if let Ok(new_client) = crate::adb::AdbClient::find_adb(new_cfg.adb_path.as_deref()) {
                                adb_client = Some(new_client.clone());
                                device_manager = Some(AdbDeviceManager::new(new_client.clone(), new_cfg.target_device_serial.clone()));
                                telephony_provider = Some(Arc::new(AdbCallStateProvider::new(Arc::new(new_client))));
                            }
                        }
                        AppCommand::Quit => {
                            info!("Quit command received in service loop.");
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(poll_duration) => {
                    // Periodic poll cycle
                    let monitoring_enabled = {
                        let cfg = self.config.read().await;
                        cfg.enable_monitoring
                    };

                    // 1. Ensure ADB client is initialized
                    if adb_client.is_none() {
                        let custom_path = self.config.read().await.adb_path.clone();
                        match crate::adb::AdbClient::find_adb(custom_path.as_deref()) {
                            Ok(c) => {
                                info!("ADB found: {}", c.executable_path().display());
                                let _ = c.start_server().await;
                                device_manager = Some(AdbDeviceManager::new(c.clone(), None));
                                telephony_provider = Some(Arc::new(AdbCallStateProvider::new(Arc::new(c.clone()))));
                                adb_client = Some(c);
                                let mut st = self.status.write().await;
                                st.error_message = None;
                            }
                            Err(e) => {
                                let mut st = self.status.write().await;
                                st.adb_connected = false;
                                st.error_message = Some(format!("ADB not found: {e}"));
                                continue;
                            }
                        }
                    }

                    let mgr = match &device_manager {
                        Some(m) => m,
                        None => continue,
                    };

                    // 2. Discover/refresh device
                    let device_opt = match mgr.refresh_device().await {
                        Ok(dev) => dev,
                        Err(e) => {
                            debug!("Error refreshing devices: {}", e);
                            None
                        }
                    };

                    {
                        let mut st = self.status.write().await;
                        st.adb_connected = true;
                        st.device_info = device_opt.clone();
                        st.monitoring_active = monitoring_enabled;
                    }

                    let device = match device_opt {
                        Some(d) => d,
                        None => {
                            // No device connected
                            if last_device_status.is_some() {
                                info!("Device detached. Resetting call state machine.");
                                call_sm.reset();
                                last_device_status = None;
                                let mut st = self.status.write().await;
                                st.current_call_state = CallState::Idle;
                                st.last_event = "Device disconnected".to_string();
                                st.last_event_time = chrono::Local::now().format("%H:%M:%S").to_string();
                            }
                            continue;
                        }
                    };

                    // Handle unauthorized device
                    if device.status == DeviceStatus::Unauthorized {
                        if last_device_status != Some(DeviceStatus::Unauthorized) {
                            warn!("Device unauthorized: {}", device.serial);
                            let _ = self.notifier.send_toast(
                                "⚠️ Device Unauthorized",
                                "Please unlock your phone and accept the USB debugging authorization prompt.",
                                true,
                            );
                            last_device_status = Some(DeviceStatus::Unauthorized);
                        }
                        let mut st = self.status.write().await;
                        st.error_message = Some("Device unauthorized. Check phone screen.".to_string());
                        continue;
                    }

                    if last_device_status == Some(DeviceStatus::Unauthorized) {
                        info!("Device is now authorized: {}", device.serial);
                    }
                    last_device_status = Some(device.status.clone());

                    if !monitoring_enabled {
                        continue;
                    }

                    // 3. Poll Telephony Call State
                    if let Some(provider) = &telephony_provider {
                        match provider.query_call_state(&device.serial).await {
                            Ok((new_state, metadata)) => {
                                {
                                    let mut st = self.status.write().await;
                                    st.error_message = None;
                                    if new_state != CallState::Unknown {
                                        st.current_call_state = new_state;
                                    }
                                }

                                if let Some(event) = call_sm.transition(new_state, Some(metadata)) {
                                    info!("Call event triggered: {:?}", event);
                                    let cfg_snap = self.config.read().await.clone();
                                    if let Err(e) = self.notifier.show_call_event(&event, &cfg_snap) {
                                        error!("Error showing notification: {}", e);
                                    }

                                    let mut st = self.status.write().await;
                                    st.last_event = format!("{}", event);
                                    st.last_event_time = chrono::Local::now().format("%H:%M:%S").to_string();
                                }
                            }
                            Err(e) => {
                                warn!("Telephony query error: {}", e);
                                let mut st = self.status.write().await;
                                st.error_message = Some(format!("Telephony error: {e}"));
                            }
                        }
                    }
                }
            }
        }

        info!("Call monitoring service stopped.");
    }
}

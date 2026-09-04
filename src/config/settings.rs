use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use tracing::{info, warn};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Custom path to adb executable. If empty or None, searches PATH and standard SDK locations.
    pub adb_path: Option<String>,

    /// Specific device serial to connect to (if multiple connected).
    pub target_device_serial: Option<String>,

    /// Polling interval in milliseconds (default: 750ms).
    pub polling_interval_ms: u64,

    /// Whether call state monitoring is actively running.
    pub enable_monitoring: bool,

    /// Notify on incoming call (RINGING).
    pub notify_incoming: bool,

    /// Notify on outgoing call (OFFHOOK directly from IDLE).
    pub notify_outgoing: bool,

    /// Notify on call connected (OFFHOOK after RINGING).
    pub notify_connected: bool,

    /// Notify on call ended (IDLE after OFFHOOK/RINGING).
    pub notify_ended: bool,

    /// Play sound with Windows toast notifications.
    pub notification_sound: bool,

    /// Log level filter (e.g. "info", "debug", "warn", "trace").
    pub log_level: String,

    /// Launch app automatically on Windows startup.
    pub start_with_windows: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            adb_path: None,
            target_device_serial: None,
            polling_interval_ms: 750,
            enable_monitoring: true,
            notify_incoming: true,
            notify_outgoing: true,
            notify_connected: true,
            notify_ended: true,
            notification_sound: true,
            log_level: "info".to_string(),
            start_with_windows: false,
        }
    }
}

impl AppConfig {
    pub fn config_file_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "wincallnotifier", "WinCallNotifier")
            .map(|dirs| dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_file_path() {
            if path.exists() {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                        Ok(config) => {
                            info!("Loaded configuration from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            warn!("Failed to deserialize config file {}: {}. Using defaults.", path.display(), e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read config file {}: {}. Using defaults.", path.display(), e);
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::config_file_path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::ConfigError(format!("Failed to create config dir: {e}")))?;
            }
            let data = serde_json::to_string_pretty(self)
                .map_err(|e| AppError::ConfigError(format!("Failed to serialize config: {e}")))?;
            std::fs::write(&path, data)
                .map_err(|e| AppError::ConfigError(format!("Failed to write config file: {e}")))?;
            info!("Saved configuration to {}", path.display());
            Ok(())
        } else {
            Err(AppError::ConfigError("Failed to locate application directory".to_string()))
        }
    }
}

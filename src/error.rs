use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("ADB executable not found. Searched PATH and standard SDK locations: {0}")]
    AdbNotFound(String),

    #[error("ADB command failed: {0}")]
    AdbCommandFailed(String),

    #[error("ADB server error: {0}")]
    AdbServer(String),

    #[error("ADB device unauthorized. Please unlock your phone and accept USB debugging prompt.")]
    DeviceUnauthorized,

    #[error("No Android device connected over USB/ADB")]
    NoDeviceConnected,

    #[error("Multiple Android devices connected. Please specify target device serial.")]
    MultipleDevices(Vec<String>),

    #[error("Device disconnected: {0}")]
    DeviceDisconnected(String),

    #[error("Telephony dumpsys error or data unavailable: {0}")]
    TelephonyUnavailable(String),

    #[error("Failed to parse call state: {0}")]
    ParseError(String),

    #[error("Windows notification error: {0}")]
    NotificationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

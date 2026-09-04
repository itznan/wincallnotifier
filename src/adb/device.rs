use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    Device,
    Unauthorized,
    Offline,
    NoPermissions,
    Authorizing,
    Unknown(String),
}

impl From<&str> for DeviceStatus {
    fn from(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "device" => DeviceStatus::Device,
            "unauthorized" => DeviceStatus::Unauthorized,
            "offline" => DeviceStatus::Offline,
            "no permissions" => DeviceStatus::NoPermissions,
            "authorizing" => DeviceStatus::Authorizing,
            other => DeviceStatus::Unknown(other.to_string()),
        }
    }
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceStatus::Device => write!(f, "Authorized"),
            DeviceStatus::Unauthorized => write!(f, "Unauthorized"),
            DeviceStatus::Offline => write!(f, "Offline"),
            DeviceStatus::NoPermissions => write!(f, "No Permissions"),
            DeviceStatus::Authorizing => write!(f, "Authorizing"),
            DeviceStatus::Unknown(s) => write!(f, "Unknown ({})", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: String,
    pub status: DeviceStatus,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    pub android_version: Option<String>,
    pub sdk_level: Option<u32>,
    pub usb_connection: bool,
}

impl DeviceInfo {
    pub fn new(serial: String, status: DeviceStatus) -> Self {
        let is_usb = !serial.contains(':'); // IP connections typically have IP:Port
        Self {
            serial,
            status,
            model: None,
            manufacturer: None,
            android_version: None,
            sdk_level: None,
            usb_connection: is_usb,
        }
    }

    pub fn display_name(&self) -> String {
        match (&self.manufacturer, &self.model) {
            (Some(mfg), Some(model)) => {
                if model.to_lowercase().starts_with(&mfg.to_lowercase()) {
                    model.clone()
                } else {
                    format!("{} {}", mfg, model)
                }
            }
            (None, Some(model)) => model.clone(),
            _ => self.serial.clone(),
        }
    }
}

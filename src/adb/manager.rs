use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use crate::adb::client::AdbClient;
use crate::adb::device::{DeviceInfo, DeviceStatus};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct AdbDeviceManager {
    client: Arc<AdbClient>,
    active_device: Arc<RwLock<Option<DeviceInfo>>>,
    target_serial: Arc<RwLock<Option<String>>>,
}

impl AdbDeviceManager {
    pub fn new(client: AdbClient, target_serial: Option<String>) -> Self {
        Self {
            client: Arc::new(client),
            active_device: Arc::new(RwLock::new(None)),
            target_serial: Arc::new(RwLock::new(target_serial)),
        }
    }

    pub fn client(&self) -> Arc<AdbClient> {
        self.client.clone()
    }

    pub async fn set_target_serial(&self, serial: Option<String>) {
        let mut target = self.target_serial.write().await;
        *target = serial;
    }

    pub async fn get_active_device(&self) -> Option<DeviceInfo> {
        self.active_device.read().await.clone()
    }

    /// Checks the connected devices and updates the active device accordingly
    pub async fn refresh_device(&self) -> Result<Option<DeviceInfo>> {
        let devices = self.client.list_devices().await?;
        let target_serial = self.target_serial.read().await.clone();

        if devices.is_empty() {
            let mut active = self.active_device.write().await;
            if active.is_some() {
                info!("Android device disconnected.");
                *active = None;
            }
            return Ok(None);
        }

        // Find candidate device
        let chosen_device = if let Some(target) = &target_serial {
            devices.into_iter().find(|d| &d.serial == target)
        } else {
            // Prefer authorized device
            let authorized: Vec<DeviceInfo> = devices.iter().filter(|d| d.status == DeviceStatus::Device).cloned().collect();
            if !authorized.is_empty() {
                Some(authorized[0].clone())
            } else {
                devices.into_iter().next()
            }
        };

        if let Some(mut dev) = chosen_device {
            if dev.status == DeviceStatus::Device && dev.model.is_none() {
                let _ = self.client.populate_device_details(&mut dev).await;
            }

            let mut active = self.active_device.write().await;
            let changed = match &*active {
                Some(current) => current.serial != dev.serial || current.status != dev.status,
                None => true,
            };

            if changed {
                info!(
                    "Active device updated: {} ({}) - Status: {}",
                    dev.serial,
                    dev.display_name(),
                    dev.status
                );
                *active = Some(dev.clone());
            }

            Ok(Some(dev))
        } else {
            let mut active = self.active_device.write().await;
            *active = None;
            Ok(None)
        }
    }
}

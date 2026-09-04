use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};
use crate::adb::device::{DeviceInfo, DeviceStatus};
use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct AdbClient {
    adb_path: PathBuf,
}

impl AdbClient {
    /// Discovers `adb.exe` from a custom path, PATH, or well-known Android SDK directories.
    pub fn find_adb(custom_path: Option<&str>) -> Result<Self> {
        if let Some(p) = custom_path {
            if !p.trim().is_empty() {
                let path = PathBuf::from(p.trim());
                if path.exists() && path.is_file() {
                    info!("Using custom ADB path: {}", path.display());
                    return Ok(Self { adb_path: path });
                } else {
                    warn!("Custom ADB path not found: {}", path.display());
                }
            }
        }

        // Search PATH environment variable
        if let Ok(path) = which_adb() {
            info!("Found ADB on PATH: {}", path.display());
            return Ok(Self { adb_path: path });
        }

        // Search standard Windows SDK paths
        let candidates = get_standard_adb_paths();
        for candidate in candidates {
            if candidate.exists() && candidate.is_file() {
                info!("Found ADB in standard location: {}", candidate.display());
                return Ok(Self { adb_path: candidate });
            }
        }

        Err(AppError::AdbNotFound(
            "Please install Android Platform Tools or configure adb_path in settings.".to_string(),
        ))
    }

    pub fn executable_path(&self) -> &Path {
        &self.adb_path
    }

    /// Helper to construct a Windows command without showing a console window
    fn build_command(&self) -> Command {
        let mut cmd = Command::new(&self.adb_path);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW = 0x08000000 to prevent flashing command prompt windows
            cmd.creation_flags(0x08000000);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd
    }

    /// Starts the ADB server
    pub async fn start_server(&self) -> Result<()> {
        let mut cmd = self.build_command();
        cmd.arg("start-server");

        let output = cmd.output().await.map_err(|e| {
            AppError::AdbServer(format!("Failed to execute adb start-server: {e}"))
        })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            warn!("adb start-server returned error: {}", err);
        } else {
            debug!("ADB server started or already running");
        }
        Ok(())
    }

    /// Restarts the ADB server (kill then start)
    pub async fn restart_server(&self) -> Result<()> {
        info!("Restarting ADB server...");
        let mut kill_cmd = self.build_command();
        kill_cmd.arg("kill-server");
        let _ = kill_cmd.output().await;

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start_server().await
    }

    /// List connected devices (`adb devices -l`)
    pub async fn list_devices(&self) -> Result<Vec<DeviceInfo>> {
        let mut cmd = self.build_command();
        cmd.args(["devices", "-l"]);

        let output = cmd.output().await.map_err(|e| {
            AppError::AdbCommandFailed(format!("Failed to run adb devices: {e}"))
        })?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::AdbCommandFailed(err.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices = parse_adb_devices_output(&stdout);
        Ok(devices)
    }

    /// Fetch device properties such as model, manufacturer, and Android release version
    pub async fn populate_device_details(&self, device: &mut DeviceInfo) -> Result<()> {
        if device.status != DeviceStatus::Device {
            return Ok(());
        }

        let mut cmd = self.build_command();
        cmd.args([
            "-s",
            &device.serial,
            "shell",
            "getprop ro.product.model; getprop ro.product.manufacturer; getprop ro.build.version.release; getprop ro.build.version.sdk",
        ]);

        let output = cmd.output().await.map_err(|e| {
            AppError::AdbCommandFailed(format!("Failed to run getprop: {e}"))
        })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().map(|s| s.trim()).collect();
            if let Some(&model) = lines.first() {
                if !model.is_empty() {
                    device.model = Some(model.to_string());
                }
            }
            if let Some(&mfg) = lines.get(1) {
                if !mfg.is_empty() {
                    device.manufacturer = Some(mfg.to_string());
                }
            }
            if let Some(&rel) = lines.get(2) {
                if !rel.is_empty() {
                    device.android_version = Some(rel.to_string());
                }
            }
            if let Some(&sdk) = lines.get(3) {
                if let Ok(level) = sdk.parse::<u32>() {
                    device.sdk_level = Some(level);
                }
            }
        }

        Ok(())
    }

    /// Execute an arbitrary shell command on a specific device
    pub async fn shell_exec(&self, serial: &str, command: &str) -> Result<String> {
        let mut cmd = self.build_command();
        cmd.args(["-s", serial, "shell", command]);

        let output = cmd.output().await.map_err(|e| {
            AppError::AdbCommandFailed(format!("Failed to execute adb shell {command}: {e}"))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            if stderr.contains("device unauthorized") {
                return Err(AppError::DeviceUnauthorized);
            } else if stderr.contains("device not found") || stderr.contains("device offline") {
                return Err(AppError::DeviceDisconnected(serial.to_string()));
            }
            return Err(AppError::AdbCommandFailed(format!("{stderr} {stdout}")));
        }

        // ADB sometimes returns 0 even if unauthorized or device offline in stderr
        if stderr.contains("device unauthorized") {
            return Err(AppError::DeviceUnauthorized);
        }

        Ok(stdout)
    }
}

/// Search for `adb.exe` in PATH
fn which_adb() -> Result<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("adb.exe");
            if candidate.exists() && candidate.is_file() {
                return Ok(candidate);
            }
            let candidate_no_ext = dir.join("adb");
            if candidate_no_ext.exists() && candidate_no_ext.is_file() {
                return Ok(candidate_no_ext);
            }
        }
    }
    Err(AppError::AdbNotFound("Not found in PATH".to_string()))
}

/// Get common Android SDK install locations on Windows
fn get_standard_adb_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(&local_app_data).join("Android").join("Sdk").join("platform-tools").join("adb.exe"));
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        paths.push(PathBuf::from(&user_profile).join("AppData").join("Local").join("Android").join("Sdk").join("platform-tools").join("adb.exe"));
        paths.push(PathBuf::from(&user_profile).join("platform-tools").join("adb.exe"));
        paths.push(PathBuf::from(&user_profile).join("Downloads").join("platform-tools").join("adb.exe"));
    }
    if let Ok(prog_files) = std::env::var("ProgramFiles") {
        paths.push(PathBuf::from(&prog_files).join("Android").join("platform-tools").join("adb.exe"));
    }
    if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
        paths.push(PathBuf::from(&prog_files_x86).join("Android").join("platform-tools").join("adb.exe"));
    }
    paths.push(PathBuf::from("C:\\platform-tools\\adb.exe"));
    paths.push(PathBuf::from("D:\\platform-tools\\adb.exe"));
    paths.push(PathBuf::from("C:\\adb\\adb.exe"));

    paths
}

/// Parses the output of `adb devices -l`
pub fn parse_adb_devices_output(output: &str) -> Vec<DeviceInfo> {
    let mut devices = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        let serial = parts[0].to_string();
        let status_str = parts.get(1).unwrap_or(&"unknown");
        let status = DeviceStatus::from(*status_str);

        let mut device = DeviceInfo::new(serial, status);

        // Parse optional key:value tags in line, e.g. model:Pixel_8 device:shiba
        for part in &parts[2..] {
            if let Some((k, v)) = part.split_once(':') {
                match k {
                    "model" => device.model = Some(v.replace('_', " ")),
                    "device" => {
                        if device.model.is_none() {
                            device.model = Some(v.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        devices.push(device);
    }

    devices
}

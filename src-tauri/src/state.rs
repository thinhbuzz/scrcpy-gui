use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::process::Child;

#[derive(Default, Clone)]
pub struct AppState {
    pub monitoring: Arc<Mutex<bool>>,
    pub current_devices: Arc<Mutex<HashSet<String>>>,
    /// Track running scrcpy processes by device ID
    pub scrcpy_processes: Arc<Mutex<HashMap<String, Arc<Mutex<ProcessState>>>>>,
    pub adb_path: Arc<Mutex<Option<String>>>,
    pub scrcpy_path: Arc<Mutex<Option<String>>>,
}

pub enum ProcessState {
    Starting,
    Running(Child),
    StopRequested,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub label: String,
}

#[derive(Clone)]
pub struct AdbDevice {
    pub id: String,
    pub device_name: Option<String>,
    pub model_name: Option<String>,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LogPayload {
    pub device_id: String,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolPaths {
    pub adb_path: Option<String>,
    pub scrcpy_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceApp {
    pub name: String,
    pub package_name: String,
    pub version_name: String,
    pub version_code: u32,
    pub is_system_app: bool,
    pub base64_icon: String,
    pub is_installed_for_user: bool,
    pub is_disabled: bool,
}

#[derive(serde::Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(serde::Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
}

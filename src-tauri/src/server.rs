use crate::adb::{adb_push, args_from, run_adb_shell_capture};
use crate::state::DeviceApp;
use std::path::PathBuf;
use tauri::Manager;

pub const SERVER_FILE_NAME: &str = "scrcpy-gui-server";
pub const SERVER_DEVICE_PATH: &str = "/data/local/tmp/scrcpy-gui-server";
pub const SERVER_CLASS_NAME: &str = "me.thinhbuzz.scrcpy.gui.server.Server";
pub const SERVER_BYTES: &[u8] = include_bytes!("../scrcpy-gui-server");

pub fn ensure_local_server_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir: {}", err))?;
    let path = dir.join(SERVER_FILE_NAME);
    if let Ok(metadata) = std::fs::metadata(&path) {
        if metadata.len() == SERVER_BYTES.len() as u64 {
            return Ok(path);
        }
    }
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create data dir: {}", err))?;
    std::fs::write(&path, SERVER_BYTES)
        .map_err(|err| format!("Failed to write server file: {}", err))?;
    Ok(path)
}

pub fn parse_list_command_output(output: &str) -> Result<Vec<DeviceApp>, String> {
    if output.contains("ListCommand failed") {
        return Err(output.trim().to_string());
    }
    let marker = "ListCommand successfully:";
    let json = output
        .split_once(marker)
        .map(|(_, rest)| rest.trim())
        .ok_or_else(|| "ListCommand output not found".to_string())?;
    serde_json::from_str::<Vec<DeviceApp>>(json)
        .map_err(|err| format!("Failed to parse ListCommand output: {}", err))
}

pub async fn run_server_list(
    adb_path: Option<String>,
    device_id: &str,
) -> Result<Vec<DeviceApp>, String> {
    let mut args = Vec::new();
    args.push(format!("CLASSPATH={}", SERVER_DEVICE_PATH));
    args.extend(args_from(&[
        "app_process",
        "/",
        SERVER_CLASS_NAME,
        "--list",
        "app",
        "--list-type",
        "all",
    ]));
    let output = run_adb_shell_capture(adb_path, device_id, &args).await?;
    parse_list_command_output(&output)
}

pub async fn ensure_server_on_device(
    app: &tauri::AppHandle,
    adb_path: Option<String>,
    device_id: &str,
) -> Result<(), String> {
    let local_path = ensure_local_server_file(app)?;
    let local_size = SERVER_BYTES.len() as u64;

    let mut needs_push = true;
    if let Ok(output) = run_adb_shell_capture(
        adb_path.clone(),
        device_id,
        &args_from(&["stat", "-c", "%s", SERVER_DEVICE_PATH]),
    )
    .await
    {
        if let Ok(remote_size) = output.trim().parse::<u64>() {
            if remote_size == local_size {
                needs_push = false;
            }
        }
    }

    if needs_push {
        adb_push(adb_path, device_id, &local_path, SERVER_DEVICE_PATH).await?;
    }

    Ok(())
}

pub async fn remove_server_on_device(
    adb_path: Option<String>,
    device_id: &str,
) -> Result<(), String> {
    run_adb_shell_capture(
        adb_path,
        device_id,
        &args_from(&["rm", "-f", SERVER_DEVICE_PATH]),
    )
    .await
    .map(|_| ())
}

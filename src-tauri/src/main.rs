// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Default, Clone)]
struct AppState {
    monitoring: Arc<Mutex<bool>>,
    current_devices: Arc<Mutex<HashSet<String>>>,
    // Track running scrcpy processes by device ID
    scrcpy_processes: Arc<Mutex<HashMap<String, Arc<Mutex<Option<Child>>>>>>,
}

fn create_command(binary: &str) -> Command {
    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let mut command = Command::new(format!("{}{}", binary, binary_ext));

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

#[tauri::command]
async fn get_connected_devices(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let devices = state.current_devices.lock().map_err(|e| e.to_string())?;
    Ok(devices.iter().cloned().collect())
}

#[tauri::command]
async fn start_device_monitoring(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut monitoring = state.monitoring.lock().map_err(|e| e.to_string())?;
    if *monitoring {
        return Ok(());
    }
    *monitoring = true;
    drop(monitoring);

    spawn_monitor_loop(app, state.inner().clone());
    Ok(())
}

#[tauri::command]
async fn stop_device_monitoring(state: State<'_, AppState>) -> Result<(), String> {
    let mut monitoring = state.monitoring.lock().map_err(|e| e.to_string())?;
    *monitoring = false;
    Ok(())
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LogPayload {
    device_id: String,
    message: String,
}

#[tauri::command]
async fn start_scrcpy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    device_id: String,
    args: Vec<String>,
) -> Result<(), String> {
    {
        let processes = state.scrcpy_processes.lock().map_err(|e| e.to_string())?;
        if processes.contains_key(&device_id) {
            return Err("Scrcpy is already running for this device".to_string());
        }
    }

    let mut command = create_command("scrcpy");
    command.args(&args);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to spawn scrcpy: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let child_arc = Arc::new(Mutex::new(Some(child)));
    {
        let mut processes = state.scrcpy_processes.lock().map_err(|e| e.to_string())?;
        processes.insert(device_id.clone(), child_arc.clone());
    }

    let app_handle = app.clone();
    let device_id_event = device_id.clone();
    let state_clone = state.inner().clone();

    // Spawn log readers
    let mut reader_out = BufReader::new(stdout);
    let mut reader_err = BufReader::new(stderr);

    let app_out = app_handle.clone();
    let id_out = device_id.clone();
    tauri::async_runtime::spawn(async move {
        let mut line = String::new();
        while let Ok(n) = reader_out.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            let _ = app_out.emit(
                "scrcpy-log",
                LogPayload {
                    device_id: id_out.clone(),
                    message: line.clone(),
                },
            );
            line.clear();
        }
    });

    let app_err = app_handle.clone();
    let id_err = device_id.clone();
    tauri::async_runtime::spawn(async move {
        let mut line = String::new();
        while let Ok(n) = reader_err.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            let _ = app_err.emit(
                "scrcpy-log",
                LogPayload {
                    device_id: id_err.clone(),
                    message: line.clone(),
                },
            );
            line.clear();
        }
    });

    // Monitor for exit
    tauri::async_runtime::spawn(async move {
        let mut exit_code_captured = None;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let mut should_break = false;
            {
                if let Ok(mut child_lock) = child_arc.lock() {
                    if let Some(child) = child_lock.as_mut() {
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                exit_code_captured = status.code();
                                let _ = child_lock.take();
                                should_break = true;
                            }
                            Ok(None) => {}
                            Err(_) => {
                                let _ = child_lock.take();
                                should_break = true;
                            }
                        }
                    } else {
                        should_break = true;
                    }
                }
            }
            if should_break {
                break;
            }
        }

        {
            if let Ok(mut processes) = state_clone.scrcpy_processes.lock() {
                processes.remove(&device_id_event);
            }
        }

        let _ = app_handle.emit("scrcpy-exit", (device_id_event, exit_code_captured));
    });

    Ok(())
}

#[tauri::command]
async fn stop_scrcpy(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    let child_arc_opt = {
        let mut processes = state.scrcpy_processes.lock().map_err(|e| e.to_string())?;
        processes.remove(&device_id)
    };

    if let Some(child_arc) = child_arc_opt {
        let mut child_opt = None;
        if let Ok(mut child_lock) = child_arc.lock() {
            child_opt = child_lock.take();
        }
        if let Some(mut child) = child_opt {
            let _ = child.kill().await;
        }
    }
    Ok(())
}

fn spawn_monitor_loop(app: tauri::AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            {
                if let Ok(is_monitoring) = state.monitoring.lock() {
                    if !*is_monitoring {
                        break;
                    }
                }
            }

            let devices = get_adb_devices().await.unwrap_or_default();
            let devices_set: HashSet<String> = devices.into_iter().collect();

            let (new_devices, removed_devices) = {
                if let Ok(mut previous_devices) = state.current_devices.lock() {
                    let new_devs: Vec<String> =
                        devices_set.difference(&previous_devices).cloned().collect();
                    let removed_devs: Vec<String> =
                        previous_devices.difference(&devices_set).cloned().collect();
                    *previous_devices = devices_set;
                    (new_devs, removed_devs)
                } else {
                    (vec![], vec![])
                }
            };

            if !new_devices.is_empty() {
                let _ = app.emit("device-connected", new_devices);
            }
            if !removed_devices.is_empty() {
                let _ = app.emit("device-disconnected", removed_devices);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
}

async fn get_adb_devices() -> Result<Vec<String>, String> {
    let mut command = create_command("adb");
    let output = command
        .arg("devices")
        .output()
        .await
        .map_err(|e| format!("Failed to execute adb: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = stdout
        .lines()
        .skip(1)
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "device" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(devices)
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_os::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_device_monitoring,
            stop_device_monitoring,
            get_connected_devices,
            start_scrcpy,
            stop_scrcpy
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            if let Ok(mut monitoring) = state.monitoring.lock() {
                *monitoring = true;
            }
            spawn_monitor_loop(app.handle().clone(), state.inner().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let state = app_handle.state::<AppState>();
            if let Ok(mut processes) = state.inner().scrcpy_processes.lock() {
                for (device_id, child_arc) in processes.drain() {
                    if let Ok(mut child_lock) = child_arc.lock() {
                        if let Some(mut child) = child_lock.take() {
                            println!(
                                "Killing scrcpy process for device: {} due to app exit",
                                device_id
                            );
                            let _ = tauri::async_runtime::block_on(child.kill());
                        }
                    }
                }
            }
        }
    });
}

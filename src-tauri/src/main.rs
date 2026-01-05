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

    // We don't drop the lock immediately if we want to be strict, but for spawning we do.
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
    println!("[Backend] Received start_scrcpy request for {}", device_id);
    {
        let processes = state.scrcpy_processes.lock().map_err(|e| e.to_string())?;
        if processes.contains_key(&device_id) {
            return Err("Scrcpy is already running for this device".to_string());
        }
    }

    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let mut command = Command::new(format!("scrcpy{}", binary_ext));

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    command.args(&args);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    println!(
        "[Backend] Spawning scrcpy for {} with args: {:?}",
        device_id, args
    );

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

    // Initial log to confirm start
    let _ = app.emit(
        "scrcpy-log",
        LogPayload {
            device_id: device_id.clone(),
            message: format!("[Backend] Starting scrcpy for {}...\n", device_id),
        },
    );

    let app_handle = app.clone();
    let device_id_log = device_id.clone();
    let device_id_event = device_id.clone();
    let state_clone = state.inner().clone();

    // Spawn log readers
    let mut reader_out = BufReader::new(stdout);
    let mut reader_err = BufReader::new(stderr);

    let app_handle_out = app_handle.clone();
    let device_id_out = device_id_log.clone();
    tauri::async_runtime::spawn(async move {
        let mut line = String::new();
        loop {
            match reader_out.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    println!("[{}] stdout: {}", device_id_out, line);
                    let _ = app_handle_out.emit(
                        "scrcpy-log",
                        LogPayload {
                            device_id: device_id_out.clone(),
                            message: line.clone(),
                        },
                    );
                    line.clear();
                }
                Err(e) => {
                    eprintln!("[{}] stdout error: {}", device_id_out, e);
                    break;
                }
            }
        }
    });

    let app_handle_err = app_handle.clone();
    let device_id_err = device_id_log.clone();
    tauri::async_runtime::spawn(async move {
        let mut line = String::new();
        loop {
            match reader_err.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    println!("[{}] stderr: {}", device_id_err, line);
                    let _ = app_handle_err.emit(
                        "scrcpy-log",
                        LogPayload {
                            device_id: device_id_err.clone(),
                            message: line.clone(),
                        },
                    );
                    line.clear();
                }
                Err(e) => {
                    eprintln!("[{}] stderr error: {}", device_id_err, e);
                    break;
                }
            }
        }
    });

    // Monitor for exit via polling to allow external kill()
    tauri::async_runtime::spawn(async move {
        let mut exit_code_captured = None;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            let mut should_break = false;
            {
                let mut child_lock = child_arc.lock().unwrap();
                if let Some(child) = child_lock.as_mut() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            exit_code_captured = status.code();
                            let _ = child_lock.take();
                            should_break = true;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("[Backend] Error waiting for process: {}", e);
                            let _ = child_lock.take();
                            should_break = true;
                        }
                    }
                } else {
                    should_break = true;
                }
            }
            if should_break {
                break;
            }
        }

        {
            let mut processes = state_clone.scrcpy_processes.lock().unwrap();
            processes.remove(&device_id_event);
        }

        println!(
            "[Backend] Scrcpy for {} exited with code {:?}",
            device_id_event, exit_code_captured
        );
        let _ = app_handle.emit("scrcpy-exit", (device_id_event, exit_code_captured));
    });

    Ok(())
}

#[tauri::command]
async fn stop_scrcpy(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    println!("Stopping scrcpy for {}", device_id);
    let child_arc_opt = {
        let mut processes = state.scrcpy_processes.lock().map_err(|e| e.to_string())?;
        processes.remove(&device_id)
    };

    if let Some(child_arc) = child_arc_opt {
        let child_opt = {
            let mut child_lock = child_arc.lock().unwrap();
            child_lock.take()
        };
        if let Some(mut child) = child_opt {
            let _ = child.kill().await;
        }
    }
    Ok(())
}

fn spawn_monitor_loop(app: tauri::AppHandle, state: AppState) {
    let monitoring = state.monitoring.clone();
    let current_devices = state.current_devices.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            // Check if we should stop
            {
                let is_monitoring = monitoring.lock().unwrap();
                if !*is_monitoring {
                    break;
                }
            }

            // Get current devices
            let devices = match get_adb_devices().await {
                Ok(devs) => devs,
                Err(e) => {
                    eprintln!("Error fetching devices: {}", e);
                    Vec::new()
                }
            };

            let devices_set: HashSet<String> = devices.iter().cloned().collect();

            // Compare with previous devices
            let (new_devices, removed_devices) = {
                let mut previous_devices = current_devices.lock().unwrap();

                let new_devs: Vec<String> =
                    devices_set.difference(&previous_devices).cloned().collect();

                let removed_devs: Vec<String> =
                    previous_devices.difference(&devices_set).cloned().collect();

                // Update state
                *previous_devices = devices_set;

                (new_devs, removed_devs)
            };

            // Emit events
            if !new_devices.is_empty() {
                let _ = app.emit("device-connected", new_devices);
            }

            if !removed_devices.is_empty() {
                let _ = app.emit("device-disconnected", removed_devices);
            }

            // Wait 2 seconds
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });
}

async fn get_adb_devices() -> Result<Vec<String>, String> {
    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let mut command = Command::new(format!("adb{}", binary_ext));

    // Windows-specific: CREATE_NO_WINDOW
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000);
    }

    let output = command
        .arg("devices")
        .output()
        .await
        .map_err(|e| format!("Failed to execute adb: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "List of devices attached" {
            continue;
        }

        if let Some(device_id) = line.split_whitespace().next() {
            if line.contains("device") && !device_id.is_empty() {
                devices.push(device_id.to_string());
            }
        }
    }

    Ok(devices)
}

fn main() {
    tauri::Builder::default()
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

            // Start monitoring automatically
            {
                let mut monitoring = state.monitoring.lock().unwrap();
                *monitoring = true;
            }

            spawn_monitor_loop(app.handle().clone(), state.inner().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

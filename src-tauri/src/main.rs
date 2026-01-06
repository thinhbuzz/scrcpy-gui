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
    scrcpy_processes: Arc<Mutex<HashMap<String, Arc<Mutex<ProcessState>>>>>,
}

enum ProcessState {
    Starting,
    Running(Child),
    StopRequested,
}

fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
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
async fn get_connected_devices(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let devices = match get_adb_devices().await {
        Ok(devices) => devices,
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to get connected devices: {}\n", err),
            );
            return Err(err);
        }
    };
    let devices_set: HashSet<String> = devices.iter().cloned().collect();
    match state.current_devices.lock() {
        Ok(mut current_devices) => {
            *current_devices = devices_set;
        }
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock current devices: {}\n", err),
            );
        }
    }
    Ok(devices)
}

#[tauri::command]
async fn start_device_monitoring(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut monitoring = match state.monitoring.lock() {
        Ok(guard) => guard,
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock monitoring state: {}\n", err),
            );
            return Err(err.to_string());
        }
    };
    if *monitoring {
        return Ok(());
    }
    *monitoring = true;
    drop(monitoring);

    spawn_monitor_loop(app, state.inner().clone());
    Ok(())
}

#[tauri::command]
async fn stop_device_monitoring(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut monitoring = match state.monitoring.lock() {
        Ok(guard) => guard,
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock monitoring state: {}\n", err),
            );
            return Err(err.to_string());
        }
    };
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
    let child_arc = {
        let mut processes = match state.scrcpy_processes.lock() {
            Ok(guard) => guard,
            Err(err) => {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                );
                return Err(err.to_string());
            }
        };
        if processes.contains_key(&device_id) {
            emit_app_log(
                &app,
                format!(
                    "[Backend] Scrcpy already running for device: {}\n",
                    device_id
                ),
            );
            return Err("Scrcpy is already running for this device".to_string());
        }
        let placeholder = Arc::new(Mutex::new(ProcessState::Starting));
        processes.insert(device_id.clone(), placeholder.clone());
        placeholder
    };

    let mut command = create_command("scrcpy");
    command.args(&args);
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            match state.scrcpy_processes.lock() {
                Ok(mut processes) => {
                    processes.remove(&device_id);
                }
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
            emit_app_log(
                &app,
                format!("[Backend] Failed to spawn scrcpy: {}\n", e),
            );
            return Err(format!("Failed to spawn scrcpy: {}", e));
        }
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill().await;
            match state.scrcpy_processes.lock() {
                Ok(mut processes) => {
                    processes.remove(&device_id);
                }
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
            emit_app_log(&app, "[Backend] Failed to capture stdout\n");
            return Err("Failed to capture stdout".to_string());
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = child.kill().await;
            match state.scrcpy_processes.lock() {
                Ok(mut processes) => {
                    processes.remove(&device_id);
                }
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
            emit_app_log(&app, "[Backend] Failed to capture stderr\n");
            return Err("Failed to capture stderr".to_string());
        }
    };

    let stop_requested = match child_arc.lock() {
        Ok(child_lock) => Some(matches!(*child_lock, ProcessState::StopRequested)),
        Err(_) => None,
    };

    if stop_requested.is_none() {
        let _ = child.kill().await;
        match state.scrcpy_processes.lock() {
            Ok(mut processes) => {
                processes.remove(&device_id);
            }
            Err(err) => {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                );
            }
        }
        emit_app_log(&app, "[Backend] Failed to lock scrcpy process\n");
        return Err("Failed to start scrcpy due to lock error".to_string());
    }

    if stop_requested == Some(true) {
        let _ = child.kill().await;
        match state.scrcpy_processes.lock() {
            Ok(mut processes) => {
                processes.remove(&device_id);
            }
            Err(err) => {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                );
            }
        }
        emit_app_log(
            &app,
            format!("[Backend] Scrcpy start canceled for {}\n", device_id),
        );
        return Err("Scrcpy start canceled".to_string());
    }

    let mut child_opt = Some(child);
    let set_running = match child_arc.lock() {
        Ok(mut child_lock) => {
            if let Some(child) = child_opt.take() {
                *child_lock = ProcessState::Running(child);
                true
            } else {
                false
            }
        }
        Err(_) => false,
    };

    if !set_running {
        if let Some(mut child) = child_opt {
            let _ = child.kill().await;
        }
        match state.scrcpy_processes.lock() {
            Ok(mut processes) => {
                processes.remove(&device_id);
            }
            Err(err) => {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                );
            }
        }
        emit_app_log(&app, "[Backend] Failed to lock scrcpy process\n");
        return Err("Failed to start scrcpy due to lock error".to_string());
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
                    match &mut *child_lock {
                        ProcessState::Running(child) => match child.try_wait() {
                            Ok(Some(status)) => {
                                exit_code_captured = status.code();
                                *child_lock = ProcessState::StopRequested;
                                should_break = true;
                            }
                            Ok(None) => {}
                            Err(err) => {
                                emit_app_log(
                                    &app_handle,
                                    format!(
                                        "[Backend] Failed to poll scrcpy for {}: {}\n",
                                        device_id_event, err
                                    ),
                                );
                                *child_lock = ProcessState::StopRequested;
                                should_break = true;
                            }
                        },
                        ProcessState::Starting => {}
                        ProcessState::StopRequested => {
                            should_break = true;
                        }
                    }
                }
            }
            if should_break {
                break;
            }
        }

        {
            match state_clone.scrcpy_processes.lock() {
                Ok(mut processes) => {
                    processes.remove(&device_id_event);
                }
                Err(err) => {
                    emit_app_log(
                        &app_handle,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
        }

        let _ = app_handle.emit("scrcpy-exit", (device_id_event, exit_code_captured));
    });

    Ok(())
}

#[tauri::command]
async fn stop_scrcpy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    device_id: String,
) -> Result<(), String> {
    let child_arc_opt = match state.scrcpy_processes.lock() {
        Ok(processes) => processes.get(&device_id).cloned(),
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock scrcpy map: {}\n", err),
            );
            return Err(err.to_string());
        }
    };

    if let Some(child_arc) = child_arc_opt {
        let mut child_opt = None;
        if let Ok(mut child_lock) = child_arc.lock() {
            match std::mem::replace(&mut *child_lock, ProcessState::StopRequested) {
                ProcessState::Running(child) => child_opt = Some(child),
                ProcessState::Starting | ProcessState::StopRequested => {}
            }
        } else {
            emit_app_log(&app, "[Backend] Failed to lock scrcpy process\n");
        }
        match state.scrcpy_processes.lock() {
            Ok(mut processes) => {
                processes.remove(&device_id);
            }
            Err(err) => {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                );
            }
        }
        if let Some(mut child) = child_opt {
            if let Err(err) = child.kill().await {
                emit_app_log(
                    &app,
                    format!("[Backend] Failed to stop scrcpy for {}: {}\n", device_id, err),
                );
            }
        }
    }
    Ok(())
}

fn spawn_monitor_loop(app: tauri::AppHandle, state: AppState) {
    tauri::async_runtime::spawn(async move {
        loop {
            match state.monitoring.lock() {
                Ok(is_monitoring) => {
                    if !*is_monitoring {
                        break;
                    }
                }
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to lock monitoring state: {}\n", err),
                    );
                    break;
                }
            }

            let devices = match get_adb_devices().await {
                Ok(list) => list,
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to read adb devices: {}\n", err),
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            let devices_set: HashSet<String> = devices.into_iter().collect();

            let (new_devices, removed_devices) = match state.current_devices.lock() {
                Ok(mut previous_devices) => {
                    let new_devs: Vec<String> =
                        devices_set.difference(&previous_devices).cloned().collect();
                    let removed_devs: Vec<String> =
                        previous_devices.difference(&devices_set).cloned().collect();
                    *previous_devices = devices_set;
                    (new_devs, removed_devs)
                }
                Err(err) => {
                    emit_app_log(
                        &app,
                        format!("[Backend] Failed to lock current devices: {}\n", err),
                    );
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
            match state.inner().scrcpy_processes.lock() {
                Ok(mut processes) => {
                    for (device_id, child_arc) in processes.drain() {
                        match child_arc.lock() {
                            Ok(mut child_lock) => {
                                if let ProcessState::Running(mut child) =
                                    std::mem::replace(&mut *child_lock, ProcessState::StopRequested)
                                {
                                    println!(
                                        "Killing scrcpy process for device: {} due to app exit",
                                        device_id
                                    );
                                    if let Err(err) = tauri::async_runtime::block_on(child.kill()) {
                                        emit_app_log(
                                            &app_handle,
                                            format!(
                                                "[Backend] Failed to kill scrcpy for {}: {}\n",
                                                device_id, err
                                            ),
                                        );
                                    }
                                }
                            }
                            Err(err) => {
                                emit_app_log(
                                    &app_handle,
                                    format!(
                                        "[Backend] Failed to lock scrcpy process for {}: {}\n",
                                        device_id, err
                                    ),
                                );
                            }
                        }
                    }
                }
                Err(err) => {
                    emit_app_log(
                        &app_handle,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
        }
    });
}

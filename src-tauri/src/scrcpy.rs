use crate::state::{AppState, LogPayload, ProcessState};
use crate::tools::{
    create_scrcpy_command, resolve_or_read_adb_path, resolve_or_read_scrcpy_path,
};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Emit a log message to the frontend.
fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
}

/// Start scrcpy for the given device.
///
/// The function spawns the scrcpy process, captures stdout/stderr for log
/// forwarding, and monitors the process for exit. The start/stop race
/// condition is handled by atomically checking for a stop request and
/// transitioning from `Starting` to `Running` in a single lock acquisition.
pub async fn start_scrcpy(
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

    let scrcpy_path = resolve_or_read_scrcpy_path(state.inner(), &app);
    let adb_path = resolve_or_read_adb_path(state.inner(), &app);
    let mut command = create_scrcpy_command(scrcpy_path.as_deref());
    if let Some(adb) = adb_path.as_deref() {
        if !adb.trim().is_empty() {
            command.env("ADB", adb);
        }
    }
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

    // Atomically check stop status and transition to Running.
    // This single lock acquisition eliminates the race window between
    // checking StopRequested and setting Running that existed in the
    // previous two-phase approach.
    let mut child_opt = Some(child);
    {
        let kill_needed = match child_arc.lock() {
            Ok(mut guard) => match &*guard {
                ProcessState::StopRequested => true,
                ProcessState::Starting => {
                    *guard = match child_opt.take() {
                        Some(c) => ProcessState::Running(c),
                        None => {
                            // Should never happen — Starting state always has a child.
                            // Clean up and return an error.
                            drop(guard);
                            match state.scrcpy_processes.lock() {
                                Ok(mut processes) => {
                                    processes.remove(&device_id);
                                }
                                Err(err) => {
                                    emit_app_log(
                                        &app,
                                        format!(
                                            "[Backend] Failed to lock scrcpy map: {}\n",
                                            err
                                        ),
                                    );
                                }
                            }
                            emit_app_log(
                                &app,
                                "[Backend] Unexpected: child was None during start\n",
                            );
                            return Err(
                                "Failed to start scrcpy due to internal error".to_string()
                            );
                        }
                    };
                    false
                }
                _ => true,
            },
            Err(_) => true,
        };

        if kill_needed {
            // child wasn't moved into the mutex — kill it properly
            if let Some(mut child_to_kill) = child_opt.take() {
                let _ = child_to_kill.kill().await;
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
            emit_app_log(
                &app,
                "[Backend] Scrcpy start canceled or state error\n",
            );
            return Err("Scrcpy start canceled".to_string());
        }
    }
    // child is now safely stored in the mutex as Running

    // Spawn log readers.
    let mut reader_out = BufReader::new(stdout);
    let mut reader_err = BufReader::new(stderr);

    let app_out = app.clone();
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

    let app_err = app.clone();
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

    // Monitor for exit.
    let app_handle = app.clone();
    let device_id_event = device_id.clone();
    let state_clone = state.inner().clone();
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

/// Stop scrcpy for the given device.
pub async fn stop_scrcpy(
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
                    format!(
                        "[Backend] Failed to stop scrcpy for {}: {}\n",
                        device_id, err
                    ),
                );
            }
        }
    }
    Ok(())
}

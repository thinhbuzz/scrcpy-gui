// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adb;
mod monitor;
mod scrcpy;
mod server;
mod state;
mod terminal;
mod tools;

use state::{AppState, DeviceApp, DeviceInfo, ProcessState, ToolPaths};
use std::collections::HashSet;
use std::io::ErrorKind;
use tauri::{Emitter, Manager};

fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
}

// ── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn get_connected_devices(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DeviceInfo>, String> {
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    let devices = match adb::get_adb_devices(&app, adb_path).await {
        Ok(devices) => devices,
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to get connected devices: {}\n", err),
            );
            return Err(err);
        }
    };
    let devices_set: HashSet<String> = devices.iter().map(|device| device.id.clone()).collect();
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
    Ok(devices.into_iter().map(adb::build_device_info).collect())
}

#[tauri::command]
async fn start_device_monitoring(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
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

    monitor::spawn_monitor_loop(app, state.inner().clone());
    Ok(())
}

#[tauri::command]
async fn stop_device_monitoring(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut monitoring = match state.monitoring.lock() {
        Ok(guard) => guard,
        Err(err) => {
            emit_app_log(
                &app,
                format!(
                    "[Backend] Failed to lock monitoring state: {}\n",
                    err
                ),
            );
            return Err(err.to_string());
        }
    };
    *monitoring = false;
    Ok(())
}

#[tauri::command]
fn set_adb_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    let normalized = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    match state.adb_path.lock() {
        Ok(mut stored) => {
            *stored = normalized;
        }
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock adb path: {}\n", err),
            );
            return Err(err.to_string());
        }
    }
    tools::persist_tool_paths(&app, state.inner())?;
    Ok(())
}

#[tauri::command]
fn set_scrcpy_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    let normalized = path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    match state.scrcpy_path.lock() {
        Ok(mut stored) => {
            *stored = normalized;
        }
        Err(err) => {
            emit_app_log(
                &app,
                format!("[Backend] Failed to lock scrcpy path: {}\n", err),
            );
            return Err(err.to_string());
        }
    }
    tools::persist_tool_paths(&app, state.inner())?;
    Ok(())
}

#[tauri::command]
fn get_tool_paths(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ToolPaths, String> {
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    let scrcpy_path = tools::resolve_or_read_scrcpy_path(state.inner(), &app);
    Ok(ToolPaths {
        adb_path,
        scrcpy_path,
    })
}

#[tauri::command]
async fn download_and_install_scrcpy(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ToolPaths, String> {
    tools::download_and_install_scrcpy(&app, state.inner()).await
}

#[tauri::command]
async fn start_scrcpy(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
    args: Vec<String>,
) -> Result<(), String> {
    scrcpy::start_scrcpy(app, state, device_id, args).await
}

#[tauri::command]
async fn stop_scrcpy(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
) -> Result<(), String> {
    scrcpy::stop_scrcpy(app, state, device_id).await
}

#[tauri::command]
async fn open_device_terminal(
    app: tauri::AppHandle,
    device_id: String,
) -> Result<(), String> {
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err("Device ID is empty".to_string());
    }

    let result = if cfg!(target_os = "windows") {
        terminal::open_windows_terminal(trimmed)
    } else if cfg!(target_os = "macos") {
        terminal::open_macos_terminal(trimmed)
    } else {
        terminal::open_linux_terminal(trimmed)
    };
    result.map_err(|err| {
        emit_app_log(
            &app,
            format!("[Backend] Failed to open terminal: {}\n", err),
        );
        err
    })
}

#[tauri::command]
async fn list_device_apps(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
) -> Result<Vec<DeviceApp>, String> {
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err("Device ID is empty".to_string());
    }
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    server::ensure_server_on_device(&app, adb_path.clone(), trimmed).await?;
    let list = server::run_server_list(adb_path.clone(), trimmed).await;
    let _ = server::remove_server_on_device(adb_path.clone(), trimmed).await;
    let mut apps = list?;
    apps.sort_by(|a, b| {
        let left = if a.name.is_empty() {
            &a.package_name
        } else {
            &a.name
        };
        let right = if b.name.is_empty() {
            &b.package_name
        } else {
            &b.name
        };
        left.to_lowercase()
            .cmp(&right.to_lowercase())
            .then_with(|| a.package_name.cmp(&b.package_name))
    });
    Ok(apps)
}

#[tauri::command]
async fn uninstall_package(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
    package_name: String,
    is_system: bool,
) -> Result<(), String> {
    let trimmed_device = device_id.trim();
    let trimmed_package = package_name.trim();
    if trimmed_device.is_empty() {
        return Err("Device ID is empty".to_string());
    }
    if trimmed_package.is_empty() {
        return Err("Package name is empty".to_string());
    }
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    let args = if is_system {
        vec![
            "pm".to_string(),
            "uninstall".to_string(),
            "--user".to_string(),
            "0".to_string(),
            trimmed_package.to_string(),
        ]
    } else {
        vec![
            "pm".to_string(),
            "uninstall".to_string(),
            trimmed_package.to_string(),
        ]
    };
    let output = adb::run_adb_shell(adb_path, trimmed_device, &args).await?;
    if output.to_lowercase().contains("failure") {
        return Err(output.trim().to_string());
    }
    Ok(())
}

#[tauri::command]
async fn set_package_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
    package_name: String,
    enabled: bool,
) -> Result<(), String> {
    let trimmed_device = device_id.trim();
    let trimmed_package = package_name.trim();
    if trimmed_device.is_empty() {
        return Err("Device ID is empty".to_string());
    }
    if trimmed_package.is_empty() {
        return Err("Package name is empty".to_string());
    }
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    let args = if enabled {
        vec![
            "pm".to_string(),
            "enable".to_string(),
            "--user".to_string(),
            "0".to_string(),
            trimmed_package.to_string(),
        ]
    } else {
        vec![
            "pm".to_string(),
            "disable-user".to_string(),
            "--user".to_string(),
            "0".to_string(),
            trimmed_package.to_string(),
        ]
    };
    let output = adb::run_adb_shell(adb_path, trimmed_device, &args).await?;
    if output.to_lowercase().contains("failure") {
        return Err(output.trim().to_string());
    }
    Ok(())
}

#[tauri::command]
async fn install_existing_package(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    device_id: String,
    package_name: String,
) -> Result<(), String> {
    let trimmed_device = device_id.trim();
    let trimmed_package = package_name.trim();
    if trimmed_device.is_empty() {
        return Err("Device ID is empty".to_string());
    }
    if trimmed_package.is_empty() {
        return Err("Package name is empty".to_string());
    }
    let adb_path = tools::resolve_or_read_adb_path(state.inner(), &app);
    let args = vec![
        "pm".to_string(),
        "install-existing".to_string(),
        trimmed_package.to_string(),
    ];
    let output = adb::run_adb_shell(adb_path, trimmed_device, &args).await?;
    if output.to_lowercase().contains("failure") {
        return Err(output.trim().to_string());
    }
    Ok(())
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_device_monitoring,
            stop_device_monitoring,
            get_connected_devices,
            set_adb_path,
            set_scrcpy_path,
            get_tool_paths,
            download_and_install_scrcpy,
            start_scrcpy,
            stop_scrcpy,
            open_device_terminal,
            list_device_apps,
            uninstall_package,
            install_existing_package,
            set_package_enabled
        ])
        .setup(|app| {
            // Set window title with app version
            if let Some(window) = app.get_webview_window("main") {
                let version = app.package_info().version.to_string();
                let _ = window.set_title(&format!("Scrcpy GUI v{}", version));
            }
            let state = app.state::<AppState>();
            if let Ok(path) = tools::tool_paths_file(app.handle()) {
                match std::fs::read_to_string(&path) {
                    Ok(data) => match serde_json::from_str::<ToolPaths>(&data) {
                        Ok(tool_paths) => {
                            if let Ok(mut adb_path) = state.adb_path.lock() {
                                if adb_path.is_none() && tool_paths.adb_path.is_some() {
                                    *adb_path = tool_paths.adb_path;
                                }
                            }
                            if let Ok(mut scrcpy_path) = state.scrcpy_path.lock() {
                                if scrcpy_path.is_none()
                                    && tool_paths.scrcpy_path.is_some()
                                {
                                    *scrcpy_path = tool_paths.scrcpy_path;
                                }
                            }
                        }
                        Err(err) => {
                            emit_app_log(
                                app.handle(),
                                format!(
                                    "[Backend] Failed to parse tool paths: {}\n",
                                    err
                                ),
                            );
                        }
                    },
                    Err(err) => {
                        if err.kind() != ErrorKind::NotFound {
                            emit_app_log(
                                app.handle(),
                                format!(
                                    "[Backend] Failed to read tool paths: {}\n",
                                    err
                                ),
                            );
                        }
                    }
                }
            }
            if let Ok(mut adb_path) = state.adb_path.lock() {
                if adb_path.is_none() {
                    *adb_path = tools::resolve_binary_from_env("adb");
                }
            }
            if let Ok(mut scrcpy_path) = state.scrcpy_path.lock() {
                if scrcpy_path.is_none() {
                    *scrcpy_path = tools::resolve_binary_from_env("scrcpy");
                }
            }
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
                                    std::mem::replace(
                                        &mut *child_lock,
                                        ProcessState::StopRequested,
                                    )
                                {
                                    println!(
                                        "Killing scrcpy process for device: {} due to app exit",
                                        device_id
                                    );
                                    if let Err(err) =
                                        tauri::async_runtime::block_on(child.kill())
                                    {
                                        emit_app_log(
                                            app_handle,
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
                                    app_handle,
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
                        app_handle,
                        format!("[Backend] Failed to lock scrcpy map: {}\n", err),
                    );
                }
            }
        }
    });
}

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
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
    adb_path: Arc<Mutex<Option<String>>>,
    scrcpy_path: Arc<Mutex<Option<String>>>,
}

enum ProcessState {
    Starting,
    Running(Child),
    StopRequested,
}

fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
}

fn tool_paths_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir: {}", err))?;
    Ok(dir.join("tool-paths.json"))
}

fn persist_tool_paths(app: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let path = tool_paths_file(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create data dir: {}", err))?;
    }
    let adb_path = match state.adb_path.lock() {
        Ok(path) => path.clone(),
        Err(err) => {
            emit_app_log(
                app,
                format!("[Backend] Failed to lock adb path: {}\n", err),
            );
            None
        }
    };
    let scrcpy_path = match state.scrcpy_path.lock() {
        Ok(path) => path.clone(),
        Err(err) => {
            emit_app_log(
                app,
                format!("[Backend] Failed to lock scrcpy path: {}\n", err),
            );
            None
        }
    };
    let payload = ToolPaths {
        adb_path,
        scrcpy_path,
    };
    let json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Failed to serialize tool paths: {}", err))?;
    std::fs::write(&path, json).map_err(|err| format!("Failed to write tool paths: {}", err))?;
    Ok(())
}

fn resolve_binary_from_env(binary: &str) -> Option<String> {
    let env_key = format!("{}_PATH", binary.to_uppercase());
    if let Ok(value) = env::var(&env_key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let path_var = env::var_os("PATH")?;
    let binary_ext = if cfg!(target_os = "windows") {
        format!("{}.exe", binary)
    } else {
        binary.to_string()
    };
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(&binary_ext);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

fn create_command(binary: &str) -> Command {
    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let command = Command::new(format!("{}{}", binary, binary_ext));

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

fn create_command_with_override(binary: &str, override_path: Option<&str>) -> Command {
    if let Some(path) = override_path {
        if !path.trim().is_empty() {
            return Command::new(path);
        }
    }
    create_command(binary)
}

fn resolve_or_read_adb_path(state: &AppState, app: &tauri::AppHandle) -> Option<String> {
    match state.adb_path.lock() {
        Ok(mut path) => {
            if path.is_none() {
                let resolved = resolve_binary_from_env("adb");
                if resolved.is_some() {
                    *path = resolved.clone();
                }
            }
            path.clone()
        }
        Err(err) => {
            emit_app_log(
                app,
                format!("[Backend] Failed to lock adb path: {}\n", err),
            );
            None
        }
    }
}

fn resolve_or_read_scrcpy_path(state: &AppState, app: &tauri::AppHandle) -> Option<String> {
    match state.scrcpy_path.lock() {
        Ok(mut path) => {
            if path.is_none() {
                let resolved = resolve_binary_from_env("scrcpy");
                if resolved.is_some() {
                    *path = resolved.clone();
                }
            }
            path.clone()
        }
        Err(err) => {
            emit_app_log(
                app,
                format!("[Backend] Failed to lock scrcpy path: {}\n", err),
            );
            None
        }
    }
}

#[tauri::command]
async fn get_connected_devices(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let adb_path = resolve_or_read_adb_path(state.inner(), &app);
    let devices = match get_adb_devices(adb_path).await {
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

#[tauri::command]
fn set_adb_path(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
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
    persist_tool_paths(&app, state.inner())?;
    Ok(())
}

#[tauri::command]
fn set_scrcpy_path(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
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
    persist_tool_paths(&app, state.inner())?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ToolPaths {
    adb_path: Option<String>,
    scrcpy_path: Option<String>,
}

#[tauri::command]
fn get_tool_paths(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<ToolPaths, String> {
    let adb_path = resolve_or_read_adb_path(state.inner(), &app);
    let scrcpy_path = resolve_or_read_scrcpy_path(state.inner(), &app);
    Ok(ToolPaths {
        adb_path,
        scrcpy_path,
    })
}

#[derive(serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

fn pick_scrcpy_asset<'a>(
    os: &str,
    arch: &str,
    assets: &'a [GithubAsset],
) -> Option<&'a GithubAsset> {
    let (prefix, ext) = match (os, arch) {
        ("macos", "aarch64") => ("scrcpy-macos-aarch64-v", ".tar.gz"),
        ("macos", "x86_64") => ("scrcpy-macos-x86_64-v", ".tar.gz"),
        ("linux", "x86_64") => ("scrcpy-linux-x86_64-v", ".tar.gz"),
        ("windows", "x86_64") => ("scrcpy-win64-v", ".zip"),
        ("windows", "x86") | ("windows", "i686") => ("scrcpy-win32-v", ".zip"),
        _ => return None,
    };
    assets.iter().find(|asset| {
        asset.name.starts_with(prefix) && asset.name.ends_with(ext)
    })
}

fn find_file_recursive(root: &Path, file_name: &str) -> Option<std::path::PathBuf> {
    if !root.is_dir() {
        return None;
    }
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return None,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, file_name) {
                return Some(found);
            }
        } else if let Some(name) = path.file_name() {
            if name == file_name {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(unix)]
fn ensure_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(path, perms);
    }
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    if archive_path.extension().and_then(|ext| ext.to_str()) == Some("zip") {
        let file = std::fs::File::open(archive_path)
            .map_err(|err| format!("Failed to open archive: {}", err))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|err| format!("Failed to read zip: {}", err))?;
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|err| format!("Failed to read zip entry: {}", err))?;
            let out_path = dest_dir.join(file.name());
            if file.is_dir() {
                std::fs::create_dir_all(&out_path)
                    .map_err(|err| format!("Failed to create dir: {}", err))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| format!("Failed to create dir: {}", err))?;
                }
                let mut out_file = std::fs::File::create(&out_path)
                    .map_err(|err| format!("Failed to write file: {}", err))?;
                std::io::copy(&mut file, &mut out_file)
                    .map_err(|err| format!("Failed to extract file: {}", err))?;
            }
        }
        return Ok(());
    }

    if archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.ends_with(".tar.gz"))
        .unwrap_or(false)
    {
        let file = std::fs::File::open(archive_path)
            .map_err(|err| format!("Failed to open archive: {}", err))?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(dest_dir)
            .map_err(|err| format!("Failed to extract tar.gz: {}", err))?;
        return Ok(());
    }

    Err("Unsupported archive format".to_string())
}

#[tauri::command]
async fn download_and_install_scrcpy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ToolPaths, String> {
    let client = reqwest::Client::builder()
        .user_agent("scrcpy-gui")
        .build()
        .map_err(|err| format!("Failed to create HTTP client: {}", err))?;
    let release = client
        .get("https://api.github.com/repos/Genymobile/scrcpy/releases/latest")
        .send()
        .await
        .map_err(|err| format!("Failed to fetch scrcpy release: {}", err))?
        .error_for_status()
        .map_err(|err| format!("Failed to fetch scrcpy release: {}", err))?
        .json::<GithubRelease>()
        .await
        .map_err(|err| format!("Failed to parse scrcpy release: {}", err))?;

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let asset = pick_scrcpy_asset(os, arch, &release.assets).ok_or_else(|| {
        format!("No compatible scrcpy asset for {}/{}", os, arch)
    })?;

    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir: {}", err))?;
    let install_root = app_dir.join("scrcpy");
    let version_dir = install_root.join(release.tag_name.trim_start_matches('v'));
    std::fs::create_dir_all(&version_dir)
        .map_err(|err| format!("Failed to create install dir: {}", err))?;

    let archive_path = version_dir.join(&asset.name);
    let download = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|err| format!("Failed to download scrcpy: {}", err))?
        .error_for_status()
        .map_err(|err| format!("Failed to download scrcpy: {}", err))?;
    let bytes = download
        .bytes()
        .await
        .map_err(|err| format!("Failed to read download: {}", err))?;
    tokio::fs::write(&archive_path, &bytes)
        .await
        .map_err(|err| format!("Failed to write archive: {}", err))?;

    let extract_dir = version_dir.join("extracted");
    if extract_dir.exists() {
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    std::fs::create_dir_all(&extract_dir)
        .map_err(|err| format!("Failed to create extract dir: {}", err))?;

    let archive_path_clone = archive_path.clone();
    let extract_dir_clone = extract_dir.clone();
    tokio::task::spawn_blocking(move || extract_archive(&archive_path_clone, &extract_dir_clone))
        .await
        .map_err(|err| format!("Failed to extract scrcpy: {}", err))?
        .map_err(|err| err)?;

    let scrcpy_name = if cfg!(target_os = "windows") {
        "scrcpy.exe"
    } else {
        "scrcpy"
    };
    let adb_name = if cfg!(target_os = "windows") {
        "adb.exe"
    } else {
        "adb"
    };
    let scrcpy_path = find_file_recursive(&extract_dir, scrcpy_name)
        .ok_or_else(|| "Failed to locate scrcpy binary".to_string())?;
    let adb_path = find_file_recursive(&extract_dir, adb_name)
        .ok_or_else(|| "Failed to locate adb binary".to_string())?;

    #[cfg(unix)]
    {
        ensure_executable(&scrcpy_path);
        ensure_executable(&adb_path);
    }

    let scrcpy_path_str = scrcpy_path.to_string_lossy().to_string();
    let adb_path_str = adb_path.to_string_lossy().to_string();

    if let Ok(mut stored) = state.scrcpy_path.lock() {
        *stored = Some(scrcpy_path_str.clone());
    }
    if let Ok(mut stored) = state.adb_path.lock() {
        *stored = Some(adb_path_str.clone());
    }
    persist_tool_paths(&app, state.inner())?;

    Ok(ToolPaths {
        adb_path: Some(adb_path_str),
        scrcpy_path: Some(scrcpy_path_str),
    })
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

    let scrcpy_path = resolve_or_read_scrcpy_path(state.inner(), &app);
    let adb_path = resolve_or_read_adb_path(state.inner(), &app);
    let mut command = create_command_with_override("scrcpy", scrcpy_path.as_deref());
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

#[tauri::command]
async fn open_device_terminal(
    app: tauri::AppHandle,
    device_id: String,
) -> Result<(), String> {
    let trimmed = device_id.trim();
    if trimmed.is_empty() {
        return Err("Device ID is empty".to_string());
    }

    if cfg!(target_os = "windows") {
        open_windows_terminal(trimmed)
    } else if cfg!(target_os = "macos") {
        open_macos_terminal(trimmed)
    } else {
        open_linux_terminal(trimmed).map_err(|err| {
            emit_app_log(
                &app,
                format!("[Backend] Failed to open terminal: {}\n", err),
            );
            err
        })
    }
}

fn open_windows_terminal(device_id: &str) -> Result<(), String> {
    let command = format!(
        "title {} & doskey adb=adb -s {} $*",
        device_id, device_id
    );
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "start", "", "cmd", "/k", &command]);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(0x00000010); // CREATE_NEW_CONSOLE
    }

    cmd.spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to open Windows terminal: {}", err))
}

fn escape_applescript(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn escape_shell_single(input: &str) -> String {
    input.replace('\'', "'\\''")
}

fn open_macos_terminal(device_id: &str) -> Result<(), String> {
    let escaped = escape_shell_single(device_id);
    let command = format!(
        "printf '\\033]0;{0}\\007'; alias adb='adb -s {0}'; echo 'adb => adb -s {0}'",
        escaped
    );
    let script = format!(
        "tell application \"Terminal\" to do script \"{}\"",
        escape_applescript(&command)
    );
    Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to open macOS Terminal: {}", err))
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let path_env = env::var_os("PATH")?;
    for path in env::split_paths(&path_env) {
        let full = path.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

fn write_shell_rc(device_id: &str) -> Result<PathBuf, String> {
    let escaped = escape_shell_single(device_id);
    let content = format!(
        "printf '\\033]0;{0}\\007'\nalias adb='adb -s {0}'\necho 'adb => adb -s {0}'\n",
        escaped
    );
    let mut path = env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    path.push(format!("scrcpy-gui-adb-{}.rc", stamp));
    fs::write(&path, content).map_err(|err| format!("Failed to write rc file: {}", err))?;
    Ok(path)
}

fn open_linux_terminal(device_id: &str) -> Result<(), String> {
    let rc_path = write_shell_rc(device_id)?;
    let rc_path_str = rc_path.to_string_lossy().to_string();
    let bash = find_executable("bash").unwrap_or_else(|| PathBuf::from("bash"));
    let bash_str = bash.to_string_lossy().to_string();

    let candidates = [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "mate-terminal",
        "lxterminal",
        "xterm",
        "alacritty",
        "kitty",
        "tilix",
    ];

    for terminal in candidates {
        if find_executable(terminal).is_none() {
            continue;
        }

        let mut command = Command::new(terminal);
        match terminal {
            "gnome-terminal" => {
                command.args([
                    "--",
                    &bash_str,
                    "--rcfile",
                    &rc_path_str,
                    "-i",
                ]);
            }
            "xfce4-terminal" | "mate-terminal" | "lxterminal" | "tilix" => {
                command.args([
                    "-e",
                    &format!("{} --rcfile {} -i", bash_str, rc_path_str),
                ]);
            }
            _ => {
                command.args(["-e", &bash_str, "--rcfile", &rc_path_str, "-i"]);
            }
        }

        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    Err("No supported terminal emulator found".to_string())
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

            let adb_path = resolve_or_read_adb_path(&state, &app);
            let devices = match get_adb_devices(adb_path).await {
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

async fn get_adb_devices(adb_path: Option<String>) -> Result<Vec<String>, String> {
    let mut command = create_command_with_override("adb", adb_path.as_deref());
    let output = command
        .arg("devices")
        .output()
        .await
        .map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                let path = env::var("PATH").unwrap_or_else(|_| "<unset>".to_string());
                let configured = adb_path.as_deref().unwrap_or("<unset>");
                format!(
                    "Failed to execute adb: {}. App PATH: {}. Configured adb path: {}",
                    e, path, configured
                )
            } else {
                format!("Failed to execute adb: {}", e)
            }
        })?;

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
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
            open_device_terminal
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            if let Ok(mut monitoring) = state.monitoring.lock() {
                *monitoring = true;
            }
            if let Ok(path) = tool_paths_file(app.handle()) {
                match std::fs::read_to_string(&path) {
                    Ok(data) => match serde_json::from_str::<ToolPaths>(&data) {
                        Ok(tool_paths) => {
                            if let Ok(mut adb_path) = state.adb_path.lock() {
                                if adb_path.is_none() && tool_paths.adb_path.is_some() {
                                    *adb_path = tool_paths.adb_path;
                                }
                            }
                            if let Ok(mut scrcpy_path) = state.scrcpy_path.lock() {
                                if scrcpy_path.is_none() && tool_paths.scrcpy_path.is_some() {
                                    *scrcpy_path = tool_paths.scrcpy_path;
                                }
                            }
                        }
                        Err(err) => {
                            emit_app_log(
                                app.handle(),
                                format!("[Backend] Failed to parse tool paths: {}\n", err),
                            );
                        }
                    },
                    Err(err) => {
                        if err.kind() != ErrorKind::NotFound {
                            emit_app_log(
                                app.handle(),
                                format!("[Backend] Failed to read tool paths: {}\n", err),
                            );
                        }
                    }
                }
            }
            if let Ok(mut adb_path) = state.adb_path.lock() {
                if adb_path.is_none() {
                    *adb_path = resolve_binary_from_env("adb");
                }
            }
            if let Ok(mut scrcpy_path) = state.scrcpy_path.lock() {
                if scrcpy_path.is_none() {
                    *scrcpy_path = resolve_binary_from_env("scrcpy");
                }
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

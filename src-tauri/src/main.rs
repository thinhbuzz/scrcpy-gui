// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tokio::process::Command;

#[derive(Default)]
struct DeviceMonitor {
    // We don't need 'running' mutex for the command anymore if we just have one background task
    // But we might want to stop it? For now, let's keep it simple.
    // Making it global state to allow "stop" if needed.
    monitoring: Arc<Mutex<bool>>,
    current_devices: Arc<Mutex<HashSet<String>>>,
}

#[tauri::command]
async fn get_connected_devices(monitor: State<'_, DeviceMonitor>) -> Result<Vec<String>, String> {
    let devices = monitor.current_devices.lock().map_err(|e| e.to_string())?;
    Ok(devices.iter().cloned().collect())
}

#[tauri::command]
async fn start_device_monitoring(
    app: tauri::AppHandle,
    monitor: State<'_, DeviceMonitor>,
) -> Result<(), String> {
    let mut monitoring = monitor.monitoring.lock().map_err(|e| e.to_string())?;
    if *monitoring {
        return Ok(());
    }
    *monitoring = true;

    // We don't drop the lock immediately if we want to be strict, but for spawning we do.
    drop(monitoring);

    spawn_monitor_loop(app, monitor.clone());
    Ok(())
}

#[tauri::command]
async fn stop_device_monitoring(monitor: State<'_, DeviceMonitor>) -> Result<(), String> {
    let mut monitoring = monitor.monitoring.lock().map_err(|e| e.to_string())?;
    *monitoring = false;
    Ok(())
}

fn spawn_monitor_loop(app: tauri::AppHandle, monitor_state: State<'_, DeviceMonitor>) {
    let monitoring = monitor_state.monitoring.clone();
    let current_devices = monitor_state.current_devices.clone();

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
        .manage(DeviceMonitor::default())
        .invoke_handler(tauri::generate_handler![
            start_device_monitoring,
            stop_device_monitoring,
            get_connected_devices
        ])
        .setup(|app| {
            let monitor_state = app.state::<DeviceMonitor>();

            // Start monitoring automatically
            {
                let mut monitoring = monitor_state.monitoring.lock().unwrap();
                *monitoring = true;
            }

            spawn_monitor_loop(app.handle().clone(), monitor_state.clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

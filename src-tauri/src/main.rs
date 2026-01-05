// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashSet;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

struct DeviceMonitor {
    running: Arc<Mutex<bool>>,
    current_devices: Arc<Mutex<HashSet<String>>>,
}

#[tauri::command]
async fn start_device_monitoring(
    app: tauri::AppHandle,
    monitor: State<'_, DeviceMonitor>,
) -> Result<(), String> {
    let mut running = monitor.running.lock().unwrap();
    if *running {
        return Ok(()); // Already running
    }
    *running = true;
    drop(running);

    let app_handle = app.clone();
    let running_clone = monitor.running.clone();
    let current_devices_clone = monitor.current_devices.clone();

    tokio::spawn(async move {
        loop {
            // Check if we should stop
            {
                let running = running_clone.lock().unwrap();
                if !*running {
                    break;
                }
            }

            // Get current devices
            let devices = get_adb_devices().unwrap_or_default();
            let devices_set: HashSet<String> = devices.iter().cloned().collect();

            // Compare with previous devices
            let mut previous_devices = current_devices_clone.lock().unwrap();
            let previous_set = previous_devices.clone();

            // Find new devices
            let new_devices: Vec<String> = devices_set
                .difference(&previous_set)
                .cloned()
                .collect();

            // Find removed devices
            let removed_devices: Vec<String> = previous_set
                .difference(&devices_set)
                .cloned()
                .collect();

            // Update current devices
            *previous_devices = devices_set;

            // Emit events
            if !new_devices.is_empty() {
                app_handle
                    .emit_all("device-connected", new_devices)
                    .unwrap_or_default();
            }

            if !removed_devices.is_empty() {
                app_handle
                    .emit_all("device-disconnected", removed_devices)
                    .unwrap_or_default();
            }

            // Wait 2 seconds before next check
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    });

    Ok(())
}

#[tauri::command]
async fn stop_device_monitoring(monitor: State<'_, DeviceMonitor>) -> Result<(), String> {
    let mut running = monitor.running.lock().unwrap();
    *running = false;
    Ok(())
}

fn get_adb_devices() -> Result<Vec<String>, String> {
    // Determine binary extension based on platform
    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };

    let output = Command::new(format!("adb{}", binary_ext))
        .arg("devices")
        .output()
        .map_err(|e| format!("Failed to execute adb: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "List of devices attached" {
            continue;
        }

        // Parse line like "ABC123    device"
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
        .manage(DeviceMonitor {
            running: Arc::new(Mutex::new(false)),
            current_devices: Arc::new(Mutex::new(HashSet::new())),
        })
        .invoke_handler(tauri::generate_handler![
            start_device_monitoring,
            stop_device_monitoring
        ])
        .setup(|app| {
            // Auto-start monitoring when app starts
            let app_handle = app.handle().clone();
            let monitor_state = app.state::<DeviceMonitor>();
            let running_clone = monitor_state.running.clone();
            let current_devices_clone = monitor_state.current_devices.clone();
            
            tauri::async_runtime::spawn(async move {
                let mut running = running_clone.lock().unwrap();
                *running = true;
                drop(running);

                loop {
                    // Check if we should stop
                    {
                        let running = running_clone.lock().unwrap();
                        if !*running {
                            break;
                        }
                    }

                    // Get current devices
                    let devices = get_adb_devices().unwrap_or_default();
                    let devices_set: HashSet<String> = devices.iter().cloned().collect();

                    // Compare with previous devices
                    let mut previous_devices = current_devices_clone.lock().unwrap();
                    let previous_set = previous_devices.clone();

                    // Find new devices
                    let new_devices: Vec<String> = devices_set
                        .difference(&previous_set)
                        .cloned()
                        .collect();

                    // Find removed devices
                    let removed_devices: Vec<String> = previous_set
                        .difference(&devices_set)
                        .cloned()
                        .collect();

                    // Update current devices
                    *previous_devices = devices_set;

                    // Emit events
                    if !new_devices.is_empty() {
                        app_handle
                            .emit_all("device-connected", new_devices)
                            .unwrap_or_default();
                    }

                    if !removed_devices.is_empty() {
                        app_handle
                            .emit_all("device-disconnected", removed_devices)
                            .unwrap_or_default();
                    }

                    // Wait 2 seconds before next check
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

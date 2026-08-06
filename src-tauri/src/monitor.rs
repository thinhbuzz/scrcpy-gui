use crate::adb::get_adb_devices;
use crate::state::AppState;
use crate::tools::resolve_or_read_adb_path;
use std::collections::HashSet;
use tauri::Emitter;

/// Emit a log message to the frontend.
fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
}

/// Spawn a background loop that polls for device connect/disconnect events.
pub fn spawn_monitor_loop(app: tauri::AppHandle, state: AppState) {
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
                        format!(
                            "[Backend] Failed to lock monitoring state: {}\n",
                            err
                        ),
                    );
                    break;
                }
            }

            let adb_path = resolve_or_read_adb_path(&state, &app);
            let devices = match get_adb_devices(&app, adb_path).await {
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
            let devices_set: HashSet<String> =
                devices.into_iter().map(|device| device.id).collect();

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
                        format!(
                            "[Backend] Failed to lock current devices: {}\n",
                            err
                        ),
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

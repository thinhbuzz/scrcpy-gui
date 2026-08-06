use crate::state::{AdbDevice, DeviceInfo};
use crate::tools::create_command_with_override;
use std::env;
use std::io::ErrorKind;

pub fn args_from(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

pub fn parse_adb_device_line(line: &str) -> Option<AdbDevice> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 || parts[1] != "device" {
        return None;
    }
    let mut device_name = None;
    let mut model_name = None;
    for part in parts.iter().skip(2) {
        if let Some((key, value)) = part.split_once(':') {
            match key {
                "device" => device_name = Some(value.to_string()),
                "model" => model_name = Some(value.to_string()),
                _ => {}
            }
        }
    }
    Some(AdbDevice {
        id: parts[0].to_string(),
        device_name,
        model_name,
    })
}

pub fn build_device_info(device: AdbDevice) -> DeviceInfo {
    let extra = device.device_name.or(device.model_name);
    let label = match extra {
        Some(value) => format!("{}({})", device.id, value),
        None => device.id.clone(),
    };
    DeviceInfo {
        id: device.id,
        label,
    }
}

pub fn build_shell_args(device_id: &str, args: &[String]) -> Vec<String> {
    let mut shell_args = Vec::with_capacity(args.len() + 3);
    shell_args.push("-s".to_string());
    shell_args.push(device_id.to_string());
    shell_args.push("shell".to_string());
    shell_args.extend_from_slice(args);
    shell_args
}

pub async fn get_adb_devices(
    _app: &tauri::AppHandle,
    adb_path: Option<String>,
) -> Result<Vec<AdbDevice>, String> {
    let mut command = create_command_with_override("adb", adb_path.as_deref());
    let output = command
        .arg("devices")
        .arg("-l")
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
        .filter_map(parse_adb_device_line)
        .collect();

    Ok(devices)
}

pub async fn run_adb(
    adb_path: Option<String>,
    args: &[String],
) -> Result<std::process::Output, String> {
    let mut command = create_command_with_override("adb", adb_path.as_deref());
    for arg in args {
        command.arg(arg);
    }
    command.output().await.map_err(|e| {
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
    })
}

pub async fn run_adb_shell(
    adb_path: Option<String>,
    device_id: &str,
    args: &[String],
) -> Result<String, String> {
    let shell_args = build_shell_args(device_id, args);
    let output = run_adb(adb_path, &shell_args).await?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = if !stderr.trim().is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };
    Err(message.trim().to_string())
}

pub async fn run_adb_shell_capture(
    adb_path: Option<String>,
    device_id: &str,
    args: &[String],
) -> Result<String, String> {
    let shell_args = build_shell_args(device_id, args);
    let output = run_adb(adb_path, &shell_args).await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        if stdout.trim().is_empty() {
            return Ok(stderr);
        }
        if stderr.trim().is_empty() {
            return Ok(stdout);
        }
        return Ok(format!("{}\n{}", stdout.trim_end(), stderr.trim_end()));
    }
    let message = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    Err(message.trim().to_string())
}

pub async fn adb_push(
    adb_path: Option<String>,
    device_id: &str,
    local_path: &std::path::Path,
    remote_path: &str,
) -> Result<(), String> {
    let args = vec![
        "-s".to_string(),
        device_id.to_string(),
        "push".to_string(),
        local_path.to_string_lossy().to_string(),
        remote_path.to_string(),
    ];
    let output = run_adb(adb_path, &args).await?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = if !stderr.trim().is_empty() {
        stderr.to_string()
    } else {
        stdout.to_string()
    };
    Err(message.trim().to_string())
}

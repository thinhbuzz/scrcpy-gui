use crate::state::{AppState, GithubAsset, GithubRelease, ToolPaths};
use std::env;
use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};

/// Emit a log message to the frontend.
fn emit_app_log(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit("app-log", message.into());
}

pub fn tool_paths_file(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir: {}", err))?;
    Ok(dir.join("tool-paths.json"))
}

pub fn persist_tool_paths(app: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
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

pub fn resolve_binary_from_env(binary: &str) -> Option<String> {
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

pub fn resolve_or_read_adb_path(
    state: &AppState,
    app: &tauri::AppHandle,
) -> Option<String> {
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

pub fn resolve_or_read_scrcpy_path(
    state: &AppState,
    app: &tauri::AppHandle,
) -> Option<String> {
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

pub fn create_command(binary: &str) -> tokio::process::Command {
    let binary_ext = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let full_binary = format!("{}{}", binary, binary_ext);
    create_command_for_path(&full_binary)
}

pub fn create_command_with_override(
    binary: &str,
    override_path: Option<&str>,
) -> tokio::process::Command {
    if let Some(path) = override_path {
        if !path.trim().is_empty() {
            return create_command_for_path(path);
        }
    }
    create_command(binary)
}

pub fn create_scrcpy_command(override_path: Option<&str>) -> tokio::process::Command {
    #[cfg(target_os = "macos")]
    {
        if let Some(script_path) = crate::terminal::find_executable("script") {
            let scrcpy_exec = override_path
                .filter(|path| !path.trim().is_empty())
                .map(|path| path.to_string())
                .unwrap_or_else(|| "scrcpy".to_string());
            let mut command = tokio::process::Command::new(script_path);
            command.args(["-q", "/dev/null", &scrcpy_exec]);
            return command;
        }
    }
    create_command_with_override("scrcpy", override_path)
}

pub fn create_command_for_path(path: &str) -> tokio::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = tokio::process::Command::new(path);
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        return command;
    }

    #[cfg(not(target_os = "windows"))]
    {
        tokio::process::Command::new(path)
    }
}

pub fn pick_scrcpy_asset<'a>(
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
    assets
        .iter()
        .find(|asset| asset.name.starts_with(prefix) && asset.name.ends_with(ext))
}

pub fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
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
pub fn ensure_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
pub fn ensure_executable(_path: &Path) {
    // no-op on non-unix platforms
}

pub fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
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
            // Validate no path traversal: reject entries with `..` components
            // and ensure the resolved path stays within dest_dir.
            if out_path.components().any(|c| {
                matches!(c, std::path::Component::ParentDir)
            }) || !out_path.starts_with(dest_dir)
            {
                return Err(format!(
                    "Zip entry '{}' attempts path traversal",
                    file.name()
                ));
            }
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

pub async fn download_and_install_scrcpy(
    app: &tauri::AppHandle,
    state: &AppState,
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
    let asset =
        pick_scrcpy_asset(os, arch, &release.assets).ok_or_else(|| {
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
        .map_err(|err| format!("Failed to extract scrcpy: {}", err))??;

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
    persist_tool_paths(app, state)?;

    Ok(ToolPaths {
        adb_path: Some(adb_path_str),
        scrcpy_path: Some(scrcpy_path_str),
    })
}

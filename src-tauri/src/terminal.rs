use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Validate that a device ID contains only safe characters for shell commands.
fn is_safe_device_id(id: &str) -> bool {
    id.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == ':')
}

pub fn escape_applescript(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\"', "\\\"")
}

pub fn escape_shell_single(input: &str) -> String {
    input.replace('\'', "'\\''")
}

pub fn find_executable(name: &str) -> Option<PathBuf> {
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

pub fn open_windows_terminal(device_id: &str) -> Result<(), String> {
    // Validate device ID to prevent command injection
    if !is_safe_device_id(device_id) {
        return Err(format!(
            "Device ID contains invalid characters for terminal: {}",
            device_id
        ));
    }
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

pub fn open_macos_terminal(device_id: &str) -> Result<(), String> {
    let escaped = escape_shell_single(device_id);
    let command = format!(
        "printf '\\033]0;{0}\\007'; alias adb='adb -s {0}'; echo 'adb => adb -s {0}'",
        escaped
    );
    let script = format!(
        "tell application \"Terminal\"\n do script \"{}\"\n activate\nend tell",
        escape_applescript(&command)
    );
    Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Failed to open macOS Terminal: {}", err))
}

pub fn open_linux_terminal(device_id: &str) -> Result<(), String> {
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

        match command.spawn() {
            Ok(mut child) => {
                // Clean up the temp rc file after the terminal exits.
                // std::process::Child::wait() is blocking — safe to use in a thread.
                let rc_path_clone = rc_path.clone();
                std::thread::spawn(move || {
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&rc_path_clone);
                });
                return Ok(());
            }
            Err(_) => {
                // Try next terminal emulator
                continue;
            }
        }
    }

    // No terminal found — clean up the rc file
    let _ = std::fs::remove_file(&rc_path);
    Err("No supported terminal emulator found".to_string())
}

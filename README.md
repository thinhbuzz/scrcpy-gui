# Scrcpy GUI (Tauri)

Desktop GUI for `scrcpy` built with Tauri + Vue 3. Manage multiple Android devices, launch scrcpy sessions with common flags, and monitor logs from the app.

## Features

- Device discovery via `adb devices` with auto refresh and connection/disconnection logs.
- Start scrcpy per device with quick options (turn screen off, show touches, always on top, stay awake).
- FPS control and multi-device start/stop.
- Per-device actions: focus scrcpy window, open logs, or open a preconfigured terminal (`adb -s <device>`).
- Live log viewer with system and per-device tabs.
- Settings dialog for tool paths, download/install `scrcpy`, and OS notification settings.
- OS notifications for APK install and file push success/failure (optional).

## Requirements

- `adb` and `scrcpy` available in your `PATH`, or use the in-app download/install flow.
- Bun (used by Tauri config) and a Rust toolchain for Tauri builds.

## Development

Install dependencies:

```bash
bun install
```

Run the Tauri app:

```bash
bun run tauri dev
```

## Build

```bash
bun run tauri build
```

## Usage Notes

- Connect devices via USB or TCP/IP and ensure they appear in `adb devices`.
- Start will launch scrcpy for each selected device; Stop All terminates all running sessions.
- Use Settings to configure tool paths, enable notifications, or trigger a test notification.
- The application is unsigned, so a warning will appear when running it. Please ignore that warning and continue using the application. On macOS, you can use the application https://github.com/alienator88/Sentinel to run unsigned software.
- If `adb`/`scrcpy` are not in `PATH`, set them in Settings or via `ADB_PATH` / `SCRCPY_PATH` environment variables.
- The Download & Install button fetches the latest `scrcpy` release from GitHub and stores it in the app data directory.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

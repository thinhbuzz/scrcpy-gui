# Scrcpy GUI (Tauri)

Desktop GUI for `scrcpy` built with Tauri + Vue 3. Manage multiple Android devices, launch scrcpy sessions with common flags, and monitor logs from the app.

## Features

- Device discovery via `adb devices` with auto refresh.
- Start scrcpy per device with quick options (turn screen off, show touches, always on top, stay awake).
- FPS control and multi-device start/stop.
- Live log viewer for scrcpy output and exit status.

## Requirements

- `adb` and `scrcpy` available in your `PATH`.
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

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

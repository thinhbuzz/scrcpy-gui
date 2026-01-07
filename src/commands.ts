import { invoke } from "@tauri-apps/api/core";

export const getDevices = async (): Promise<string[]> => {
  return await invoke<string[]>("get_connected_devices");
};

export const startScrcpy = async (
  deviceId: string,
  args: string[]
): Promise<void> => {
  await invoke("start_scrcpy", { deviceId, args });
};

export const stopScrcpy = async (deviceId: string): Promise<void> => {
  await invoke("stop_scrcpy", { deviceId });
};

export const openDeviceTerminal = async (deviceId: string): Promise<void> => {
  await invoke("open_device_terminal", { deviceId });
};

export const focusScrcpyWindow = async (): Promise<void> => {
  await invoke("focus_scrcpy_window");
};

export const setAdbPath = async (
  path: string | null
): Promise<void> => {
  await invoke("set_adb_path", { path });
};

export const setScrcpyPath = async (
  path: string | null
): Promise<void> => {
  await invoke("set_scrcpy_path", { path });
};

export const getToolPaths = async (): Promise<{
  adbPath: string | null;
  scrcpyPath: string | null;
}> => {
  return await invoke("get_tool_paths");
};

export const downloadAndInstallScrcpy = async (): Promise<{
  adbPath: string | null;
  scrcpyPath: string | null;
}> => {
  return await invoke("download_and_install_scrcpy");
};

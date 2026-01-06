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

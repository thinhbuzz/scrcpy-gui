import { invoke } from "@tauri-apps/api/core";

export const getDevices = async (): Promise<string[]> => {
  try {
    return await invoke<string[]>("get_connected_devices");
  } catch (error) {
    console.error("Failed to get connected devices:", error);
    return [];
  }
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

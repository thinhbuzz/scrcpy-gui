import { invoke } from "@tauri-apps/api/core";

export const getDevices = async (): Promise<string[]> => {
  return await invoke<string[]>("get_connected_devices");
};

export const startDeviceMonitoring = async (): Promise<void> => {
  await invoke("start_device_monitoring");
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

export interface DeviceApp {
  packageName: string;
  versionName: string;
  versionCode: number;
  name: string;
  isSystemApp: boolean;
  base64Icon: string;
  isInstalledForUser: boolean;
  isDisabled: boolean;
}

export const listDeviceApps = async (deviceId: string): Promise<DeviceApp[]> => {
  return await invoke("list_device_apps", { deviceId });
};

export const uninstallPackage = async (
  deviceId: string,
  packageName: string,
  isSystem: boolean
): Promise<void> => {
  await invoke("uninstall_package", { deviceId, packageName, isSystem });
};

export const installExistingPackage = async (
  deviceId: string,
  packageName: string
): Promise<void> => {
  await invoke("install_existing_package", { deviceId, packageName });
};

export const setPackageEnabled = async (
  deviceId: string,
  packageName: string,
  enabled: boolean
): Promise<void> => {
  await invoke("set_package_enabled", { deviceId, packageName, enabled });
};

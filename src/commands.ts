import { Command, type Child } from "@tauri-apps/plugin-shell";
import { platform } from "@tauri-apps/plugin-os";
import { invoke } from "@tauri-apps/api/core";

let _binaryExtension: string | null = null;
let _platformType: string | null = null;

/**
 * Initialize platform detection and binary extension
 * Should be called once at app startup
 */
export async function initializePlatform(): Promise<void> {
  if (_binaryExtension === null) {
    _platformType = await platform();
    _binaryExtension = _platformType === "win32" ? ".exe" : "";
  }
}

/**
 * Get the binary extension for the current platform
 */
export function getBinaryExtension(): string {
  if (_binaryExtension === null) {
    // Fallback: assume Windows if not initialized
    // This should not happen if initializePlatform is called properly
    console.warn("Platform not initialized, defaulting to Windows extension");
    return ".exe";
  }
  return _binaryExtension;
}

/**
 * Get the current platform type
 */
export function getPlatformType(): string | null {
  return _platformType;
}

export const getDevices = async (): Promise<string[]> => {
  try {
    return await invoke<string[]>("get_connected_devices");
  } catch (error) {
    console.error("Failed to get connected devices:", error);
    return [];
  }
};

export const startScrcpy = async (
  args: string[],
  callback: (line: string) => void,
  onClose: (data: { code: number | null, signal: number | null }) => void
): Promise<Child> => {
  const command = Command.create("scrcpy" + getBinaryExtension(), args);
  command.on("error", callback);
  command.on('close', onClose);
  command.stdout.on("data", callback);
  command.stderr.on("data", callback);

  return await command.spawn();
};

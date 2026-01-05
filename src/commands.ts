import { Command, Child } from "@tauri-apps/api/shell";
import { type } from "@tauri-apps/api/os";

let _binaryExtension: string | null = null;
let _platformType: string | null = null;

/**
 * Initialize platform detection and binary extension
 * Should be called once at app startup
 */
export async function initializePlatform(): Promise<void> {
  if (_binaryExtension === null) {
    _platformType = await type();
    _binaryExtension = _platformType === "Windows_NT" ? ".exe" : "";
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

export const executeAdbDevices = (callback: (line: string) => void): void => {
  const command = new Command("adb" + getBinaryExtension(), ["devices"]);
  command.on("error", callback);
  command.stdout.on("data", callback);
  command.stderr.on("data", callback);

  command.spawn();
};

export const getDevices = (
  callback: (line: string) => void,
  log?: (line: string) => void
): void => {
  executeAdbDevices((line) => {
    log?.(line);
    const [, deviceId] = line.trim().match(/(.*)\s+device$/) || [];
    if (deviceId) {
      callback(deviceId);
    }
  });
};

export const startScrcpy = async (
  args: string[],
  callback: (line: string) => void,
  onClose: (data: {code: number, signal: string}) => void
): Promise<Child> => {
  const command = new Command("scrcpy" + getBinaryExtension(), args);
  command.on("error", callback);
  command.on('close', onClose);
  command.stdout.on("data", callback);
  command.stderr.on("data", callback);

  return command.spawn();
};

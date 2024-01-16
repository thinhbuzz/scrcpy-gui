import { Command, Child } from "@tauri-apps/api/shell";
import { type } from "@tauri-apps/api/os";

let _binaryExtension = ".exe";
export async function binaryExtension() {
  if ((await type()) == "Windows_NT") {
    _binaryExtension = "";
  }
}

export const executeAdbDevices = (callback: (line: string) => void): void => {
  const command = new Command("adb" + _binaryExtension, ["devices"]);
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
  const command = new Command("scrcpy" + _binaryExtension, args);
  command.on("error", callback);
  command.on('close', onClose);
  command.stdout.on("data", callback);
  command.stderr.on("data", callback);

  return command.spawn();
};

import { listen } from "@tauri-apps/api/event";
import type { Ref } from "vue";
interface UseScrcpyListenersOptions {
  selectedDevices: Ref<string[]>;
  startedDevices: Ref<string[]>;
  selectedUninstallDevice: Ref<string>;
  uninstallOpen: Ref<boolean>;
  appendSystemLog: (line: string) => void;
  refreshDevices: () => Promise<void>;
  stopScrcpy: (deviceId: string) => Promise<void>;
  handleScrcpyLog: (deviceId: string, message: string) => void;
  cleanDeviceState: (deviceId: string) => void;
}

interface LogPayload {
  deviceId: string;
  message: string;
}

export const useScrcpyListeners = (options: UseScrcpyListenersOptions) => {
  let deviceConnectedUnlisten: (() => void) | null = null;
  let deviceDisconnectedUnlisten: (() => void) | null = null;
  let scrcpyLogUnlisten: (() => void) | null = null;
  let scrcpyExitUnlisten: (() => void) | null = null;
  let appLogUnlisten: (() => void) | null = null;
  let disposed = false;

  const setupListeners = async (): Promise<void> => {
    // Clean up any previously registered listeners to prevent memory leaks
    // if setupListeners() is called multiple times.
    cleanup();
    disposed = false;

    try {
      deviceConnectedUnlisten = await listen<string[]>(
        "device-connected",
        (event) => {
          const newDevices = event.payload;
          options.appendSystemLog(
            `Device(s) connected: ${newDevices.join(", ")}\n`
          );
          options.refreshDevices();
        }
      );
      if (disposed) return;
    } catch (error) {
      options.appendSystemLog(
        `[Frontend] Error setting up device-connected listener: ${error}\n`
      );
    }

    try {
      deviceDisconnectedUnlisten = await listen<string[]>(
        "device-disconnected",
        (event) => {
          const removedDevices = event.payload;
          options.appendSystemLog(
            `Device(s) disconnected: ${removedDevices.join(", ")}\n`
          );

          removedDevices.forEach((deviceId) => {
            const selectedIndex = options.selectedDevices.value.indexOf(deviceId);
            if (selectedIndex !== -1) {
              options.selectedDevices.value.splice(selectedIndex, 1);
            }

            if (options.startedDevices.value.includes(deviceId)) {
              options.stopScrcpy(deviceId).catch((error) => {
                options.appendSystemLog(
                  `Failed to stop scrcpy for ${deviceId}: ${error}\n`
                );
              });
              options.startedDevices.value = options.startedDevices.value.filter(
                (id) => id !== deviceId
              );
            }

            // Clean up any pending install/push state for disconnected devices
            options.cleanDeviceState(deviceId);
          });

          if (
            options.uninstallOpen.value
            && options.selectedUninstallDevice.value
            && removedDevices.includes(options.selectedUninstallDevice.value)
          ) {
            options.uninstallOpen.value = false;
            options.selectedUninstallDevice.value = "";
          }

          options.refreshDevices();
        }
      );
      if (disposed) return;
    } catch (error) {
      options.appendSystemLog(
        `[Frontend] Error setting up device-disconnected listener: ${error}\n`
      );
    }

    try {
      scrcpyLogUnlisten = await listen<LogPayload>("scrcpy-log", (event) => {
        const { deviceId, message } = event.payload;
        options.handleScrcpyLog(deviceId, message);
      });
      if (disposed) return;
    } catch (error) {
      options.appendSystemLog(
        `[Frontend] Error setting up log listener: ${error}\n`
      );
    }

    try {
      scrcpyExitUnlisten = await listen<[string, number | null]>(
        "scrcpy-exit",
        (event) => {
          const [deviceId, exitCode] = event.payload;
          options.appendSystemLog(
            `Device ${deviceId} scrcpy exited with code ${exitCode ?? "null"}\n`
          );
          options.startedDevices.value = options.startedDevices.value.filter(
            (id) => id !== deviceId
          );
        }
      );
      if (disposed) return;
    } catch (error) {
      options.appendSystemLog(
        `[Frontend] Error setting up scrcpy-exit listener: ${error}\n`
      );
    }

    try {
      appLogUnlisten = await listen<string>("app-log", (event) => {
        options.appendSystemLog(event.payload);
      });
      if (disposed) return;
    } catch (error) {
      options.appendSystemLog(
        `[Frontend] Error setting up app-log listener: ${error}\n`
      );
    }
  };

  const cleanup = (): void => {
    disposed = true;
    if (deviceConnectedUnlisten) deviceConnectedUnlisten();
    if (deviceDisconnectedUnlisten) deviceDisconnectedUnlisten();
    if (scrcpyLogUnlisten) scrcpyLogUnlisten();
    if (scrcpyExitUnlisten) scrcpyExitUnlisten();
    if (appLogUnlisten) appLogUnlisten();
  };

  return { setupListeners, cleanup };
};

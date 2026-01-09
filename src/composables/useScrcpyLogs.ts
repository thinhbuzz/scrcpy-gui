import { computed, ref, watch, type Ref } from "vue";
import {
  isPermissionGranted,
  sendNotification,
} from "@tauri-apps/plugin-notification";

const maxLogLines = 1000;
const apkInstallRequestRe = /^INFO: Request to install (.+)$/;
const apkInstallSuccessRe = /^INFO:\s+(.+)\s+successfully installed$/;
const apkInstallFailedRe = /^ERROR: Failed to install (.+)$/;
const adbInstallFailedRe = /^adb: failed to install (.+?)(?:: (.+))?$/;
const pushRequestRe = /^INFO: Request to push (.+)$/;
const pushSuccessRe = /^INFO:\s+(.+)\s+successfully pushed to (.+)$/;
const pushFailedRe = /^ERROR: Failed to push (.+)$/;
const adbPushFailedRe = /^adb: failed to push (.+?)(?:: (.+))?$/;

const trimLogLines = (lines: string[]): void => {
  if (lines.length > maxLogLines) {
    lines.splice(0, lines.length - maxLogLines);
  }
};

export const useScrcpyLogs = (
  osNotificationsEnabled: Ref<boolean>,
  availableDevices: Ref<string[]>
) => {
  const systemLogLines = ref<string[]>([]);
  const deviceLogLines = ref<Record<string, string[]>>({});
  const activeLogTab = ref<string>("system");
  const pendingApkInstallByDevice = ref<Record<string, string>>({});
  const pendingApkFailureByDevice = ref<Record<string, string>>({});
  const pendingPushByDevice = ref<Record<string, string>>({});
  const pendingPushFailureByDevice = ref<Record<string, string>>({});
  const notifiedInstallKeys = new Set<string>();

  const clearAllLogs = (): void => {
    systemLogLines.value = [];
    deviceLogLines.value = {};
    activeLogTab.value = "system";
  };

  const appendSystemLog = (line: string): void => {
    systemLogLines.value.push(line);
    trimLogLines(systemLogLines.value);
  };

  const appendDeviceLog = (deviceId: string, line: string): void => {
    if (!deviceLogLines.value[deviceId]) {
      deviceLogLines.value[deviceId] = [];
    }
    deviceLogLines.value[deviceId].push(line);
    trimLogLines(deviceLogLines.value[deviceId]);
  };

  const sendOsNotification = async (
    title: string,
    body: string
  ): Promise<void> => {
    if (!osNotificationsEnabled.value) {
      return;
    }
    try {
      let granted = await isPermissionGranted();
      if (!granted) {
        appendSystemLog("[Frontend] Notification permission not granted.\n");
        return;
      }
      await sendNotification({ title, body });
    } catch (error) {
      appendSystemLog(`[Frontend] Failed to send notification: ${error}\n`);
    }
  };

  const notifyApkInstall = (
    deviceId: string,
    path: string,
    success: boolean,
    detail?: string
  ): void => {
    const key = `${deviceId}:${path}:${success ? "success" : "error"}`;
    if (notifiedInstallKeys.has(key)) {
      return;
    }
    notifiedInstallKeys.add(key);

    const fileName = path.split(/[\\/]/).pop() ?? path;
    const description = detail
      ? `${fileName} on ${deviceId}. ${detail}`
      : `${fileName} on ${deviceId}.`;
    void sendOsNotification(
      success ? "APK installed" : "APK install failed",
      description
    );
  };

  const notifyFilePush = (
    deviceId: string,
    path: string,
    success: boolean,
    detail?: string
  ): void => {
    const key = `${deviceId}:${path}:${success ? "push-success" : "push-error"}`;
    if (notifiedInstallKeys.has(key)) {
      return;
    }
    notifiedInstallKeys.add(key);

    const fileName = path.split(/[\\/]/).pop() ?? path;
    const description = detail
      ? `${fileName} on ${deviceId}. ${detail}`
      : `${fileName} on ${deviceId}.`;
    void sendOsNotification(
      success ? "File pushed" : "File push failed",
      description
    );
  };

  const handleApkInstallLog = (deviceId: string, line: string): void => {
    line = line.trim();
    const requestMatch = line.match(apkInstallRequestRe);
    if (requestMatch) {
      pendingApkInstallByDevice.value[deviceId] = requestMatch[1];
      delete pendingApkFailureByDevice.value[deviceId];
      return;
    }

    const successMatch = line.match(apkInstallSuccessRe);
    if (successMatch) {
      const path = successMatch[1];
      notifyApkInstall(deviceId, path, true);
      delete pendingApkInstallByDevice.value[deviceId];
      delete pendingApkFailureByDevice.value[deviceId];
      return;
    }

    const failedMatch = line.match(apkInstallFailedRe);
    if (failedMatch) {
      const path = failedMatch[1];
      const detail = pendingApkFailureByDevice.value[deviceId];
      notifyApkInstall(deviceId, path, false, detail);
      delete pendingApkInstallByDevice.value[deviceId];
      delete pendingApkFailureByDevice.value[deviceId];
      return;
    }

    const adbFailedMatch = line.match(adbInstallFailedRe);
    if (adbFailedMatch) {
      const pendingPath = pendingApkInstallByDevice.value[deviceId];
      const path = pendingPath ?? adbFailedMatch[1];
      pendingApkInstallByDevice.value[deviceId] = path;
      if (adbFailedMatch[2]) {
        pendingApkFailureByDevice.value[deviceId] = adbFailedMatch[2];
      }
    }

    const pushRequestMatch = line.match(pushRequestRe);
    if (pushRequestMatch) {
      pendingPushByDevice.value[deviceId] = pushRequestMatch[1];
      delete pendingPushFailureByDevice.value[deviceId];
      return;
    }

    const pushSuccessMatch = line.match(pushSuccessRe);
    if (pushSuccessMatch) {
      const path = pushSuccessMatch[1];
      const destination = pushSuccessMatch[2];
      notifyFilePush(deviceId, path, true, `to ${destination}`);
      delete pendingPushByDevice.value[deviceId];
      delete pendingPushFailureByDevice.value[deviceId];
      return;
    }

    const pushFailedMatch = line.match(pushFailedRe);
    if (pushFailedMatch) {
      const path = pushFailedMatch[1];
      const detail = pendingPushFailureByDevice.value[deviceId];
      notifyFilePush(deviceId, path, false, detail);
      delete pendingPushByDevice.value[deviceId];
      delete pendingPushFailureByDevice.value[deviceId];
      return;
    }

    const adbPushFailedMatch = line.match(adbPushFailedRe);
    if (adbPushFailedMatch) {
      const pendingPath = pendingPushByDevice.value[deviceId];
      const path = pendingPath ?? adbPushFailedMatch[1];
      pendingPushByDevice.value[deviceId] = path;
      if (adbPushFailedMatch[2]) {
        pendingPushFailureByDevice.value[deviceId] = adbPushFailedMatch[2];
      }
    }
  };

  const handleScrcpyLog = (deviceId: string, message: string): void => {
    handleApkInstallLog(deviceId, message);
    appendDeviceLog(deviceId, message);
  };

  const logDeviceIds = computed(() => {
    const ids = new Set<string>();
    availableDevices.value.forEach((id) => ids.add(id));
    Object.keys(deviceLogLines.value).forEach((id) => ids.add(id));
    return Array.from(ids);
  });

  watch(
    logDeviceIds,
    (ids) => {
      if (activeLogTab.value !== "system" && !ids.includes(activeLogTab.value)) {
        activeLogTab.value = "system";
      }
    },
    { immediate: true }
  );

  const setActiveLogTab = (deviceId: string): void => {
    activeLogTab.value = deviceId;
  };

  return {
    systemLogLines,
    deviceLogLines,
    activeLogTab,
    logDeviceIds,
    clearAllLogs,
    appendSystemLog,
    handleScrcpyLog,
    setActiveLogTab,
  };
};

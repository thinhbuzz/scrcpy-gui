<script lang="ts" setup>
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, watch } from "vue";
import {
  CheckboxGroup,
  Button,
  CheckboxOptionType,
  InputNumber,
  Tabs,
} from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import {
  isPermissionGranted,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { listen } from "@tauri-apps/api/event";
import {
  getDevices,
  focusScrcpyWindow,
  openDeviceTerminal,
  startScrcpy,
  stopScrcpy,
} from "../commands";
import LogViewer from "./LogViewer.vue";
import DeviceList from "./DeviceList.vue";
const SettingsDialog = defineAsyncComponent(
  () => import("./SettingsDialog.vue")
);

const selectedDevices = useStorage<string[]>("selectedDevices", [], undefined, {
  mergeDefaults: true,
});
const selectedFPS = useStorage<number>("selectedFPS", 60, undefined, {
  mergeDefaults: true,
});
const selectedOptions = useStorage<string[]>(
  "selectedOptions",
  ["--turn-screen-off", "--show-touches"],
  undefined,
  {
    mergeDefaults: true,
  }
);
const osNotificationsEnabled = useStorage<boolean>(
  "osNotificationsEnabled",
  false,
  undefined,
  { mergeDefaults: true }
);
const availableDevices = ref<string[]>([]);
const startedDevices = ref<string[]>([]);
const settingsOpen = ref(false);

const maxLogLines = 1000;
const apkInstallRequestRe = /^INFO: Request to install (.+)$/;
const apkInstallSuccessRe = /^INFO:\s+(.+)\s+successfully installed$/;
const apkInstallFailedRe = /^ERROR: Failed to install (.+)$/;
const adbInstallFailedRe = /^adb: failed to install (.+?)(?:: (.+))?$/;
const pushRequestRe = /^INFO: Request to push (.+)$/;
const pushSuccessRe = /^INFO:\s+(.+)\s+successfully pushed to (.+)$/;
const pushFailedRe = /^ERROR: Failed to push (.+)$/;
const adbPushFailedRe = /^adb: failed to push (.+?)(?:: (.+))?$/;

// Log management
const systemLogLines = ref<string[]>([]);
const deviceLogLines = ref<Record<string, string[]>>({});
const activeLogTab = ref<string>("system");
const pendingApkInstallByDevice = ref<Record<string, string>>({});
const pendingApkFailureByDevice = ref<Record<string, string>>({});
const pendingPushByDevice = ref<Record<string, string>>({});
const pendingPushFailureByDevice = ref<Record<string, string>>({});
const notifiedInstallKeys = new Set<string>();

const trimLogLines = (lines: string[]): void => {
  if (lines.length > maxLogLines) {
    lines.splice(0, lines.length - maxLogLines);
  }
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

const sendOsNotification = async (title: string, body: string): Promise<void> => {
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

const refreshDevices = async (): Promise<void> => {
  try {
    const devices = await getDevices();
    availableDevices.value = devices;
  } catch (error) {
    appendSystemLog(`Failed to get connected devices: ${error}\n`);
  }
};

let deviceConnectedUnlisten: (() => void) | null = null;
let deviceDisconnectedUnlisten: (() => void) | null = null;
let scrcpyLogUnlisten: (() => void) | null = null;
let scrcpyExitUnlisten: (() => void) | null = null;
let appLogUnlisten: (() => void) | null = null;

interface LogPayload {
  deviceId: string;
  message: string;
}

onMounted(async () => {
  refreshDevices();

  // Listen for device connection events
  deviceConnectedUnlisten = await listen<string[]>(
    "device-connected",
    (event) => {
      const newDevices = event.payload;
      appendSystemLog(`Device(s) connected: ${newDevices.join(", ")}\n`);
      refreshDevices();
    }
  );

  // Listen for device disconnection events
  deviceDisconnectedUnlisten = await listen<string[]>(
    "device-disconnected",
    (event) => {
      const removedDevices = event.payload;
      appendSystemLog(`Device(s) disconnected: ${removedDevices.join(", ")}\n`);
      
      removedDevices.forEach((deviceId) => {
        // Remove from selected if disconnected
        const selectedIndex = selectedDevices.value.indexOf(deviceId);
        if (selectedIndex !== -1) {
          selectedDevices.value.splice(selectedIndex, 1);
        }

        // Attempt to stop scrcpy if it is still running.
        if (startedDevices.value.includes(deviceId)) {
          stopScrcpy(deviceId).catch((error) => {
            appendSystemLog(`Failed to stop scrcpy for ${deviceId}: ${error}\n`);
          });
          startedDevices.value = startedDevices.value.filter((id) => id !== deviceId);
        }

      });
      
      refreshDevices();
    }
  );

  // Listen for scrcpy logs from backend
  try {
    scrcpyLogUnlisten = await listen<LogPayload>(
      "scrcpy-log",
      (event) => {
        const { deviceId, message } = event.payload;
        handleApkInstallLog(deviceId, message);
        appendDeviceLog(deviceId, message);
      }
    );
  } catch (e) {
    appendSystemLog(`[Frontend] Error setting up log listener: ${e}\n`);
  }

  // Listen for scrcpy exit from backend
  scrcpyExitUnlisten = await listen<[string, number | null]>(
    "scrcpy-exit",
    (event) => {
      const [deviceId, exitCode] = event.payload;
      appendSystemLog(
        `Device ${deviceId} scrcpy exited with code ${exitCode ?? "null"}\n`
      );
      startedDevices.value = startedDevices.value.filter(id => id !== deviceId);
    }
  );

  appLogUnlisten = await listen<string>("app-log", (event) => {
    appendSystemLog(event.payload);
  });
});

onUnmounted(() => {
  if (deviceConnectedUnlisten) deviceConnectedUnlisten();
  if (deviceDisconnectedUnlisten) deviceDisconnectedUnlisten();
  if (scrcpyLogUnlisten) scrcpyLogUnlisten();
  if (scrcpyExitUnlisten) scrcpyExitUnlisten();
  if (appLogUnlisten) appLogUnlisten();
});

const availableOptions: CheckboxOptionType[] = [
  // --turn-screen-off
  { label: "Turn Screen Off", value: "--turn-screen-off" },
  // --show-touches
  { label: "Show Touches", value: "--show-touches" },
  // --always-on-top
  { label: "Always On Top", value: "--always-on-top" },
  // --stay-awake
  { label: "Stay Awake", value: "--stay-awake" },
];

const { TabPane } = Tabs;

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
const openTerminal = async (deviceId: string): Promise<void> => {
  try {
    await openDeviceTerminal(deviceId);
    appendSystemLog(`Opened terminal for ${deviceId}\n`);
  } catch (error) {
    appendSystemLog(`Failed to open terminal for ${deviceId}: ${error}\n`);
  }
};

const openLog = (deviceId: string): void => {
  activeLogTab.value = deviceId;
};

const focusDevice = async (deviceId: string): Promise<void> => {
  if (!startedDevices.value.includes(deviceId)) {
    appendSystemLog(`Device ${deviceId} is not running scrcpy.\n`);
    return;
  }
  try {
    await focusScrcpyWindow();
    appendSystemLog(`Focused scrcpy window for ${deviceId}\n`);
  } catch (error) {
    appendSystemLog(`Failed to focus scrcpy window: ${error}\n`);
  }
};

const openSettings = (): void => {
  settingsOpen.value = true;
};

const startDevice = async (deviceId: string): Promise<void> => {
  if (!availableDevices.value.includes(deviceId)) {
    appendSystemLog(`Device ${deviceId} is not available.\n`);
    return;
  }
  if (startedDevices.value.includes(deviceId)) {
    return;
  }

  const args = ["-s", deviceId]
    .concat(selectedOptions.value)
    .concat(["--max-fps", selectedFPS.value.toString()]);

  try {
    appendSystemLog(`[Frontend] Requesting start for ${deviceId}...\n`);
    await startScrcpy(deviceId, args);
    startedDevices.value.push(deviceId);
  } catch (error) {
    appendSystemLog(`Error starting ${deviceId}: ${error}\n`);
  }
};

const stopDevice = async (deviceId: string): Promise<void> => {
  if (!startedDevices.value.includes(deviceId)) {
    return;
  }
  try {
    await stopScrcpy(deviceId);
  } catch (error) {
    appendSystemLog(`Failed to stop scrcpy for ${deviceId}: ${error}\n`);
  } finally {
    startedDevices.value = startedDevices.value.filter((id) => id !== deviceId);
  }
};

const startProcess = async (): Promise<void> => {
  const toStart = selectedDevices.value.filter((deviceId) => {
    return availableDevices.value.includes(deviceId) && !startedDevices.value.includes(deviceId);
  });

  for (const deviceId of toStart) {
    await startDevice(deviceId);
  }
};

const stopProcesses = async (): Promise<void> => {
  for (const deviceId of [...startedDevices.value]) {
    await stopDevice(deviceId);
  }
};
</script>

<template>
  <div class="config-panel">
    <div class="config-column">
      <div class="config-container common-box">
        <div class="config-header">
          <h3>Configurations</h3>
          <Button size="small" @click="openSettings">Settings</Button>
        </div>
        <CheckboxGroup
          v-model:value="selectedOptions"
          name="selectedOptions"
          :options="availableOptions"
        />
        <div class="fps">
          <InputNumber
            placeholder="FPS"
            :min="0"
            v-model:value="selectedFPS"
            size="small"
          />
        </div>
        <div class="config-tools">
          <Button
            type="primary"
            @click="startProcess"
            :disabled="!selectedDevices.length"
            >Start</Button
          >
          <Button
            danger
            @click="stopProcesses"
            :disabled="!startedDevices.length"
            >Stop All</Button
          >
        </div>
      </div>
      <SettingsDialog v-model:open="settingsOpen" />
      <DeviceList
        :availableDevices="availableDevices"
        :startedDevices="startedDevices"
        v-model:selectedDevices="selectedDevices"
        @refresh="refreshDevices"
        @start="startDevice"
        @stop="stopDevice"
        @focus="focusDevice"
        @open-log="openLog"
        @open-terminal="openTerminal"
      />
    </div>
    <div class="log-column">
      <div class="common-box log-panel">
        <h3>Logs</h3>
        <Tabs v-model:activeKey="activeLogTab" class="log-tabs">
          <TabPane key="system" tab="System">
            <LogViewer :log-lines="systemLogLines" title="" />
          </TabPane>
          <TabPane v-for="deviceId in logDeviceIds" :key="deviceId" :tab="deviceId">
            <LogViewer :log-lines="deviceLogLines[deviceId] ?? []" title="" />
          </TabPane>
        </Tabs>
      </div>
    </div>
  </div>
</template>
<style lang="scss" scoped>
.config-panel {
  padding: 10px;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  gap: 10px;
  align-items: stretch;
}
.common-box {
  border: 1px solid #d9d9d9;
  padding: 16px;
}

.config-column {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.log-column {
  width: 100%;
  display: flex;
  flex-direction: column;
}

.log-panel {
  display: flex;
  flex-direction: column;
  min-height: 320px;
}

.log-tabs {
  flex: 1;
}
.log-tabs :deep(.ant-tabs-nav) {
  overflow-x: auto;
  overflow-y: hidden;
}
.log-tabs :deep(.ant-tabs-nav-list) {
  flex: 0 0 auto;
  white-space: nowrap;
}

.config-container {
  width: 100%;
  display: flex;
  flex-direction: column;
  .config-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .adb-path {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .path-row {
    display: flex;
    gap: 6px;
    align-items: center;
    :deep(.ant-input) {
      flex: 1;
    }
  }
  .config-tools {
    display: flex;
    justify-content: space-between;
  }
  > * {
    width: 100%;
    margin-bottom: 5px;
  }
  // reset the margin of the last element
  > *:last-child {
    margin-bottom: 0;
  }
}

h3 {
  margin-bottom: 10px;
}
</style>

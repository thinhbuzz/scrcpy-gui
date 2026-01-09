<script lang="ts" setup>
import { defineAsyncComponent, onMounted, onUnmounted, ref, watch } from "vue";
import {
  Badge,
  CheckboxGroup,
  Button,
  CheckboxOptionType,
  InputNumber,
  Modal,
  Tabs,
} from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import {
  getDevices,
  openDeviceTerminal,
  startDeviceMonitoring,
  startScrcpy,
  stopScrcpy,
  type DeviceInfo,
} from "../commands";
import DeviceList from "./DeviceList.vue";
import { useScrcpyLogs } from "../composables/useScrcpyLogs";
import { useToolPaths } from "../composables/useToolPaths";
import { useScrcpyListeners } from "../composables/useScrcpyListeners";
const SettingsDialog = defineAsyncComponent(
  () => import("./SettingsDialog.vue")
);
const UninstallDialog = defineAsyncComponent(
  () => import("./UninstallDialog.vue")
);
const LogViewer = defineAsyncComponent(
  () => import("./LogViewer.vue")
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
const availableDevices = ref<DeviceInfo[]>([]);
const startedDevices = ref<string[]>([]);
const settingsOpen = ref(false);
const uninstallOpen = ref(false);
const toolWarningOpen = ref(false);
const selectedUninstallDevice = ref<string>("");
const hasSeenToolWarning = useStorage<boolean>(
  "hasSeenToolWarning",
  false,
  undefined,
  { mergeDefaults: true }
);
const {
  systemLogLines,
  deviceLogLines,
  activeLogTab,
  logDeviceIds,
  clearAllLogs,
  appendSystemLog,
  handleScrcpyLog,
  setActiveLogTab,
} = useScrcpyLogs(osNotificationsEnabled, availableDevices);
const {
  toolPaths,
  refreshToolPaths,
  toolsMissing,
  scrcpyMissing,
  toolWarningTitle,
  toolWarningMessage,
} = useToolPaths(appendSystemLog);

const refreshDevices = async (): Promise<void> => {
  try {
    const devices = await getDevices();
    availableDevices.value = devices;
  } catch (error) {
    appendSystemLog(`Failed to get connected devices: ${error}\n`);
  }
};
const { setupListeners, cleanup } = useScrcpyListeners({
  selectedDevices,
  startedDevices,
  selectedUninstallDevice,
  uninstallOpen,
  appendSystemLog,
  refreshDevices,
  stopScrcpy,
  handleScrcpyLog,
});

const startMonitoringAfterPaint = (): void => {
  window.requestAnimationFrame(() => {
    setTimeout(() => {
      startDeviceMonitoring()
        .then(refreshDevices)
        .catch((error) => {
          appendSystemLog(`Failed to start device monitoring: ${error}\n`);
        });
    }, 0);
  });
};

onMounted(() => {
  void setupListeners();
  startMonitoringAfterPaint();
  void refreshToolPaths().then(() => {
    if (toolsMissing.value && !hasSeenToolWarning.value) {
      toolWarningOpen.value = true;
      hasSeenToolWarning.value = true;
    }
  });
});

onUnmounted(() => {
  cleanup();
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

const openTerminal = async (deviceId: string): Promise<void> => {
  try {
    await openDeviceTerminal(deviceId);
    appendSystemLog(`Opened terminal for ${deviceId}\n`);
  } catch (error) {
    appendSystemLog(`Failed to open terminal for ${deviceId}: ${error}\n`);
  }
};

const openLog = (deviceId: string): void => {
  setActiveLogTab(deviceId);
};

const openSettings = (): void => {
  settingsOpen.value = true;
};

const openSettingsFromWarning = (): void => {
  toolWarningOpen.value = false;
  settingsOpen.value = true;
};

const openUninstall = (deviceId?: string): void => {
  selectedUninstallDevice.value = deviceId ?? "";
  uninstallOpen.value = true;
};

const isAvailableDevice = (deviceId: string): boolean =>
  availableDevices.value.some((device) => device.id === deviceId);

const startDevice = async (deviceId: string): Promise<void> => {
  if (!isAvailableDevice(deviceId)) {
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
    await refreshToolPaths();
    if (scrcpyMissing.value && toolPaths.value?.adbPath) {
      appendSystemLog(
        "[Frontend] scrcpy not found. Configure scrcpy path in Settings.\n"
      );
      toolWarningOpen.value = true;
      return;
    }
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
    return isAvailableDevice(deviceId) && !startedDevices.value.includes(deviceId);
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

watch(
  () => settingsOpen.value,
  (value, previous) => {
    if (previous && !value) {
      void refreshToolPaths();
    }
  }
);
</script>

<template>
  <div class="config-panel">
    <div class="config-column">
      <div class="config-container common-box">
        <div class="config-header">
          <h3>Configurations</h3>
          <div class="header-actions">
            <Badge :dot="toolsMissing">
              <Button size="small" :danger="toolsMissing" @click="openSettings">
                Settings
              </Button>
            </Badge>
            <Button size="small" danger @click="openUninstall()">Uninstall</Button>
          </div>
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
      <Modal
        v-model:open="toolWarningOpen"
        :title="toolWarningTitle"
        ok-text="Open Settings"
        cancel-text="Later"
        @ok="openSettingsFromWarning"
      >
        <div class="tool-warning">
          {{ toolWarningMessage }}
        </div>
      </Modal>
      <SettingsDialog v-if="settingsOpen" v-model:open="settingsOpen" />
      <UninstallDialog
        v-if="uninstallOpen"
        v-model:open="uninstallOpen"
        :deviceId="selectedUninstallDevice"
      />
      <DeviceList
        :availableDevices="availableDevices"
        :startedDevices="startedDevices"
        v-model:selectedDevices="selectedDevices"
        @refresh="refreshDevices"
        @start="startDevice"
        @stop="stopDevice"
        @open-log="openLog"
        @open-terminal="openTerminal"
        @open-uninstall="openUninstall"
      />
    </div>
    <div class="log-column">
      <div class="common-box log-panel">
        <div class="log-header">
          <h3>Logs</h3>
          <Button size="small" @click="clearAllLogs">Clear All</Button>
        </div>
        <Tabs v-model:activeKey="activeLogTab" class="log-tabs">
          <TabPane key="system" tab="System">
            <LogViewer :log-lines="systemLogLines" title="" />
          </TabPane>
          <TabPane v-for="deviceId in logDeviceIds" :key="deviceId" :tab="deviceId">
            <LogViewer
              :log-lines="deviceLogLines[deviceId] ?? []"
              title=""
            />
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
.log-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
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
  .header-actions {
    display: flex;
    gap: 6px;
    align-items: center;
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

.tool-warning {
  color: #a61d24;
}

h3 {
  margin-bottom: 10px;
}
</style>

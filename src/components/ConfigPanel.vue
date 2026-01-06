<script lang="ts" setup>
import { onMounted, onUnmounted, ref } from "vue";
import {
  CheckboxGroup,
  Button,
  CheckboxOptionType,
  InputNumber,
} from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import { listen } from "@tauri-apps/api/event";
import { getDevices, startScrcpy, stopScrcpy } from "../commands";
import LogViewer from "./LogViewer.vue";
import DeviceList from "./DeviceList.vue";

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
const availableDevices = ref<string[]>([]);
const startedDevices = ref<string[]>([]);

// Log management
const logLines = ref<string[]>([]);
const writeLog = (line: string): void => {
  logLines.value.push(line);
  if (logLines.value.length > 1000) {
    logLines.value.splice(0, logLines.value.length - 1000);
  }
};

const refreshDevices = async (): Promise<void> => {
  try {
    const devices = await getDevices();
    availableDevices.value = devices;
  } catch (error) {
    writeLog(`Failed to get connected devices: ${error}\n`);
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
      writeLog(`Device(s) connected: ${newDevices.join(", ")}\n`);
      refreshDevices();
    }
  );

  // Listen for device disconnection events
  deviceDisconnectedUnlisten = await listen<string[]>(
    "device-disconnected",
    (event) => {
      const removedDevices = event.payload;
      writeLog(`Device(s) disconnected: ${removedDevices.join(", ")}\n`);
      
      removedDevices.forEach((deviceId) => {
        // Remove from selected if disconnected
        const selectedIndex = selectedDevices.value.indexOf(deviceId);
        if (selectedIndex !== -1) {
          selectedDevices.value.splice(selectedIndex, 1);
        }

        // Attempt to stop scrcpy if it is still running.
        if (startedDevices.value.includes(deviceId)) {
          stopScrcpy(deviceId).catch((error) => {
            writeLog(`Failed to stop scrcpy for ${deviceId}: ${error}\n`);
          });
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
        writeLog(`[${deviceId}] ${message}`);
      }
    );
  } catch (e) {
    writeLog(`[Frontend] Error setting up log listener: ${e}\n`);
  }

  // Listen for scrcpy exit from backend
  scrcpyExitUnlisten = await listen<[string, number | null]>(
    "scrcpy-exit",
    (event) => {
      const [deviceId, exitCode] = event.payload;
      writeLog(`Device ${deviceId} scrcpy exited with code ${exitCode ?? 'null'}\n`);
      startedDevices.value = startedDevices.value.filter(id => id !== deviceId);
    }
  );

  appLogUnlisten = await listen<string>("app-log", (event) => {
    writeLog(event.payload);
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

const startProcess = async (): Promise<void> => {
  const toStart = selectedDevices.value.filter((deviceId) => {
    return availableDevices.value.includes(deviceId) && !startedDevices.value.includes(deviceId);
  });

  for (const deviceId of toStart) {
    const args = ["-s", deviceId]
      .concat(selectedOptions.value)
      .concat(["--max-fps", selectedFPS.value.toString()]);
    
    try {
      writeLog(`[Frontend] Requesting start for ${deviceId}...\n`);
      await startScrcpy(deviceId, args);
      startedDevices.value.push(deviceId);
    } catch (error) {
      writeLog(`Error starting ${deviceId}: ${error}\n`);
    }
  }
};

const stopProcesses = async (): Promise<void> => {
  for (const deviceId of startedDevices.value) {
    try {
      await stopScrcpy(deviceId);
    } catch (error) {
      writeLog(`Failed to stop scrcpy for ${deviceId}: ${error}\n`);
    }
  }
  startedDevices.value = [];
};
</script>

<template>
  <div class="config-pannel">
    <LogViewer :log-lines="logLines" />
    <div class="config-column">
      <div class="config-container common-box flex-item">
        <h3>Configurations</h3>
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
      <DeviceList
        :availableDevices="availableDevices"
        v-model:selectedDevices="selectedDevices"
        @refresh="refreshDevices"
      />
    </div>
  </div>
</template>
<style lang="scss" scoped>
.config-pannel {
  padding: 10px;
  display: flex;
  flex-direction: row;
  flex-wrap: wrap-reverse;
  justify-content: space-between;
  gap: 10px;
  align-items: start;
}
.common-box {
  border: 1px solid #d9d9d9;
  padding: 16px;
  margin-bottom: 16px;
}
.flex-item {
  flex: 1;
}

.config-column {
  max-width: 600px;
}


.config-container {
  width: 100%;
  display: flex;
  flex-direction: column;
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

<script lang="ts" setup>
import { onMounted, onUnmounted, ref } from "vue";
import {
  CheckboxGroup,
  Button,
  CheckboxOptionType,
  InputNumber,
} from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import { type Child } from "@tauri-apps/plugin-shell";
import { listen } from "@tauri-apps/api/event";

import { initializePlatform, getDevices, startScrcpy } from "../commands";
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
const startedDevices = ref<{ deviceId: string; process: Child }[]>([]);

// Log management
const logLines = ref<string[]>([]);
const writeLog = (line: string): void => {
  logLines.value.push(line);
};

const refreshDevices = async (): Promise<void> => {
  const devices = await getDevices();
  availableDevices.value = devices;
  writeLog(`Refreshed device list. Found ${devices.length} device(s).\n`);
};

let deviceConnectedUnlisten: (() => void) | null = null;
let deviceDisconnectedUnlisten: (() => void) | null = null;

onMounted(async () => {
  await initializePlatform();
  refreshDevices();

  // Listen for device connection events
  deviceConnectedUnlisten = await listen<string[]>(
    "device-connected",
    (event) => {
      const newDevices = event.payload;
      writeLog(`Device(s) connected: ${newDevices.join(", ")}\n`);
      
      // Add new devices to the list
      newDevices.forEach((deviceId) => {
        if (availableDevices.value.indexOf(deviceId) === -1) {
          availableDevices.value.push(deviceId);
        }
      });
      
      // Refresh to ensure we have the latest state
      refreshDevices();
    }
  );

  // Listen for device disconnection events
  deviceDisconnectedUnlisten = await listen<string[]>(
    "device-disconnected",
    (event) => {
      const removedDevices = event.payload;
      writeLog(`Device(s) disconnected: ${removedDevices.join(", ")}\n`);
      
      // Remove disconnected devices from the list
      removedDevices.forEach((deviceId) => {
        const index = availableDevices.value.indexOf(deviceId);
        if (index !== -1) {
          availableDevices.value.splice(index, 1);
        }
        
        // Also remove from selected devices if selected
        const selectedIndex = selectedDevices.value.indexOf(deviceId);
        if (selectedIndex !== -1) {
          selectedDevices.value.splice(selectedIndex, 1);
        }
        
        // Stop scrcpy if it's running for this device
        const startedIndex = startedDevices.value.findIndex(
          (item) => item.deviceId === deviceId
        );
        if (startedIndex !== -1) {
          startedDevices.value[startedIndex].process.kill();
          startedDevices.value.splice(startedIndex, 1);
        }
      });
      
      // Refresh to ensure we have the latest state
      refreshDevices();
    }
  );
});

onUnmounted(() => {
  // Clean up event listeners
  if (deviceConnectedUnlisten) {
    deviceConnectedUnlisten();
  }
  if (deviceDisconnectedUnlisten) {
    deviceDisconnectedUnlisten();
  }
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
  await Promise.all(
    selectedDevices.value
      .filter((deviceId) => {
        return (
          availableDevices.value.indexOf(deviceId) !== -1 &&
          startedDevices.value.findIndex(
            (item) => item.deviceId === deviceId
          ) === -1
        );
      })
      .map((deviceId) => {
        return startScrcpy(
          ["-s", deviceId]
            .concat(selectedOptions.value)
            .concat(["--max-fps", selectedFPS.value.toString()]),
          writeLog,
          (data) => {
            writeLog(
              `Device ${deviceId} disconnected with code ${data.code ?? 'null'} and signal ${data.signal ?? 'null'}\n`
            );
            startedDevices.value = startedDevices.value.filter(
              (item) => item.deviceId !== deviceId
            );
          }
        ).then((child) => {
          startedDevices.value.push({ deviceId, process: child });
        });
      })
  );
};
const stopProcesses = async (): Promise<void> => {
  await Promise.all(
    startedDevices.value.map(({ process }) => {
      return process.kill();
    })
  );
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

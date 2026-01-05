<script lang="ts" setup>
import { VNodeRef, onBeforeMount, onMounted, ref } from "vue";
import {
  CheckboxGroup,
  Button,
  Textarea,
  CheckboxOptionType,
  InputNumber,
} from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import { Child } from "@tauri-apps/api/shell";

import { initializePlatform, getDevices, startScrcpy } from "../commands";
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

const logRef = ref<VNodeRef | undefined>(undefined);
const writeLog = (line: string): void => {
  if (logRef.value) {
    const textArea = (logRef.value as unknown as typeof Textarea)
      .resizableTextArea.textArea;
    textArea.value += line;
    textArea.scrollTop = textArea.scrollHeight;
  }
};
const refreshDevices = (): void => {
  availableDevices.value = [];
  getDevices((deviceId: string): void => {
    if (availableDevices.value.indexOf(deviceId) === -1) {
      availableDevices.value.push(deviceId);
    }
  }, writeLog);
};
const selectAllDevices = (isSelect: boolean): void => {
  selectedDevices.value = isSelect ? [...availableDevices.value] : [];
};
onMounted(async () => {
  await initializePlatform();
  refreshDevices();
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
              `Device ${deviceId} disconnected with code ${data.code} and signal ${data.signal}\n`
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
    <div class="log-container common-box flex-item">
      <h3>Logs</h3>
      <div class="log-scroller">
        <Textarea
          :rows="20"
          ref="logRef"
          :readonly="true"
          :autoSize="false"
        ></Textarea>
      </div>
    </div>
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
      <div class="device-container common-box">
        <div class="device-header">
          <h3>Devices</h3>
          <Button
            danger
            size="small"
            v-on:click="selectAllDevices(true)"
            :disabled="!availableDevices.length"
          >
            Select All
          </Button>
          <Button
            danger
            size="small"
            v-on:click="selectAllDevices(false)"
            :disabled="!availableDevices.length"
          >
            UnSelect All
          </Button>
          <Button type="primary" size="small" v-on:click="refreshDevices">
            Refresh
          </Button>
        </div>
        <CheckboxGroup
          v-model:value="selectedDevices"
          name="selectedDevices"
          :options="availableDevices"
          class="device-list vertical-checkbox-group"
        />
      </div>
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
.log-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  .log-scroller {
    flex-grow: 1;
    overflow-x: hidden;
    overflow-y: auto;
  }
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
.device-container {
  width: 100%;
  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
}
.vertical-checkbox-group {
  display: flex;
  flex-direction: row;
}
.vertical-checkbox-group {
  overflow-x: hidden;
  overflow-y: auto;
}
h3 {
  margin-bottom: 10px;
}
</style>

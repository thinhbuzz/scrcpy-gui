<script lang="ts" setup>
import { computed } from "vue";
import { Checkbox, Button } from "ant-design-vue";

const props = defineProps<{
  availableDevices: string[];
  selectedDevices: string[];
  startedDevices: string[];
}>();

const emit = defineEmits<{
  (e: "update:selectedDevices", value: string[]): void;
  (e: "refresh"): void;
  (e: "start", deviceId: string): void;
  (e: "stop", deviceId: string): void;
  (e: "open-log", deviceId: string): void;
  (e: "open-terminal", deviceId: string): void;
}>();

const selected = computed({
  get: () => props.selectedDevices,
  set: (val) => emit("update:selectedDevices", val),
});

const selectAllDevices = (isSelect: boolean): void => {
  selected.value = isSelect ? [...props.availableDevices] : [];
};

const toggleDeviceSelection = (deviceId: string, checked: boolean): void => {
  const next = new Set(selected.value);
  if (checked) {
    next.add(deviceId);
  } else {
    next.delete(deviceId);
  }
  selected.value = Array.from(next);
};
</script>

<template>
  <div class="device-container common-box">
    <div class="device-header">
      <h3>Devices</h3>
      <div class="header-buttons">
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
          <Button type="primary" size="small" v-on:click="emit('refresh')">
            Refresh
          </Button>
      </div>
    </div>
    <div class="device-list">
      <div v-if="!availableDevices.length" class="device-empty">
        No devices detected.
      </div>
      <div v-for="deviceId in availableDevices" :key="deviceId" class="device-row">
        <div class="device-info">
          <Checkbox
            :checked="selected.includes(deviceId)"
            @change="(event) => toggleDeviceSelection(deviceId, event.target.checked)"
          />
          <span class="device-id">{{ deviceId }}</span>
        </div>
        <div class="device-actions">
          <Button
            type="primary"
            size="small"
            :disabled="startedDevices.includes(deviceId)"
            @click="emit('start', deviceId)"
          >
            Start
          </Button>
          <Button
            danger
            size="small"
            :disabled="!startedDevices.includes(deviceId)"
            @click="emit('stop', deviceId)"
          >
            Stop
          </Button>
          <Button size="small" @click="emit('open-log', deviceId)">Logs</Button>
          <Button size="small" @click="emit('open-terminal', deviceId)">
            Terminal
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.device-container {
  width: 100%;
  max-width: 100%;
  .device-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    
    .header-buttons {
      display: flex;
      gap: 5px;
    }
  }
}
h3 {
  margin-bottom: 0; // Handled by header layout
}
.device-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 280px;
  overflow-y: auto;
}
.device-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 6px 8px;
  border: 1px solid #f0f0f0;
  border-radius: 6px;
}
.device-info {
  display: flex;
  align-items: center;
  gap: 8px;
}
.device-id {
  font-family: monospace;
}
.device-actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  justify-content: flex-end;
}
.device-empty {
  color: #666;
  font-style: italic;
}
</style>

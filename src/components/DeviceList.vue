<script lang="ts" setup>
import { computed } from "vue";
import { CheckboxGroup, Button } from "ant-design-vue";

const props = defineProps<{
  availableDevices: string[];
  selectedDevices: string[];
}>();

const emit = defineEmits<{
  (e: "update:selectedDevices", value: string[]): void;
  (e: "refresh"): void;
}>();

const selected = computed({
  get: () => props.selectedDevices,
  set: (val) => emit("update:selectedDevices", val),
});

const selectAllDevices = (isSelect: boolean): void => {
  selected.value = isSelect ? [...props.availableDevices] : [];
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
    <CheckboxGroup
      v-model:value="selected"
      name="selectedDevices"
      :options="availableDevices"
      class="device-list vertical-checkbox-group"
    />
  </div>
</template>

<style lang="scss" scoped>
.device-container {
  width: 100%;
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
.vertical-checkbox-group {
  display: flex;
  flex-direction: row; // Original was row, let's keep it.
  overflow-x: hidden;
  overflow-y: auto;
}
h3 {
  margin-bottom: 0; // Handled by header layout
}
</style>

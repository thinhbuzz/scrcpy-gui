<script setup lang="ts">
import { Button, Checkbox, Popconfirm } from "ant-design-vue";
import type { DeviceApp } from "../commands";

defineProps<{
  batchEnableTargets: DeviceApp[];
  batchDisableTargets: DeviceApp[];
  batchUninstallTargets: DeviceApp[];
  batchInstallTargets: DeviceApp[];
  batchRunning: boolean;
  filteredCount: number;
  selectedCount: number;
  allFilteredSelected: boolean;
  someFilteredSelected: boolean;
}>();

const emit = defineEmits<{
  (e: "select-all-filtered", selected: boolean): void;
  (e: "batch-enable"): void;
  (e: "batch-disable"): void;
  (e: "batch-uninstall"): void;
  (e: "batch-install"): void;
}>();
</script>

<template>
  <div class="bulk-actions">
    <div class="bulk-left">
      <Checkbox
        :checked="allFilteredSelected"
        :indeterminate="someFilteredSelected"
        @update:checked="emit('select-all-filtered', $event)"
      >
        Select all filtered ({{ filteredCount }})
      </Checkbox>
      <span class="bulk-count">Selected {{ selectedCount }}</span>
    </div>
    <div class="bulk-buttons">
      <Popconfirm
        title="Enable selected apps?"
        ok-text="Enable"
        cancel-text="Cancel"
        @confirm="emit('batch-enable')"
      >
        <Button
          size="small"
          :disabled="batchRunning || batchEnableTargets.length === 0"
          :loading="batchRunning && batchEnableTargets.length > 0"
        >
          Enable ({{ batchEnableTargets.length }})
        </Button>
      </Popconfirm>
      <Popconfirm
        title="Disable selected apps?"
        ok-text="Disable"
        cancel-text="Cancel"
        @confirm="emit('batch-disable')"
      >
        <Button
          size="small"
          :disabled="batchRunning || batchDisableTargets.length === 0"
          :loading="batchRunning && batchDisableTargets.length > 0"
        >
          Disable ({{ batchDisableTargets.length }})
        </Button>
      </Popconfirm>
      <Popconfirm
        title="Uninstall selected apps?"
        ok-text="Uninstall"
        cancel-text="Cancel"
        @confirm="emit('batch-uninstall')"
      >
        <Button
          size="small"
          danger
          :disabled="batchRunning || batchUninstallTargets.length === 0"
          :loading="batchRunning && batchUninstallTargets.length > 0"
        >
          Uninstall ({{ batchUninstallTargets.length }})
        </Button>
      </Popconfirm>
      <Popconfirm
        title="Install selected apps?"
        ok-text="Install"
        cancel-text="Cancel"
        @confirm="emit('batch-install')"
      >
        <Button
          size="small"
          type="primary"
          :disabled="batchRunning || batchInstallTargets.length === 0"
          :loading="batchRunning && batchInstallTargets.length > 0"
        >
          Install ({{ batchInstallTargets.length }})
        </Button>
      </Popconfirm>
    </div>
  </div>
</template>

<style scoped lang="scss">
.bulk-actions {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 8px 10px;
  border: 1px solid #ececec;
  border-radius: 8px;
  margin-bottom: 12px;
  background: #fafafa;
}

.bulk-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.bulk-count {
  font-size: 12px;
  color: #6d6d6d;
}

.bulk-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  justify-content: flex-end;
}

@media (max-width: 720px) {
  .bulk-actions {
    grid-template-columns: 1fr;
    align-items: stretch;
  }

  .bulk-buttons {
    justify-content: flex-start;
  }
}
</style>

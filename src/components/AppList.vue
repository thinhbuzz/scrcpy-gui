<script setup lang="ts">
import { Empty, Spin } from "ant-design-vue";
import type { DeviceApp } from "../commands";
import AppRow from "./AppRow.vue";

defineProps<{
  filteredApps: DeviceApp[];
  selectedPackages: Set<string>;
  uninstalling: Record<string, boolean>;
  installing: Record<string, boolean>;
  toggling: Record<string, boolean>;
  batchRunning: boolean;
  loading: boolean;
}>();

const emit = defineEmits<{
  (e: "select", packageName: string, checked: boolean): void;
  (e: "uninstall", app: DeviceApp): void;
  (e: "install", app: DeviceApp): void;
  (e: "toggle-enabled", app: DeviceApp): void;
}>();
</script>

<template>
  <Spin :spinning="loading">
    <div v-if="!filteredApps.length" class="empty-state">
      <Empty description="No apps found" />
    </div>
    <div v-else class="app-list">
      <AppRow
        v-for="app in filteredApps"
        :key="app.packageName"
        :app="app"
        :selected="selectedPackages.has(app.packageName)"
        :uninstalling="Boolean(uninstalling[app.packageName])"
        :installing="Boolean(installing[app.packageName])"
        :toggling="Boolean(toggling[app.packageName])"
        :batchRunning="batchRunning"
        @select="(checked: boolean) => emit('select', app.packageName, checked)"
        @uninstall="() => emit('uninstall', app)"
        @install="() => emit('install', app)"
        @toggle-enabled="() => emit('toggle-enabled', app)"
      />
    </div>
  </Spin>
</template>

<style scoped lang="scss">
.app-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 520px;
  overflow: auto;
  padding-right: 4px;
}

.empty-state {
  padding: 24px 0;
}
</style>

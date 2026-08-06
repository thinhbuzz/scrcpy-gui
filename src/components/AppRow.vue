<script setup lang="ts">
import { Button, Checkbox, Popconfirm, Tag } from "ant-design-vue";
import type { DeviceApp } from "../commands";
import { DEFAULT_APP_ICON_BASE64 } from "../constants";

defineProps<{
  app: DeviceApp;
  selected: boolean;
  uninstalling: boolean;
  installing: boolean;
  toggling: boolean;
  batchRunning: boolean;
}>();

const emit = defineEmits<{
  (e: "select", checked: boolean): void;
  (e: "uninstall"): void;
  (e: "install"): void;
  (e: "toggle-enabled"): void;
}>();

const appLabel = (app: DeviceApp): string => app.name || app.packageName;

const appIconSrc = (app: DeviceApp): string => {
  const icon = app.base64Icon?.trim() || DEFAULT_APP_ICON_BASE64;
  return `data:image/png;base64,${icon}`;
};
</script>

<template>
  <div class="app-row">
    <div class="app-select">
      <Checkbox
        :checked="selected"
        :disabled="batchRunning"
        :aria-label="`Select ${appLabel(app)}`"
        @update:checked="(checked: boolean) => emit('select', checked)"
      />
    </div>
    <div class="app-icon">
      <img :src="appIconSrc(app)" alt="" />
    </div>
    <div class="app-info">
      <div class="app-name">
        <span class="app-title">{{ appLabel(app) }}</span>
        <span class="app-tags">
          <Tag v-if="app.isSystemApp" color="geekblue">System</Tag>
          <Tag
            v-if="app.isInstalledForUser"
            :color="app.isDisabled ? 'red' : 'green'"
          >
            {{ app.isDisabled ? "Disabled" : "Enabled" }}
          </Tag>
        </span>
      </div>
      <div class="app-package">
        {{ app.packageName }} - {{ app.versionName }} ({{ app.versionCode }})
      </div>
    </div>
    <div class="app-actions">
      <Popconfirm
        v-if="app.isInstalledForUser"
        :title="
          app.isDisabled
            ? `Enable ${appLabel(app)}?`
            : `Disable ${appLabel(app)}?`
        "
        :ok-text="app.isDisabled ? 'Enable' : 'Disable'"
        cancel-text="Cancel"
        @confirm="() => emit('toggle-enabled')"
      >
        <Button
          size="small"
          :loading="toggling"
          :disabled="batchRunning"
        >
          {{ app.isDisabled ? "Enable" : "Disable" }}
        </Button>
      </Popconfirm>
      <Popconfirm
        v-if="app.isInstalledForUser"
        :title="`Uninstall ${appLabel(app)}?`"
        ok-text="Uninstall"
        cancel-text="Cancel"
        @confirm="() => emit('uninstall')"
      >
        <Button
          size="small"
          danger
          :loading="uninstalling"
          :disabled="batchRunning"
        >
          Uninstall
        </Button>
      </Popconfirm>
      <Popconfirm
        v-else
        :title="`Install ${appLabel(app)}?`"
        ok-text="Install"
        cancel-text="Cancel"
        @confirm="() => emit('install')"
      >
        <Button
          size="small"
          type="primary"
          :loading="installing"
          :disabled="batchRunning"
        >
          Install
        </Button>
      </Popconfirm>
    </div>
  </div>
</template>

<style scoped lang="scss">
.app-row {
  display: grid;
  grid-template-columns: auto auto 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 10px;
  border: 1px solid #ececec;
  border-radius: 8px;
}

.app-select {
  display: flex;
  align-items: center;
  justify-content: center;
}

.app-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f4f4f4;
  overflow: hidden;
}

.app-icon img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.app-info {
  min-width: 0;
}

.app-name {
  font-weight: 600;
  color: #1b1b1b;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.app-title {
  min-width: 0;
}

.app-tags {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.app-package {
  font-size: 12px;
  color: #6d6d6d;
  word-break: break-all;
}

.app-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}

@media (max-width: 720px) {
  .app-row {
    grid-template-columns: auto 1fr;
  }
}
</style>

<script setup lang="ts">
import { Button, Checkbox, Popconfirm, Tag } from "ant-design-vue";
import type { DeviceApp } from "../commands";

const props = defineProps<{
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

const defaultAppIcon =
  "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";

const appLabel = (app: DeviceApp): string => app.name || app.packageName;

const appIconSrc = (app: DeviceApp): string => {
  const icon = app.base64Icon?.trim() || defaultAppIcon;
  return `data:image/png;base64,${icon}`;
};
</script>

<template>
  <div class="app-row">
    <div class="app-select">
      <Checkbox
        :checked="selected"
        :disabled="batchRunning"
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

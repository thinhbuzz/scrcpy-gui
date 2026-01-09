<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Button,
  Empty,
  Input,
  Modal,
  Popconfirm,
  Select,
  Spin,
  Tag,
  message,
} from "ant-design-vue";
import {
  getDevices,
  installExistingPackage,
  listDeviceApps,
  setPackageEnabled,
  uninstallPackage,
  type DeviceApp,
} from "../commands";

const props = defineProps<{ open: boolean; deviceId?: string }>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const openModel = computed({
  get: () => props.open,
  set: (value: boolean) => emit("update:open", value),
});

const devices = ref<string[]>([]);
const selectedDeviceId = ref<string>("");
const apps = ref<DeviceApp[]>([]);
const loading = ref(false);
const uninstalling = ref<Record<string, boolean>>({});
const installing = ref<Record<string, boolean>>({});
const toggling = ref<Record<string, boolean>>({});
const searchTerm = ref("");
const systemFilter = ref<"all" | "system" | "user">("all");

const deviceOptions = computed(() =>
  devices.value.map((deviceId) => ({ value: deviceId, label: deviceId }))
);

const defaultAppIcon =
  "iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAAGYktHRAD/AP8A/6C9p5MAAAAHdElNRQfqAQgFGCgyumFTAAAHr0lEQVRYw8WXza8mRRXGf+dUdfX73k8HRnBGMSBEEmRAQGfhhkQzYUMC0YVuXJLgf2Bi3OPSFSEuDHHjhsgCiDHERIkhccGHEENUIKDj8DGXYe7c96O765zjovvOvSOzNLGSfrv6PdWnq845z1NPwf+5yWHnweeeIN/bI5cz1oCKkkQYwgigJTNgOIEiJJSKAdBIpoYRBILQkOioABQShuMRpAFit1JfL/zh4R8fTeDB556gvVMQhCqOuKUAOoyGNA6sgStoSkQEWJDyaAsPAkM14+7ggScIAsxRSYhCkmRag0jC6s2Bl777UzJAto9QvxmSnGg1P4am+7Mk3RVl5QNBsFVaVjFgYWQSpWSW3gOwWVp6rwxh5JSZacOBd6OtaRnCqJiDvOLuv6Cvlz58/kWAcQI63+LgJtjd08cW1v3MCYooM21Y1Y5AyE1iXXuqG0UzASzr+JEciXUM9D6QZYzK+B7krKx9oHcjhX5/YyW6e+/ZJy6cfg0ABdh/9zz5qbcyZt90AgvHIvBwahiG4eE4geFUnAAMH/N7+A6GRxARGOPl4XiM40wcDz/78qM/adbd4igC68sH1KXJdkgqkjGMIokimUYbBCjSYBKgQtFMkUwreSw0yTgG0lBoKNJQZJhsDS4QU+EqprGTJMSPJiAIFacRxVXwEIpkijbMwhCCVjKhjoZSJDGTTKcNAK1miEBQColWM200QNDqeJeARCIJVHea4GgCFk4OofPKQR2oGK1khjD2bYkECMrSenoGimRqtOzbaoKSsrKenkohUTGu1CUhI7LW3tNFJZOYeSDhOH5UA58lB5l+9WofYlrJ0X8SMvWPGCWIqz4kjvkMuer5eMsASZSIyjwVNDcYTpFEK2UqseCO9gvUcN4ZPqaQmGszlSRspxlZlD4G7mxOIwpvcQFB2E5ziiR6NRJKThUXQUWPIuDhIDBYZWU9S+tZWc/aOtbWs7aBpXd8ffZl8GBha8yMbEpjipmztI7kifvmt7GwjpX1rLxn7QMrH64+91ZRkZHMDiMQACJUjOoDA45ooKL0VCSE88MnnE838NDWGebScPfGLWxKIYBF9Ly5/Cc1jPf6j/nXsIeFQcCglSEqPZUUiRSByn+l4DCrSZRGM4KPKJBMIYPASd3hzOxLPLR7L3NpUJFp5mP+v7VxB6sY+O3l1/l7d4G1V5QRTS5OxOg/y2H5HZ+AKipCIxlLSo6gSGauhV4rn9ctHj/5bc5ufoVEwiPwOFZhU3dDC4+euJ9T5XM8efH3XLR9ZlpABJFEEiGnSkQgIkc1UH0M18p7rtQV+7biii3ZtyVr63lk9wHObt7Ofrfinf0PsDi+hrE5zrsHH7Dfrzm7eTuP7NxHZwP7tmS/Lti3Bft1xcp60rEauC4Mj5wGD8xv5dzO3SRRXvnobzz52rOxsjVyDGOCsKgdT/31BV795G0SyrmdM5zJXyTcrwO+z8BQMJy5FlJqMIIiiU2dcW73HrZ1RrjxjZvv5Lbd0zLPM67NQLCZZjx+18Pslg08nJ005zsbd3H+4h7tZqGNZoShVvFjKcgjDAMNGKKych83Fkmc1C2+2p4CHUlnt91kt92EGNN+SE8CqAi3bt901YbAXVu30LwXXLIDYiOhorTuIVwHhoJQI+ijTvQSLKzjmXf+yLxXmpSpE88lEZIqPQYitJPNidGWRoW0sI4VA+vFkiDTzFvypKiuSYFMgcyiNJLRMIpkFtLz7PIVVu/vMauZTgyToKRMmxsW0aMqbJQZfThVnEYT86ZlaR2IsNtuUSTTd4ZmJ5EIlavUPcJQlAgokiSSYpEommmlsHNil61mjl84QNdrXIKSGkrTgCdQ2EpzOiqmjBNI7SjdVJg1BSGRxUiDIkNgAtmOpcDCyQIrH+Kg1lEPeGamxmVbkebCxuktun8v6buOGk4NZ0GPoCiJTioDTkPGBA6im+Iq9ExyzRPtEFjvmNl1YDiRW8TRrgYQEeStltmpE+gsEwIiE38KE9EIHG4yqoSO3q+O0dGOCMchdBWGTjAjQ8r4RMUzbSapDVs6Q7cTG1oY9hYUUzTGj203Ywp6cYpmNnILIYgK23lOkYYqTpJEGiqhgapeiwIdzwDSuY3qXxwBukn5tpLprGeYBXJyjl3qWPcVEaHQ0IuNkl6EXpw+KoIwYPQYgxhZgyJO1ozoMR4YFmuaTwdqmI8wHIGpYXRhCNCH0U+7Wts2pBs3sYtLwsbqHzBqBOBkxmeJ8ZxRdXwOIIuELioxHFNEH774Jq/9/PnBP+3eUB93rcMri05aTlHVMYwIeVaY37BD0xZSSuOVp7sqKSkpJTSlo74Lw97yjfd/8+f+0sv/OIrA3p/e5ke8wLtPv/T0zee+tpNPbN6jmsQlkXxAANceDSMzik/Xjmw90hmxXiAYKo5qA2kgSYcgeHagkokYPln+5cLv3vjlD+PX/Ep+cFj3IHMhVocEylZzYmNbcpIjoj0E1HU2lWuqOo5GTTx96MHNYvhkeQU4AOLwm+PYoqAQ7pQbtzj1vfspN2yjAZVD/a74RLcaQpLpcOpBcsFj1I4ypa7KSOdJMi5Bd+kKHzzzKv3eAaIKDtH70ZIkKzQTJCoco4H/TZMp4QIMEHVc2H8AUdM3nuP0qQ0AAAAldEVYdGRhdGU6Y3JlYXRlADIwMjYtMDEtMDhUMDU6MjQ6MzQrMDA6MDDs9Ko9AAAAJXRFWHRkYXRlOm1vZGlmeQAyMDI2LTAxLTA4VDA1OjI0OjM0KzAwOjAwnakSgQAAACh0RVh0ZGF0ZTp0aW1lc3RhbXAAMjAyNi0wMS0wOFQwNToyNDo0MCswMDowMDQ2HlQAAAAASUVORK5CYII=";

const appLabel = (app: DeviceApp): string => app.name || app.packageName;

const appIconSrc = (app: DeviceApp): string => {
  const icon = app.base64Icon?.trim() || defaultAppIcon;
  return `data:image/png;base64,${icon}`;
};

const filteredApps = computed(() => {
  const term = searchTerm.value.trim().toLowerCase();
  return apps.value.filter((app) => {
    if (systemFilter.value === "system" && !app.isSystemApp) {
      return false;
    }
    if (systemFilter.value === "user" && app.isSystemApp) {
      return false;
    }
    if (!term) {
      return true;
    }
    return (
      appLabel(app).toLowerCase().includes(term) ||
      app.packageName.toLowerCase().includes(term)
    );
  });
});

const refreshDevices = async (): Promise<void> => {
  try {
    devices.value = await getDevices();
    const preferred = props.deviceId?.trim();
    if (preferred && devices.value.includes(preferred)) {
      selectedDeviceId.value = preferred;
    } else if (!selectedDeviceId.value || !devices.value.includes(selectedDeviceId.value)) {
      selectedDeviceId.value = devices.value[0] ?? "";
    }
  } catch (error) {
    message.error(`Failed to read devices: ${error}`);
    devices.value = [];
    selectedDeviceId.value = "";
  }
};

const refreshApps = async (): Promise<void> => {
  if (!selectedDeviceId.value) {
    apps.value = [];
    return;
  }
  loading.value = true;
  try {
    apps.value = await listDeviceApps(selectedDeviceId.value);
  } catch (error) {
    apps.value = [];
    message.error(`Failed to load apps: ${error}`);
  } finally {
    loading.value = false;
  }
};

const uninstallApp = async (app: DeviceApp): Promise<void> => {
  if (uninstalling.value[app.packageName]) {
    return;
  }
  uninstalling.value = {
    ...uninstalling.value,
    [app.packageName]: true,
  };
  try {
    await uninstallPackage(selectedDeviceId.value, app.packageName, app.isSystemApp);
    message.success(`Uninstalled ${appLabel(app)}`);
    await refreshApps();
  } catch (error) {
    message.error(`Failed to uninstall ${appLabel(app)}: ${error}`);
  } finally {
    uninstalling.value = {
      ...uninstalling.value,
      [app.packageName]: false,
    };
  }
};

const installApp = async (app: DeviceApp): Promise<void> => {
  if (installing.value[app.packageName]) {
    return;
  }
  installing.value = {
    ...installing.value,
    [app.packageName]: true,
  };
  try {
    await installExistingPackage(selectedDeviceId.value, app.packageName);
    message.success(`Installed ${appLabel(app)}`);
    await refreshApps();
  } catch (error) {
    message.error(`Failed to install ${appLabel(app)}: ${error}`);
  } finally {
    installing.value = {
      ...installing.value,
      [app.packageName]: false,
    };
  }
};

const toggleAppEnabled = async (
  app: DeviceApp,
  enabled: boolean
): Promise<void> => {
  if (toggling.value[app.packageName]) {
    return;
  }
  toggling.value = {
    ...toggling.value,
    [app.packageName]: true,
  };
  try {
    await setPackageEnabled(selectedDeviceId.value, app.packageName, enabled);
    message.success(
      `${enabled ? "Enabled" : "Disabled"} ${appLabel(app)}`
    );
    await refreshApps();
  } catch (error) {
    message.error(
      `Failed to ${enabled ? "enable" : "disable"} ${appLabel(app)}: ${error}`
    );
  } finally {
    toggling.value = {
      ...toggling.value,
      [app.packageName]: false,
    };
  }
};

watch(
  () => props.open,
  (value) => {
    if (value) {
      refreshDevices().then(() => refreshApps());
    }
  },
  { immediate: true }
);

watch(
  () => props.deviceId,
  (value) => {
    if (!value || !openModel.value) {
      return;
    }
    if (devices.value.includes(value)) {
      selectedDeviceId.value = value;
    }
  }
);

watch(
  () => selectedDeviceId.value,
  () => {
    if (openModel.value) {
      refreshApps();
    }
  }
);
</script>

<template>
  <Modal v-model:open="openModel" title="Uninstall Apps" :footer="null" width="760">
    <div class="toolbar">
      <Select
        v-model:value="selectedDeviceId"
        :options="deviceOptions"
        placeholder="Select device"
        size="small"
        class="device-select"
      />
      <Select
        v-model:value="systemFilter"
        size="small"
        class="filter-select"
        :options="[
          { value: 'all', label: 'All apps' },
          { value: 'system', label: 'System apps' },
          { value: 'user', label: 'User apps' },
        ]"
      />
      <Input
        v-model:value="searchTerm"
        placeholder="Search by app name or package"
        size="small"
        allow-clear
      />
      <Button size="small" @click="refreshDevices().then(() => refreshApps())">
        Refresh
      </Button>
    </div>

    <Spin :spinning="loading">
      <div v-if="!filteredApps.length" class="empty-state">
        <Empty description="No apps found" />
      </div>
      <div v-else class="app-list">
        <div v-for="app in filteredApps" :key="app.packageName" class="app-row">
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
            <div class="app-package">{{ app.packageName }} - {{ app.versionName }} ({{ app.versionCode }})</div>
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
              @confirm="() => toggleAppEnabled(app, app.isDisabled)"
            >
              <Button
                size="small"
                :loading="Boolean(toggling[app.packageName])"
              >
                {{ app.isDisabled ? "Enable" : "Disable" }}
              </Button>
            </Popconfirm>
            <Popconfirm
              v-if="app.isInstalledForUser"
              :title="`Uninstall ${appLabel(app)}?`"
              ok-text="Uninstall"
              cancel-text="Cancel"
              @confirm="() => uninstallApp(app)"
            >
              <Button
                size="small"
                danger
                :loading="Boolean(uninstalling[app.packageName])"
              >
                Uninstall
              </Button>
            </Popconfirm>
            <Popconfirm
              v-else
              :title="`Install ${appLabel(app)}?`"
              ok-text="Install"
              cancel-text="Cancel"
              @confirm="() => installApp(app)"
            >
              <Button
                size="small"
                type="primary"
                :loading="Boolean(installing[app.packageName])"
              >
                Install
              </Button>
            </Popconfirm>
          </div>
        </div>
      </div>
    </Spin>
  </Modal>
</template>

<style scoped lang="scss">
.toolbar {
  display: grid;
  grid-template-columns: 1.2fr 0.9fr 2fr auto;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.device-select,
.filter-select {
  width: 100%;
}

.app-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 520px;
  overflow: auto;
  padding-right: 4px;
}

.app-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 10px;
  border: 1px solid #ececec;
  border-radius: 8px;
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

.empty-state {
  padding: 24px 0;
}

@media (max-width: 720px) {
  .toolbar {
    grid-template-columns: 1fr;
  }

  .app-row {
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto auto;
  }

  .app-status,
  .app-actions {
    justify-content: flex-start;
  }
}
</style>

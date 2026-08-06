<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Button, Input, Modal, Select, message } from "ant-design-vue";
import {
  getDevices,
  installExistingPackage,
  listDeviceApps,
  setPackageEnabled,
  uninstallPackage,
  type DeviceApp,
  type DeviceInfo,
} from "../commands";
import AppList from "./AppList.vue";
import BatchActions from "./BatchActions.vue";

const props = defineProps<{ open: boolean; deviceId?: string }>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const openModel = computed({
  get: () => props.open,
  set: (value: boolean) => emit("update:open", value),
});

const devices = ref<DeviceInfo[]>([]);
const selectedDeviceId = ref<string>("");
const apps = ref<DeviceApp[]>([]);
const loading = ref(false);
const uninstalling = ref<Record<string, boolean>>({});
const installing = ref<Record<string, boolean>>({});
const toggling = ref<Record<string, boolean>>({});
const batchRunning = ref(false);
const searchTerm = ref("");
const systemFilter = ref<"all" | "system" | "user">("all");
const selectedPackages = ref<Set<string>>(new Set());
// Request sequencing to prevent stale responses from overwriting newer ones
let appsRequestSeq = 0;

const appLabel = (app: DeviceApp): string => app.name || app.packageName;

const deviceOptions = computed(() =>
  devices.value.map((device) => ({ value: device.id, label: device.label }))
);

const filteredApps = computed(() => {
  const term = searchTerm.value.trim().toLowerCase();
  return apps.value.filter((app) => {
    if (systemFilter.value === "system" && !app.isSystemApp) return false;
    if (systemFilter.value === "user" && app.isSystemApp) return false;
    if (!term) return true;
    return (
      appLabel(app).toLowerCase().includes(term) ||
      app.packageName.toLowerCase().includes(term)
    );
  });
});

const selectedApps = computed(() =>
  apps.value.filter((app) => selectedPackages.value.has(app.packageName))
);

const selectedCount = computed(() => selectedApps.value.length);

const filteredSelectedCount = computed(
  () =>
    filteredApps.value.filter((app) => selectedPackages.value.has(app.packageName))
      .length
);

const allFilteredSelected = computed(
  () =>
    filteredApps.value.length > 0 &&
    filteredSelectedCount.value === filteredApps.value.length
);

const someFilteredSelected = computed(
  () => filteredSelectedCount.value > 0 && !allFilteredSelected.value
);

const batchDisableTargets = computed(() =>
  selectedApps.value.filter((app) => app.isInstalledForUser && !app.isDisabled)
);
const batchEnableTargets = computed(() =>
  selectedApps.value.filter((app) => app.isInstalledForUser && app.isDisabled)
);
const batchUninstallTargets = computed(() =>
  selectedApps.value.filter((app) => app.isInstalledForUser)
);
const batchInstallTargets = computed(() =>
  selectedApps.value.filter((app) => !app.isInstalledForUser)
);

// Selection helpers
const updateSelected = (updater: (next: Set<string>) => void): void => {
  const next = new Set(selectedPackages.value);
  updater(next);
  selectedPackages.value = next;
};

const setSelected = (packageName: string, selected: boolean): void => {
  updateSelected((next) => {
    if (selected) next.add(packageName);
    else next.delete(packageName);
  });
};

const toggleSelectAllFiltered = (selected: boolean): void => {
  updateSelected((next) => {
    for (const app of filteredApps.value) {
      if (selected) next.add(app.packageName);
      else next.delete(app.packageName);
    }
  });
};

const pruneSelection = (): void => {
  updateSelected((next) => {
    const available = new Set(apps.value.map((app) => app.packageName));
    for (const pkg of next) {
      if (!available.has(pkg)) next.delete(pkg);
    }
  });
};

const clearSelection = (): void => {
  selectedPackages.value = new Set();
};

// Data fetching
const refreshDevices = async (): Promise<void> => {
  try {
    devices.value = await getDevices();
    const preferred = props.deviceId?.trim();
    if (preferred && devices.value.some((d) => d.id === preferred)) {
      selectedDeviceId.value = preferred;
    } else if (
      !selectedDeviceId.value ||
      !devices.value.some((d) => d.id === selectedDeviceId.value)
    ) {
      selectedDeviceId.value = devices.value[0]?.id ?? "";
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
    clearSelection();
    return;
  }
  const seq = ++appsRequestSeq;
  const deviceId = selectedDeviceId.value;
  loading.value = true;
  try {
    const result = await listDeviceApps(deviceId);
    // Only apply the result if this is still the latest request and the
    // device selection hasn't changed while we were fetching.
    if (seq === appsRequestSeq && selectedDeviceId.value === deviceId) {
      apps.value = result;
      pruneSelection();
    }
  } catch (error) {
    if (seq === appsRequestSeq && selectedDeviceId.value === deviceId) {
      apps.value = [];
      clearSelection();
      message.error(`Failed to load apps: ${error}`);
    }
  } finally {
    if (seq === appsRequestSeq && selectedDeviceId.value === deviceId) {
      loading.value = false;
    }
  }
};

// Individual actions
const uninstallApp = async (app: DeviceApp): Promise<void> => {
  if (uninstalling.value[app.packageName]) return;
  uninstalling.value = { ...uninstalling.value, [app.packageName]: true };
  try {
    await uninstallPackage(
      selectedDeviceId.value,
      app.packageName,
      app.isSystemApp
    );
    message.success(`Uninstalled ${appLabel(app)}`);
    await refreshApps();
  } catch (error) {
    message.error(`Failed to uninstall ${appLabel(app)}: ${error}`);
  } finally {
    uninstalling.value = { ...uninstalling.value, [app.packageName]: false };
  }
};

const installApp = async (app: DeviceApp): Promise<void> => {
  if (installing.value[app.packageName]) return;
  installing.value = { ...installing.value, [app.packageName]: true };
  try {
    await installExistingPackage(selectedDeviceId.value, app.packageName);
    message.success(`Installed ${appLabel(app)}`);
    await refreshApps();
  } catch (error) {
    message.error(`Failed to install ${appLabel(app)}: ${error}`);
  } finally {
    installing.value = { ...installing.value, [app.packageName]: false };
  }
};

const toggleAppEnabled = async (
  app: DeviceApp,
  enabled: boolean
): Promise<void> => {
  if (toggling.value[app.packageName]) return;
  toggling.value = { ...toggling.value, [app.packageName]: true };
  try {
    await setPackageEnabled(selectedDeviceId.value, app.packageName, enabled);
    message.success(`${enabled ? "Enabled" : "Disabled"} ${appLabel(app)}`);
    await refreshApps();
  } catch (error) {
    message.error(
      `Failed to ${enabled ? "enable" : "disable"} ${appLabel(app)}: ${error}`
    );
  } finally {
    toggling.value = { ...toggling.value, [app.packageName]: false };
  }
};

// Batch helpers
const setBulkBusy = (
  store: typeof uninstalling | typeof installing | typeof toggling,
  appList: DeviceApp[],
  busy: boolean
): void => {
  const next = { ...store.value };
  for (const app of appList) next[app.packageName] = busy;
  store.value = next;
};

const runBatch = async (
  appList: DeviceApp[],
  store: typeof uninstalling | typeof installing | typeof toggling,
  action: (app: DeviceApp) => Promise<void>,
  successLabel: string,
  failureLabel: string
): Promise<void> => {
  if (!appList.length || batchRunning.value) return;
  batchRunning.value = true;
  setBulkBusy(store, appList, true);
  const failures: Array<{ app: DeviceApp; error: unknown }> = [];
  for (const app of appList) {
    try {
      await action(app);
    } catch (error) {
      failures.push({ app, error });
    }
  }
  setBulkBusy(store, appList, false);
  if (failures.length === 0) {
    message.success(`${successLabel} ${appList.length} apps`);
  } else if (failures.length === appList.length) {
    message.error(`${failureLabel} ${appList.length} apps failed`);
  } else {
    message.warning(
      `${successLabel} ${appList.length - failures.length} apps, ` +
        `failed to ${failureLabel.toLowerCase()} ${failures.length} apps`
    );
  }
  await refreshApps();
  clearSelection();
  batchRunning.value = false;
};

const batchDisable = () =>
  runBatch(
    batchDisableTargets.value,
    toggling,
    (app) => setPackageEnabled(selectedDeviceId.value, app.packageName, false),
    "Disabled",
    "Disable"
  );
const batchEnable = () =>
  runBatch(
    batchEnableTargets.value,
    toggling,
    (app) => setPackageEnabled(selectedDeviceId.value, app.packageName, true),
    "Enabled",
    "Enable"
  );
const batchUninstall = () =>
  runBatch(
    batchUninstallTargets.value,
    uninstalling,
    (app) =>
      uninstallPackage(selectedDeviceId.value, app.packageName, app.isSystemApp),
    "Uninstalled",
    "Uninstall"
  );
const batchInstall = () =>
  runBatch(
    batchInstallTargets.value,
    installing,
    (app) => installExistingPackage(selectedDeviceId.value, app.packageName),
    "Installed",
    "Install"
  );

// Watchers
watch(
  () => props.open,
  (value) => {
    if (value) {
      clearSelection();
      void refreshDevices();
      // refreshApps() is triggered by the selectedDeviceId watcher below,
      // which fires when refreshDevices sets the default device.
    }
  },
  { immediate: true }
);

watch(
  () => props.deviceId,
  (value) => {
    if (!value || !openModel.value) return;
    if (devices.value.some((d) => d.id === value)) {
      selectedDeviceId.value = value;
    }
  }
);

watch(
  () => selectedDeviceId.value,
  (newVal, oldVal) => {
    if (openModel.value && newVal !== oldVal) {
      clearSelection();
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
        :disabled="batchRunning"
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
        :disabled="batchRunning"
      />
      <Input
        v-model:value="searchTerm"
        placeholder="Search by app name or package"
        size="small"
        allow-clear
        :disabled="batchRunning"
      />
      <Button size="small" :disabled="batchRunning" @click="refreshDevices().then(() => refreshApps())">
        Refresh
      </Button>
    </div>

    <BatchActions
      :batch-enable-targets="batchEnableTargets"
      :batch-disable-targets="batchDisableTargets"
      :batch-uninstall-targets="batchUninstallTargets"
      :batch-install-targets="batchInstallTargets"
      :batch-running="batchRunning"
      :filtered-count="filteredApps.length"
      :selected-count="selectedCount"
      :all-filtered-selected="allFilteredSelected"
      :some-filtered-selected="someFilteredSelected"
      @select-all-filtered="toggleSelectAllFiltered"
      @batch-enable="batchEnable"
      @batch-disable="batchDisable"
      @batch-uninstall="batchUninstall"
      @batch-install="batchInstall"
    />

    <AppList
      :filtered-apps="filteredApps"
      :selected-packages="selectedPackages"
      :uninstalling="uninstalling"
      :installing="installing"
      :toggling="toggling"
      :batch-running="batchRunning"
      :loading="loading"
      @select="setSelected"
      @uninstall="uninstallApp"
      @install="installApp"
      @toggle-enabled="(app) => toggleAppEnabled(app, app.isDisabled)"
    />
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

@media (max-width: 720px) {
  .toolbar {
    grid-template-columns: 1fr;
  }
}
</style>

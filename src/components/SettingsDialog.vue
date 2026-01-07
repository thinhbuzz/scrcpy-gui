<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { Button, Input, Modal, Switch } from "ant-design-vue";
import { useStorage } from "@vueuse/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import {
  downloadAndInstallScrcpy,
  getToolPaths,
  setAdbPath,
  setScrcpyPath,
} from "../commands";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
}>();

const openModel = computed({
  get: () => props.open,
  set: (value: boolean) => emit("update:open", value),
});

const adbPath = useStorage<string>("adbPath", "", undefined, {
  mergeDefaults: true,
});
const scrcpyPath = useStorage<string>("scrcpyPath", "", undefined, {
  mergeDefaults: true,
});
const osNotificationsEnabled = useStorage<boolean>(
  "osNotificationsEnabled",
  false,
  undefined,
  { mergeDefaults: true }
);

const toolPathsLoaded = ref(false);
const isDownloadingScrcpy = ref(false);
const permissionGranted = ref<boolean | null>(null);
const permissionNote = ref<string>("");
const isNotificationBusy = ref(false);

const refreshPermission = async (): Promise<void> => {
  try {
    const granted = await isPermissionGranted();
    permissionGranted.value = granted;
  } catch (error) {
    permissionNote.value = `Failed to read permission: ${error}`;
  }
};

const toggleNotifications = async (checked: boolean): Promise<void> => {
  isNotificationBusy.value = true;
  permissionNote.value = "";
  try {
    if (!checked) {
      osNotificationsEnabled.value = false;
      return;
    }
    let granted = await isPermissionGranted();
    if (!granted) {
      const result = await requestPermission();
      granted = result === "granted";
    }
    permissionGranted.value = granted;
    if (!granted) {
      osNotificationsEnabled.value = false;
      permissionNote.value =
        "Notification permission denied. Enable it in system settings.";
      return;
    }
    osNotificationsEnabled.value = true;
  } catch (error) {
    osNotificationsEnabled.value = false;
    permissionNote.value = `Failed to request permission: ${error}`;
  } finally {
    isNotificationBusy.value = false;
  }
};

const sendTestNotification = async (): Promise<void> => {
  isNotificationBusy.value = true;
  permissionNote.value = "";
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const result = await requestPermission();
      granted = result === "granted";
    }
    permissionGranted.value = granted;
    if (!granted) {
      permissionNote.value =
        "Notification permission denied. Enable it in system settings.";
      return;
    }
    const sendPromise = sendNotification({
      title: "Scrcpy GUI",
      body: "This is a test notification.",
    });
    const timeoutMs = 3000;
    const timeoutPromise = new Promise<"timeout">((resolve) => {
      setTimeout(() => resolve("timeout"), timeoutMs);
    });
    const result = await Promise.race([sendPromise, timeoutPromise]);
    if (result === "timeout") {
      return;
    }
  } catch (error) {
    permissionNote.value = `Failed to send test notification: ${error}`;
  } finally {
    isNotificationBusy.value = false;
  }
};

const pickPath = async (label: string): Promise<string | null> => {
  const selected = await open({
    multiple: false,
    directory: false,
    title: `Select ${label} binary`,
  });
  if (typeof selected === "string") {
    return selected;
  }
  return null;
};

const pickAdbPath = async (): Promise<void> => {
  const selected = await pickPath("adb");
  if (selected) {
    adbPath.value = selected;
  }
};

const pickScrcpyPath = async (): Promise<void> => {
  const selected = await pickPath("scrcpy");
  if (selected) {
    scrcpyPath.value = selected;
  }
};

const syncToolPaths = async (): Promise<void> => {
  try {
    const adbValue = adbPath.value.trim();
    const scrcpyValue = scrcpyPath.value.trim();
    await setAdbPath(adbValue.length > 0 ? adbValue : null);
    await setScrcpyPath(scrcpyValue.length > 0 ? scrcpyValue : null);
  } catch (error) {
  }
};

const downloadScrcpy = async (): Promise<void> => {
  if (isDownloadingScrcpy.value) {
    return;
  }
  isDownloadingScrcpy.value = true;
  try {
    const paths = await downloadAndInstallScrcpy();
    if (paths.adbPath) {
      adbPath.value = paths.adbPath;
    }
    if (paths.scrcpyPath) {
      scrcpyPath.value = paths.scrcpyPath;
    }
  } catch (error) {
  } finally {
    isDownloadingScrcpy.value = false;
  }
};

watch(
  () => adbPath.value,
  async (value) => {
    if (!toolPathsLoaded.value) {
      return;
    }
    try {
      const trimmed = value.trim();
      await setAdbPath(trimmed.length > 0 ? trimmed : null);
    } catch (error) {
    }
  }
);

watch(
  () => scrcpyPath.value,
  async (value) => {
    if (!toolPathsLoaded.value) {
      return;
    }
    try {
      const trimmed = value.trim();
      await setScrcpyPath(trimmed.length > 0 ? trimmed : null);
    } catch (error) {
    }
  }
);

watch(
  () => props.open,
  (value) => {
    if (value) {
      refreshPermission();
    }
  }
);

onMounted(async () => {
  const toolPaths = await getToolPaths().catch(() => {
    return null;
  });
  toolPathsLoaded.value = true;
  if (toolPaths) {
    if (!adbPath.value.trim() && toolPaths.adbPath) {
      adbPath.value = toolPaths.adbPath;
    }
    if (!scrcpyPath.value.trim() && toolPaths.scrcpyPath) {
      scrcpyPath.value = toolPaths.scrcpyPath;
    }
  }
  await syncToolPaths();
});
</script>

<template>
  <Modal v-model:open="openModel" title="Settings" :footer="null">
    <div class="settings-section">
      <h4>Tool Paths</h4>
      <div class="path-group">
        <label for="adb-path-input">ADB Path (optional)</label>
        <div class="path-row">
          <Input id="adb-path-input" v-model:value="adbPath" size="small" allow-clear />
          <Button size="small" @click="pickAdbPath">Browse</Button>
        </div>
      </div>
      <div class="path-group">
        <label for="scrcpy-path-input">scrcpy Path (optional)</label>
        <div class="path-row">
          <Input
            id="scrcpy-path-input"
            v-model:value="scrcpyPath"
            size="small"
            allow-clear
          />
          <Button size="small" @click="pickScrcpyPath">Browse</Button>
          <Button size="small" @click="downloadScrcpy" :loading="isDownloadingScrcpy">
            Download & Install
          </Button>
        </div>
      </div>
    </div>
    <div class="settings-section">
      <h4>Notifications</h4>
      <div class="setting-row">
        <div class="setting-label">OS Notifications</div>
        <Switch
          :checked="osNotificationsEnabled"
          :loading="isNotificationBusy"
          @change="(checked) => toggleNotifications(Boolean(checked))"
        />
      </div>
      <div class="setting-hint">
        Receive APK install success/failure notifications even when the app is in the
        background.
      </div>
      <div class="setting-row">
        <div class="setting-label">Test Notification</div>
        <Button size="small" :loading="isNotificationBusy" @click="sendTestNotification">
          Send Test
        </Button>
      </div>
      <div class="setting-status">
        Permission:
        {{
          permissionGranted === null
            ? "unknown"
            : permissionGranted
            ? "granted"
            : "not granted"
        }}
      </div>
      <div v-if="permissionNote" class="setting-note">{{ permissionNote }}</div>
    </div>
  </Modal>
</template>

<style lang="scss" scoped>
.settings-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-bottom: 12px;
  margin-bottom: 12px;
  border-bottom: 1px solid #ececec;
}

.settings-section:last-child {
  padding-bottom: 0;
  margin-bottom: 0;
  border-bottom: 0;
}

.path-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.path-row {
  display: flex;
  gap: 6px;
  align-items: center;
  :deep(.ant-input) {
    flex: 1;
  }
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.setting-label {
  font-weight: 600;
}

.setting-hint {
  color: #6b6b6b;
}

.setting-status {
  font-size: 12px;
  color: #4a4a4a;
}

.setting-note {
  font-size: 12px;
  color: #a61d24;
}

.debug-log {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid #e6e6e6;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas,
    "Liberation Mono", "Courier New", monospace;
  font-size: 12px;
  color: #3b3b3b;
}

.debug-title {
  font-weight: 600;
  margin-bottom: 6px;
}

.debug-empty {
  color: #7a7a7a;
}

.debug-lines {
  display: grid;
  gap: 4px;
}
</style>

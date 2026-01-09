import { computed, ref } from "vue";
import { getToolPaths } from "../commands";

export interface ToolPaths {
  adbPath: string | null;
  scrcpyPath: string | null;
}

export const useToolPaths = (appendSystemLog: (line: string) => void) => {
  const toolPaths = ref<ToolPaths | null>(null);

  const refreshToolPaths = async (): Promise<void> => {
    try {
      toolPaths.value = await getToolPaths();
    } catch (error) {
      appendSystemLog(`Failed to read tool paths: ${error}\n`);
    }
  };

  const toolsMissing = computed(() => {
    if (!toolPaths.value) {
      return false;
    }
    return !toolPaths.value.adbPath || !toolPaths.value.scrcpyPath;
  });

  const adbMissing = computed(() => {
    if (!toolPaths.value) {
      return false;
    }
    return !toolPaths.value.adbPath;
  });

  const scrcpyMissing = computed(() => {
    if (!toolPaths.value) {
      return false;
    }
    return !toolPaths.value.scrcpyPath;
  });

  const toolWarningTitle = computed(() => {
    if (scrcpyMissing.value && !adbMissing.value) {
      return "Missing scrcpy";
    }
    if (adbMissing.value && !scrcpyMissing.value) {
      return "Missing adb";
    }
    return "Missing scrcpy/adb";
  });

  const toolWarningMessage = computed(() => {
    if (scrcpyMissing.value && !adbMissing.value) {
      return "scrcpy not found. Open Settings to configure the scrcpy path or download and install scrcpy.";
    }
    if (adbMissing.value && !scrcpyMissing.value) {
      return "adb not found. Open Settings to configure the adb path or download and install adb.";
    }
    return "scrcpy/adb not found. Open Settings to configure the paths or download and install scrcpy/adb.";
  });

  return {
    toolPaths,
    refreshToolPaths,
    toolsMissing,
    adbMissing,
    scrcpyMissing,
    toolWarningTitle,
    toolWarningMessage,
  };
};

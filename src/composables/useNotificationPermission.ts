import { useStorage } from "@vueuse/core";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";
import { platform } from "@tauri-apps/plugin-os";

/**
 * Shared notification permission handling.
 *
 * On Windows, `isPermissionGranted()` can return false after app restart
 * even though the user previously granted permission in system settings.
 * This composable provides a consistent cache-based fallback and re-registration
 * strategy used across the app.
 */
export const useNotificationPermission = () => {
  const cache = useStorage<boolean>(
    "notificationPermissionGrantedCache",
    false,
    undefined,
    { mergeDefaults: true }
  );

  /**
   * Check permission without side effects (no dialog, no re-registration).
   * Uses the Windows cache fallback if `isPermissionGranted()` is unreliable.
   */
  const checkPermission = async (): Promise<boolean> => {
    let granted = await isPermissionGranted();
    if (!granted && platform() === "windows" && cache.value) {
      granted = true;
    }
    return granted;
  };

  /**
   * Ensure permission is granted, requesting it if necessary.
   * On Windows this re-registers the COM toast activator without showing
   * a dialog when system permission is already granted.
   * Updates the cache with the result.
   */
  const ensurePermission = async (): Promise<boolean> => {
    let granted = await isPermissionGranted();
    if (!granted && platform() === "windows" && cache.value) {
      granted = true;
    }
    if (!granted) {
      const result = await requestPermission();
      granted = result === "granted";
    }
    cache.value = granted;
    return granted;
  };

  return { checkPermission, ensurePermission, cache };
};

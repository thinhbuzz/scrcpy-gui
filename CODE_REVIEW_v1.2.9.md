# Code Review — v1.2.9

**Reviewed:** 2026-08-06
**Scope:** Commits `d73b7f4` → `aad5ba9` (Windows notification fix + dependency updates)

---

## Tổng quan

| Hạng mục | Đánh giá |
|----------|----------|
| Notification fix | ✅ Correct, có thể cải thiện |
| CI workflow | ✅ Tốt |
| Dependency updates | ⚠️ Cần chú ý TS pin |
| Code duplication | ⚠️ Cần refactor |

---

## 1. Notification Fix (`useScrcpyLogs.ts` + `ConfigPanel.vue`)

### 1.1 Logic 3-tier fallback — CORRECT

```typescript
// useScrcpyLogs.ts:72-86
let granted = await isPermissionGranted();                // (1) Check thật
if (!granted && platform() === "windows" && cache.value)  // (2) Windows cache fallback
  granted = true;
if (!granted) {                                           // (3) Last resort: requestPermission
  const result = await requestPermission();
  granted = result === "granted";
  cache.value = granted;
}
```

**Đánh giá:** Chuỗi fallback hợp lý. Trên Windows, `isPermissionGranted()` mất COM registration sau restart → tier (2) bỏ qua nếu cache có. Nếu cache cũng trống → tier (3) gọi `requestPermission()` để re-register.

**Rủi ro thấp:** `requestPermission()` trên Windows sẽ không hiện dialog nếu user đã grant trong system settings → an toàn để gọi trong background.

### 1.2 Proactive init trong `ConfigPanel.vue:137-141` — GOOD

```typescript
if (osNotificationsEnabled.value && platform() === "windows") {
  requestPermission().then((result) => {
    notificationPermissionGrantedCache.value = result === "granted";
  });
}
```

Chạy sớm khi app mount, re-register COM activator trước khi notification đầu tiên được gửi. Cache được update từ kết quả.

### 1.3 Vấn đề: Code duplication — WARNING

Logic permission (check → cache fallback → request → update cache) xuất hiện ở **3 nơi**:

| File | Function |
|------|----------|
| `SettingsDialog.vue` | `toggleNotifications()`, `sendTestNotification()`, `refreshPermission()` |
| `useScrcpyLogs.ts` | `sendOsNotification()` |
| `ConfigPanel.vue` | `onMounted()` proactive init |

**Đề xuất:** Extract thành 1 composable `useNotificationPermission()` dùng chung:

```typescript
// src/composables/useNotificationPermission.ts
export const useNotificationPermission = () => {
  const cache = useStorage("notificationPermissionGrantedCache", false);
  
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
  
  return { ensurePermission, cache };
};
```

Điều này giảm duplication 3 file → 1, đảm bảo logic nhất quán, và fix được bug `sendTestNotification` không dùng Windows cache fallback.

### 1.4 `sendTestNotification` timeout — QUESTIONABLE

```typescript
// SettingsDialog.vue:121-128
const sendPromise = sendNotification({ title, body });
const timeoutPromise = new Promise<"timeout">((resolve) => {
  setTimeout(() => resolve("timeout"), 3000);
});
const result = await Promise.race([sendPromise, timeoutPromise]);
if (result === "timeout") return; // silent ignore
```

**Vấn đề:** Nếu `sendNotification` treo >3s, user không nhận được feedback nào — không biết là timeout hay thành công. Nên thêm log hoặc hiển thị message.

### 1.5 `toggleNotifications` — missing Windows cache fallback

```typescript
// SettingsDialog.vue:77
let granted = await isPermissionGranted();
if (!granted) {
  const result = await requestPermission();
  granted = result === "granted";
}
```

Hàm này không dùng cache fallback như `refreshPermission()` và `sendOsNotification()`. Nếu user đã grant permission nhưng `isPermissionGranted()` sai, `requestPermission()` sẽ được gọi lại — vẫn hoạt động nhưng không nhất quán với các nơi khác.

---

## 2. CI Workflow (`ci.yml`)

### 2.1 Action version bumps — GOOD

| Action | Old | New | Node |
|--------|-----|-----|------|
| `actions/checkout` | @v4 | **@v5** | 20 → 24 |
| `actions/setup-node` | @v4 | **@v5** | 20 → 24 |
| `tauri-apps/tauri-action` | @v0 | **@v1** | 20 → 24 |

All 3 đã migrate lên Node 24, hết warning deprecated. `tauri-action@v1` có breaking changes nhưng không ảnh hưởng đến config hiện tại (các options đang dùng: `tagName`, `releaseName`, `releaseBody`, `releaseDraft`, `prerelease`, `args` — đều được giữ nguyên).

### 2.2 `rust-cache@v2` và `setup-bun@v2` — OK as-is

Không có v3 cho cả 2 action. `rust-cache@v2` đã migrate lên Node 20, `setup-bun@v2` cũng dùng Node 20. Chưa có phiên bản Node 24 nhưng cũng chưa bị deprecated (khác với Node 16→20 migration trước đây).

---

## 3. Dependency Updates

### 3.1 Frontend — OK, 1 caveat

| Package | Old → New | Risk |
|---------|-----------|------|
| `vue` | 3.5.30 → 3.5.41 | Patch — an toàn |
| `vite` | 7.3.1 → 8.2.1 | **Major** — cần test kỹ |
| `@tauri-apps/cli` | 2.10.1 → 2.11.4 | Minor — an toàn |
| `typescript` | ^5.9.3 → ^5.9 | Pin — tránh auto-upgrade lên TS7 |
| `sass` | 1.98.0 → 1.102.0 | Minor — an toàn |
| `@vueuse/core` | 14.2.1 → 14.4.0 | Minor — an toàn |

**Vite 8⚠️:** Major version bump từ 7→8. Đã test `vue-tsc --noEmit` + `vite build` pass. Runtime behavior cần được test thực tế.

**TypeScript pin⚠️:** `bun update --latest` tự động bump lên TS 7.0.2, nhưng `vue-tsc` không tương thích (import internal path `typescript/lib/tsc` bị thay đổi exports). Đã pin về `^5.9`. Cần theo dõi khi `vue-tsc` release bản hỗ trợ TS7.

### 3.2 Rust — GOOD

| Crate | Old → New | Risk |
|-------|-----------|------|
| `tauri` | 2.11.1 → 2.11.5 | Patch — an toàn |
| `tauri-plugin-dialog` | 2.6.0 → 2.7.2 | Minor — an toàn |
| `tokio` | 1.50.0 → 1.53.1 | Minor — an toàn |
| `reqwest` | 0.13.2 → 0.13.4 | Patch — an toàn |

Cargo.toml dùng version `"2"` cho Tauri plugins → luôn lấy compatible latest. Các transitive deps được `cargo update` nâng lên mới nhất trong phạm vi semver-compatible. **Không có breaking changes.**

---

## 4. Tổng kết

### Nên làm

1. **[P2]** Extract `useNotificationPermission` composable để xóa duplication giữa 3 file
2. **[P3]** `sendTestNotification` timeout nên có feedback thay vì silent ignore
3. **[P3]** Test Vite 8 runtime behavior trên macOS + Windows

### Đã tốt

- ✅ 3-tier notification fallback logic chính xác
- ✅ Proactive COM registration trên startup
- ✅ CI actions updated lên Node 24
- ✅ TypeScript pin tránh TS7 breakage
- ✅ Rust deps updated trong phạm vi an toàn

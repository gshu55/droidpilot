<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { storeToRefs } from "pinia";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAdbStore } from "./stores/adb";
import NoticeToast from "./components/NoticeToast.vue";
import { matchAppShortcut, normalizeAppShortcutMap, type AppShortcutAction } from "./config/appShortcuts";

const adbStore = useAdbStore();
const route = useRoute();
const router = useRouter();
const {
  notice,
  checking,
  loadingDevices,
  devices,
  selectedSerial,
  screenshotSource,
  mirrorToolbarSide,
} =
  storeToRefs(adbStore);
const adbStoreRefs = storeToRefs(adbStore);
const configuredAppShortcuts = computed(() => normalizeAppShortcutMap(adbStoreRefs.appShortcuts?.value));
const dropdownOpen = ref(false);
const wifiConnecting = ref(false);
const wifiDisconnectingSerial = ref("");
const deviceControlRef = ref<HTMLElement | null>(null);
const wifiAddress = ref(localStorage.getItem("droidpilot.wifiAddress") || localStorage.getItem("adbtools.wifiAddress") || "");
const isMaximized = ref(false);
const titlebarStyle = ref<"windows" | "macos" | "linux">("windows");
let maximizeSyncTimer: number | null = null;
const appBackgroundColor = "#f8fafc";
const mirrorBackgroundColor = "#f8fafc";
const appWindow = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
})();
const currentWebview = (() => {
  try {
    return getCurrentWebview();
  } catch {
    return null;
  }
})();
const isMirrorRoute = computed(() => route.path === "/" || route.path === "/mirror");

type AppIconName =
  | "arrowLeft"
  | "camera"
  | "fileUp"
  | "home"
  | "layers"
  | "link"
  | "unlink"
  | "play"
  | "power"
  | "refresh"
  | "settings"
  | "square"
  | "terminal"
  | "video";

interface MirrorToolbarState {
  canUseMirrorControls: boolean;
  canSendKeys: boolean;
  canMirrorScreenshot: boolean;
  canRecord: boolean;
  mirrorToggleDanger: boolean;
  mirrorToggleDisabled: boolean;
  mirrorToggleLabel: string;
  quickBusy: boolean;
  recording: boolean;
  recordingElapsedLabel: string;
  savingLocalRecording: boolean;
}

type MirrorToolbarAction =
  | "toggleMirror"
  | "back"
  | "home"
  | "tasks"
  | "power"
  | "screenshot"
  | "record"
  | "transfer"
  | "terminal"
  | "settings";
type WindowControlKey = "minimize" | "maximize" | "close";

const appIconPaths: Record<AppIconName, string[]> = {
  arrowLeft: ["M19 12H5", "M12 19l-7-7 7-7"],
  camera: [
    "M14.5 4l1.5 2H20a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l1.5-2h5z",
    "M12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
  ],
  fileUp: ["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2v6h6", "M12 12v6", "M9 15l3-3 3 3"],
  home: ["M3 11l9-8 9 8", "M5 10v10h14V10", "M9 20v-6h6v6"],
  layers: ["M12 2l10 5-10 5L2 7l10-5z", "M2 12l10 5 10-5", "M2 17l10 5 10-5"],
  link: ["M10 13a5 5 0 0 0 7.1 0l2-2a5 5 0 0 0-7.1-7.1l-1.2 1.2", "M14 11a5 5 0 0 0-7.1 0l-2 2A5 5 0 0 0 12 20.1l1.2-1.2"],
  unlink: ["M18.5 5.5a5 5 0 0 1 .6 7l-1.6 1.6", "M5.5 18.5a5 5 0 0 1-.6-7l1.6-1.6", "M8 16a5 5 0 0 0 5.1-.9", "M16 8a5 5 0 0 0-5.1.9", "M4 4l16 16"],
  play: ["M8 5v14l11-7-11-7z"],
  power: ["M12 2v10", "M18.4 6.6a9 9 0 1 1-12.8 0"],
  refresh: ["M19 15a7.5 7.5 0 1 1-1.8-7.8", "M21 4v6h-6"],
  settings: [
    "M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4A2 2 0 0 0 4 9.8l.2.1a2 2 0 0 1 1 1.7v.6a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.6a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z",
    "M9 12a3 3 0 1 0 6 0 3 3 0 0 0-6 0z",
  ],
  square: ["M6 6h12v12H6z"],
  terminal: ["M4 5h16v14H4z", "M7 9l3 3-3 3", "M12 15h5"],
  video: ["M4 6h11a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z", "M17 10l5-3v10l-5-3"],
};

const windowIconPaths = {
  minimize: ["M4 8h8"],
  maximize: ["M4.5 4.5h7v7h-7z"],
  restore: ["M6.5 3.5h6v6", "M3.5 6.5h6v6h-6z"],
  close: ["M4.5 4.5l7 7", "M11.5 4.5l-7 7"],
};

const mirrorToolbar = ref<MirrorToolbarState>({
  canUseMirrorControls: true,
  canSendKeys: false,
  canMirrorScreenshot: false,
  canRecord: false,
  mirrorToggleDanger: false,
  mirrorToggleDisabled: false,
  mirrorToggleLabel: "开始投屏",
  quickBusy: false,
  recording: false,
  recordingElapsedLabel: "00:00",
  savingLocalRecording: false,
});

const titlebarControls = computed(() => {
  const standardControls = [
    { key: "minimize" as const, label: "最小化" },
    { key: "maximize" as const, label: isMaximized.value ? "还原" : "放大" },
    { key: "close" as const, label: "关闭" },
  ];

  if (titlebarStyle.value === "macos") {
    return [
      { key: "close" as const, label: "关闭" },
      { key: "minimize" as const, label: "最小化" },
      { key: "maximize" as const, label: isMaximized.value ? "还原" : "放大" },
    ];
  }

  return standardControls;
});

function windowControlIconPaths(key: WindowControlKey) {
  if (key === "maximize" && isMaximized.value) {
    return windowIconPaths.restore;
  }
  return windowIconPaths[key];
}

const selectedDevice = computed(() => devices.value.find((item) => item.serial === selectedSerial.value) || null);
const canNativeScreenshot = computed(() => Boolean(selectedDevice.value?.status === "device" && !mirrorToolbar.value.quickBusy));
const canToolbarScreenshot = computed(() => {
  if (screenshotSource.value === "mirror") {
    return Boolean(mirrorToolbar.value.canMirrorScreenshot && !mirrorToolbar.value.quickBusy);
  }
  return canNativeScreenshot.value;
});
const screenshotActionLabel = computed(() => (screenshotSource.value === "mirror" ? "投屏截图" : "设备截图"));
const screenshotActionTitle = computed(() => {
  if (canToolbarScreenshot.value) {
    return screenshotActionLabel.value;
  }
  return screenshotSource.value === "mirror" ? "需要切换到正在运行的投屏标签页" : "需要已授权设备";
});
const selectedDeviceLabel = computed(() => {
  if (!selectedSerial.value) {
    return loadingDevices.value ? "加载设备中..." : "请选择设备";
  }
  if (!selectedDevice.value) {
    return selectedSerial.value;
  }
  return `${selectedDevice.value.serial} (${selectedDevice.value.status})`;
});

const canConnectWifi = computed(() => wifiAddress.value.trim().length > 0 && !wifiConnecting.value);

async function refreshDevices() {
  await adbStore.refreshAll();
}

function onSelectDevice(value: string) {
  adbStore.selectDevice(value);
  dropdownOpen.value = false;
}

function isWifiDevice(serial: string) {
  return serial.includes(":");
}

async function disconnectWifiDevice(serial: string) {
  if (wifiDisconnectingSerial.value) {
    return;
  }
  wifiDisconnectingSerial.value = serial;
  try {
    await adbStore.disconnectWifiDevice(serial);
  } finally {
    wifiDisconnectingSerial.value = "";
  }
}

function toggleDropdown() {
  if (loadingDevices.value) {
    return;
  }
  dropdownOpen.value = !dropdownOpen.value;
}

function closeDropdown() {
  dropdownOpen.value = false;
}

function sanitizeWifiAddressInput(event: Event) {
  const input = event.target as HTMLInputElement;
  const sanitized = input.value.replace(/[^\d.:]/g, "");
  if (input.value !== sanitized) {
    input.value = sanitized;
  }
  wifiAddress.value = sanitized;
}

async function connectWifiAdb() {
  if (wifiConnecting.value) {
    return;
  }

  const target = wifiAddress.value.trim();
  if (!target) {
    adbStore.setNotice("error", "请输入 IP:端口，例如 192.168.1.100:5555");
    return;
  }

  wifiConnecting.value = true;
  localStorage.setItem("droidpilot.wifiAddress", target);
  try {
    await adbStore.executeCommand(`connect ${target}`);
    await adbStore.refreshAll();
    closeDropdown();
  } finally {
    wifiConnecting.value = false;
  }
}

function onMirrorToolbarState(event: Event) {
  const detail = (event as CustomEvent<Partial<MirrorToolbarState>>).detail;
  mirrorToolbar.value = {
    ...mirrorToolbar.value,
    ...detail,
  };
}

function sendMirrorToolbarAction(action: MirrorToolbarAction) {
  if (action === "settings") {
    void router.push("/settings");
    return;
  }
  window.dispatchEvent(new CustomEvent("droidpilot:mirror-toolbar-action", { detail: action }));
}

function isEditableShortcutTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.closest(".xterm-helper-textarea")) {
    return false;
  }
  return Boolean(target.closest("input, textarea, select, [contenteditable], .settings-shortcut-key"));
}

async function sendMirrorToolbarActionFromShortcut(action: MirrorToolbarAction) {
  if (!isMirrorRoute.value) {
    await router.push("/mirror");
    await nextTick();
  }
  window.setTimeout(() => sendMirrorToolbarAction(action), 0);
}

function appShortcutToToolbarAction(action: AppShortcutAction): MirrorToolbarAction {
  switch (action) {
    case "mirror":
      return "toggleMirror";
    case "transfer":
      return "transfer";
    case "screenshot":
      return "screenshot";
    case "record":
      return "record";
    case "terminal":
      return "terminal";
  }
}

function handleAppShortcutKeydown(event: KeyboardEvent) {
  if (isEditableShortcutTarget(event.target)) {
    return;
  }

  const shortcut = matchAppShortcut(event, configuredAppShortcuts.value);
  if (!shortcut) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  void sendMirrorToolbarActionFromShortcut(appShortcutToToolbarAction(shortcut.action));
}

function syncDocumentBackground(isMirror: boolean) {
  document.documentElement.classList.toggle("mirror-window-bg", isMirror);
  document.body.classList.toggle("mirror-window-bg", isMirror);
}

async function syncNativeBackground(isMirror: boolean) {
  const color = isMirror ? mirrorBackgroundColor : appBackgroundColor;
  await Promise.allSettled([
    appWindow?.setBackgroundColor(color),
    currentWebview?.setBackgroundColor(color),
  ]);
}

function syncResizeBackground(isMirror: boolean) {
  syncDocumentBackground(isMirror);
  void syncNativeBackground(isMirror);
}

function onWindowClick(event: MouseEvent) {
  if (!deviceControlRef.value) {
    return;
  }
  const target = event.target as Node;
  if (!deviceControlRef.value.contains(target)) {
    closeDropdown();
  }
}

function disableContextMenu(event: MouseEvent) {
  event.stopPropagation();
  event.preventDefault();
}

function disableRightButtonPress(event: MouseEvent | PointerEvent) {
  if (event.button !== 2) {
    return;
  }
  event.stopPropagation();
  event.preventDefault();
}

async function syncMaximizeState() {
  if (!appWindow) {
    isMaximized.value = false;
    return;
  }
  try {
    isMaximized.value = await appWindow.isMaximized();
  } catch {
    isMaximized.value = false;
  }
}

function scheduleMaximizeStateSync() {
  if (maximizeSyncTimer !== null) {
    window.clearTimeout(maximizeSyncTimer);
  }
  maximizeSyncTimer = window.setTimeout(() => {
    maximizeSyncTimer = null;
    void syncMaximizeState();
  }, 120);
}

async function minimizeWindow() {
  if (!appWindow) {
    return;
  }
  await appWindow.minimize();
}

async function toggleMaximizeWindow() {
  if (!appWindow) {
    return;
  }
  await appWindow.toggleMaximize();
  await syncMaximizeState();
}

async function closeWindow() {
  if (!appWindow) {
    return;
  }
  await appWindow.close();
}

function onTitlebarMouseDown(event: PointerEvent) {
  if (!appWindow) {
    return;
  }
  if (event.button !== 0) {
    return;
  }

  const target = event.target as HTMLElement | null;
  if (target?.closest(".window-no-drag, button, a, input, select, textarea")) {
    return;
  }

  void appWindow.startDragging();
}

async function onTitlebarDblClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest(".window-no-drag, button, a, input, select, textarea")) {
    return;
  }

  await toggleMaximizeWindow();
}

async function onTitlebarAction(action: "minimize" | "maximize" | "close") {
  if (action === "minimize") {
    await minimizeWindow();
    return;
  }
  if (action === "maximize") {
    await toggleMaximizeWindow();
    return;
  }
  await closeWindow();
}

onMounted(async () => {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) {
    titlebarStyle.value = "macos";
  } else if (ua.includes("linux")) {
    titlebarStyle.value = "linux";
  } else {
    titlebarStyle.value = "windows";
  }

  await syncMaximizeState();
  await refreshDevices();
  window.addEventListener("click", onWindowClick);
  window.addEventListener("contextmenu", disableContextMenu, true);
  document.addEventListener("contextmenu", disableContextMenu, true);
  window.addEventListener("mousedown", disableRightButtonPress, true);
  document.addEventListener("mousedown", disableRightButtonPress, true);
  window.addEventListener("pointerdown", disableRightButtonPress, true);
  document.addEventListener("pointerdown", disableRightButtonPress, true);
  window.addEventListener("resize", scheduleMaximizeStateSync);
  window.addEventListener("droidpilot:mirror-toolbar-state", onMirrorToolbarState);
  window.addEventListener("keydown", handleAppShortcutKeydown, true);
});

onBeforeUnmount(() => {
  if (maximizeSyncTimer !== null) {
    window.clearTimeout(maximizeSyncTimer);
    maximizeSyncTimer = null;
  }
  window.removeEventListener("click", onWindowClick);
  window.removeEventListener("contextmenu", disableContextMenu, true);
  document.removeEventListener("contextmenu", disableContextMenu, true);
  window.removeEventListener("mousedown", disableRightButtonPress, true);
  document.removeEventListener("mousedown", disableRightButtonPress, true);
  window.removeEventListener("pointerdown", disableRightButtonPress, true);
  document.removeEventListener("pointerdown", disableRightButtonPress, true);
  window.removeEventListener("resize", scheduleMaximizeStateSync);
  window.removeEventListener("droidpilot:mirror-toolbar-state", onMirrorToolbarState);
  window.removeEventListener("keydown", handleAppShortcutKeydown, true);
  syncDocumentBackground(false);
});

watch(isMirrorRoute, syncResizeBackground, { immediate: true });
</script>

<template>
  <div
    class="page-bg layout adb-layout"
    :class="[
      `platform-${titlebarStyle}`,
      {
        'has-mirror-side': isMirrorRoute,
        'mirror-toolbar-left': isMirrorRoute && mirrorToolbarSide === 'left',
        'mirror-toolbar-right': isMirrorRoute && mirrorToolbarSide === 'right',
      },
    ]"
  >
    <div class="mesh mesh-left"></div>
    <div class="mesh mesh-right"></div>
    <header
      class="topbar card the-titlebar"
      @pointerdown="onTitlebarMouseDown"
      @dblclick="onTitlebarDblClick"
      @contextmenu.prevent.stop
    >
      <div class="device-select-wrap" ref="deviceControlRef">
        <div class="device-dropdown">
          <button
            type="button"
            class="device-select-btn mono"
            :disabled="loadingDevices"
            @click.stop="toggleDropdown"
          >
            <span>{{ selectedDeviceLabel }}</span>
            <svg class="caret" :class="{ open: dropdownOpen }" viewBox="0 0 1024 1024" aria-hidden="true">
              <path d="M150.3 305.16c14.72-14.72 38.3-15.61 54.08-2.03l2.19 2.03L544.11 642.7l337.54-337.54c14.72-14.74 38.32-15.62 54.1-2.03l2.17 2.03c14.72 14.72 15.61 38.3 2.03 54.08l-2.03 2.19L586.3 713.04c-22.34 22.33-58.22 23.38-81.83 2.39l-2.55-2.39-351.64-351.63c-15.53-15.53-15.53-40.72 0-56.25h0.02z" />
            </svg>
          </button>

          <div v-if="dropdownOpen" class="device-menu">
            <div class="wifi-connect-inline" @click.stop>
              <div class="wifi-connect-title">Wi-Fi ADB</div>
              <div class="wifi-connect-row">
                <input
                  class="wifi-connect-input mono"
                  v-model="wifiAddress"
                  placeholder="192.168.1.100:5555"
                  inputmode="decimal"
                  pattern="[0-9.:]*"
                  @input="sanitizeWifiAddressInput"
                  @keydown.enter.prevent="connectWifiAdb"
                />
                <button
                  class="wifi-connect-submit"
                  type="button"
                  :disabled="!canConnectWifi"
                  :title="wifiConnecting ? '连接中' : '连接 Wi-Fi ADB'"
                  :aria-label="wifiConnecting ? '连接中' : '连接 Wi-Fi ADB'"
                  @click="connectWifiAdb"
                >
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in appIconPaths.link" :key="path" :d="path" />
                  </svg>
                </button>
              </div>
            </div>
            <div
              v-for="item in devices"
              :key="item.serial"
              class="device-option"
              :class="{ active: item.serial === selectedSerial }"
            >
              <button type="button" class="device-option-main" @click="onSelectDevice(item.serial)">
                <span class="mono option-text">{{ item.serial }}</span>
                <span class="badge" :class="item.status">{{ item.status }}</span>
              </button>
              <button
                v-if="isWifiDevice(item.serial)"
                type="button"
                class="device-disconnect"
                :disabled="wifiDisconnectingSerial === item.serial"
                :title="wifiDisconnectingSerial === item.serial ? `正在断开 ${item.serial}` : `断开 ${item.serial}`"
                :aria-label="wifiDisconnectingSerial === item.serial ? `正在断开 ${item.serial}` : `断开 ${item.serial}`"
                @click.stop="disconnectWifiDevice(item.serial)"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path v-for="path in appIconPaths.unlink" :key="path" :d="path" />
                </svg>
              </button>
            </div>
            <div v-if="devices.length === 0" class="device-empty">暂无设备，请点击刷新</div>
          </div>
        </div>
        <button
          class="toolbar-icon-btn refresh-btn"
          :disabled="checking || loadingDevices"
          aria-label="刷新设备"
          title="刷新设备"
          @click="refreshDevices"
        >
          <svg class="toolbar-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path v-for="path in appIconPaths.refresh" :key="path" :d="path" />
          </svg>
        </button>
      </div>

      <div v-if="titlebarStyle === 'macos'" class="native-window-spacer" aria-hidden="true"></div>
      <div v-else class="window-controls" :class="[`platform-${titlebarStyle}`]">
        <button
          v-for="control in titlebarControls"
          :key="control.key"
          type="button"
          class="window-btn"
          :class="[`window-btn-${control.key}`, `platform-${titlebarStyle}`, { 'window-btn-maximize-normal': control.key === 'maximize' && !isMaximized }]"
          :aria-label="control.label"
          :title="control.label"
          @click="onTitlebarAction(control.key)"
        >
          <svg class="window-btn-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path v-for="path in windowControlIconPaths(control.key)" :key="path" :d="path" />
          </svg>
        </button>
      </div>
    </header>

    <div class="layout-content" :class="`toolbar-${mirrorToolbarSide}`">
      <aside v-if="isMirrorRoute" class="root-mirror-side layout-content__left">
        <div class="root-side-section">
          <div class="root-mirror-actions">
            <button
              class="root-icon-btn"
              :class="mirrorToolbar.mirrorToggleDanger ? 'root-icon-btn-danger' : 'root-icon-btn-primary'"
              type="button"
              :disabled="!mirrorToolbar.canUseMirrorControls || mirrorToolbar.mirrorToggleDisabled"
              :aria-label="mirrorToolbar.mirrorToggleLabel"
              :title="mirrorToolbar.mirrorToggleLabel"
              @click="sendMirrorToolbarAction('toggleMirror')"
            >
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in mirrorToolbar.mirrorToggleDanger ? appIconPaths.square : appIconPaths.play" :key="path" :d="path" />
              </svg>
              <span>{{ mirrorToolbar.mirrorToggleLabel }}</span>
            </button>
          </div>
        </div>

        <div class="root-side-section">
          <div class="root-key-grid">
            <button class="root-icon-btn" type="button" :disabled="!mirrorToolbar.canSendKeys" aria-label="返回" title="返回" @click="sendMirrorToolbarAction('back')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.arrowLeft" :key="path" :d="path" />
              </svg>
              <span>返回</span>
            </button>
            <button class="root-icon-btn" type="button" :disabled="!mirrorToolbar.canSendKeys" aria-label="Home" title="Home" @click="sendMirrorToolbarAction('home')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.home" :key="path" :d="path" />
              </svg>
              <span>Home</span>
            </button>
            <button class="root-icon-btn" type="button" :disabled="!mirrorToolbar.canSendKeys" aria-label="多任务" title="多任务" @click="sendMirrorToolbarAction('tasks')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.layers" :key="path" :d="path" />
              </svg>
              <span>多任务</span>
            </button>
            <button class="root-icon-btn" type="button" :disabled="!mirrorToolbar.canSendKeys" aria-label="电源" title="电源" @click="sendMirrorToolbarAction('power')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.power" :key="path" :d="path" />
              </svg>
              <span>电源</span>
            </button>
          </div>
        </div>

        <div class="root-side-section root-quick-section">
          <div class="root-quick-action-list">
            <button class="root-icon-btn" type="button" :disabled="!canToolbarScreenshot" :aria-label="screenshotActionLabel" :title="screenshotActionTitle" @click="sendMirrorToolbarAction('screenshot')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.camera" :key="path" :d="path" />
              </svg>
              <span>{{ screenshotActionLabel }}</span>
            </button>
            <button
              class="root-icon-btn"
              :class="{ 'root-icon-btn-recording': mirrorToolbar.recording }"
              type="button"
              :disabled="!mirrorToolbar.canRecord || mirrorToolbar.quickBusy || mirrorToolbar.savingLocalRecording"
              :aria-label="mirrorToolbar.savingLocalRecording ? '保存录屏' : mirrorToolbar.recording ? '停止录屏' : '录屏'"
              :title="mirrorToolbar.savingLocalRecording ? '保存录屏' : mirrorToolbar.recording ? '停止录屏' : '录屏'"
              @click="sendMirrorToolbarAction('record')"
            >
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in mirrorToolbar.recording ? appIconPaths.square : appIconPaths.video" :key="path" :d="path" />
              </svg>
              <span>{{ mirrorToolbar.savingLocalRecording ? "保存中" : mirrorToolbar.recording ? mirrorToolbar.recordingElapsedLabel : "录屏" }}</span>
            </button>
            <button class="root-icon-btn" type="button" :disabled="mirrorToolbar.quickBusy" aria-label="文件传输" title="文件传输" @click="sendMirrorToolbarAction('transfer')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.fileUp" :key="path" :d="path" />
              </svg>
              <span>文件传输</span>
            </button>
            <button class="root-icon-btn" type="button" :disabled="mirrorToolbar.quickBusy" aria-label="快捷终端" title="快捷终端" @click="sendMirrorToolbarAction('terminal')">
              <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in appIconPaths.terminal" :key="path" :d="path" />
              </svg>
              <span>快捷终端</span>
            </button>
          </div>
        </div>
        <div class="root-side-section root-settings-section">
          <button class="root-icon-btn root-settings-action-btn" type="button" aria-label="设置" title="设置" @click="sendMirrorToolbarAction('settings')">
            <svg class="root-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
              <path v-for="path in appIconPaths.settings" :key="path" :d="path" />
            </svg>
            <span>设置</span>
          </button>
        </div>
      </aside>

      <div class="app-shell layout-content__right">
        <main class="app-content-scroll" :class="{ 'app-content-mirror': isMirrorRoute, 'app-content-settings': route.path === '/settings' }">
          <NoticeToast :notice="notice" @close="adbStore.clearNotice" />
          <RouterView />
        </main>
      </div>
    </div>
  </div>
</template>

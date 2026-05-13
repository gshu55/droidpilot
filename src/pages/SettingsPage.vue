<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import { storeToRefs } from "pinia";
import { downloadDir } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";
import {
  type MirrorToolbarSide,
  type PickerCopyFormat,
  type ScreenshotMode,
  type ScreenshotSource,
  useAdbStore,
} from "../stores/adb";
import {
  appShortcutDefinitions,
  defaultAppShortcutMap,
  displayAppShortcutValue,
  normalizeAppShortcutFromEvent,
  normalizeAppShortcutMap,
  type AppShortcutAction,
} from "../config/appShortcuts";
import AboutPage from "./AboutPage.vue";
import HelpPage from "./HelpPage.vue";

type SettingsSection = "preferences" | "shortcuts" | "help" | "about";
type SettingsIconName = "arrowLeft" | "preferences" | "shortcuts" | "help" | "about" | "folder";

const adbStore = useAdbStore();
const activeSection = ref<SettingsSection>("preferences");
const adbStoreRefs = storeToRefs(adbStore);
const {
  defaultLocalDir,
  defaultDevicePath,
  screenshotMode,
  screenshotSource,
  pickerCopyFormat,
  mirrorToolbarSide,
} = adbStoreRefs;
const capturingShortcutAction = ref<AppShortcutAction | null>(null);
const configuredAppShortcuts = computed(() => normalizeAppShortcutMap(adbStoreRefs.appShortcuts?.value));
const shortcutRows = computed(() =>
  appShortcutDefinitions.map((shortcut) => {
    const value = configuredAppShortcuts.value[shortcut.action] || defaultAppShortcutMap[shortcut.action];
    return {
      ...shortcut,
      value,
      displayValue: displayAppShortcutValue(value) || displayAppShortcutValue(shortcut.defaultValue),
    };
  }),
);

const settingsSections: Array<{ key: SettingsSection; title: string; icon: SettingsIconName }> = [
  { key: "preferences", title: "偏好设置", icon: "preferences" },
  { key: "shortcuts", title: "快捷键", icon: "shortcuts" },
  { key: "help", title: "帮助", icon: "help" },
  { key: "about", title: "关于", icon: "about" },
];

const settingsIconPaths: Record<SettingsIconName, string[]> = {
  arrowLeft: ["M19 12H5", "M12 19l-7-7 7-7"],
  preferences: ["M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5z", "M19.4 15a1.8 1.8 0 0 0 .4 2l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.8 1.8 0 0 0-2-.4 1.8 1.8 0 0 0-1 1.6V21a2 2 0 1 1-4 0v-.1a1.8 1.8 0 0 0-1-1.6 1.8 1.8 0 0 0-2 .4l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.8 1.8 0 0 0 .4-2 1.8 1.8 0 0 0-1.6-1H3a2 2 0 1 1 0-4h.1a1.8 1.8 0 0 0 1.6-1 1.8 1.8 0 0 0-.4-2l-.1-.1A2 2 0 1 1 7 4.1l.1.1a1.8 1.8 0 0 0 2 .4 1.8 1.8 0 0 0 1-1.6V3a2 2 0 1 1 4 0v.1a1.8 1.8 0 0 0 1 1.6 1.8 1.8 0 0 0 2-.4l.1-.1A2 2 0 1 1 20 7l-.1.1a1.8 1.8 0 0 0-.4 2 1.8 1.8 0 0 0 1.6 1h.1a2 2 0 1 1 0 4h-.1a1.8 1.8 0 0 0-1.7 1z"],
  shortcuts: ["M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z", "M6 9h.01", "M10 9h.01", "M14 9h.01", "M18 9h.01", "M7 13h10", "M9 16h6"],
  help: ["M9.1 9a3 3 0 1 1 5.8 1c-.6 1.1-1.9 1.5-2.4 2.5", "M12 17h.01", "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z"],
  about: ["M12 16v-4", "M12 8h.01", "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z"],
  folder: ["M3 7h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z", "M3 7V5a2 2 0 0 1 2-2h4l2 2h5"],
};

const screenshotModeOptions: Array<{ value: ScreenshotMode; label: string }> = [
  { value: "save", label: "仅传输" },
  { value: "copy", label: "仅复制" },
  { value: "save-copy", label: "传输 + 复制" },
];

const screenshotSourceOptions: Array<{ value: ScreenshotSource; label: string }> = [
  { value: "native", label: "按原生截图" },
  { value: "mirror", label: "按投屏截图" },
];

const pickerCopyFormatOptions: Array<{ value: PickerCopyFormat; label: string }> = [
  { value: "hex", label: "HEX" },
  { value: "rgb", label: "RGB" },
];

const mirrorToolbarSideOptions: Array<{ value: MirrorToolbarSide; label: string }> = [
  { value: "right", label: "右侧" },
  { value: "left", label: "左侧" },
];

async function pickDownloadDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择默认本地保存目录",
  });

  if (typeof selected === "string") {
    adbStore.setDefaultLocalDir(selected);
  }
}

async function fillSystemDownloadDir() {
  try {
    const dir = await downloadDir();
    if (dir && dir.trim()) {
      adbStore.setDefaultLocalDir(dir);
    }
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

function updateDefaultDevicePath(value: string) {
  adbStore.setDefaultDevicePath(value);
}

function shortcutValue(action: AppShortcutAction) {
  return configuredAppShortcuts.value[action] || defaultAppShortcutMap[action];
}

function beginShortcutCapture(action: AppShortcutAction) {
  capturingShortcutAction.value = action;
}

function onShortcutCapture(event: KeyboardEvent, action: AppShortcutAction) {
  if (capturingShortcutAction.value !== action) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();

  if (event.key === "Escape") {
    capturingShortcutAction.value = null;
    return;
  }

  const nextShortcut = normalizeAppShortcutFromEvent(event);
  if (!nextShortcut) {
    return;
  }

  const duplicate = appShortcutDefinitions.find((item) => item.action !== action && shortcutValue(item.action) === nextShortcut);
  if (duplicate) {
    adbStore.setNotice("error", `快捷键已被「${duplicate.label}」使用`);
    return;
  }

  adbStore.setAppShortcut(action, nextShortcut);
  capturingShortcutAction.value = null;
}

function resetShortcut(action: AppShortcutAction) {
  adbStore.setAppShortcut(action, defaultAppShortcutMap[action]);
  if (capturingShortcutAction.value === action) {
    capturingShortcutAction.value = null;
  }
}

onMounted(async () => {
  if (typeof defaultLocalDir.value !== "string" || !defaultLocalDir.value.trim()) {
    await fillSystemDownloadDir();
  }
});
</script>

<template>
  <div class="settings-layout">
    <aside class="settings-sidebar">
      <RouterLink class="settings-nav-item settings-back-link" to="/mirror" aria-label="返回投屏" title="返回投屏">
        <svg class="settings-nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path v-for="path in settingsIconPaths.arrowLeft" :key="path" :d="path" />
        </svg>
        <div class="back">返回</div>
      </RouterLink>

      <button
        v-for="item in settingsSections"
        :key="item.key"
        type="button"
        class="settings-nav-item"
        :class="{ active: activeSection === item.key }"
        @click="activeSection = item.key"
      >
        <svg class="settings-nav-icon" viewBox="0 0 24 24" aria-hidden="true">
          <path v-for="path in settingsIconPaths[item.icon]" :key="path" :d="path" />
        </svg>
        <strong>{{ item.title }}</strong>
      </button>
    </aside>

    <section class="settings-content">
      <div v-if="activeSection === 'preferences'" class="settings-panel">
        <header class="settings-head">
          <h4>偏好设置</h4>
        </header>

        <div class="settings-grid">
          <label>
            <span>默认本地保存目录</span>
            <div class="field-row">
              <input
                class="input mono"
                :value="defaultLocalDir"
                placeholder="截图、录屏、获取文件保存目录"
                @input="adbStore.setDefaultLocalDir(($event.target as HTMLInputElement).value)"
              />
              <button class="settings-icon-btn" type="button" title="选择默认本地保存目录" aria-label="选择默认本地保存目录" @click="pickDownloadDir">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path v-for="path in settingsIconPaths.folder" :key="path" :d="path" />
                </svg>
              </button>
            </div>
            <small class="field-note">截图和视频录制会直接存储到本机此目录。</small>
          </label>

          <label>
            <span>默认设备路径</span>
            <input
              class="input mono"
              :value="defaultDevicePath"
              placeholder="/storage/emulated/0/Download/"
              @input="updateDefaultDevicePath(($event.target as HTMLInputElement).value)"
            />
          </label>

          <label>
            <span>截图处理方式</span>
            <div class="segmented-control">
              <button
                v-for="item in screenshotModeOptions"
                :key="item.value"
                type="button"
                :class="{ active: screenshotMode === item.value }"
                @click="adbStore.setScreenshotMode(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
          </label>

          <label>
            <span>截图来源</span>
            <div class="segmented-control two">
              <button
                v-for="item in screenshotSourceOptions"
                :key="item.value"
                type="button"
                :class="{ active: screenshotSource === item.value }"
                @click="adbStore.setScreenshotSource(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
            <small class="field-note">原生截图通过设备 screencap 获取；投屏截图截取当前标签页中的投屏画面。</small>
          </label>

          <label>
            <span>取色复制</span>
            <div class="segmented-control two compact">
              <button
                v-for="item in pickerCopyFormatOptions"
                :key="item.value"
                type="button"
                :class="{ active: pickerCopyFormat === item.value }"
                @click="adbStore.setPickerCopyFormat(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
            <small class="field-note">截图编辑页取色器悬停时按 C 复制，按 Shift 快速切换 HEX/RGB。</small>
          </label>

          <label>
            <span>侧边栏位置</span>
            <div class="segmented-control two">
              <button
                v-for="item in mirrorToolbarSideOptions"
                :key="item.value"
                type="button"
                :class="{ active: mirrorToolbarSide === item.value }"
                @click="adbStore.setMirrorToolbarSide(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
          </label>
        </div>
      </div>

      <div v-else-if="activeSection === 'shortcuts'" class="settings-panel shortcuts-panel">
        <header class="settings-head">
          <h4>快捷键</h4>
          <p>应用窗口获得焦点时生效，不注册系统全局热键；快捷键需包含 Ctrl、Alt 或 Win。</p>
        </header>

        <section class="settings-shortcuts" aria-label="快捷键">
          <div class="settings-shortcut-list">
            <div v-for="shortcut in shortcutRows" :key="shortcut.action" class="settings-shortcut-item">
              <div>
                <strong>{{ shortcut.label }}</strong>
                <small>{{ shortcut.description }}</small>
              </div>
              <button
                class="settings-shortcut-key"
                type="button"
                :class="{ capturing: capturingShortcutAction === shortcut.action }"
                :aria-label="`修改${shortcut.label}快捷键`"
                :title="capturingShortcutAction === shortcut.action ? '按下新的快捷键，Esc 取消' : '点击后按下新的快捷键'"
                @click="beginShortcutCapture(shortcut.action)"
                @keydown="onShortcutCapture($event, shortcut.action)"
              >
                {{ capturingShortcutAction === shortcut.action ? "按下快捷键" : shortcut.displayValue }}
              </button>
              <button class="settings-shortcut-reset" type="button" title="恢复默认快捷键" aria-label="恢复默认快捷键" @click="resetShortcut(shortcut.action)">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M3 12a9 9 0 1 0 2.6-6.4" />
                  <path d="M3 4v6h6" />
                </svg>
              </button>
            </div>
          </div>
        </section>
      </div>

      <HelpPage v-else-if="activeSection === 'help'" />
      <AboutPage v-else />
    </section>
  </div>
</template>

<style scoped>
.settings-layout {
  display: grid;
  grid-template-columns: 148px minmax(0, 1fr);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.settings-sidebar {
  display: grid;
  align-content: start;
  align-self: stretch;
  gap: 4px;
  padding: 8px;
  background: #f8fafc;
  overflow: hidden;
}

.settings-nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--control-text);
  padding: 8px 10px;
  text-align: left;
  text-decoration: none;
  cursor: pointer;
  user-select: none;
  -webkit-user-select: none;
}

.settings-nav-item:hover,
.settings-nav-item.active {
  background: var(--primary-soft);
  color: var(--primary);
}

.settings-nav-item strong {
  font-size: 16px;
  font-weight: 400;
}

.settings-nav-item .back {
  font-size: 16px;
  font-weight: 400;
  color: #7F7F7F;
}

.settings-back-link {
  margin-bottom: 10px;
  color: var(--text-muted);
}

.settings-back-link:hover {
  color: var(--control-text);
}

.settings-nav-icon {
  width: 15px;
  height: 15px;
  flex: 0 0 auto;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
  color: #7F7F7F;
}

.settings-content {
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  scrollbar-gutter: stable;
}

.settings-panel {
  max-width: 920px;
  padding: 12px;
}

.settings-head {
  margin-bottom: 12px;
}

.settings-head h4 {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.25;
}

.settings-head p {
  margin: 0;
  color: var(--text-sub);
  font-size: 12px;
  line-height: 1.45;
}

.settings-grid {
  display: grid;
  gap: 12px;
}

.settings-grid label {
  display: grid;
  gap: 6px;
  color: var(--text-sub);
  font-size: 12px;
}

.field-row {
  display: flex;
  gap: 6px;
}

.field-row .input {
  min-width: 0;
  flex: 1;
}

.settings-grid .input {
  border-radius: 8px;
}

.settings-icon-btn {
  display: inline-grid;
  place-items: center;
  width: 34px;
  height: 34px;
  flex: 0 0 34px;
  border: 1px solid var(--control-border);
  border-radius: 8px;
  background: var(--control-bg);
  color: var(--control-text);
  padding: 0;
  cursor: pointer;
}

.settings-icon-btn svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.settings-icon-btn:hover:not(:disabled) {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.field-note {
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.45;
}

.segmented-control {
  display: inline-grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0;
  width: min(420px, 100%);
  border: 1px solid var(--control-border);
  border-radius: 8px;
  overflow: hidden;
  background: var(--control-bg);
}

.segmented-control.two {
  grid-template-columns: repeat(2, minmax(0, 1fr));
  width: min(280px, 100%);
}

.segmented-control.compact {
  width: 132px;
}

.segmented-control button {
  height: 32px;
  border: 0;
  border-right: 1px solid var(--control-border);
  background: transparent;
  color: var(--text-sub);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.segmented-control button:last-child {
  border-right: 0;
}

.segmented-control button.active {
  background: var(--primary);
  color: #fff;
}

.segmented-control button:hover:not(.active) {
  background: var(--primary-soft);
  color: var(--primary);
}

.settings-pair {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  max-width: 520px;
}

.settings-shortcuts {
  display: grid;
  gap: 10px;
}

.shortcuts-panel {
  max-width: 760px;
}

.settings-shortcut-list {
  display: grid;
  gap: 6px;
}

.settings-shortcut-item {
  display: grid;
  grid-template-columns: minmax(220px, 1fr) 190px 34px;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 7px 8px 7px 10px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #fff;
}

.settings-shortcut-item div {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.settings-shortcut-item strong {
  color: var(--text-main);
  font-size: 13px;
  font-weight: 600;
}

.settings-shortcut-item small {
  color: var(--text-sub);
  font-size: 12px;
  line-height: 1.35;
}

.settings-shortcut-key,
.settings-shortcut-reset {
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  background: #f1f5f9;
  color: #0f172a;
  cursor: pointer;
}

.settings-shortcut-key {
  width: 190px;
  height: 30px;
  padding: 0 10px;
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace;
  font-size: 12px;
  line-height: 1.2;
  text-align: center;
  white-space: nowrap;
}

.settings-shortcut-key:hover,
.settings-shortcut-key.capturing,
.settings-shortcut-reset:hover {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.settings-shortcut-key.capturing {
  font-family: inherit;
}

.settings-shortcut-reset {
  display: inline-grid;
  place-items: center;
  width: 34px;
  height: 30px;
  padding: 0;
}

.settings-shortcut-reset svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

:deep(.help-page),
:deep(.about-page) {
  margin: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
}

:deep(.help-page.panel),
:deep(.about-page.panel) {
  padding: 12px;
}

:deep(.help-page h2),
:deep(.about-page h2) {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 20px;
  font-weight: 600;
  line-height: 1.25;
}

:deep(.help-section),
:deep(.about-item) {
  background: #f8fafc;
}

@media (max-width: 700px) {
  .settings-layout {
    grid-template-columns: 1fr;
    grid-template-rows: auto minmax(0, 1fr);
  }

  .settings-sidebar {
    grid-auto-flow: column;
    grid-auto-columns: max-content;
    overflow-x: auto;
    overflow-y: hidden;
    border-right: 0;
    border-bottom: 0;
  }

  .field-row,
  .settings-pair {
    grid-template-columns: 1fr;
    flex-direction: column;
  }

  .settings-shortcut-item {
    grid-template-columns: 1fr;
    align-items: start;
  }

  .settings-shortcut-key {
    width: 100%;
  }
}
</style>

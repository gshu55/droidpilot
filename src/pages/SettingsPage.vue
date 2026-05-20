<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { storeToRefs } from "pinia";
import { downloadDir } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";
import {
  type AppLanguage,
  type AppTheme,
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
import { appLanguageOptions, appThemeOptions, useI18n } from "../i18n";
import AboutPage from "./AboutPage.vue";

type SettingsSection = "preferences" | "shortcuts" | "about";
type SettingsIconName = "preferences" | "shortcuts" | "about" | "folder";

const adbStore = useAdbStore();
const { t } = useI18n();
const activeSection = ref<SettingsSection>("preferences");
const adbStoreRefs = storeToRefs(adbStore);
const {
  language,
  theme,
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
      label: t(`shortcuts.${shortcut.action}.label`),
      description: t(`shortcuts.${shortcut.action}.description`),
      value,
      displayValue: displayAppShortcutValue(value) || displayAppShortcutValue(shortcut.defaultValue),
    };
  }),
);

const settingsSections: Array<{ key: SettingsSection; titleKey: string; icon: SettingsIconName }> = [
  { key: "preferences", titleKey: "common.preferences", icon: "preferences" },
  { key: "shortcuts", titleKey: "common.shortcuts", icon: "shortcuts" },
  { key: "about", titleKey: "common.about", icon: "about" },
];

const settingsIconPaths: Record<SettingsIconName, string[]> = {
  preferences: ["M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5z", "M19.4 15a1.8 1.8 0 0 0 .4 2l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.8 1.8 0 0 0-2-.4 1.8 1.8 0 0 0-1 1.6V21a2 2 0 1 1-4 0v-.1a1.8 1.8 0 0 0-1-1.6 1.8 1.8 0 0 0-2 .4l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.8 1.8 0 0 0 .4-2 1.8 1.8 0 0 0-1.6-1H3a2 2 0 1 1 0-4h.1a1.8 1.8 0 0 0 1.6-1 1.8 1.8 0 0 0-.4-2l-.1-.1A2 2 0 1 1 7 4.1l.1.1a1.8 1.8 0 0 0 2 .4 1.8 1.8 0 0 0 1-1.6V3a2 2 0 1 1 4 0v.1a1.8 1.8 0 0 0 1 1.6 1.8 1.8 0 0 0 2-.4l.1-.1A2 2 0 1 1 20 7l-.1.1a1.8 1.8 0 0 0-.4 2 1.8 1.8 0 0 0 1.6 1h.1a2 2 0 1 1 0 4h-.1a1.8 1.8 0 0 0-1.7 1z"],
  shortcuts: ["M4 5h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z", "M6 9h.01", "M10 9h.01", "M14 9h.01", "M18 9h.01", "M7 13h10", "M9 16h6"],
  about: ["M12 16v-4", "M12 8h.01", "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z"],
  folder: ["M3 7h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z", "M3 7V5a2 2 0 0 1 2-2h4l2 2h5"],
};

const screenshotModeOptions: Array<{ value: ScreenshotMode; label: string }> = [
  { value: "save", label: "settings.screenshotModeSave" },
  { value: "copy", label: "settings.screenshotModeCopy" },
  { value: "save-copy", label: "settings.screenshotModeSaveCopy" },
];

const screenshotSourceOptions: Array<{ value: ScreenshotSource; label: string }> = [
  { value: "native", label: "settings.screenshotSourceNative" },
  { value: "mirror", label: "settings.screenshotSourceMirror" },
];

const pickerCopyFormatOptions: Array<{ value: PickerCopyFormat; label: string }> = [
  { value: "hex", label: "HEX" },
  { value: "rgb", label: "RGB" },
];

const mirrorToolbarSideOptions: Array<{ value: MirrorToolbarSide; label: string }> = [
  { value: "right", label: "settings.toolbarRight" },
  { value: "left", label: "settings.toolbarLeft" },
];

async function pickDownloadDir() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: t("settings.pickDirTitle"),
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

function updateLanguage(value: string) {
  adbStore.setLanguage(value as AppLanguage);
}

function updateTheme(value: string) {
  adbStore.setTheme(value as AppTheme);
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
    adbStore.setNotice("error", t("settings.duplicateShortcut", { label: t(`shortcuts.${duplicate.action}.label`) }));
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
    <nav class="settings-tabbar" aria-label="设置分类">
      <div class="settings-tab-list">
        <button
          v-for="item in settingsSections"
          :key="item.key"
          type="button"
          class="settings-tab"
          :class="{ active: activeSection === item.key }"
          @click="activeSection = item.key"
        >
          <svg class="settings-tab-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path v-for="path in settingsIconPaths[item.icon]" :key="path" :d="path" />
          </svg>
          <span>{{ t(item.titleKey) }}</span>
        </button>
      </div>
    </nav>

    <section class="settings-content">
      <div v-if="activeSection === 'preferences'" class="settings-panel">
        <header class="settings-head">
          <h4>{{ t("common.preferences") }}</h4>
        </header>

        <div class="settings-grid">
          <label>
            <span>{{ t("settings.language") }}</span>
            <div class="segmented-control two compact language-control">
              <button
                v-for="item in appLanguageOptions"
                :key="item.value"
                type="button"
                :class="{ active: language === item.value }"
                @click="updateLanguage(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
          </label>

          <label>
            <span>{{ t("settings.theme") }}</span>
            <div class="segmented-control theme-control">
              <button
                v-for="item in appThemeOptions"
                :key="item.value"
                type="button"
                :class="{ active: theme === item.value }"
                @click="updateTheme(item.value)"
              >
                {{ t(item.labelKey) }}
              </button>
            </div>
          </label>

          <label>
            <span>{{ t("settings.defaultLocalDir") }}</span>
            <div class="field-row">
              <input
                class="input mono"
                :value="defaultLocalDir"
                :placeholder="t('settings.defaultLocalDirPlaceholder')"
                @input="adbStore.setDefaultLocalDir(($event.target as HTMLInputElement).value)"
              />
              <button class="settings-icon-btn" type="button" :title="t('settings.chooseDefaultLocalDir')" :aria-label="t('settings.chooseDefaultLocalDir')" @click="pickDownloadDir">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path v-for="path in settingsIconPaths.folder" :key="path" :d="path" />
                </svg>
              </button>
            </div>
            <small class="field-note">{{ t("settings.defaultLocalDirNote") }}</small>
          </label>

          <label>
            <span>{{ t("settings.defaultDevicePath") }}</span>
            <input
              class="input mono"
              :value="defaultDevicePath"
              placeholder="/storage/emulated/0/Download/"
              @input="updateDefaultDevicePath(($event.target as HTMLInputElement).value)"
            />
          </label>

          <label>
            <span>{{ t("settings.screenshotMode") }}</span>
            <div class="segmented-control">
              <button
                v-for="item in screenshotModeOptions"
                :key="item.value"
                type="button"
                :class="{ active: screenshotMode === item.value }"
                @click="adbStore.setScreenshotMode(item.value)"
              >
                {{ t(item.label) }}
              </button>
            </div>
          </label>

          <label>
            <span>{{ t("settings.screenshotSource") }}</span>
            <div class="segmented-control two">
              <button
                v-for="item in screenshotSourceOptions"
                :key="item.value"
                type="button"
                :class="{ active: screenshotSource === item.value }"
                @click="adbStore.setScreenshotSource(item.value)"
              >
                {{ t(item.label) }}
              </button>
            </div>
            <small class="field-note">{{ t("settings.screenshotSourceNote") }}</small>
          </label>

          <label>
            <span>{{ t("settings.pickerCopy") }}</span>
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
            <small class="field-note">{{ t("settings.pickerCopyNote") }}</small>
          </label>

          <label>
            <span>{{ t("settings.toolbarSide") }}</span>
            <div class="segmented-control two">
              <button
                v-for="item in mirrorToolbarSideOptions"
                :key="item.value"
                type="button"
                :class="{ active: mirrorToolbarSide === item.value }"
                @click="adbStore.setMirrorToolbarSide(item.value)"
              >
                {{ t(item.label) }}
              </button>
            </div>
          </label>
        </div>
      </div>

      <div v-else-if="activeSection === 'shortcuts'" class="settings-panel shortcuts-panel">
        <header class="settings-head">
          <h4>{{ t("common.shortcuts") }}</h4>
          <p>{{ t("settings.shortcutsNote") }}</p>
        </header>

        <section class="settings-shortcuts" :aria-label="t('common.shortcuts')">
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
                :aria-label="t('settings.shortcutChangeAria', { label: shortcut.label })"
                :title="capturingShortcutAction === shortcut.action ? t('settings.shortcutCapturingTitle') : t('settings.shortcutChangeTitle')"
                @click="beginShortcutCapture(shortcut.action)"
                @keydown="onShortcutCapture($event, shortcut.action)"
              >
                {{ capturingShortcutAction === shortcut.action ? t("settings.shortcutCapture") : shortcut.displayValue }}
              </button>
              <button class="settings-shortcut-reset" type="button" :title="t('settings.shortcutReset')" :aria-label="t('settings.shortcutReset')" @click="resetShortcut(shortcut.action)">
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M3 12a9 9 0 1 0 2.6-6.4" />
                  <path d="M3 4v6h6" />
                </svg>
              </button>
            </div>
          </div>
        </section>
      </div>

      <AboutPage v-else />
    </section>
  </div>
</template>

<style scoped>
.settings-layout {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.settings-tabbar {
  display: flex;
  align-items: center;
  min-width: 0;
  height: 42px;
  min-height: 42px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-solid);
  padding: 6px 8px;
  box-shadow: 0 1px 0 rgb(15 23 42 / 0.06);
}

.settings-tab-list {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  min-width: 0;
  overflow-x: auto;
  overflow-y: hidden;
  overscroll-behavior-x: contain;
  scrollbar-width: none;
}

.settings-tab-list::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.settings-tab {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  flex: 0 0 124px;
  min-width: 0;
  height: 30px;
  border: 1px solid var(--control-border);
  border-radius: 7px;
  background: var(--control-bg);
  color: var(--control-text);
  padding: 0 12px;
  cursor: pointer;
  font: inherit;
  user-select: none;
  -webkit-user-select: none;
}

.settings-tab:hover,
.settings-tab.active {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.settings-tab span {
  min-width: 0;
  overflow: hidden;
  color: inherit;
  font-size: 12px;
  font-weight: 700;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-tab-icon {
  width: 15px;
  height: 15px;
  flex: 0 0 auto;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
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

.language-control {
  width: 180px;
}

.theme-control {
  width: min(360px, 100%);
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
  color: var(--primary-contrast);
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
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-solid);
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
  background: var(--control-bg-hover);
  color: var(--text-main);
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

:deep(.about-page) {
  margin: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
}

:deep(.about-page.panel) {
  padding: 12px;
}

:deep(.about-page h2) {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 20px;
  font-weight: 600;
  line-height: 1.25;
}

:deep(.about-item) {
  background: var(--surface-alt);
}

@media (max-width: 700px) {
  .settings-tab {
    flex-basis: 104px;
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

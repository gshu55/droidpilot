import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import {
  defaultAppShortcutMap,
  normalizeAppShortcutMap,
  normalizeAppShortcutValue,
  type AppShortcutAction,
  type AppShortcutMap,
} from "../config/appShortcuts";

export interface AdbAvailability {
  available: boolean;
  version?: string;
  error?: string;
}

export interface AdbDevice {
  serial: string;
  status: string;
}

export interface AdbExecResult {
  success: boolean;
  statusCode: number;
  stdout: string;
  stderr: string;
  fullCommand: string;
}

export type CommandRunStatus = "success" | "failed";
export type TransferDirection = "push" | "pull";
export type NoticeType = "success" | "error" | "info";
export type ScreenshotMode = "save" | "copy" | "save-copy";
export type ScreenshotSource = "native" | "mirror";
export type PickerCopyFormat = "hex" | "rgb";
export type MirrorToolbarSide = "left" | "right";

export interface Notice {
  type: NoticeType;
  message: string;
}

export interface CommandRecord {
  id: string;
  command: string;
  deviceSerial: string;
  status: CommandRunStatus;
  statusCode: number;
  stdout: string;
  stderr: string;
  fullCommand: string;
  executedAt: string;
}

export interface TransferRecord {
  id: string;
  direction: TransferDirection;
  deviceSerial: string;
  localPath: string;
  remotePath: string;
  status: CommandRunStatus;
  statusCode: number;
  stdout: string;
  stderr: string;
  fullCommand: string;
  executedAt: string;
}

interface PersistedState {
  commandHistory: string[];
  favoriteCommands: string[];
  commandRecords: CommandRecord[];
  transferRecords: TransferRecord[];
  selectedSerial: string;
  defaultDevicePath: string;
  defaultLocalDir: string;
  screenshotMode: ScreenshotMode;
  screenshotSource: ScreenshotSource;
  pickerCopyShortcut: string;
  pickerCopyFormat: PickerCopyFormat;
  mirrorBitRateMbps: number;
  mirrorMaxFps: number;
  mirrorToolbarSide: MirrorToolbarSide;
  appShortcuts: AppShortcutMap;
}

interface AdbState {
  checking: boolean;
  loadingDevices: boolean;
  availability: AdbAvailability;
  devices: AdbDevice[];
  selectedSerial: string;
  lastUpdatedAt: string;
  errorMessage: string;
  notice: Notice | null;

  commandRunningCount: number;
  commandInput: string;
  commandHistory: string[];
  favoriteCommands: string[];
  commandRecords: CommandRecord[];

  transferRunning: boolean;
  uploadLocalPath: string;
  defaultDevicePath: string;
  defaultLocalDir: string;
  screenshotMode: ScreenshotMode;
  screenshotSource: ScreenshotSource;
  pickerCopyShortcut: string;
  pickerCopyFormat: PickerCopyFormat;
  mirrorBitRateMbps: number;
  mirrorMaxFps: number;
  mirrorToolbarSide: MirrorToolbarSide;
  appShortcuts: AppShortcutMap;
  transferRecords: TransferRecord[];
}

const APP_STORAGE_KEY = "droidpilot.persist.v1";
const LEGACY_APP_STORAGE_KEY = "adbtools.persist.v1";
const MAX_HISTORY_COUNT = 30;
const MAX_RECORD_COUNT = 80;

export const COMMON_REMOTE_PATHS = [
  "/storage/emulated/0/",
  "/storage/emulated/0/Download/",
  "/storage/emulated/0/DCIM/",
  "/data/local/tmp/",
];

function makeId() {
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function safeArrayOfString(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is string => typeof item === "string");
}

function safeCommandRecords(value: unknown): CommandRecord[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is CommandRecord => !!item && typeof item === "object");
}

function safeTransferRecords(value: unknown): TransferRecord[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item): item is TransferRecord => !!item && typeof item === "object");
}

function safePositiveNumber(value: unknown, fallback: number) {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : fallback;
}

function safeScreenshotMode(value: unknown): ScreenshotMode {
  return value === "save" || value === "copy" || value === "save-copy" ? value : "save-copy";
}

function safeScreenshotSource(value: unknown): ScreenshotSource {
  return value === "mirror" || value === "native" ? value : "native";
}

function safePickerCopyShortcut(value: unknown) {
  if (typeof value !== "string" || !value.trim()) {
    return "C";
  }
  return value.trim() === "Ctrl+Shift+C" ? "C" : value.trim();
}

function safePickerCopyFormat(value: unknown): PickerCopyFormat {
  return value === "rgb" || value === "hex" ? value : "hex";
}

function safeMirrorToolbarSide(value: unknown): MirrorToolbarSide {
  return value === "left" || value === "right" ? value : "right";
}

function normalizeDefaultRemoteDir(value: unknown, fallback: string) {
  if (typeof value !== "string" || !value.trim()) {
    return fallback;
  }
  const trimmed = value.trim();
  return trimmed === "/sdcard" || trimmed === "/sdcard/" ? fallback : value;
}

function loadPersistedState(): PersistedState {
  const raw = localStorage.getItem(APP_STORAGE_KEY) || localStorage.getItem(LEGACY_APP_STORAGE_KEY);
  if (!raw) {
    return {
      commandHistory: [],
      favoriteCommands: [],
      commandRecords: [],
      transferRecords: [],
      selectedSerial: "",
      defaultDevicePath: "/storage/emulated/0/",
      defaultLocalDir: "",
      screenshotMode: "save-copy",
      screenshotSource: "native",
      pickerCopyShortcut: "C",
      pickerCopyFormat: "hex",
      mirrorBitRateMbps: 8,
      mirrorMaxFps: 60,
      mirrorToolbarSide: "right",
      appShortcuts: { ...defaultAppShortcutMap },
    };
  }

  try {
    const parsed = JSON.parse(raw) as Partial<PersistedState>;
    const legacy = parsed as Partial<PersistedState> & {
      uploadRemotePath?: unknown;
      downloadRemotePath?: unknown;
      downloadLocalDir?: unknown;
    };
    const defaultDevicePath =
      typeof parsed.defaultDevicePath === "string" && parsed.defaultDevicePath.trim()
        ? parsed.defaultDevicePath
        : typeof legacy.downloadRemotePath === "string" && legacy.downloadRemotePath.trim()
          ? legacy.downloadRemotePath
          : normalizeDefaultRemoteDir(legacy.uploadRemotePath, "/storage/emulated/0/");
    const defaultLocalDir =
      typeof parsed.defaultLocalDir === "string"
        ? parsed.defaultLocalDir
        : typeof legacy.downloadLocalDir === "string"
          ? legacy.downloadLocalDir
          : "";

    return {
      commandHistory: safeArrayOfString(parsed.commandHistory).slice(0, MAX_HISTORY_COUNT),
      favoriteCommands: safeArrayOfString(parsed.favoriteCommands).slice(0, MAX_HISTORY_COUNT),
      commandRecords: safeCommandRecords(parsed.commandRecords).slice(0, MAX_RECORD_COUNT),
      transferRecords: safeTransferRecords(parsed.transferRecords).slice(0, MAX_RECORD_COUNT),
      selectedSerial: typeof parsed.selectedSerial === "string" ? parsed.selectedSerial : "",
      defaultDevicePath,
      defaultLocalDir,
      screenshotMode: safeScreenshotMode(parsed.screenshotMode),
      screenshotSource: safeScreenshotSource(parsed.screenshotSource),
      pickerCopyShortcut: safePickerCopyShortcut(parsed.pickerCopyShortcut),
      pickerCopyFormat: safePickerCopyFormat(parsed.pickerCopyFormat),
      mirrorBitRateMbps: safePositiveNumber(parsed.mirrorBitRateMbps, 8),
      mirrorMaxFps: safePositiveNumber(parsed.mirrorMaxFps, 60),
      mirrorToolbarSide: safeMirrorToolbarSide(parsed.mirrorToolbarSide),
      appShortcuts: normalizeAppShortcutMap(parsed.appShortcuts),
    };
  } catch {
    return {
      commandHistory: [],
      favoriteCommands: [],
      commandRecords: [],
      transferRecords: [],
      selectedSerial: "",
      defaultDevicePath: "/storage/emulated/0/",
      defaultLocalDir: "",
      screenshotMode: "save-copy",
      screenshotSource: "native",
      pickerCopyShortcut: "C",
      pickerCopyFormat: "hex",
      mirrorBitRateMbps: 8,
      mirrorMaxFps: 60,
      mirrorToolbarSide: "right",
      appShortcuts: { ...defaultAppShortcutMap },
    };
  }
}

function persistState(state: PersistedState) {
  localStorage.setItem(APP_STORAGE_KEY, JSON.stringify(state));
}

export const useAdbStore = defineStore("adb", {
  state: (): AdbState => {
    const persisted = loadPersistedState();

    return {
      checking: false,
      loadingDevices: false,
      availability: {
        available: false,
      },
      devices: [],
      selectedSerial: persisted.selectedSerial,
      lastUpdatedAt: "",
      errorMessage: "",
      notice: null,

      commandRunningCount: 0,
      commandInput: "",
      commandHistory: persisted.commandHistory,
      favoriteCommands: persisted.favoriteCommands,
      commandRecords: persisted.commandRecords,

      transferRunning: false,
      uploadLocalPath: "",
      defaultDevicePath: persisted.defaultDevicePath,
      defaultLocalDir: persisted.defaultLocalDir,
      screenshotMode: persisted.screenshotMode,
      screenshotSource: persisted.screenshotSource,
      pickerCopyShortcut: persisted.pickerCopyShortcut,
      pickerCopyFormat: persisted.pickerCopyFormat,
      mirrorBitRateMbps: persisted.mirrorBitRateMbps,
      mirrorMaxFps: persisted.mirrorMaxFps,
      mirrorToolbarSide: persisted.mirrorToolbarSide,
      appShortcuts: persisted.appShortcuts,
      transferRecords: persisted.transferRecords,
    };
  },
  getters: {
    hasDevices: (state) => state.devices.length > 0,
    selectedDevice: (state) => state.devices.find((item) => item.serial === state.selectedSerial),
    commandRunning: (state) => state.commandRunningCount > 0,
    canExecute: (state) => state.commandInput.trim().length > 0,
    isFavoriteCommand: (state) => (command: string) =>
      state.favoriteCommands.includes(command.trim()),
  },
  actions: {
    savePersisted() {
      persistState({
        commandHistory: this.commandHistory,
        favoriteCommands: this.favoriteCommands,
        commandRecords: this.commandRecords,
        transferRecords: this.transferRecords,
        selectedSerial: this.selectedSerial,
        defaultDevicePath: this.defaultDevicePath,
        defaultLocalDir: this.defaultLocalDir,
        screenshotMode: this.screenshotMode,
        screenshotSource: this.screenshotSource,
        pickerCopyShortcut: this.pickerCopyShortcut,
        pickerCopyFormat: this.pickerCopyFormat,
        mirrorBitRateMbps: this.mirrorBitRateMbps,
        mirrorMaxFps: this.mirrorMaxFps,
        mirrorToolbarSide: this.mirrorToolbarSide,
        appShortcuts: this.appShortcuts,
      });
    },
    setNotice(type: NoticeType, message: string) {
      this.notice = {
        type,
        message,
      };
    },
    clearNotice() {
      this.notice = null;
    },
    setError(message: string) {
      this.errorMessage = message;
      this.setNotice("error", message);
    },
    clearError() {
      this.errorMessage = "";
    },
    validateCommandInput(command: string): string | null {
      const normalized = command.trim();
      if (!normalized) {
        return "请输入要执行的 adb 子命令";
      }

      const placeholders = normalized.match(/<[A-Za-z][A-Za-z0-9_:-]*>/g);
      if (placeholders) {
        return `命令中的 ${placeholders.join("、")} 是模板占位符，请替换成真实值后再执行`;
      }

      const segments = normalized.split(/\s+/);
      const head = segments[0];

      if ((head === "install" || head === "uninstall") && segments.length < 2) {
        return `${head} 命令缺少参数`;
      }

      if ((head === "push" || head === "pull") && segments.length < 3) {
        return `${head} 命令参数不足，应包含源路径和目标路径`;
      }

      return null;
    },
    validateUpload(): string | null {
      if (!this.selectedSerial) {
        return "上传前请先选择设备";
      }
      if (!this.uploadLocalPath.trim()) {
        return "请选择本地文件或目录";
      }
      if (!this.defaultDevicePath.trim()) {
        return "请输入远端目标路径";
      }
      if (this.defaultDevicePath.includes("<") || this.defaultDevicePath.includes(">")) {
        return "远端路径包含占位符，请填写完整路径";
      }
      return null;
    },
    validateDownload(): string | null {
      if (!this.selectedSerial) {
        return "下载前请先选择设备";
      }
      if (!this.defaultDevicePath.trim()) {
        return "请输入远端源文件路径";
      }
      if (!this.defaultLocalDir.trim()) {
        return "请选择本地保存目录";
      }
      if (this.defaultDevicePath.includes("<") || this.defaultDevicePath.includes(">")) {
        return "远端路径包含占位符，请填写完整路径";
      }
      return null;
    },
    async checkAdb() {
      this.checking = true;
      this.clearError();

      try {
        this.availability = await invoke<AdbAvailability>("check_adb_availability");
        if (!this.availability.available) {
          this.setNotice("error", this.availability.error || "adb 不可用");
        }
      } catch (error) {
        this.availability = {
          available: false,
          error: String(error),
        };
        this.setError(String(error));
      } finally {
        this.checking = false;
      }
    },
    async fetchDevices() {
      this.loadingDevices = true;
      this.clearError();

      try {
        this.devices = await invoke<AdbDevice[]>("list_adb_devices");
        if (this.devices.length === 0) {
          this.selectedSerial = "";
          this.setNotice("info", "未检测到设备，请检查 USB 连接与授权状态");
          this.savePersisted();
          return;
        }

        if (!this.devices.some((item) => item.serial === this.selectedSerial)) {
          this.selectedSerial = this.devices[0].serial;
          this.savePersisted();
        }
      } catch (error) {
        this.devices = [];
        this.selectedSerial = "";
        this.setError(String(error));
      } finally {
        this.loadingDevices = false;
      }
    },
    selectDevice(serial: string) {
      this.selectedSerial = serial;
      this.savePersisted();
    },
    setCommandInput(value: string) {
      this.commandInput = value;
    },
    applyHistoryCommand(command: string) {
      this.commandInput = command;
    },
    clearRecords() {
      this.commandRecords = [];
      this.savePersisted();
    },
    clearCommandHistory() {
      this.commandHistory = [];
      this.savePersisted();
      this.setNotice("success", "最近历史已清空");
    },
    clearTransferRecords() {
      this.transferRecords = [];
      this.savePersisted();
    },
    pushHistory(command: string) {
      const normalized = command.trim();
      if (!normalized) {
        return;
      }

      const next = [normalized, ...this.commandHistory.filter((item) => item !== normalized)].slice(
        0,
        MAX_HISTORY_COUNT,
      );
      this.commandHistory = next;
      this.savePersisted();
    },
    toggleFavorite(command: string) {
      const normalized = command.trim();
      if (!normalized) {
        this.setNotice("info", "请输入命令后再收藏");
        return;
      }

      const exists = this.favoriteCommands.includes(normalized);
      this.favoriteCommands = exists
        ? this.favoriteCommands.filter((item) => item !== normalized)
        : [normalized, ...this.favoriteCommands].slice(0, MAX_HISTORY_COUNT);
      this.savePersisted();
      this.setNotice("success", exists ? "已取消收藏" : "已加入收藏");
    },
    setUploadLocalPath(value: string) {
      this.uploadLocalPath = value;
    },
    setDefaultDevicePath(value: string) {
      this.defaultDevicePath = value;
      this.savePersisted();
    },
    setDefaultLocalDir(value: string) {
      this.defaultLocalDir = value;
      this.savePersisted();
    },
    setScreenshotMode(value: ScreenshotMode) {
      this.screenshotMode = value;
      this.savePersisted();
    },
    setScreenshotSource(value: ScreenshotSource) {
      this.screenshotSource = value;
      this.savePersisted();
    },
    setPickerCopyShortcut(value: string) {
      this.pickerCopyShortcut = safePickerCopyShortcut(value);
      this.savePersisted();
    },
    setPickerCopyFormat(value: PickerCopyFormat) {
      this.pickerCopyFormat = value;
      this.savePersisted();
    },
    setMirrorBitRateMbps(value: number) {
      if (!Number.isFinite(value) || value <= 0) {
        return;
      }
      this.mirrorBitRateMbps = value;
      this.savePersisted();
    },
    setMirrorMaxFps(value: number) {
      if (!Number.isFinite(value) || value <= 0) {
        return;
      }
      this.mirrorMaxFps = value;
      this.savePersisted();
    },
    setMirrorToolbarSide(value: MirrorToolbarSide) {
      this.mirrorToolbarSide = value;
      this.savePersisted();
    },
    setAppShortcut(action: AppShortcutAction, value: string) {
      const normalized = normalizeAppShortcutValue(value) || defaultAppShortcutMap[action];
      this.appShortcuts = {
        ...normalizeAppShortcutMap(this.appShortcuts),
        [action]: normalized,
      };
      this.savePersisted();
    },
    resetAppShortcuts() {
      this.appShortcuts = { ...defaultAppShortcutMap };
      this.savePersisted();
    },
    async disconnectWifiDevice(serial: string) {
      const target = serial.trim();
      if (!target) {
        this.setNotice("error", "设备 serial 不能为空");
        return;
      }
      if (!target.includes(":")) {
        this.setNotice("info", "仅支持断开 Wi-Fi ADB 设备");
        return;
      }

      await this.executeCommand(`disconnect ${target}`);
      await this.fetchDevices();

      if (!this.devices.some((item) => item.serial === this.selectedSerial)) {
        this.selectedSerial = this.devices[0]?.serial ?? "";
        this.savePersisted();
      }

      const latest = this.commandRecords[0];
      if (latest && latest.command === `disconnect ${target}` && latest.status === "success") {
        this.setNotice("success", `已断开设备 ${target}`);
      }
    },
    async executeCommand(command?: string) {
      const finalCommand = (command ?? this.commandInput).trim();
      const validationError = this.validateCommandInput(finalCommand);
      if (validationError) {
        this.setError(validationError);
        return;
      }

      this.commandRunningCount++;
      this.clearError();

      try {
        const result = await invoke<AdbExecResult>("execute_adb_command", {
          commandInput: finalCommand,
          serial: this.selectedSerial || null,
        });

        const record: CommandRecord = {
          id: makeId(),
          command: finalCommand,
          deviceSerial: this.selectedSerial,
          status: result.success ? "success" : "failed",
          statusCode: result.statusCode,
          stdout: result.stdout,
          stderr: result.stderr,
          fullCommand: result.fullCommand,
          executedAt: new Date().toLocaleString(),
        };

        this.commandRecords = [record, ...this.commandRecords].slice(0, MAX_RECORD_COUNT);
        this.pushHistory(finalCommand);
        this.savePersisted();

        if (result.success) {
          this.setNotice("success", "命令执行完成");
        } else {
          this.setNotice("error", result.stderr || `命令执行失败，退出码 ${result.statusCode}`);
        }
      } catch (error) {
        const message = String(error);
        this.setError(message);
        const record: CommandRecord = {
          id: makeId(),
          command: finalCommand,
          deviceSerial: this.selectedSerial,
          status: "failed",
          statusCode: -1,
          stdout: "",
          stderr: message,
          fullCommand: `adb ${finalCommand}`,
          executedAt: new Date().toLocaleString(),
        };
        this.commandRecords = [record, ...this.commandRecords].slice(0, MAX_RECORD_COUNT);
        this.savePersisted();
      } finally {
        this.commandRunningCount--;
      }
    },
    async uploadFile() {
      const error = this.validateUpload();
      if (error) {
        this.setError(error);
        return;
      }

      await this.runTransfer("push", this.uploadLocalPath, this.defaultDevicePath);
    },
    async quickInstallApk() {
      if (!this.selectedSerial) {
        this.setError("安装前请先选择设备");
        return;
      }

      const apkPath = this.uploadLocalPath.trim();
      if (!apkPath) {
        this.setError("请先选择本地 APK 文件");
        return;
      }

      if (!apkPath.toLowerCase().endsWith(".apk")) {
        this.setError("仅支持安装 .apk 文件");
        return;
      }

      this.transferRunning = true;
      this.clearError();

      try {
        const result = await invoke<AdbExecResult>("install_apk_file", {
          serial: this.selectedSerial,
          apkPath,
        });

        const record: CommandRecord = {
          id: makeId(),
          command: `install \"${apkPath}\"`,
          deviceSerial: this.selectedSerial,
          status: result.success ? "success" : "failed",
          statusCode: result.statusCode,
          stdout: result.stdout,
          stderr: result.stderr,
          fullCommand: result.fullCommand,
          executedAt: new Date().toLocaleString(),
        };

        this.commandRecords = [record, ...this.commandRecords].slice(0, MAX_RECORD_COUNT);
        this.pushHistory(record.command);
        this.savePersisted();

        if (result.success) {
          this.setNotice("success", "APK 安装完成");
        } else {
          this.setNotice("error", result.stderr || `APK 安装失败，退出码 ${result.statusCode}`);
        }
      } catch (error) {
        this.setError(String(error));
      } finally {
        this.transferRunning = false;
      }
    },
    async downloadFile() {
      const error = this.validateDownload();
      if (error) {
        this.setError(error);
        return;
      }

      await this.runTransfer("pull", this.defaultLocalDir, this.defaultDevicePath);
    },
    async runTransfer(direction: TransferDirection, localPath: string, remotePath: string) {
      this.transferRunning = true;
      this.clearError();

      try {
        const result = await invoke<AdbExecResult>("transfer_file", {
          request: {
            direction,
            serial: this.selectedSerial,
            localPath,
            remotePath,
          },
        });

        const record: TransferRecord = {
          id: makeId(),
          direction,
          deviceSerial: this.selectedSerial,
          localPath,
          remotePath,
          status: result.success ? "success" : "failed",
          statusCode: result.statusCode,
          stdout: result.stdout,
          stderr: result.stderr,
          fullCommand: result.fullCommand,
          executedAt: new Date().toLocaleString(),
        };
        this.transferRecords = [record, ...this.transferRecords].slice(0, MAX_RECORD_COUNT);
        this.savePersisted();

        if (result.success) {
          this.setNotice("success", direction === "push" ? "上传完成" : "下载完成");
        } else {
          this.setNotice("error", result.stderr || `传输失败，退出码 ${result.statusCode}`);
        }
      } catch (error) {
        const message = String(error);
        this.setError(message);

        const record: TransferRecord = {
          id: makeId(),
          direction,
          deviceSerial: this.selectedSerial,
          localPath,
          remotePath,
          status: "failed",
          statusCode: -1,
          stdout: "",
          stderr: message,
          fullCommand: `adb -s ${this.selectedSerial} ${direction} ...`,
          executedAt: new Date().toLocaleString(),
        };
        this.transferRecords = [record, ...this.transferRecords].slice(0, MAX_RECORD_COUNT);
        this.savePersisted();
      } finally {
        this.transferRunning = false;
      }
    },
    async refreshAll() {
      this.clearNotice();
      await this.checkAdb();
      if (this.availability.available) {
        await this.fetchDevices();
        if (this.devices.length > 0) {
          this.setNotice("success", `已连接 ${this.devices.length} 台设备`);
        }
      } else {
        this.devices = [];
        this.selectedSerial = "";
      }
      this.lastUpdatedAt = new Date().toLocaleString();
    },
  },
});

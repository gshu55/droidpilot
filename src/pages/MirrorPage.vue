<script setup lang="ts">
import { computed, markRaw, nextTick, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import { storeToRefs } from "pinia";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { downloadDir, homeDir } from "@tauri-apps/api/path";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { useAdbStore } from "../stores/adb";
import { appShortcutDefinitions, appShortcutKeyParts, defaultAppShortcutMap, normalizeAppShortcutMap, type AppShortcut } from "../config/appShortcuts";
import { useI18n } from "../i18n";
import { formatBytes, formatEntrySize } from "../utils/fileEntryFormat";
import { clampSplitPanePercent, splitPaneGridTemplateColumns } from "../utils/splitPane";
import SettingsPage from "./SettingsPage.vue";

const SYNTHETIC_PINCH_POINTER_A = 100_000;
const SYNTHETIC_PINCH_POINTER_B = 100_001;
const SYNTHETIC_PAN_POINTER_A = 100_002;
const SYNTHETIC_PAN_POINTER_B = 100_003;
const SYNTHETIC_PINCH_END_DELAY_MS = 180;

interface AdbExecResult {
  success: boolean;
  statusCode: number;
  stdout: string;
  stderr: string;
  fullCommand: string;
}

interface ScrcpyStartResult {
  sessionId: number;
  serial: string;
  deviceName: string;
  codec: string;
  width: number;
  height: number;
  version: string;
}

interface ScrcpyFramePayload {
  sessionId: number;
  pts: number;
  keyFrame: boolean;
  config: boolean;
  data: string;
}

interface ScrcpyStatusPayload {
  sessionId: number;
  status: string;
  message: string;
}

interface RemoteEntry {
  name: string;
  path: string;
  isDir: boolean;
  sizeBytes?: number | null;
  createdAtMs?: number | null;
}

interface LocalEntry {
  name: string;
  path: string;
  isDir: boolean;
  sizeBytes?: number | null;
  createdAtMs?: number | null;
}

type TransferEntryScope = "local" | "remote";
type TransferEntry = LocalEntry | RemoteEntry;
type FileEntryIconName = "folder" | "apk" | "image" | "video" | "pdf" | "code" | "text" | "archive" | "audio" | "file";
type FileEntrySortKey = "name" | "size" | "date";
type FileEntrySortDirection = "asc" | "desc";
type TransferToolIconName = "chooseFolder" | "copyToDevice" | "copyToComputer" | "apk" | "preview" | "refresh" | "delete";
type TransferMenuAction = "copyToDevice" | "copyToComputer" | "install" | "preview" | "refresh" | "delete";

interface FileEntrySortState {
  key: FileEntrySortKey;
  direction: FileEntrySortDirection;
}

interface TransferContextMenuState {
  scope: TransferEntryScope;
  entry: TransferEntry;
  x: number;
  y: number;
}

interface TransferContextMenuItem {
  action: TransferMenuAction;
  label: string;
  icon: TransferToolIconName;
  disabled: boolean;
  danger?: boolean;
}

interface TransferProgressPayload {
  direction: "push" | "pull";
  localPath: string;
  remotePath: string;
  percent: number | null;
  message: string;
  done: boolean;
  success: boolean | null;
}

interface DeleteResult {
  success: boolean;
  message: string;
}

interface TextPreviewMeta {
  path: string;
  name: string;
  sizeBytes: number;
}

interface TextPreviewChunk {
  offset: number;
  nextOffset: number;
  sizeBytes: number;
  text: string;
  eof: boolean;
}

type TextPreviewSource = "local" | "remote";
type TextPreviewMode = "text" | "html" | "pdf" | "image" | "video";
interface TextPreviewLine {
  id: number;
  text: string;
}

interface PreviewFileResult {
  path: string;
  name: string;
  sizeBytes: number;
}

interface ImagePreviewResult extends PreviewFileResult {
  dataUrl: string;
}

interface TerminalSpawnResult {
  sessionId: number;
  shell: string;
}

interface TerminalOutputPayload {
  sessionId: number;
  dataBase64: string;
}

interface TerminalExitPayload {
  sessionId: number;
  code: number | null;
  message: string;
}

interface DeviceScreenshotResult {
  dataBase64: string;
}

type MirrorState = "idle" | "starting" | "running" | "stopping" | "error";
type DeleteConfirmTarget = { scope: "local" | "remote"; path: string } | null;
type QuickCommandGroup = "common" | "device" | "screen" | "media";
type QuickCommandItem = { label: string; command: string; desc: string; group: QuickCommandGroup; tag?: string };
type TouchPoint = { x: number; y: number };
interface SyntheticTouchGesture {
  active: boolean;
  center: TouchPoint;
  distance: number;
  finishTimer: ReturnType<typeof window.setTimeout> | null;
}
interface SyntheticPanGesture {
  active: boolean;
  sourcePointerId: number | null;
  center: TouchPoint;
  distance: number;
}
interface MirrorTab {
  id: number;
  title: string;
  serial: string;
  state: MirrorState;
  statusText: string;
  startToken: number;
  startRecoveryCount: number;
  autoRestarting: boolean;
  session: ScrcpyStartResult | null;
  decodedFrames: number;
  droppedFrames: number;
  decoderReady: boolean;
  canvas: HTMLCanvasElement | null;
  decoder: any;
  waitingForCanvas: boolean;
  configuredCodec: string;
  latestConfigPacket: Uint8Array | null;
  pendingConfigPacket: Uint8Array | null;
  activePointers: Map<number, number>;
  syntheticPinch: SyntheticTouchGesture;
  syntheticPan: SyntheticPanGesture;
  lastMoveAt: Map<number, number>;
}
type ImageEditorTool = "crop" | "rect" | "arrow" | "polyline" | "mosaic" | "text" | "eraser" | "picker";
type ImageEditorShape =
  | { id: string; type: "rect"; x1: number; y1: number; x2: number; y2: number; color: string; lineWidth: number }
  | { id: string; type: "arrow"; x1: number; y1: number; x2: number; y2: number; color: string; lineWidth: number }
  | { id: string; type: "polyline"; points: Array<{ x: number; y: number }>; color: string; lineWidth: number }
  | { id: string; type: "mosaic"; points: Array<{ x: number; y: number }>; blockSize: number }
  | { id: string; type: "eraser"; points: Array<{ x: number; y: number }>; lineWidth: number }
  | { id: string; type: "text"; x: number; y: number; text: string; color: string; fontSize: number };
interface ImageEditorSnapshot {
  baseDataUrl: string;
  width: number;
  height: number;
  shapes: ImageEditorShape[];
  cropRect: { x1: number; y1: number; x2: number; y2: number } | null;
}
type CropHandle = "left" | "right" | "top" | "bottom" | "topLeft" | "topRight" | "bottomLeft" | "bottomRight";
type CropDragState =
  | { mode: "new"; startPoint: { x: number; y: number } }
  | { mode: "move"; startPoint: { x: number; y: number }; startRect: { x1: number; y1: number; x2: number; y2: number } }
  | { mode: "resize"; handle: CropHandle; startPoint: { x: number; y: number }; startRect: { x1: number; y1: number; x2: number; y2: number } };
type ShapeMoveState = { shapeId: string; startPoint: { x: number; y: number }; startShape: ImageEditorShape; moved: boolean; historyCaptured: boolean };
type ShapeResizeState = {
  shapeId: string;
  handle: CropHandle;
  startPoint: { x: number; y: number };
  startShape: ImageEditorShape;
  startBounds: { left: number; top: number; right: number; bottom: number };
  moved: boolean;
  historyCaptured: boolean;
};
interface ImageEditorTab {
  id: number;
  key: string;
  title: string;
  source: "native" | "mirror";
  baseDataUrl: string;
  baseImage: HTMLImageElement;
  width: number;
  height: number;
  tool: ImageEditorTool;
  color: string;
  lineWidth: number;
  shapes: ImageEditorShape[];
  draft: ImageEditorShape | null;
  cropRect: { x1: number; y1: number; x2: number; y2: number } | null;
  cropDrag: CropDragState | null;
  shapeMove: ShapeMoveState | null;
  shapeResize: ShapeResizeState | null;
  shapeHoverId: string | null;
  shapeResizeHover: CropHandle | null;
  cropHover: CropHandle | "move" | null;
  drawingStart: { x: number; y: number } | null;
  textInput: {
    id: string;
    x: number;
    y: number;
    value: string;
    color?: string;
    fontSize?: number;
    historyCaptured?: boolean;
    originalShape?: Extract<ImageEditorShape, { type: "text" }>;
  } | null;
  pickerPreview: { x: number; y: number; color: string; rgb: [number, number, number]; magnifier: string } | null;
  undoStack: ImageEditorSnapshot[];
  redoStack: ImageEditorSnapshot[];
  saving: boolean;
}
type WorkspaceTab =
  | { key: string; kind: "mirror"; title: string; status: string; running: boolean; closable: boolean; mirror: MirrorTab }
  | { key: string; kind: "screenshot"; title: string; status: string; running: boolean; closable: boolean; editor: ImageEditorTab }
  | { key: string; kind: "textPreview"; title: string; status: string; running: boolean; closable: boolean }
  | { key: string; kind: "transfer"; title: string; status: string; running: boolean; closable: boolean }
  | { key: string; kind: "terminal"; title: string; status: string; running: boolean; closable: boolean }
  | { key: string; kind: "settings"; title: string; status: string; running: boolean; closable: boolean };
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

const adbStore = useAdbStore();
const { t } = useI18n();
const adbStoreRefs = storeToRefs(adbStore);
const {
  selectedSerial,
  transferRunning,
  defaultLocalDir,
  screenshotMode,
  screenshotSource,
  pickerCopyShortcut,
  pickerCopyFormat,
  mirrorBitRateMbps,
  mirrorMaxFps,
} = adbStoreRefs;
const configuredAppShortcuts = computed(() => normalizeAppShortcutMap(adbStoreRefs.appShortcuts?.value));

const mirrorTabListRef = ref<HTMLDivElement | null>(null);
const terminalHostRef = ref<HTMLDivElement | null>(null);
const transferBodyRef = ref<HTMLDivElement | null>(null);
const transferPanelOpen = ref(false);
const recording = ref(false);
const recordingMirrorTabId = ref<number | null>(null);
const savingLocalRecording = ref(false);
const recordingElapsedSeconds = ref(0);
const defaultDeviceRoot = "/storage/emulated/0/";
const localDrivesRoot = "__DROIDPILOT_WINDOWS_DRIVES__";
const remoteBrowserPath = ref(defaultDeviceRoot);
const remoteEntries = ref<RemoteEntry[]>([]);
const browsingRemote = ref(false);
const remoteBrowseError = ref("");
const localBrowserPath = ref("");
const localEntries = ref<LocalEntry[]>([]);
const browsingLocal = ref(false);
const localBrowseError = ref("");
const selectedLocalPath = ref("");
const selectedRemotePath = ref("");
const localEntrySort = ref<FileEntrySortState>({ key: "name", direction: "asc" });
const remoteEntrySort = ref<FileEntrySortState>({ key: "name", direction: "asc" });
const transferContextMenu = ref<TransferContextMenuState | null>(null);
const transferPaneMinWidth = 300;
const transferPaneHandleWidth = 8;
const transferPaneSplitPercent = ref(50);
const transferPaneResizeActive = ref(false);
let transferPaneResizePointerId: number | null = null;
let transferPaneResizeHandle: HTMLElement | null = null;
const fileDropActive = ref(false);
const fileDropBusy = ref(false);
const fileDropMessage = ref("");
const copyDirection = ref<"push" | "pull" | "install" | null>(null);
const copyProgressPercent = ref<number | null>(null);
const copyProgressMessage = ref("");
const deleteConfirmTarget = ref<DeleteConfirmTarget>(null);
const textPreviewOpen = ref(false);
const textPreviewSource = ref<TextPreviewSource>("local");
const textPreviewMode = ref<TextPreviewMode>("text");
const textPreviewMeta = ref<TextPreviewMeta | null>(null);
const textPreviewAssetUrl = ref("");
const textPreviewLines = ref<TextPreviewLine[]>([]);
const textPreviewPendingLine = ref("");
const textPreviewNextOffset = ref(0);
const textPreviewEof = ref(false);
const textPreviewLoading = ref(false);
const textPreviewError = ref("");
const textPreviewScrollTop = ref(0);
const textPreviewViewportHeight = ref(220);
const textPreviewBodyRef = ref<HTMLDivElement | null>(null);
const textPreviewRequestId = ref(0);
const textPreviewSearchQuery = ref("");
const textPreviewSearchCaseSensitive = ref(false);
const textPreviewSearchWholeWord = ref(false);
const textPreviewSearchRegex = ref(false);
const textPreviewSearchActiveLine = ref<number | null>(null);
const textPreviewSearchMessage = ref("");
const textPreviewSearchSearching = ref(false);
const imagePreviewScale = ref(1);
const imagePreviewOffset = ref({ x: 0, y: 0 });
const imagePreviewDragging = ref(false);
let imagePreviewDragState: {
  pointerId: number;
  startX: number;
  startY: number;
  offsetX: number;
  offsetY: number;
} | null = null;
const terminalSessionId = ref<number | null>(null);
const terminalShell = ref("");
const terminalStarting = ref(false);
const terminalQuickGroup = ref<QuickCommandGroup>("common");
const activeTerminalCommand = ref("");
const terminalPanelOpen = ref(false);
const settingsPanelOpen = ref(false);
const commandPickerCollapsed = ref(true);
const imageEditorTabs = ref<ImageEditorTab[]>([]);
const activeMirrorTabId = ref(0);
const activeWorkspaceTabKey = ref("");
const workspaceTabOrder = ref<string[]>([]);
const mirrorTabs = ref<MirrorTab[]>([]);

let unlistenFrame: UnlistenFn | null = null;
let unlistenStatus: UnlistenFn | null = null;
let unlistenTransferProgress: UnlistenFn | null = null;
let unlistenTerminalOutput: UnlistenFn | null = null;
let unlistenTerminalExit: UnlistenFn | null = null;
let unlistenFileDrop: UnlistenFn | null = null;
let resolveMirrorEventListenersReady: (() => void) | null = null;
let mirrorEventListenersReady = new Promise<void>((resolve) => {
  resolveMirrorEventListenersReady = resolve;
});
let localMediaRecorder: MediaRecorder | null = null;
let localRecordingStream: MediaStream | null = null;
let localRecordingChunks: Blob[] = [];
let localRecordingMimeType = "";
let terminal: Terminal | null = null;
let terminalFitAddon: FitAddon | null = null;
let terminalDataDisposable: { dispose: () => void } | null = null;
let terminalResizeObserver: ResizeObserver | null = null;
let terminalResizeTimer: number | null = null;
let terminalOutputQueue: Uint8Array[] = [];
let terminalOutputQueuedBytes = 0;
let terminalOutputWriteActive = false;
let transferContextMenuOpenedAt = 0;
let recordingTimer: number | null = null;
let recordingStartedAt = 0;
let terminalLastRows = 0;
let terminalLastCols = 0;
let terminalLastHostWidth = 0;
let terminalLastHostHeight = 0;
let nextMirrorTabId = 1;
let nextMirrorStartToken = 1;
let nextImageEditorTabId = 1;
const imageEditorCanvases = new Map<string, HTMLCanvasElement>();
const pendingScrcpyFrames = new Map<number, ScrcpyFramePayload[]>();
const pendingScrcpyStatuses = new Map<number, ScrcpyStatusPayload>();
const mirrorFirstFrameTimers = new Map<number, number>();
const MIRROR_START_TIMEOUT_MS = 32_000;
const MIRROR_AUTO_RESTART_DELAY_MS = 5_000;
const MAX_PENDING_SCRCPY_FRAMES = 360;
const MAX_INITIAL_SCRCPY_FLUSH_TAIL_FRAMES = 2;
const TERMINAL_OUTPUT_MAX_QUEUE_BYTES = 4 * 1024 * 1024;
const TERMINAL_OUTPUT_CHUNK_BYTES = 128 * 1024;

function mirrorDebug(_event: string, _details?: unknown) {
}

function mirrorWarn(_event: string, _details?: unknown) {
}

async function ensureMirrorEventListenersReady() {
  await mirrorEventListenersReady;
}

function createMirrorTab(id = nextMirrorTabId++): MirrorTab {
  return {
    id,
    title: `投屏 ${id}`,
    serial: "",
    state: "idle",
    statusText: "等待启动",
    startToken: 0,
    startRecoveryCount: 0,
    autoRestarting: false,
    session: null,
    decodedFrames: 0,
    droppedFrames: 0,
    decoderReady: false,
    canvas: null,
    decoder: null,
    waitingForCanvas: false,
    configuredCodec: "avc1.42E01F",
    latestConfigPacket: null,
    pendingConfigPacket: null,
    activePointers: new Map(),
    syntheticPinch: {
      active: false,
      center: { x: 0, y: 0 },
      distance: 0,
      finishTimer: null,
    },
    syntheticPan: {
      active: false,
      sourcePointerId: null,
      center: { x: 0, y: 0 },
      distance: 0,
    },
    lastMoveAt: new Map(),
  };
}

const quickBusy = computed(() => transferRunning.value);
const activeMirrorTab = computed(() => {
  return mirrorTabs.value.find((tab) => tab.id === activeMirrorTabId.value) || mirrorTabs.value[0] || null;
});
const workspaceTabs = computed<WorkspaceTab[]>(() => {
  const tabs: WorkspaceTab[] = mirrorTabs.value.map((tab) => ({
    key: mirrorTabKey(tab.id),
    kind: "mirror",
    title: tab.title,
    status: mirrorTabStatus(tab),
    running: tab.state === "running",
    closable: true,
    mirror: tab,
  }));
  for (const editor of imageEditorTabs.value) {
    tabs.push({
      key: editor.key,
      kind: "screenshot",
      title: editor.title,
      status: editor.saving ? "保存中" : "编辑中",
      running: editor.saving,
      closable: !editor.saving,
      editor,
    });
  }
  if (textPreviewOpen.value) {
    tabs.push({
      key: "text-preview",
      kind: "textPreview",
      title: textPreviewMeta.value?.name || t("mirror.textPreview"),
      status: textPreviewLoading.value ? t("mirror.opening") : t("common.preview"),
      running: textPreviewLoading.value,
      closable: true,
    });
  }
  if (transferPanelOpen.value) {
    tabs.push({
      key: "transfer",
      kind: "transfer",
      title: t("app.fileTransfer"),
      status: transferRunning.value ? (copyDirection.value === "install" ? t("mirror.transferInstalling") : t("mirror.transferRunning")) : t("common.ready"),
      running: transferRunning.value,
      closable: !transferRunning.value,
    });
  }
  if (terminalPanelOpen.value) {
    tabs.push({
      key: "terminal",
      kind: "terminal",
      title: t("app.terminal"),
      status: terminalStarting.value ? t("mirror.terminalStarting") : terminalSessionId.value ? t("mirror.terminalConnected") : t("mirror.terminalClosed"),
      running: Boolean(terminalSessionId.value),
      closable: true,
    });
  }
  if (settingsPanelOpen.value) {
    tabs.push({
      key: "settings",
      kind: "settings",
      title: t("common.settings"),
      status: t("mirror.tabSettingsStatus"),
      running: false,
      closable: true,
    });
  }
  const orderedKeys = workspaceTabOrder.value;
  const tabMap = new Map(tabs.map((tab) => [tab.key, tab]));
  const orderedTabs = orderedKeys
    .map((key) => tabMap.get(key))
    .filter((tab): tab is WorkspaceTab => Boolean(tab));
  const orderedKeySet = new Set(orderedKeys);
  const newTabs = tabs.filter((tab) => !orderedKeySet.has(tab.key));
  return [...orderedTabs, ...newTabs];
});
const state = computed(() => activeMirrorTab.value?.state || "idle");
const atSdcardRoot = computed(() => remoteBrowserPath.value === defaultDeviceRoot);
const atLocalDrivesRoot = computed(() => localBrowserPath.value === localDrivesRoot);
const localBrowserPathLabel = computed(() => atLocalDrivesRoot.value ? "此电脑" : (localBrowserPath.value || "系统目录"));
const canGoLocalParent = computed(() => {
  return Boolean(parentLocalPath(localBrowserPath.value));
});
const sortedLocalEntries = computed(() => sortTransferEntries(localEntries.value, localEntrySort.value));
const sortedRemoteEntries = computed(() => sortTransferEntries(remoteEntries.value, remoteEntrySort.value));
const selectedLocalEntry = computed(() => localEntries.value.find((entry) => entry.path === selectedLocalPath.value) || null);
const canCopyLocalToRemote = computed(() => Boolean(selectedLocalPath.value && selectedSerial.value && !isWindowsDrivePath(selectedLocalPath.value) && !quickBusy.value));
const canCopyRemoteToLocal = computed(() => Boolean(selectedRemotePath.value && localBrowserPath.value && !atLocalDrivesRoot.value && !quickBusy.value));
const canInstallSelectedApk = computed(() => {
  return Boolean(selectedSerial.value && selectedLocalPath.value && !selectedLocalEntry.value?.isDir && isApkPath(selectedLocalPath.value) && !quickBusy.value);
});
const canDeleteLocal = computed(() => Boolean(selectedLocalPath.value && !isWindowsDrivePath(selectedLocalPath.value) && !quickBusy.value && !browsingLocal.value));
const canDeleteRemote = computed(() => Boolean(selectedRemotePath.value && selectedSerial.value && !quickBusy.value && !browsingRemote.value));
const deleteConfirmTitle = computed(() => (deleteConfirmTarget.value?.scope === "local" ? "删除本地项目" : "删除设备项目"));
const canPreviewSelectedLocalText = computed(() => Boolean(selectedLocalPath.value && !selectedLocalEntry.value?.isDir && isPreviewablePath(selectedLocalPath.value)));
const selectedRemoteEntry = computed(() => remoteEntries.value.find((entry) => entry.path === selectedRemotePath.value) || null);
const canPreviewSelectedRemoteText = computed(() => Boolean(selectedRemotePath.value && selectedSerial.value && !selectedRemoteEntry.value?.isDir && isPreviewablePath(selectedRemotePath.value)));
const transferPaneGridStyle = computed<Record<string, string>>(() => ({
  "--transfer-pane-columns": splitPaneGridTemplateColumns(
    transferPaneSplitPercent.value,
    transferPaneMinWidth,
    transferPaneHandleWidth,
  ),
}));
const transferContextMenuItems = computed<TransferContextMenuItem[]>(() => {
  const menu = transferContextMenu.value;
  if (!menu) {
    return [];
  }

  const common = [
    { action: "preview" as const, label: "预览", icon: "preview" as const, disabled: menu.entry.isDir || !isPreviewablePath(menu.entry.path) },
    { action: "refresh" as const, label: "刷新", icon: "refresh" as const, disabled: menu.scope === "local" ? browsingLocal.value : browsingRemote.value },
    { action: "delete" as const, label: "删除", icon: "delete" as const, danger: true, disabled: menu.scope === "local" ? !canDeleteLocal.value : !canDeleteRemote.value },
  ];

  if (menu.scope === "local") {
    return [
      { action: "copyToDevice" as const, label: "传到设备", icon: "copyToDevice" as const, disabled: !canCopyLocalToRemote.value },
      { action: "install" as const, label: "安装 APK", icon: "apk" as const, disabled: !canInstallSelectedApk.value },
      ...common,
    ];
  }

  return [
    { action: "copyToComputer" as const, label: "传到电脑", icon: "copyToComputer" as const, disabled: !canCopyRemoteToLocal.value },
    ...common,
  ];
});
const recordingElapsedLabel = computed(() => formatDuration(recordingElapsedSeconds.value));
const textPreviewLineHeight = 20;
const textPreviewChunkSize = 256 * 1024;
const textPreviewTotalHeight = computed(() => textPreviewLines.value.length * textPreviewLineHeight);
const textPreviewSearchNeedle = computed(() => textPreviewSearchQuery.value.trim());
const textPreviewSearchPattern = computed(() => {
  const query = textPreviewSearchNeedle.value;
  if (!query) {
    return { query, regex: null as RegExp | null, error: "" };
  }
  try {
    return { query, regex: buildTextPreviewSearchRegex(query), error: "" };
  } catch (error) {
    return {
      query,
      regex: null as RegExp | null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
});
const textPreviewSearchMatches = computed(() => {
  const pattern = textPreviewSearchPattern.value;
  if (!pattern.query || !pattern.regex || textPreviewMode.value !== "text") {
    return [];
  }
  return textPreviewLines.value
    .filter((line) => textPreviewLineMatches(line.text))
    .map((line) => line.id);
});
const textPreviewSearchActiveIndex = computed(() => {
  if (textPreviewSearchActiveLine.value === null) {
    return -1;
  }
  return textPreviewSearchMatches.value.indexOf(textPreviewSearchActiveLine.value);
});
const textPreviewSearchLabel = computed(() => {
  const query = textPreviewSearchNeedle.value;
  if (!query) {
    return "";
  }
  if (textPreviewSearchPattern.value.error) {
    return "正则错误";
  }
  if (textPreviewSearchSearching.value) {
    return "查找中";
  }
  const matches = textPreviewSearchMatches.value.length;
  if (matches === 0) {
    return textPreviewSearchMessage.value || "0";
  }
  const active = textPreviewSearchActiveIndex.value;
  return active >= 0 ? `${active + 1}/${matches}` : `${matches}`;
});
const textPreviewVisibleRange = computed(() => {
  const start = Math.max(0, Math.floor(textPreviewScrollTop.value / textPreviewLineHeight) - 8);
  const count = Math.ceil(textPreviewViewportHeight.value / textPreviewLineHeight) + 16;
  return {
    start,
    end: Math.min(textPreviewLines.value.length, start + count),
  };
});
const textPreviewVisibleLines = computed(() => {
  const range = textPreviewVisibleRange.value;
  const hasSearch = Boolean(textPreviewSearchPattern.value.query && textPreviewSearchPattern.value.regex);
  const activeLine = textPreviewSearchActiveLine.value;
  return textPreviewLines.value.slice(range.start, range.end).map((line, index) => ({
    ...line,
    top: (range.start + index) * textPreviewLineHeight,
    lineNumber: range.start + index + 1,
    searchMatch: hasSearch && textPreviewLineMatches(line.text),
    searchActive: activeLine === line.id,
  }));
});
const textPreviewHtmlContent = computed(() => sanitizePreviewHtml(textPreviewLines.value.map((line) => line.text).join("\n")));
const imagePreviewTransform = computed(() => {
  return `translate(-50%, -50%) translate(${imagePreviewOffset.value.x}px, ${imagePreviewOffset.value.y}px) scale(${imagePreviewScale.value})`;
});
const terminalQuickGroups: Array<{ key: QuickCommandGroup; label: string }> = [
  { key: "common", label: "常用" },
  { key: "device", label: "设备" },
  { key: "screen", label: "屏幕" },
  { key: "media", label: "媒体" },
];
const imageEditorTools: Array<{ key: ImageEditorTool; label: string }> = [
  { key: "crop", label: "裁剪" },
  { key: "rect", label: "矩形" },
  { key: "arrow", label: "箭头" },
  { key: "polyline", label: "折线" },
  { key: "mosaic", label: "马赛克" },
  { key: "text", label: "文字" },
  { key: "eraser", label: "橡皮擦" },
  { key: "picker", label: "取色" },
];
const imageEditorToolIconPaths: Record<ImageEditorTool, string[]> = {
  crop: ["M6 2v14a2 2 0 0 0 2 2h14", "M18 22V8a2 2 0 0 0-2-2H2"],
  rect: ["M5 5h14v14H5z"],
  arrow: ["M5 19L19 5", "M10 5h9v9"],
  polyline: ["M4 18l5-8 5 5 6-9", "M4 18h.01", "M9 10h.01", "M14 15h.01", "M20 6h.01"],
  mosaic: ["M4 4h6v6H4z", "M14 4h6v6h-6z", "M4 14h6v6H4z", "M14 14h6v6h-6z"],
  text: ["M4 6h16", "M12 6v12", "M8 18h8"],
  eraser: ["M4 16l8-8 6 6-5 5H7l-3-3z", "M14 20h6"],
  picker: ["M19 3l2 2-9 9-4 1 1-4 10-8z", "M4 20h7"],
};
const imageEditorActionIconPaths = {
  undo: ["M995.648 459.296C684.448 281.568 406.08 318.656 171.232 567.904L32 416v384h352L223.264 624.672c211.168-227.072 452.192-259.776 734.304-98.688a38.4 38.4 0 0 0 38.08-66.688z"],
  applyCrop: ["M6 2v14a2 2 0 0 0 2 2h14", "M18 22V8a2 2 0 0 0-2-2H2", "M9 12l2 2 4-5"],
  clear: ["M4 16l8-8 6 6-5 5H7l-3-3z", "M14 20h6", "M18 6l3-3", "M21 6l-3-3"],
  save: ["M5 3h12l2 2v16H5z", "M8 3v6h8V3", "M8 21v-7h8v7"],
};
const transferToolIconPaths = {
  chooseFolder: ["M3 7h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z", "M3 7V5a2 2 0 0 1 2-2h4l2 2h5"],
  copyToDevice: ["M4 12h14", "M12 6l6 6-6 6", "M5 5h3", "M5 19h3"],
  copyToComputer: ["M20 12H6", "M12 6l-6 6 6 6", "M16 5h3", "M16 19h3"],
  apk: ["M7 4h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M9 9h6", "M9 13h6", "M10 4V2", "M14 4V2"],
  preview: ["M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6z", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"],
  refresh: ["M20 12a8 8 0 1 1-2.3-5.7", "M20 4v6h-6"],
  delete: ["M4 7h16", "M10 11v6", "M14 11v6", "M6 7l1 13h10l1-13", "M9 7V4h6v3"],
};
const fileEntryIconPaths: Record<FileEntryIconName, string[]> = {
  folder: ["M3 7h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z", "M3 7V5a2 2 0 0 1 2-2h4l2 2h5"],
  apk: ["M7 4h10a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "M9 9h6", "M9 13h6", "M10 4V2", "M14 4V2"],
  image: ["M4 5h16v14H4z", "M8 13l2.5-3 3.5 4 2-2.5 4 5.5", "M8.5 8.5h.01"],
  video: ["M4 6h12v12H4z", "M16 10l5-3v10l-5-3z", "M8 6v12"],
  pdf: ["M6 3h9l4 4v14H6z", "M15 3v5h5", "M8 16h8", "M8 12h6"],
  code: ["M9 8l-4 4 4 4", "M15 8l4 4-4 4", "M13 5l-2 14"],
  text: ["M6 3h9l4 4v14H6z", "M15 3v5h5", "M8 12h8", "M8 16h6"],
  archive: ["M6 3h12v18H6z", "M10 3v18", "M10 7h4", "M10 11h4", "M10 15h4"],
  audio: ["M9 18V6l8-2v12", "M9 18a3 3 0 1 1-2-2.83", "M17 16a3 3 0 1 1-2-2.83"],
  file: ["M6 3h9l4 4v14H6z", "M15 3v5h5"],
};
const fileEntrySortIconPaths: Record<FileEntrySortDirection, string[]> = {
  asc: ["M12 19V5", "M6 11l6-6 6 6"],
  desc: ["M12 5v14", "M6 13l6 6 6-6"],
};
const quickCommands = computed<QuickCommandItem[]>(() => {
  const localDir = defaultLocalDir.value.trim();
  const localScreenshot = terminalPath(localDir ? `${localDir.replace(/[\\/]+$/, "")}/droidpilot_screen.png` : "droidpilot_screen.png");
  const localMediaDir = terminalPath(localDir || ".");

  return [
    { group: "common", label: "查看设备", desc: "列出当前 ADB 设备", command: "adb devices", tag: "list" },
    { group: "common", label: "ADB 版本", desc: "查看 adb 工具版本", command: "adb version", tag: "info" },
    { group: "common", label: "启动服务", desc: "启动 ADB server", command: "adb start-server", tag: "server" },
    { group: "common", label: "重启服务", desc: "停止并重新启动 ADB", command: "adb kill-server && adb start-server", tag: "server" },
    { group: "common", label: "设备型号", desc: "读取设备型号", command: adbTerminalCommand("shell getprop ro.product.model"), tag: "prop" },
    { group: "common", label: "Android 版本", desc: "读取系统版本", command: adbTerminalCommand("shell getprop ro.build.version.release"), tag: "prop" },
    { group: "common", label: "屏幕分辨率", desc: "查看当前物理分辨率", command: adbTerminalCommand("shell wm size"), tag: "wm" },
    { group: "common", label: "电池信息", desc: "查看电量与充电状态", command: adbTerminalCommand("shell dumpsys battery"), tag: "dump" },
    { group: "device", label: "设备厂商", desc: "读取品牌厂商", command: adbTerminalCommand("shell getprop ro.product.manufacturer"), tag: "prop" },
    { group: "device", label: "CPU 架构", desc: "查看 ABI 架构", command: adbTerminalCommand("shell getprop ro.product.cpu.abi"), tag: "abi" },
    { group: "device", label: "设备序列号", desc: "读取序列号", command: adbTerminalCommand("get-serialno"), tag: "id" },
    { group: "device", label: "内存信息", desc: "读取 /proc/meminfo", command: adbTerminalCommand("shell cat /proc/meminfo"), tag: "mem" },
    { group: "device", label: "存储信息", desc: "查看分区空间", command: adbTerminalCommand("shell df -h"), tag: "disk" },
    { group: "device", label: "网络地址", desc: "查看设备 IP 路由", command: adbTerminalCommand("shell ip route"), tag: "net" },
    { group: "device", label: "Wi-Fi 连接", desc: "修改 IP:端口后回车", command: "adb connect 192.168.1.100:5555", tag: "wifi" },
    { group: "device", label: "Wi-Fi 断开", desc: "修改 IP:端口后回车", command: "adb disconnect 192.168.1.100:5555", tag: "wifi" },
    { group: "device", label: "无线配对", desc: "修改配对地址后回车", command: "adb pair 192.168.1.100:37099", tag: "pair" },
    { group: "device", label: "启用 5555", desc: "切换到 TCP/IP 调试", command: adbTerminalCommand("tcpip 5555"), tag: "tcpip" },
    { group: "screen", label: "屏幕密度", desc: "查看当前 DPI", command: adbTerminalCommand("shell wm density"), tag: "wm" },
    { group: "screen", label: "窗口信息", desc: "查看窗口与焦点信息", command: adbTerminalCommand("shell dumpsys window"), tag: "dump" },
    { group: "screen", label: "返回键", desc: "发送返回按键", command: adbTerminalCommand("shell input keyevent 4"), tag: "key" },
    { group: "screen", label: "Home 键", desc: "发送 Home 按键", command: adbTerminalCommand("shell input keyevent 3"), tag: "key" },
    { group: "screen", label: "电源键", desc: "发送电源按键", command: adbTerminalCommand("shell input keyevent 26"), tag: "key" },
    { group: "screen", label: "截图到设备", desc: "保存到设备默认目录", command: adbTerminalCommand("shell screencap -p /storage/emulated/0/droidpilot_screen.png"), tag: "png" },
    { group: "screen", label: "截图到本机", desc: "保存到设置中的本地目录", command: `${adbTerminalCommand("exec-out screencap -p")} > ${localScreenshot}`, tag: "png" },
    { group: "media", label: "开始录屏", desc: "前台录制，Ctrl+C 停止", command: adbTerminalCommand("shell screenrecord /storage/emulated/0/Movies/screen.mp4"), tag: "rec" },
    { group: "media", label: "高码率录屏", desc: "8Mbps 前台录制", command: adbTerminalCommand("shell screenrecord --bit-rate 8000000 /storage/emulated/0/Movies/screen.mp4"), tag: "rec" },
    { group: "media", label: "720P 录屏", desc: "1280x720 前台录制", command: adbTerminalCommand("shell screenrecord --size 1280x720 /storage/emulated/0/Movies/screen.mp4"), tag: "rec" },
    { group: "media", label: "停止录屏", desc: "尝试停止设备端录屏", command: adbTerminalCommand("shell pkill -INT screenrecord"), tag: "stop" },
    { group: "media", label: "拉取录屏", desc: "下载设备 Movies 目录", command: `${adbTerminalCommand("pull /storage/emulated/0/Movies")} ${localMediaDir}`, tag: "pull" },
    { group: "media", label: "刷新媒体库", desc: "扫描默认录屏文件", command: adbTerminalCommand("shell am broadcast -a android.intent.action.MEDIA_SCANNER_SCAN_FILE -d file:///storage/emulated/0/Movies/screen.mp4"), tag: "scan" },
  ];
});
const filteredQuickCommands = computed(() => {
  return quickCommands.value.filter((item) => item.group === terminalQuickGroup.value);
});
const existingActiveMirrorTab = computed(() => {
  return mirrorTabs.value.find((tab) => tab.state !== "idle") || null;
});
const mirrorToggleLabel = computed(() => {
  if (!isActiveWorkspaceMirror.value) {
    return existingActiveMirrorTab.value ? t("mirror.openMirror") : t("mirror.startMirror");
  }
  if (activeMirrorAutoRestarting.value) {
    return t("mirror.reconnecting");
  }
  if (state.value === "starting") {
    return t("mirror.cancelStart");
  }
  if (state.value === "running" || state.value === "error") {
    return t("mirror.stopMirror");
  }
  if (state.value === "stopping") {
    return t("mirror.stopping");
  }
  return t("mirror.startMirror");
});
const isActiveWorkspaceMirror = computed(() => activeWorkspaceTabKey.value.startsWith("mirror:"));
const activeMirrorRecording = computed(() => recording.value && recordingMirrorTabId.value === activeMirrorTab.value?.id);
const activeMirrorSavingRecording = computed(() => savingLocalRecording.value && recordingMirrorTabId.value === activeMirrorTab.value?.id);
const activeMirrorAutoRestarting = computed(() => Boolean(activeMirrorTab.value?.autoRestarting));
const canRecordActiveMirror = computed(() => {
  return isActiveWorkspaceMirror.value
    && state.value === "running"
    && (!recording.value || activeMirrorRecording.value);
});
const canSendKeysActiveMirror = computed(() => {
  const tab = activeMirrorTab.value;
  return Boolean(tab?.session && tab.state === "running" && !tab.autoRestarting);
});
const canScreenshotActiveMirror = computed(() => {
  const tab = activeMirrorTab.value;
  return Boolean(
    isActiveWorkspaceMirror.value
      && tab
      && tab.state === "running"
      && tab.decoderReady
      && tab.canvas
      && tab.canvas.width > 0
      && tab.canvas.height > 0,
  );
});
const mirrorToggleDisabled = computed(() => isActiveWorkspaceMirror.value && (state.value === "stopping" || activeMirrorAutoRestarting.value));
const mirrorToggleDanger = computed(() => isActiveWorkspaceMirror.value && !activeMirrorAutoRestarting.value && (state.value === "starting" || state.value === "running" || state.value === "error"));
function emitMirrorToolbarState() {
  window.dispatchEvent(
    new CustomEvent("droidpilot:mirror-toolbar-state", {
      detail: {
        canUseMirrorControls: true,
        canSendKeys: canSendKeysActiveMirror.value,
        mirrorToggleDanger: mirrorToggleDanger.value,
        mirrorToggleDisabled: mirrorToggleDisabled.value,
        mirrorToggleLabel: mirrorToggleLabel.value,
        quickBusy: quickBusy.value,
        canMirrorScreenshot: canScreenshotActiveMirror.value,
        canRecord: canRecordActiveMirror.value,
        recording: activeMirrorRecording.value,
        recordingElapsedLabel: recordingElapsedLabel.value,
        savingLocalRecording: activeMirrorSavingRecording.value,
      },
    }),
  );
}

watchEffect(() => {
  emitMirrorToolbarState();
});

function syncMirrorToolbarState() {
  emitMirrorToolbarState();
  void nextTick(emitMirrorToolbarState);
}

function screenPlaceholderFor(tab: MirrorTab) {
  if (tab.state === "error") {
    return {
      title: t("mirror.mirrorError"),
      message: tab.statusText || t("mirror.mirrorConnectFailed"),
    };
  }

  if (!selectedSerial.value) {
    return {
      title: t("mirror.selectDevice"),
      message: t("mirror.selectDeviceMessage"),
    };
  }

  return {
    title: t("mirror.waitingStart"),
    message: tab.statusText === "已停止" ? t("mirror.stoppedRestart") : t("mirror.clickStart"),
  };
}

function shouldShowScreenPlaceholder(tab: MirrorTab) {
  return tab.state !== "starting" && tab.state !== "stopping" && tab.state !== "running";
}

function setState(tab: MirrorTab, nextState: MirrorState, message: string) {
  tab.state = nextState;
  tab.statusText = message;
  syncMirrorToolbarState();
}

function homeShortcutKeyParts(shortcut: AppShortcut) {
  return appShortcutKeyParts(configuredAppShortcuts.value[shortcut.action] || defaultAppShortcutMap[shortcut.action]);
}

function terminalPath(value: string) {
  return `"${value.replace(/"/g, '\\"')}"`;
}

function formatDuration(totalSeconds: number) {
  const safeSeconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function updateRecordingElapsed() {
  if (!recordingStartedAt) {
    recordingElapsedSeconds.value = 0;
    return;
  }
  recordingElapsedSeconds.value = Math.floor((Date.now() - recordingStartedAt) / 1000);
}

function startRecordingTimer() {
  recordingStartedAt = Date.now();
  recordingElapsedSeconds.value = 0;
  if (recordingTimer !== null) {
    window.clearInterval(recordingTimer);
  }
  recordingTimer = window.setInterval(updateRecordingElapsed, 1000);
}

function stopRecordingTimer() {
  if (recordingStartedAt) {
    updateRecordingElapsed();
  }
  if (recordingTimer !== null) {
    window.clearInterval(recordingTimer);
    recordingTimer = null;
  }
  recordingStartedAt = 0;
}

function stopRecordingForMirror(tab: MirrorTab) {
  if (recordingMirrorTabId.value !== tab.id) {
    return;
  }

  if (localMediaRecorder && localMediaRecorder.state !== "inactive") {
    localMediaRecorder.stop();
    return;
  }

  if (savingLocalRecording.value) {
    return;
  }

  stopRecordingTimer();
  localRecordingStream?.getTracks().forEach((track) => track.stop());
  localRecordingStream = null;
  localMediaRecorder = null;
  localRecordingChunks = [];
  localRecordingMimeType = "";
  recording.value = false;
  savingLocalRecording.value = false;
  recordingMirrorTabId.value = null;
}

function adbTerminalCommand(args: string) {
  const serial = selectedSerial.value.trim();
  return serial ? `adb -s ${serial} ${args}` : `adb ${args}`;
}

const fileEntryDateFormatter = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
});
const fileEntryNameCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

function formatEntryCreatedAt(entry: TransferEntry) {
  const value = typeof entry.createdAtMs === "number" ? entry.createdAtMs : 0;
  if (!Number.isFinite(value) || value <= 0) {
    return "--";
  }
  return fileEntryDateFormatter.format(new Date(value));
}

function compareEntryName(a: TransferEntry, b: TransferEntry, direction: FileEntrySortDirection) {
  const result = fileEntryNameCollator.compare(a.name, b.name);
  return direction === "asc" ? result : -result;
}

function compareOptionalNumber(a: number | null | undefined, b: number | null | undefined, direction: FileEntrySortDirection) {
  const aValid = typeof a === "number" && Number.isFinite(a);
  const bValid = typeof b === "number" && Number.isFinite(b);
  if (!aValid && !bValid) {
    return 0;
  }
  if (!aValid) {
    return 1;
  }
  if (!bValid) {
    return -1;
  }
  return direction === "asc" ? a - b : b - a;
}

function compareTransferEntry(a: TransferEntry, b: TransferEntry, sort: FileEntrySortState) {
  if (a.isDir !== b.isDir) {
    return a.isDir ? -1 : 1;
  }

  let result = 0;
  if (sort.key === "name") {
    result = compareEntryName(a, b, sort.direction);
  } else if (sort.key === "size") {
    result = compareOptionalNumber(a.sizeBytes, b.sizeBytes, sort.direction);
  } else {
    result = compareOptionalNumber(a.createdAtMs, b.createdAtMs, sort.direction);
  }

  return result || compareEntryName(a, b, "asc") || fileEntryNameCollator.compare(a.path, b.path);
}

function sortTransferEntries<T extends TransferEntry>(entries: T[], sort: FileEntrySortState) {
  return [...entries].sort((a, b) => compareTransferEntry(a, b, sort));
}

function toggleFileEntrySort(scope: TransferEntryScope, key: FileEntrySortKey) {
  const target = scope === "local" ? localEntrySort : remoteEntrySort;
  target.value = target.value.key === key
    ? { key, direction: target.value.direction === "asc" ? "desc" : "asc" }
    : { key, direction: "asc" };
}

function fileEntrySortIconDirection(sort: FileEntrySortState, key: FileEntrySortKey): FileEntrySortDirection {
  return sort.key === key ? sort.direction : "asc";
}

function fileEntrySortButtonLabel(sort: FileEntrySortState, key: FileEntrySortKey, label: string) {
  if (sort.key !== key) {
    return `按${label}排序`;
  }
  return `${label}${sort.direction === "asc" ? "升序" : "降序"}`;
}

function requireSelectedDevice() {
  if (selectedSerial.value) {
    return true;
  }
  adbStore.setNotice("error", "请先选择设备");
  return false;
}

function isApkPath(path: string) {
  return path.trim().toLowerCase().endsWith(".apk");
}

function localPathDisplayName(path: string) {
  const normalized = path.trim().replace(/[\\/]+$/, "");
  return normalized.split(/[\\/]/).pop() || normalized || "拖入项目";
}

function normalizeRemoteDropTargetPath() {
  const configured = adbStore.defaultDevicePath.trim() || defaultDeviceRoot;
  return configured.endsWith("/") ? configured : `${configured}/`;
}

function fileDropSummaryMessage(paths: string[]) {
  const apkCount = paths.filter(isApkPath).length;
  const copyCount = paths.length - apkCount;
  if (apkCount > 0 && copyCount > 0) {
    return `释放后安装 ${apkCount} 个 APK，复制 ${copyCount} 个文件/目录`;
  }
  if (apkCount > 0) {
    return `释放后安装 ${apkCount} 个 APK`;
  }
  return `释放后复制 ${copyCount || paths.length} 个文件/目录到默认设备目录`;
}

function isTextPreviewPath(path: string) {
  return /\.(txt|log|json|xml|yaml|yml|properties|csv|ini|conf|cfg|css|js|ts|vue|rs|kt|java|gradle|sh|bat|ps1)$/i.test(path.trim());
}

function isHtmlPreviewPath(path: string) {
  return /\.(html|htm)$/i.test(path.trim());
}

function isPdfPreviewPath(path: string) {
  return /\.pdf$/i.test(path.trim());
}

function isImagePreviewPath(path: string) {
  return /\.(png|jpe?g|webp|gif|bmp|svg)$/i.test(path.trim());
}

function isVideoPreviewPath(path: string) {
  return /\.(mp4|m4v|webm|mov|ogv|ogg|mkv|avi|3gp|3gpp)$/i.test(path.trim());
}

function isPreviewablePath(path: string) {
  return isTextPreviewPath(path) || isHtmlPreviewPath(path) || isPdfPreviewPath(path) || isImagePreviewPath(path) || isVideoPreviewPath(path);
}

function fileExtension(path: string) {
  const name = path.trim().split(/[\\/]/).pop() || "";
  const dotIndex = name.lastIndexOf(".");
  return dotIndex > 0 ? name.slice(dotIndex + 1).toLowerCase() : "";
}

function fileEntryIconName(entry: TransferEntry): FileEntryIconName {
  if (entry.isDir) {
    return "folder";
  }
  if (isApkPath(entry.path)) {
    return "apk";
  }
  if (isImagePreviewPath(entry.path)) {
    return "image";
  }
  if (isVideoPreviewPath(entry.path)) {
    return "video";
  }
  if (isPdfPreviewPath(entry.path)) {
    return "pdf";
  }

  const extension = fileExtension(entry.path);
  if (["mp3", "wav", "flac", "aac", "m4a", "opus"].includes(extension)) {
    return "audio";
  }
  if (["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz"].includes(extension)) {
    return "archive";
  }
  if (["js", "ts", "vue", "rs", "kt", "java", "gradle", "sh", "bat", "ps1", "css", "html", "htm", "xml", "json", "yaml", "yml"].includes(extension)) {
    return "code";
  }
  if (isTextPreviewPath(entry.path)) {
    return "text";
  }
  return "file";
}

function previewModeForPath(path: string): TextPreviewMode {
  if (isVideoPreviewPath(path)) {
    return "video";
  }
  if (isImagePreviewPath(path)) {
    return "image";
  }
  if (isPdfPreviewPath(path)) {
    return "pdf";
  }
  if (isHtmlPreviewPath(path)) {
    return "html";
  }
  return "text";
}

function sanitizePreviewHtml(value: string) {
  const parser = new DOMParser();
  const documentValue = parser.parseFromString(value, "text/html");
  const root = documentValue.body || documentValue.documentElement;
  root.querySelectorAll("script, style, link, meta, iframe, object, embed").forEach((node) => node.remove());
  root.querySelectorAll<HTMLElement>("*").forEach((node) => {
    for (const attr of [...node.attributes]) {
      const name = attr.name.toLowerCase();
      const value = attr.value.trim().toLowerCase();
      if (name === "style" || name.startsWith("on") || (["href", "src"].includes(name) && value.startsWith("javascript:"))) {
        node.removeAttribute(attr.name);
      }
    }
  });
  return root.innerHTML;
}

async function ensureDownloadLocalDir() {
  if (defaultLocalDir.value.trim()) {
    return defaultLocalDir.value.trim();
  }

  try {
    const dir = await downloadDir();
    if (dir && dir.trim()) {
      adbStore.setDefaultLocalDir(dir);
      return dir.trim();
    }
  } catch {
    // Keep the manual settings page flow if the system directory is unavailable.
  }

  adbStore.setNotice("error", "请先在设置中填写本地保存目录");
  return "";
}

function base64ToBytes(value: string) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index++) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function supportedRecordingMimeType() {
  if (typeof MediaRecorder === "undefined") {
    return "";
  }

  return (
    [
      "video/mp4;codecs=avc1.42E01E",
      "video/mp4",
      "video/webm;codecs=vp9",
      "video/webm;codecs=vp8",
      "video/webm",
    ].find((type) => MediaRecorder.isTypeSupported(type)) || ""
  );
}

function recordingExtension(mimeType: string) {
  return mimeType.includes("mp4") ? "mp4" : "webm";
}

function blobToBase64(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error || new Error("读取录屏数据失败"));
    reader.onload = () => {
      const result = String(reader.result || "");
      resolve(result.includes(",") ? result.slice(result.indexOf(",") + 1) : result);
    };
    reader.readAsDataURL(blob);
  });
}

function canvasToPngBlob(canvas: HTMLCanvasElement) {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob) {
        resolve(blob);
      } else {
        reject(new Error("生成投屏截图失败"));
      }
    }, "image/png");
  });
}

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error || new Error("读取截图数据失败"));
    reader.onload = () => resolve(String(reader.result || ""));
    reader.readAsDataURL(blob);
  });
}

function loadImage(source: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("加载截图失败"));
    image.src = source;
  });
}

function imageEditorKey(id: number) {
  return `screenshot:${id}`;
}

function makeEditorShapeId() {
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function cloneImageEditorShapes(shapes: ImageEditorShape[]) {
  return JSON.parse(JSON.stringify(shapes)) as ImageEditorShape[];
}

function cloneCropRect(rect: ImageEditorSnapshot["cropRect"]) {
  return rect ? { ...rect } : null;
}

function makeImageEditorSnapshot(editor: ImageEditorTab): ImageEditorSnapshot {
  return {
    baseDataUrl: editor.baseDataUrl,
    width: editor.width,
    height: editor.height,
    shapes: cloneImageEditorShapes(editor.shapes),
    cropRect: cloneCropRect(editor.cropRect),
  };
}

function pushImageEditorHistory(editor: ImageEditorTab) {
  editor.undoStack = [...editor.undoStack, makeImageEditorSnapshot(editor)].slice(-80);
  editor.redoStack = [];
}

async function restoreImageEditorSnapshot(editor: ImageEditorTab, snapshot: ImageEditorSnapshot) {
  editor.baseDataUrl = snapshot.baseDataUrl;
  editor.baseImage = markRaw(await loadImage(snapshot.baseDataUrl));
  editor.width = snapshot.width;
  editor.height = snapshot.height;
  editor.shapes = cloneImageEditorShapes(snapshot.shapes);
  editor.cropRect = cloneCropRect(snapshot.cropRect);
  editor.draft = null;
  editor.cropDrag = null;
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.shapeHoverId = null;
  editor.shapeResizeHover = null;
  editor.cropHover = null;
  editor.drawingStart = null;
  editor.textInput = null;
  editor.pickerPreview = null;
  await nextTick();
  renderImageEditor(editor);
}

async function undoImageEditor(editor: ImageEditorTab) {
  const snapshot = editor.undoStack[editor.undoStack.length - 1];
  if (!snapshot) {
    return;
  }
  editor.undoStack = editor.undoStack.slice(0, -1);
  editor.redoStack = [...editor.redoStack, makeImageEditorSnapshot(editor)];
  await restoreImageEditorSnapshot(editor, snapshot);
}

async function redoImageEditor(editor: ImageEditorTab) {
  const snapshot = editor.redoStack[editor.redoStack.length - 1];
  if (!snapshot) {
    return;
  }
  editor.redoStack = editor.redoStack.slice(0, -1);
  editor.undoStack = [...editor.undoStack, makeImageEditorSnapshot(editor)].slice(-80);
  await restoreImageEditorSnapshot(editor, snapshot);
}

function setImageEditorCanvas(editor: ImageEditorTab, element: Element | null) {
  if (element instanceof HTMLCanvasElement) {
    imageEditorCanvases.set(editor.key, element);
    void renderImageEditor(editor);
  } else {
    imageEditorCanvases.delete(editor.key);
  }
}

function editorPoint(editor: ImageEditorTab, event: MouseEvent) {
  const canvas = imageEditorCanvases.get(editor.key);
  if (!canvas) {
    return null;
  }
  const bounds = canvas.getBoundingClientRect();
  if (bounds.width <= 0 || bounds.height <= 0) {
    return null;
  }
  return {
    x: Math.min(Math.max(((event.clientX - bounds.left) / bounds.width) * editor.width, 0), editor.width),
    y: Math.min(Math.max(((event.clientY - bounds.top) / bounds.height) * editor.height, 0), editor.height),
  };
}

function activeEditBounds(editor: ImageEditorTab) {
  if (!editor.cropRect) {
    return { left: 0, top: 0, right: editor.width, bottom: editor.height };
  }
  const rect = normalizeCropRect(editor.cropRect);
  return {
    left: Math.max(0, rect.left),
    top: Math.max(0, rect.top),
    right: Math.min(editor.width, rect.right),
    bottom: Math.min(editor.height, rect.bottom),
  };
}

function clampPointToActiveEditBounds(editor: ImageEditorTab, point: { x: number; y: number }) {
  const bounds = activeEditBounds(editor);
  return {
    x: Math.min(Math.max(point.x, bounds.left), bounds.right),
    y: Math.min(Math.max(point.y, bounds.top), bounds.bottom),
  };
}

function pointInsideActiveEditBounds(editor: ImageEditorTab, point: { x: number; y: number }) {
  const bounds = activeEditBounds(editor);
  return point.x >= bounds.left && point.x <= bounds.right && point.y >= bounds.top && point.y <= bounds.bottom;
}

function distanceToSegment(point: { x: number; y: number }, start: { x: number; y: number }, end: { x: number; y: number }) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;
  if (lengthSquared === 0) {
    return Math.hypot(point.x - start.x, point.y - start.y);
  }
  const t = Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared));
  return Math.hypot(point.x - (start.x + t * dx), point.y - (start.y + t * dy));
}

function imageEditorShapeBounds(shape: ImageEditorShape) {
  if (shape.type === "rect" || shape.type === "arrow") {
    return {
      left: Math.min(shape.x1, shape.x2),
      top: Math.min(shape.y1, shape.y2),
      right: Math.max(shape.x1, shape.x2),
      bottom: Math.max(shape.y1, shape.y2),
    };
  }
  if (shape.type === "text") {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("2d");
    const lines = editorTextLines(shape.text);
    context!.font = `600 ${shape.fontSize}px system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
    const width = Math.max(...lines.map((line) => context!.measureText(line || " ").width), shape.fontSize);
    const height = Math.max(shape.fontSize, lines.length * shape.fontSize * 1.25);
    return { left: shape.x, top: shape.y, right: shape.x + width, bottom: shape.y + height };
  }
  const radius = shape.type === "mosaic" ? Math.max(6, shape.blockSize) * 0.75 : shape.lineWidth / 2;
  const points = shape.points.length > 0 ? shape.points : [{ x: 0, y: 0 }];
  return {
    left: Math.min(...points.map((point) => point.x)) - radius,
    top: Math.min(...points.map((point) => point.y)) - radius,
    right: Math.max(...points.map((point) => point.x)) + radius,
    bottom: Math.max(...points.map((point) => point.y)) + radius,
  };
}

function imageEditorShapeHit(editor: ImageEditorTab, shape: ImageEditorShape, point: { x: number; y: number }) {
  if (!pointInsideActiveEditBounds(editor, point)) {
    return false;
  }
  if (shape.type === "rect") {
    const bounds = imageEditorShapeBounds(shape);
    const tolerance = Math.max(8, shape.lineWidth + 4);
    return point.x >= bounds.left - tolerance
      && point.x <= bounds.right + tolerance
      && point.y >= bounds.top - tolerance
      && point.y <= bounds.bottom + tolerance;
  }
  if (shape.type === "arrow") {
    return distanceToSegment(point, { x: shape.x1, y: shape.y1 }, { x: shape.x2, y: shape.y2 }) <= Math.max(10, shape.lineWidth + 6);
  }
  if (shape.type === "polyline") {
    const tolerance = Math.max(10, shape.lineWidth + 6);
    return shape.points.some((shapePoint, index) => {
      const next = shape.points[index + 1];
      return next
        ? distanceToSegment(point, shapePoint, next) <= tolerance
        : Math.hypot(point.x - shapePoint.x, point.y - shapePoint.y) <= tolerance;
    });
  }
  if (shape.type === "text") {
    const bounds = imageEditorShapeBounds(shape);
    const tolerance = 6;
    return point.x >= bounds.left - tolerance
      && point.x <= bounds.right + tolerance
      && point.y >= bounds.top - tolerance
      && point.y <= bounds.bottom + tolerance;
  }
  const radius = shape.type === "mosaic" ? Math.max(6, shape.blockSize) * 0.75 : shape.lineWidth / 2;
  return shape.points.some((shapePoint) => Math.hypot(point.x - shapePoint.x, point.y - shapePoint.y) <= Math.max(8, radius));
}

function isMovableImageEditorShape(shape: ImageEditorShape) {
  return shape.type === "rect" || shape.type === "arrow" || shape.type === "polyline" || shape.type === "text";
}

function findMovableImageEditorShape(editor: ImageEditorTab, point: { x: number; y: number }) {
  const shape = editor.shapes[editor.shapes.length - 1];
  if (shape && isMovableImageEditorShape(shape) && imageEditorShapeHit(editor, shape, point)) {
    return shape;
  }
  return null;
}

function latestTransformableImageEditorShape(editor: ImageEditorTab) {
  const shape = editor.shapes[editor.shapes.length - 1];
  return shape && isMovableImageEditorShape(shape) ? shape : null;
}

function imageEditorShapeResizeHandleHit(editor: ImageEditorTab, shape: ImageEditorShape, point: { x: number; y: number }) {
  if (!pointInsideActiveEditBounds(editor, point)) {
    return null;
  }
  return boundsHandleHit(editor, imageEditorShapeBounds(shape), point);
}

function updateImageEditorShapeHover(editor: ImageEditorTab, point: { x: number; y: number } | null) {
  if (editor.shapeMove || editor.shapeResize || editor.tool === "crop" || editor.tool === "picker" || editor.draft?.type === "polyline" || editor.drawingStart) {
    const changed = (editor.shapeHoverId !== null || editor.shapeResizeHover !== null) && !editor.shapeMove && !editor.shapeResize;
    if (!editor.shapeMove && !editor.shapeResize) {
      editor.shapeHoverId = null;
      editor.shapeResizeHover = null;
    }
    return changed;
  }
  const shape = point ? latestTransformableImageEditorShape(editor) : null;
  const nextResizeHover = shape && point ? imageEditorShapeResizeHandleHit(editor, shape, point) : null;
  const nextShapeId = shape && point && (nextResizeHover || imageEditorShapeHit(editor, shape, point)) ? shape.id : null;
  if (editor.shapeHoverId === nextShapeId && editor.shapeResizeHover === nextResizeHover) {
    return false;
  }
  editor.shapeHoverId = nextShapeId;
  editor.shapeResizeHover = nextResizeHover;
  return true;
}

function clampShapeDeltaToActiveEditBounds(editor: ImageEditorTab, shape: ImageEditorShape, deltaX: number, deltaY: number) {
  const bounds = activeEditBounds(editor);
  const shapeBounds = imageEditorShapeBounds(shape);
  return {
    x: Math.min(Math.max(deltaX, bounds.left - shapeBounds.left), bounds.right - shapeBounds.right),
    y: Math.min(Math.max(deltaY, bounds.top - shapeBounds.top), bounds.bottom - shapeBounds.bottom),
  };
}

function moveImageEditorShape(shape: ImageEditorShape, deltaX: number, deltaY: number): ImageEditorShape {
  if (shape.type === "rect" || shape.type === "arrow") {
    return { ...shape, x1: shape.x1 + deltaX, y1: shape.y1 + deltaY, x2: shape.x2 + deltaX, y2: shape.y2 + deltaY };
  }
  if (shape.type === "text") {
    return { ...shape, x: shape.x + deltaX, y: shape.y + deltaY };
  }
  return {
    ...shape,
    points: shape.points.map((point) => ({ x: point.x + deltaX, y: point.y + deltaY })),
  };
}

function resizeBoundsForHandle(
  editor: ImageEditorTab,
  startBounds: { left: number; top: number; right: number; bottom: number },
  handle: CropHandle,
  point: { x: number; y: number },
) {
  const active = activeEditBounds(editor);
  const minSize = 4;
  let left = startBounds.left;
  let top = startBounds.top;
  let right = startBounds.right;
  let bottom = startBounds.bottom;
  if (handle === "left" || handle === "topLeft" || handle === "bottomLeft") {
    left = Math.min(Math.max(point.x, active.left), right - minSize);
  }
  if (handle === "right" || handle === "topRight" || handle === "bottomRight") {
    right = Math.max(Math.min(point.x, active.right), left + minSize);
  }
  if (handle === "top" || handle === "topLeft" || handle === "topRight") {
    top = Math.min(Math.max(point.y, active.top), bottom - minSize);
  }
  if (handle === "bottom" || handle === "bottomLeft" || handle === "bottomRight") {
    bottom = Math.max(Math.min(point.y, active.bottom), top + minSize);
  }
  return { left, top, right, bottom };
}

function transformPointWithinBounds(
  point: { x: number; y: number },
  startBounds: { left: number; top: number; right: number; bottom: number },
  nextBounds: { left: number; top: number; right: number; bottom: number },
) {
  const startWidth = startBounds.right - startBounds.left;
  const startHeight = startBounds.bottom - startBounds.top;
  const nextWidth = nextBounds.right - nextBounds.left;
  const nextHeight = nextBounds.bottom - nextBounds.top;
  const xRatio = startWidth > 0 ? (point.x - startBounds.left) / startWidth : 0.5;
  const yRatio = startHeight > 0 ? (point.y - startBounds.top) / startHeight : 0.5;
  return {
    x: nextBounds.left + xRatio * nextWidth,
    y: nextBounds.top + yRatio * nextHeight,
  };
}

function resizeImageEditorShape(
  shape: ImageEditorShape,
  startBounds: { left: number; top: number; right: number; bottom: number },
  nextBounds: { left: number; top: number; right: number; bottom: number },
): ImageEditorShape {
  if (shape.type === "rect" || shape.type === "arrow") {
    const startPoint = transformPointWithinBounds({ x: shape.x1, y: shape.y1 }, startBounds, nextBounds);
    const endPoint = transformPointWithinBounds({ x: shape.x2, y: shape.y2 }, startBounds, nextBounds);
    return { ...shape, x1: startPoint.x, y1: startPoint.y, x2: endPoint.x, y2: endPoint.y };
  }
  if (shape.type === "text") {
    const point = transformPointWithinBounds({ x: shape.x, y: shape.y }, startBounds, nextBounds);
    const startWidth = Math.max(1, startBounds.right - startBounds.left);
    const startHeight = Math.max(1, startBounds.bottom - startBounds.top);
    const scaleX = (nextBounds.right - nextBounds.left) / startWidth;
    const scaleY = (nextBounds.bottom - nextBounds.top) / startHeight;
    const fontScale = Math.max(0.25, Math.min(8, Math.max(scaleX, scaleY)));
    return { ...shape, x: point.x, y: point.y, fontSize: Math.max(8, shape.fontSize * fontScale) };
  }
  return {
    ...shape,
    points: shape.points.map((point) => transformPointWithinBounds(point, startBounds, nextBounds)),
  };
}

function makeEditorBoxShape(editor: ImageEditorTab, start: { x: number; y: number }, end: { x: number; y: number }): ImageEditorShape | null {
  if (editor.tool === "rect") {
    return {
      id: makeEditorShapeId(),
      type: "rect",
      x1: Math.min(start.x, end.x),
      y1: Math.min(start.y, end.y),
      x2: Math.max(start.x, end.x),
      y2: Math.max(start.y, end.y),
      color: editor.color,
      lineWidth: editor.lineWidth,
    };
  }
  if (editor.tool === "arrow") {
    return {
      id: makeEditorShapeId(),
      type: "arrow",
      x1: start.x,
      y1: start.y,
      x2: end.x,
      y2: end.y,
      color: editor.color,
      lineWidth: editor.lineWidth,
    };
  }
  return null;
}

function makeEditorMosaicShape(editor: ImageEditorTab, points: Array<{ x: number; y: number }>): ImageEditorShape {
  return {
    id: makeEditorShapeId(),
    type: "mosaic",
    points,
    blockSize: Math.max(8, editor.lineWidth * 5),
  };
}

function makeEditorEraserShape(editor: ImageEditorTab, points: Array<{ x: number; y: number }>): ImageEditorShape {
  return {
    id: makeEditorShapeId(),
    type: "eraser",
    points,
    lineWidth: Math.max(10, editor.lineWidth * 5),
  };
}

function editorTextLines(text: string) {
  return text.replace(/\r\n/g, "\n").split("\n");
}

function imageEditorArrowLength(shape: Extract<ImageEditorShape, { type: "arrow" }>) {
  return Math.hypot(shape.x2 - shape.x1, shape.y2 - shape.y1);
}

function imageEditorArrowHeadSize(shape: Extract<ImageEditorShape, { type: "arrow" }>) {
  const length = imageEditorArrowLength(shape);
  const preferred = shape.lineWidth * 4.5 + length * 0.035;
  const maxByLength = Math.max(6, length * 0.42);
  return Math.min(Math.max(7, preferred), maxByLength, 64);
}

function drawEditorArrowHead(context: CanvasRenderingContext2D, fromX: number, fromY: number, toX: number, toY: number, size: number) {
  const angle = Math.atan2(toY - fromY, toX - fromX);
  context.beginPath();
  context.moveTo(toX, toY);
  context.lineTo(toX - size * Math.cos(angle - Math.PI / 6), toY - size * Math.sin(angle - Math.PI / 6));
  context.moveTo(toX, toY);
  context.lineTo(toX - size * Math.cos(angle + Math.PI / 6), toY - size * Math.sin(angle + Math.PI / 6));
  context.stroke();
}

function drawEditorTaperedArrowLine(
  context: CanvasRenderingContext2D,
  shape: Extract<ImageEditorShape, { type: "arrow" }>,
  endX: number,
  endY: number,
) {
  const dx = endX - shape.x1;
  const dy = endY - shape.y1;
  const length = Math.hypot(dx, dy);
  if (length < 2) {
    return;
  }

  const segments = Math.min(56, Math.max(8, Math.ceil(length / 8)));
  const startWidth = Math.max(1, shape.lineWidth * 0.28);
  const endWidth = Math.max(startWidth, shape.lineWidth);
  for (let index = 0; index < segments; index++) {
    const startT = index / segments;
    const endT = (index + 1) / segments;
    const widthT = (index + 0.5) / segments;
    context.lineWidth = startWidth + (endWidth - startWidth) * widthT;
    context.beginPath();
    context.moveTo(shape.x1 + dx * startT, shape.y1 + dy * startT);
    context.lineTo(shape.x1 + dx * endT, shape.y1 + dy * endT);
    context.stroke();
  }
}

function drawEditorMosaic(context: CanvasRenderingContext2D, shape: Extract<ImageEditorShape, { type: "mosaic" }>) {
  const blockSize = Math.max(6, shape.blockSize);
  const radius = blockSize * 0.75;
  for (const point of shape.points) {
    const left = Math.max(0, Math.round(point.x - radius));
    const top = Math.max(0, Math.round(point.y - radius));
    const width = Math.min(context.canvas.width - left, Math.round(radius * 2));
    const height = Math.min(context.canvas.height - top, Math.round(radius * 2));
    if (width <= 0 || height <= 0) {
      continue;
    }
    const smallWidth = Math.max(1, Math.ceil(width / blockSize));
    const smallHeight = Math.max(1, Math.ceil(height / blockSize));
    const tempCanvas = document.createElement("canvas");
    tempCanvas.width = smallWidth;
    tempCanvas.height = smallHeight;
    const tempContext = tempCanvas.getContext("2d");
    if (!tempContext) {
      continue;
    }
    tempContext.imageSmoothingEnabled = false;
    tempContext.drawImage(context.canvas, left, top, width, height, 0, 0, smallWidth, smallHeight);
    context.save();
    context.imageSmoothingEnabled = false;
    context.beginPath();
    context.arc(point.x, point.y, radius, 0, Math.PI * 2);
    context.clip();
    context.drawImage(tempCanvas, 0, 0, smallWidth, smallHeight, left, top, width, height);
    context.restore();
  }
}

function drawEditorShape(context: CanvasRenderingContext2D, shape: ImageEditorShape) {
  if (shape.type === "mosaic") {
    drawEditorMosaic(context, shape);
    return;
  }
  if (shape.type === "eraser") {
    return;
  }

  context.save();
  context.lineJoin = "round";
  context.lineCap = "round";
  if (shape.type === "rect") {
    context.strokeStyle = shape.color;
    context.lineWidth = shape.lineWidth;
    context.strokeRect(shape.x1, shape.y1, shape.x2 - shape.x1, shape.y2 - shape.y1);
  } else if (shape.type === "arrow") {
    context.strokeStyle = shape.color;
    context.lineWidth = shape.lineWidth;
    const headSize = imageEditorArrowHeadSize(shape);
    const length = imageEditorArrowLength(shape);
    const angle = Math.atan2(shape.y2 - shape.y1, shape.x2 - shape.x1);
    const lineEndX = length > headSize ? shape.x2 - Math.cos(angle) * shape.lineWidth * 0.8 : shape.x2;
    const lineEndY = length > headSize ? shape.y2 - Math.sin(angle) * shape.lineWidth * 0.8 : shape.y2;
    drawEditorTaperedArrowLine(context, shape, lineEndX, lineEndY);
    context.lineWidth = shape.lineWidth;
    drawEditorArrowHead(context, shape.x1, shape.y1, shape.x2, shape.y2, headSize);
  } else if (shape.type === "polyline") {
    context.strokeStyle = shape.color;
    context.lineWidth = shape.lineWidth;
    if (shape.points.length > 0) {
      context.beginPath();
      context.moveTo(shape.points[0].x, shape.points[0].y);
      for (const point of shape.points.slice(1)) {
        context.lineTo(point.x, point.y);
      }
      context.stroke();
    }
  } else if (shape.type === "text") {
    const lines = editorTextLines(shape.text);
    const lineHeight = shape.fontSize * 1.25;
    context.font = `600 ${shape.fontSize}px system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
    context.textBaseline = "top";
    context.fillStyle = shape.color;
    lines.forEach((line, index) => {
      const y = shape.y + index * lineHeight;
      context.fillText(line || " ", shape.x, y);
    });
  }
  context.restore();
}

function drawEditorEraser(context: CanvasRenderingContext2D, editor: ImageEditorTab, shape: Extract<ImageEditorShape, { type: "eraser" }>) {
  if (shape.points.length === 0) {
    return;
  }

  const pattern = context.createPattern(editor.baseImage, "no-repeat");
  context.save();
  context.lineJoin = "round";
  context.lineCap = "round";
  context.lineWidth = shape.lineWidth;
  context.strokeStyle = pattern || "#ffffff";
  if (shape.points.length === 1) {
    const point = shape.points[0];
    context.beginPath();
    context.arc(point.x, point.y, shape.lineWidth / 2, 0, Math.PI * 2);
    context.clip();
    context.drawImage(editor.baseImage, 0, 0, editor.width, editor.height);
  } else {
    context.beginPath();
    context.moveTo(shape.points[0].x, shape.points[0].y);
    for (const point of shape.points.slice(1)) {
      context.lineTo(point.x, point.y);
    }
    context.stroke();
  }
  context.restore();
}

function normalizeCropRect(rect: { x1: number; y1: number; x2: number; y2: number }) {
  const left = Math.min(rect.x1, rect.x2);
  const top = Math.min(rect.y1, rect.y2);
  const right = Math.max(rect.x1, rect.x2);
  const bottom = Math.max(rect.y1, rect.y2);
  return {
    left,
    top,
    right,
    bottom,
    width: right - left,
    height: bottom - top,
  };
}

function drawEditorCrop(context: CanvasRenderingContext2D, editor: ImageEditorTab) {
  if (!editor.cropRect) {
    return;
  }
  const rect = normalizeCropRect(editor.cropRect);
  const handleSize = Math.max(8, Math.min(editor.width, editor.height) / 80);
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;
  const handles = [
    { x: rect.left, y: rect.top },
    { x: centerX, y: rect.top },
    { x: rect.right, y: rect.top },
    { x: rect.left, y: centerY },
    { x: rect.right, y: centerY },
    { x: rect.left, y: rect.bottom },
    { x: centerX, y: rect.bottom },
    { x: rect.right, y: rect.bottom },
  ];
  context.save();
  context.fillStyle = "rgba(15, 23, 42, 0.38)";
  context.fillRect(0, 0, editor.width, rect.top);
  context.fillRect(0, rect.bottom, editor.width, editor.height - rect.bottom);
  context.fillRect(0, rect.top, rect.left, rect.height);
  context.fillRect(rect.right, rect.top, editor.width - rect.right, rect.height);
  context.strokeStyle = "#38bdf8";
  context.lineWidth = Math.max(2, editor.width / 900);
  context.strokeRect(rect.left, rect.top, rect.width, rect.height);
  context.fillStyle = "#38bdf8";
  context.strokeStyle = "#ffffff";
  context.lineWidth = Math.max(1, editor.width / 1600);
  for (const handle of handles) {
    context.beginPath();
    context.rect(handle.x - handleSize / 2, handle.y - handleSize / 2, handleSize, handleSize);
    context.fill();
    context.stroke();
  }
  context.beginPath();
  context.arc(centerX, centerY, handleSize * 0.55, 0, Math.PI * 2);
  context.fill();
  context.stroke();
  context.restore();
}

function drawEditorShapeMoveHandles(context: CanvasRenderingContext2D, editor: ImageEditorTab) {
  const shapeId = editor.shapeResize?.shapeId || editor.shapeMove?.shapeId || editor.shapeHoverId;
  const shape = shapeId ? editor.shapes.find((item) => item.id === shapeId) : null;
  if (!shape || !isMovableImageEditorShape(shape)) {
    return;
  }
  const latestShape = editor.shapes[editor.shapes.length - 1];
  if (latestShape?.id !== shape.id) {
    return;
  }
  const bounds = imageEditorShapeBounds(shape);
  const handleSize = Math.max(7, Math.min(editor.width, editor.height) / 120);
  const centerX = bounds.left + (bounds.right - bounds.left) / 2;
  const centerY = bounds.top + (bounds.bottom - bounds.top) / 2;
  const handles = [
    { x: bounds.left, y: bounds.top },
    { x: centerX, y: bounds.top },
    { x: bounds.right, y: bounds.top },
    { x: bounds.left, y: centerY },
    { x: bounds.right, y: centerY },
    { x: bounds.left, y: bounds.bottom },
    { x: centerX, y: bounds.bottom },
    { x: bounds.right, y: bounds.bottom },
  ];

  context.save();
  context.strokeStyle = "#38bdf8";
  context.lineWidth = Math.max(1.5, editor.width / 1400);
  context.setLineDash([Math.max(4, handleSize * 0.8), Math.max(3, handleSize * 0.55)]);
  context.strokeRect(bounds.left, bounds.top, bounds.right - bounds.left, bounds.bottom - bounds.top);
  context.setLineDash([]);
  context.fillStyle = "#ffffff";
  context.strokeStyle = "#0284c7";
  context.lineWidth = Math.max(1, editor.width / 1800);
  for (const handle of handles) {
    context.beginPath();
    context.rect(handle.x - handleSize / 2, handle.y - handleSize / 2, handleSize, handleSize);
    context.fill();
    context.stroke();
  }
  context.restore();
}

function boundsHandleHit(
  editor: ImageEditorTab,
  bounds: { left: number; top: number; right: number; bottom: number },
  point: { x: number; y: number },
): CropHandle | null {
  const rect = {
    ...bounds,
    width: bounds.right - bounds.left,
    height: bounds.bottom - bounds.top,
  };
  const handleRadius = Math.max(12, Math.min(editor.width, editor.height) / 45);
  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;
  const handles: Array<{ handle: CropHandle; x: number; y: number }> = [
    { handle: "topLeft", x: rect.left, y: rect.top },
    { handle: "top", x: centerX, y: rect.top },
    { handle: "topRight", x: rect.right, y: rect.top },
    { handle: "left", x: rect.left, y: centerY },
    { handle: "right", x: rect.right, y: centerY },
    { handle: "bottomLeft", x: rect.left, y: rect.bottom },
    { handle: "bottom", x: centerX, y: rect.bottom },
    { handle: "bottomRight", x: rect.right, y: rect.bottom },
  ];
  const pointHandle = handles.find((item) => Math.hypot(point.x - item.x, point.y - item.y) <= handleRadius)?.handle;
  if (pointHandle) {
    return pointHandle;
  }

  const edgePadding = Math.max(8, Math.min(editor.width, editor.height) / 90);
  const nearLeft = Math.abs(point.x - rect.left) <= edgePadding && point.y >= rect.top - edgePadding && point.y <= rect.bottom + edgePadding;
  const nearRight = Math.abs(point.x - rect.right) <= edgePadding && point.y >= rect.top - edgePadding && point.y <= rect.bottom + edgePadding;
  const nearTop = Math.abs(point.y - rect.top) <= edgePadding && point.x >= rect.left - edgePadding && point.x <= rect.right + edgePadding;
  const nearBottom = Math.abs(point.y - rect.bottom) <= edgePadding && point.x >= rect.left - edgePadding && point.x <= rect.right + edgePadding;
  if (nearTop && nearLeft) {
    return "topLeft";
  }
  if (nearTop && nearRight) {
    return "topRight";
  }
  if (nearBottom && nearLeft) {
    return "bottomLeft";
  }
  if (nearBottom && nearRight) {
    return "bottomRight";
  }
  if (nearTop) {
    return "top";
  }
  if (nearBottom) {
    return "bottom";
  }
  if (nearLeft) {
    return "left";
  }
  if (nearRight) {
    return "right";
  }
  return null;
}

function cropHandleHit(editor: ImageEditorTab, point: { x: number; y: number }): CropHandle | null {
  return editor.cropRect ? boundsHandleHit(editor, normalizeCropRect(editor.cropRect), point) : null;
}

function cropCursorForHandle(handle: CropHandle | "move" | null) {
  if (handle === "move") {
    return "grab";
  }
  if (handle === "topLeft" || handle === "bottomRight") {
    return "nwse-resize";
  }
  if (handle === "topRight" || handle === "bottomLeft") {
    return "nesw-resize";
  }
  if (handle === "left" || handle === "right") {
    return "ew-resize";
  }
  if (handle === "top" || handle === "bottom") {
    return "ns-resize";
  }
  return "crosshair";
}

function imageEditorCanvasCursor(editor: ImageEditorTab) {
  if (editor.shapeResize) {
    return cropCursorForHandle(editor.shapeResize.handle);
  }
  if (editor.shapeResizeHover) {
    return cropCursorForHandle(editor.shapeResizeHover);
  }
  if (editor.shapeMove) {
    return "grabbing";
  }
  if (editor.shapeHoverId) {
    return "grab";
  }
  if (editor.tool !== "crop") {
    return undefined;
  }
  const drag = editor.cropDrag;
  if (drag?.mode === "move") {
    return "grabbing";
  }
  if (drag?.mode === "resize") {
    return cropCursorForHandle(drag.handle);
  }
  return cropCursorForHandle(editor.cropHover);
}

function updateCropHover(editor: ImageEditorTab, point: { x: number; y: number } | null) {
  if (editor.tool !== "crop" || !point) {
    editor.cropHover = null;
    return;
  }
  const handle = cropHandleHit(editor, point);
  editor.cropHover = handle || (cropContainsPoint(editor, point) ? "move" : null);
}

function cropContainsPoint(editor: ImageEditorTab, point: { x: number; y: number }) {
  if (!editor.cropRect) {
    return false;
  }
  const rect = normalizeCropRect(editor.cropRect);
  return point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom;
}

function clampCropRect(editor: ImageEditorTab, rect: { x1: number; y1: number; x2: number; y2: number }) {
  return {
    x1: Math.min(Math.max(rect.x1, 0), editor.width),
    y1: Math.min(Math.max(rect.y1, 0), editor.height),
    x2: Math.min(Math.max(rect.x2, 0), editor.width),
    y2: Math.min(Math.max(rect.y2, 0), editor.height),
  };
}

function updateCropDrag(editor: ImageEditorTab, point: { x: number; y: number }) {
  const drag = editor.cropDrag;
  if (!drag) {
    return;
  }
  if (drag.mode === "new") {
    editor.cropRect = clampCropRect(editor, {
      x1: drag.startPoint.x,
      y1: drag.startPoint.y,
      x2: point.x,
      y2: point.y,
    });
    return;
  }

  if (drag.mode === "move") {
    const rect = normalizeCropRect(drag.startRect);
    const dx = point.x - drag.startPoint.x;
    const dy = point.y - drag.startPoint.y;
    const width = rect.width;
    const height = rect.height;
    const left = Math.min(Math.max(rect.left + dx, 0), editor.width - width);
    const top = Math.min(Math.max(rect.top + dy, 0), editor.height - height);
    editor.cropRect = { x1: left, y1: top, x2: left + width, y2: top + height };
    return;
  }

  const next = { ...drag.startRect };
  if (drag.handle === "left" || drag.handle === "topLeft" || drag.handle === "bottomLeft") {
    next.x1 = Math.min(point.x, drag.startRect.x2 - 4);
  }
  if (drag.handle === "right" || drag.handle === "topRight" || drag.handle === "bottomRight") {
    next.x2 = Math.max(point.x, drag.startRect.x1 + 4);
  }
  if (drag.handle === "top" || drag.handle === "topLeft" || drag.handle === "topRight") {
    next.y1 = Math.min(point.y, drag.startRect.y2 - 4);
  }
  if (drag.handle === "bottom" || drag.handle === "bottomLeft" || drag.handle === "bottomRight") {
    next.y2 = Math.max(point.y, drag.startRect.y1 + 4);
  }
  editor.cropRect = clampCropRect(editor, next);
}

function renderImageEditor(editor: ImageEditorTab) {
  const canvas = imageEditorCanvases.get(editor.key);
  if (!canvas) {
    return;
  }
  if (canvas.width !== editor.width) {
    canvas.width = editor.width;
  }
  if (canvas.height !== editor.height) {
    canvas.height = editor.height;
  }
  const context = canvas.getContext("2d");
  if (!context) {
    return;
  }
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.drawImage(editor.baseImage, 0, 0, canvas.width, canvas.height);
  const shapes = editor.draft ? [...editor.shapes, editor.draft] : editor.shapes;
  for (const shape of shapes) {
    if (shape.type === "eraser") {
      drawEditorEraser(context, editor, shape);
    } else {
      drawEditorShape(context, shape);
    }
  }
  drawEditorCrop(context, editor);
  drawEditorShapeMoveHandles(context, editor);
}

async function openScreenshotEditor(dataUrl: string, source: "native" | "mirror") {
  const image = await loadImage(dataUrl);
  const id = nextImageEditorTabId++;
  const editor: ImageEditorTab = {
    id,
    key: imageEditorKey(id),
    title: source === "mirror" ? `投屏截图 ${id}` : `设备截图 ${id}`,
    source,
    baseDataUrl: dataUrl,
    baseImage: markRaw(image),
    width: image.naturalWidth || image.width,
    height: image.naturalHeight || image.height,
    tool: "rect",
    color: "#ef4444",
    lineWidth: 3,
    shapes: [],
    draft: null,
    cropRect: null,
    cropDrag: null,
    shapeMove: null,
    shapeResize: null,
    shapeHoverId: null,
    shapeResizeHover: null,
    cropHover: null,
    drawingStart: null,
    textInput: null,
    pickerPreview: null,
    undoStack: [],
    redoStack: [],
    saving: false,
  };
  imageEditorTabs.value = [...imageEditorTabs.value, editor];
  registerWorkspaceTab(editor.key);
  activeWorkspaceTabKey.value = editor.key;
  scrollWorkspaceTabIntoView(editor.key);
  await nextTick();
  renderImageEditor(editor);
}

function updateImageEditorTool(editor: ImageEditorTab, tool: ImageEditorTool) {
  commitImageEditorText(editor);
  editor.tool = tool;
  editor.draft = null;
  editor.drawingStart = null;
  editor.cropDrag = null;
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.shapeHoverId = null;
  editor.shapeResizeHover = null;
  editor.cropHover = null;
  editor.pickerPreview = null;
  void renderImageEditor(editor);
}

function updateImageEditorColor(editor: ImageEditorTab, value: string) {
  editor.color = value;
}

function commitImageEditorText(editor: ImageEditorTab, inputId?: string) {
  const input = editor.textInput;
  if (inputId && input?.id !== inputId) {
    return;
  }
  if (!input) {
    return;
  }
  const text = input.value.replace(/\r\n/g, "\n").replace(/\s+$/g, "");
  if (text) {
    if (!input.historyCaptured) {
      pushImageEditorHistory(editor);
    }
    const point = clampPointToActiveEditBounds(editor, input);
    editor.shapes.push({
      id: input.id,
      type: "text",
      x: point.x,
      y: point.y,
      text,
      color: input.color || editor.color,
      fontSize: input.fontSize || Math.max(14, editor.lineWidth * 6),
    });
  }
  editor.textInput = null;
  markLatestShapeIfMovable(editor);
  renderImageEditor(editor);
}

function cancelImageEditorText(editor: ImageEditorTab, inputId?: string) {
  const input = editor.textInput;
  if (inputId && input?.id !== inputId) {
    return;
  }
  if (input?.originalShape && !editor.shapes.some((shape) => shape.id === input.originalShape?.id)) {
    editor.shapes.push(input.originalShape);
  }
  editor.textInput = null;
  markLatestShapeIfMovable(editor);
  renderImageEditor(editor);
}

function resizeImageEditorTextInput(event: Event) {
  const textarea = event.target as HTMLTextAreaElement | null;
  if (!textarea) {
    return;
  }
  textarea.style.height = "auto";
  textarea.style.width = "auto";
  textarea.style.height = `${textarea.scrollHeight}px`;
  textarea.style.width = `${Math.min(Math.max(textarea.scrollWidth + 1, 80), 360)}px`;
}

function imageEditorTextInputStyle(editor: ImageEditorTab) {
  const input = editor.textInput;
  const canvas = imageEditorCanvases.get(editor.key);
  const parent = canvas?.parentElement;
  if (!input || !canvas || !parent) {
    return {};
  }
  const canvasRect = canvas.getBoundingClientRect();
  const parentRect = parent.getBoundingClientRect();
  const fontSize = Math.max(14, editor.lineWidth * 6);
  const displayScale = canvasRect.width > 0 && editor.width > 0 ? canvasRect.width / editor.width : 1;
  const displayFontSize = (input.fontSize || fontSize) * displayScale;
  return {
    left: `${canvasRect.left - parentRect.left + (input.x / editor.width) * canvasRect.width}px`,
    top: `${canvasRect.top - parentRect.top + (input.y / editor.height) * canvasRect.height}px`,
    color: input.color || editor.color,
    fontSize: `${displayFontSize}px`,
  };
}

function editorColorAt(editor: ImageEditorTab, point: { x: number; y: number }) {
  const canvas = imageEditorCanvases.get(editor.key);
  const context = canvas?.getContext("2d");
  if (!canvas || !context) {
    return null;
  }
  const x = Math.min(Math.max(Math.round(point.x), 0), canvas.width - 1);
  const y = Math.min(Math.max(Math.round(point.y), 0), canvas.height - 1);
  const pixel = context.getImageData(x, y, 1, 1).data;
  const rgb: [number, number, number] = [pixel[0], pixel[1], pixel[2]];
  return {
    color: `#${rgb.map((value) => value.toString(16).padStart(2, "0")).join("")}`,
    rgb,
    magnifier: buildPickerMagnifier(context, x, y),
  };
}

function buildPickerMagnifier(context: CanvasRenderingContext2D, x: number, y: number) {
  const sourceSize = 11;
  const scale = 8;
  const half = Math.floor(sourceSize / 2);
  const sourceCanvas = context.canvas;
  const magnifierCanvas = document.createElement("canvas");
  magnifierCanvas.width = sourceSize * scale;
  magnifierCanvas.height = sourceSize * scale;
  const magnifierContext = magnifierCanvas.getContext("2d");
  if (!magnifierContext) {
    return "";
  }

  magnifierContext.imageSmoothingEnabled = false;
  magnifierContext.fillStyle = "#f8fafc";
  magnifierContext.fillRect(0, 0, magnifierCanvas.width, magnifierCanvas.height);
  magnifierContext.drawImage(
    sourceCanvas,
    Math.max(0, x - half),
    Math.max(0, y - half),
    Math.min(sourceSize, sourceCanvas.width - Math.max(0, x - half)),
    Math.min(sourceSize, sourceCanvas.height - Math.max(0, y - half)),
    0,
    0,
    sourceSize * scale,
    sourceSize * scale,
  );
  magnifierContext.strokeStyle = "rgba(15, 23, 42, 0.22)";
  magnifierContext.lineWidth = 1;
  for (let index = 1; index < sourceSize; index += 1) {
    const pos = index * scale + 0.5;
    magnifierContext.beginPath();
    magnifierContext.moveTo(pos, 0);
    magnifierContext.lineTo(pos, magnifierCanvas.height);
    magnifierContext.stroke();
    magnifierContext.beginPath();
    magnifierContext.moveTo(0, pos);
    magnifierContext.lineTo(magnifierCanvas.width, pos);
    magnifierContext.stroke();
  }
  magnifierContext.strokeStyle = "#ef4444";
  magnifierContext.lineWidth = 2;
  magnifierContext.strokeRect(half * scale + 1, half * scale + 1, scale - 2, scale - 2);
  return magnifierCanvas.toDataURL("image/png");
}

function updatePickerPreview(editor: ImageEditorTab, point: { x: number; y: number } | null) {
  if (editor.tool !== "picker" || !point) {
    editor.pickerPreview = null;
    return;
  }
  const sample = editorColorAt(editor, point);
  if (sample) {
    editor.pickerPreview = { x: point.x, y: point.y, ...sample };
  }
}

function pickEditorColorAt(editor: ImageEditorTab, point: { x: number; y: number }) {
  const sample = editorColorAt(editor, point);
  if (sample) {
    editor.color = sample.color;
    editor.pickerPreview = { x: point.x, y: point.y, ...sample };
  }
}

function imageEditorPickerStyle(editor: ImageEditorTab) {
  const preview = editor.pickerPreview;
  const canvas = imageEditorCanvases.get(editor.key);
  const parent = canvas?.parentElement;
  if (!preview || !canvas || !parent) {
    return {};
  }
  const canvasRect = canvas.getBoundingClientRect();
  const parentRect = parent.getBoundingClientRect();
  const popoverWidth = 220;
  const popoverHeight = 104;
  const margin = 8;
  const offset = 14;
  const anchorX = canvasRect.left - parentRect.left + (preview.x / editor.width) * canvasRect.width;
  const anchorY = canvasRect.top - parentRect.top + (preview.y / editor.height) * canvasRect.height;
  const maxLeft = Math.max(margin, parentRect.width - popoverWidth - margin);
  const maxTop = Math.max(margin, parentRect.height - popoverHeight - margin);
  const preferredLeft = anchorX + offset + popoverWidth <= parentRect.width - margin
    ? anchorX + offset
    : anchorX - popoverWidth - offset;
  const preferredTop = anchorY + offset + popoverHeight <= parentRect.height - margin
    ? anchorY + offset
    : anchorY - popoverHeight - offset;
  const left = Math.max(margin, Math.min(preferredLeft, maxLeft));
  const top = Math.max(margin, Math.min(preferredTop, maxTop));
  return {
    left: `${left}px`,
    top: `${top}px`,
  };
}

async function applyEditorCrop(editor: ImageEditorTab) {
  commitImageEditorText(editor);
  const crop = editor.cropRect;
  if (!crop) {
    return;
  }
  pushImageEditorHistory(editor);
  const left = Math.max(0, Math.min(crop.x1, crop.x2));
  const top = Math.max(0, Math.min(crop.y1, crop.y2));
  const width = Math.min(editor.width - left, Math.abs(crop.x2 - crop.x1));
  const height = Math.min(editor.height - top, Math.abs(crop.y2 - crop.y1));
  if (width < 4 || height < 4) {
    adbStore.setNotice("error", "裁剪区域太小");
    return;
  }

  const canvas = imageEditorCanvases.get(editor.key);
  if (!canvas) {
    return;
  }
  editor.cropRect = null;
  renderImageEditor(editor);
  const targetCanvas = document.createElement("canvas");
  targetCanvas.width = Math.round(width);
  targetCanvas.height = Math.round(height);
  const context = targetCanvas.getContext("2d");
  if (!context) {
    return;
  }
  context.drawImage(canvas, left, top, width, height, 0, 0, targetCanvas.width, targetCanvas.height);
  const blob = await canvasToPngBlob(targetCanvas);
  editor.baseDataUrl = await blobToDataUrl(blob);
  editor.baseImage = markRaw(await loadImage(editor.baseDataUrl));
  editor.width = targetCanvas.width;
  editor.height = targetCanvas.height;
  editor.shapes = [];
  editor.draft = null;
  editor.cropRect = null;
  editor.cropDrag = null;
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.shapeHoverId = null;
  editor.shapeResizeHover = null;
  editor.cropHover = null;
  editor.drawingStart = null;
  editor.textInput = null;
  editor.pickerPreview = null;
  await nextTick();
  renderImageEditor(editor);
}

function resetImageEditor(editor: ImageEditorTab) {
  pushImageEditorHistory(editor);
  editor.shapes = [];
  editor.draft = null;
  editor.cropRect = null;
  editor.cropDrag = null;
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.shapeHoverId = null;
  editor.shapeResizeHover = null;
  editor.cropHover = null;
  editor.drawingStart = null;
  editor.textInput = null;
  editor.pickerPreview = null;
  void renderImageEditor(editor);
}

function startImageEditorShapeMove(editor: ImageEditorTab, shape: ImageEditorShape, point: { x: number; y: number }, pointerId: number) {
  editor.shapeHoverId = shape.id;
  editor.shapeResizeHover = null;
  editor.shapeResize = null;
  editor.shapeMove = {
    shapeId: shape.id,
    startPoint: point,
    startShape: cloneImageEditorShapes([shape])[0],
    moved: false,
    historyCaptured: false,
  };
  editor.draft = null;
  editor.drawingStart = point;
  imageEditorCanvases.get(editor.key)?.setPointerCapture(pointerId);
}

function startImageEditorShapeResize(editor: ImageEditorTab, shape: ImageEditorShape, handle: CropHandle, point: { x: number; y: number }, pointerId: number) {
  editor.shapeHoverId = shape.id;
  editor.shapeResizeHover = handle;
  editor.shapeMove = null;
  editor.shapeResize = {
    shapeId: shape.id,
    handle,
    startPoint: point,
    startShape: cloneImageEditorShapes([shape])[0],
    startBounds: imageEditorShapeBounds(shape),
    moved: false,
    historyCaptured: false,
  };
  editor.draft = null;
  editor.drawingStart = point;
  imageEditorCanvases.get(editor.key)?.setPointerCapture(pointerId);
}

function updateImageEditorShapeMove(editor: ImageEditorTab, point: { x: number; y: number }) {
  const move = editor.shapeMove;
  if (!move) {
    return;
  }
  const rawDeltaX = point.x - move.startPoint.x;
  const rawDeltaY = point.y - move.startPoint.y;
  const delta = clampShapeDeltaToActiveEditBounds(editor, move.startShape, rawDeltaX, rawDeltaY);
  const moved = move.moved || Math.hypot(delta.x, delta.y) >= 1;
  if (moved && !move.historyCaptured) {
    pushImageEditorHistory(editor);
  }
  const movedShape = moveImageEditorShape(move.startShape, delta.x, delta.y);
  editor.shapes = editor.shapes.map((shape) => (shape.id === move.shapeId ? movedShape : shape));
  editor.shapeMove = {
    ...move,
    moved,
    historyCaptured: move.historyCaptured || moved,
  };
}

function updateImageEditorShapeResize(editor: ImageEditorTab, point: { x: number; y: number }) {
  const resize = editor.shapeResize;
  if (!resize) {
    return;
  }
  const nextBounds = resizeBoundsForHandle(editor, resize.startBounds, resize.handle, point);
  const moved = resize.moved || Math.hypot(point.x - resize.startPoint.x, point.y - resize.startPoint.y) >= 1;
  if (moved && !resize.historyCaptured) {
    pushImageEditorHistory(editor);
  }
  const resizedShape = resizeImageEditorShape(resize.startShape, resize.startBounds, nextBounds);
  editor.shapes = editor.shapes.map((shape) => (shape.id === resize.shapeId ? resizedShape : shape));
  editor.shapeResize = {
    ...resize,
    moved,
    historyCaptured: resize.historyCaptured || moved,
  };
}

function finishImageEditorPolyline(editor: ImageEditorTab) {
  const draft = editor.draft;
  if (draft?.type !== "polyline") {
    return false;
  }
  const points = draft.points;
  const finalPoints = (points.length > 2 ? points.slice(0, -1) : points).reduce<Array<{ x: number; y: number }>>((next, point) => {
    const previous = next[next.length - 1];
    if (!previous || Math.hypot(point.x - previous.x, point.y - previous.y) >= 1) {
      next.push(point);
    }
    return next;
  }, []);
  if (finalPoints.length >= 2) {
    pushImageEditorHistory(editor);
    editor.shapes.push({ ...draft, points: finalPoints });
    markLatestShapeIfMovable(editor);
  }
  editor.draft = null;
  editor.drawingStart = null;
  void renderImageEditor(editor);
  return true;
}

function onImageEditorDoubleClick(editor: ImageEditorTab, event: MouseEvent) {
  const point = editorPoint(editor, event);
  const latestShape = editor.shapes[editor.shapes.length - 1];
  if (point && latestShape?.type === "text" && imageEditorShapeHit(editor, latestShape, point)) {
    event.preventDefault();
    beginEditLatestTextShape(editor, latestShape);
    return;
  }
  if (editor.tool === "polyline") {
    event.preventDefault();
    finishImageEditorPolyline(editor);
  }
}

function cancelImageEditorPolyline(editor: ImageEditorTab) {
  if (editor.draft?.type !== "polyline") {
    return false;
  }
  editor.draft = null;
  editor.drawingStart = null;
  void renderImageEditor(editor);
  return true;
}

function focusImageEditorTextInput(editor: ImageEditorTab) {
  void nextTick(() => {
    const input = document.querySelector<HTMLTextAreaElement>(`[data-editor-text-input="${editor.key}"]`);
    input?.focus();
    input?.select();
    if (input) {
      resizeImageEditorTextInput({ target: input } as unknown as Event);
    }
  });
}

function beginEditLatestTextShape(editor: ImageEditorTab, shape: Extract<ImageEditorShape, { type: "text" }>) {
  pushImageEditorHistory(editor);
  editor.shapes = editor.shapes.filter((item) => item.id !== shape.id);
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.shapeHoverId = null;
  editor.shapeResizeHover = null;
  editor.drawingStart = null;
  editor.textInput = {
    id: shape.id,
    x: shape.x,
    y: shape.y,
    value: shape.text,
    color: shape.color,
    fontSize: shape.fontSize,
    historyCaptured: true,
    originalShape: cloneImageEditorShapes([shape])[0] as Extract<ImageEditorShape, { type: "text" }>,
  };
  focusImageEditorTextInput(editor);
  void renderImageEditor(editor);
}

function markLatestShapeIfMovable(editor: ImageEditorTab) {
  const latestShape = editor.shapes[editor.shapes.length - 1];
  editor.shapeHoverId = latestShape && isMovableImageEditorShape(latestShape) ? latestShape.id : null;
  editor.shapeResizeHover = null;
}

function onImageEditorPointerLeave(editor: ImageEditorTab) {
  updatePickerPreview(editor, null);
  if (editor.shapeMove || editor.shapeResize) {
    return;
  }
  if (editor.shapeHoverId || editor.shapeResizeHover) {
    editor.shapeHoverId = null;
    editor.shapeResizeHover = null;
    void renderImageEditor(editor);
  }
}

function onImageEditorPointerDown(editor: ImageEditorTab, event: PointerEvent) {
  if (event.button !== 0 || editor.saving) {
    return;
  }
  if (editor.tool !== "text") {
    commitImageEditorText(editor);
  }
  const point = editorPoint(editor, event);
  if (!point) {
    return;
  }
  updateCropHover(editor, point);
  event.preventDefault();

  if (editor.tool === "picker") {
    updatePickerPreview(editor, point);
    pickEditorColorAt(editor, point);
    return;
  }
  if (editor.tool !== "crop" && !(editor.tool === "polyline" && editor.draft?.type === "polyline")) {
    if (editor.tool === "text") {
      commitImageEditorText(editor);
    }
    const transformShape = latestTransformableImageEditorShape(editor);
    const resizeHandle = transformShape ? imageEditorShapeResizeHandleHit(editor, transformShape, point) : null;
    if (transformShape && resizeHandle) {
      startImageEditorShapeResize(editor, transformShape, resizeHandle, point, event.pointerId);
      return;
    }
    const hitShape = findMovableImageEditorShape(editor, point);
    if (hitShape) {
      startImageEditorShapeMove(editor, hitShape, point, event.pointerId);
      return;
    }
  }
  if (editor.tool === "text") {
    const textPoint = clampPointToActiveEditBounds(editor, point);
    editor.textInput = { id: makeEditorShapeId(), x: textPoint.x, y: textPoint.y, value: "" };
    focusImageEditorTextInput(editor);
    return;
  }
  const boundedPoint = editor.tool === "crop" ? point : clampPointToActiveEditBounds(editor, point);
  editor.drawingStart = boundedPoint;
  if (editor.tool === "crop") {
    pushImageEditorHistory(editor);
    const handle = cropHandleHit(editor, boundedPoint);
    if (handle && editor.cropRect) {
      const rect = normalizeCropRect(editor.cropRect);
      editor.cropDrag = { mode: "resize", handle, startPoint: boundedPoint, startRect: { x1: rect.left, y1: rect.top, x2: rect.right, y2: rect.bottom } };
    } else if (cropContainsPoint(editor, boundedPoint) && editor.cropRect) {
      const rect = normalizeCropRect(editor.cropRect);
      editor.cropDrag = { mode: "move", startPoint: boundedPoint, startRect: { x1: rect.left, y1: rect.top, x2: rect.right, y2: rect.bottom } };
    } else {
      editor.cropDrag = { mode: "new", startPoint: boundedPoint };
      editor.cropRect = { x1: boundedPoint.x, y1: boundedPoint.y, x2: boundedPoint.x, y2: boundedPoint.y };
    }
  } else if (editor.tool === "mosaic") {
    editor.draft = makeEditorMosaicShape(editor, [boundedPoint]);
  } else if (editor.tool === "eraser") {
    editor.draft = makeEditorEraserShape(editor, [boundedPoint]);
  } else if (editor.tool === "polyline") {
    if (editor.draft?.type === "polyline") {
      const fixedPoints = editor.draft.points.length > 1 ? [...editor.draft.points.slice(0, -1), boundedPoint] : [boundedPoint];
      editor.draft = { ...editor.draft, points: [...fixedPoints, boundedPoint] };
    } else {
      editor.draft = {
        id: makeEditorShapeId(),
        type: "polyline",
        points: [boundedPoint, boundedPoint],
        color: editor.color,
        lineWidth: editor.lineWidth,
      };
    }
  } else {
    editor.draft = makeEditorBoxShape(editor, boundedPoint, boundedPoint);
  }
  if (editor.tool !== "polyline") {
    imageEditorCanvases.get(editor.key)?.setPointerCapture(event.pointerId);
  }
  void renderImageEditor(editor);
}

function onImageEditorPointerMove(editor: ImageEditorTab, event: PointerEvent) {
  const start = editor.drawingStart;
  const rawPoint = editorPoint(editor, event);
  const point = editor.tool === "crop" || !rawPoint ? rawPoint : clampPointToActiveEditBounds(editor, rawPoint);
  if (editor.tool === "picker") {
    updatePickerPreview(editor, rawPoint);
    return;
  }
  if (editor.shapeResize && point) {
    event.preventDefault();
    updateImageEditorShapeResize(editor, point);
    void renderImageEditor(editor);
    return;
  }
  if (editor.shapeMove && point) {
    event.preventDefault();
    updateImageEditorShapeMove(editor, point);
    void renderImageEditor(editor);
    return;
  }
  if (editor.tool === "polyline" && editor.draft?.type === "polyline" && point) {
    const points = editor.draft.points;
    if (points.length > 0) {
      editor.draft = { ...editor.draft, points: [...points.slice(0, -1), point] };
      void renderImageEditor(editor);
    }
    return;
  }
  if (!start) {
    const shapeHoverChanged = updateImageEditorShapeHover(editor, point);
    updateCropHover(editor, point);
    if (shapeHoverChanged) {
      void renderImageEditor(editor);
    }
  }
  if (!start || !point || editor.saving) {
    return;
  }
  event.preventDefault();
  if (editor.tool === "crop" && editor.cropRect) {
    updateCropDrag(editor, point);
  } else if (editor.tool === "mosaic" && editor.draft?.type === "mosaic") {
    const last = editor.draft.points[editor.draft.points.length - 1];
    if (!last || Math.hypot(point.x - last.x, point.y - last.y) >= Math.max(2, editor.lineWidth * 1.5)) {
      editor.draft = {
        ...editor.draft,
        points: [...editor.draft.points, point],
      };
    }
  } else if (editor.tool === "eraser" && editor.draft?.type === "eraser") {
    const last = editor.draft.points[editor.draft.points.length - 1];
    if (!last || Math.hypot(point.x - last.x, point.y - last.y) >= Math.max(2, editor.lineWidth * 1.5)) {
      editor.draft = {
        ...editor.draft,
        points: [...editor.draft.points, point],
      };
    }
  } else {
    editor.draft = makeEditorBoxShape(editor, start, point);
  }
  void renderImageEditor(editor);
}

function onImageEditorPointerUp(editor: ImageEditorTab, event: PointerEvent) {
  const start = editor.drawingStart;
  const rawPoint = editorPoint(editor, event);
  const point = editor.tool === "crop" || !rawPoint ? rawPoint : clampPointToActiveEditBounds(editor, rawPoint);
  imageEditorCanvases.get(editor.key)?.releasePointerCapture(event.pointerId);
  if (editor.shapeResize) {
    event.preventDefault();
    editor.shapeResize = null;
    editor.shapeResizeHover = null;
    editor.drawingStart = null;
    markLatestShapeIfMovable(editor);
    void renderImageEditor(editor);
    return;
  }
  if (editor.shapeMove) {
    event.preventDefault();
    editor.shapeMove = null;
    editor.drawingStart = null;
    markLatestShapeIfMovable(editor);
    void renderImageEditor(editor);
    return;
  }
  if (editor.tool === "polyline" && editor.draft?.type === "polyline") {
    return;
  }
  if (!start || !point || editor.saving) {
    return;
  }
  event.preventDefault();
  const distance = Math.hypot(point.x - start.x, point.y - start.y);
  if ((editor.tool === "mosaic" && editor.draft?.type === "mosaic") || (editor.tool === "eraser" && editor.draft?.type === "eraser")) {
    pushImageEditorHistory(editor);
    editor.shapes.push(editor.draft);
  } else if (editor.tool !== "crop" && editor.draft && distance >= 4) {
    pushImageEditorHistory(editor);
    editor.shapes.push(editor.draft);
  }
  markLatestShapeIfMovable(editor);
  editor.cropDrag = null;
  editor.shapeMove = null;
  editor.shapeResize = null;
  editor.draft = null;
  editor.drawingStart = null;
  updateCropHover(editor, point);
  void renderImageEditor(editor);
}

async function saveImageEditor(editor: ImageEditorTab) {
  if (editor.saving) {
    return;
  }
  commitImageEditorText(editor);
  if (editor.cropRect) {
    await applyEditorCrop(editor);
  }
  const canvas = imageEditorCanvases.get(editor.key);
  if (!canvas) {
    return;
  }
  const shouldSave = screenshotMode.value === "save" || screenshotMode.value === "save-copy";
  const shouldCopy = screenshotMode.value === "copy" || screenshotMode.value === "save-copy";
  const localDir = shouldSave ? await ensureDownloadLocalDir() : "";
  if (shouldSave && !localDir) {
    return;
  }

  editor.saving = true;
  try {
    renderImageEditor(editor);
    const blob = await canvasToPngBlob(canvas);
    const dataBase64 = await blobToBase64(blob);
    const result = await invoke<AdbExecResult>("save_local_screenshot", {
      localDir: shouldSave ? localDir : null,
      fileName: editor.source === "mirror" ? "droidpilot_mirror.png" : "droidpilot_screen.png",
      dataBase64,
      copyToClipboard: shouldCopy,
    });
    adbStore.setNotice(result.success ? "success" : "error", result.stdout || result.stderr || "截图已保存");
  } catch (error) {
    adbStore.setNotice("error", String(error));
  } finally {
    editor.saving = false;
  }
}

function findNalUnits(data: Uint8Array) {
  const units: Uint8Array[] = [];
  let start = -1;
  let index = 0;

  while (index < data.length - 3) {
    const isThreeByteStart = data[index] === 0 && data[index + 1] === 0 && data[index + 2] === 1;
    const isFourByteStart =
      data[index] === 0 && data[index + 1] === 0 && data[index + 2] === 0 && data[index + 3] === 1;

    if (isThreeByteStart || isFourByteStart) {
      if (start >= 0) {
        units.push(data.subarray(start, index));
      }
      start = index + (isFourByteStart ? 4 : 3);
      index = start;
      continue;
    }

    index++;
  }

  if (start >= 0 && start < data.length) {
    units.push(data.subarray(start));
  }

  return units;
}

function hexByte(value: number) {
  return value.toString(16).padStart(2, "0").toUpperCase();
}

function codecFromSps(data: Uint8Array) {
  for (const unit of findNalUnits(data)) {
    if (unit.length < 4) {
      continue;
    }
    const nalType = unit[0] & 0x1f;
    if (nalType === 7) {
      return `avc1.${hexByte(unit[1])}${hexByte(unit[2])}${hexByte(unit[3])}`;
    }
  }
  return "avc1.42E01F";
}

function hasSps(data: Uint8Array) {
  return findNalUnits(data).some((unit) => unit.length > 0 && (unit[0] & 0x1f) === 7);
}

function prependBytes(prefix: Uint8Array, data: Uint8Array) {
  const merged = new Uint8Array(prefix.length + data.length);
  merged.set(prefix, 0);
  merged.set(data, prefix.length);
  return merged;
}

function mediaPacketForWebCodecs(tab: MirrorTab, data: Uint8Array, keyFrame: boolean) {
  const hasInBandConfig = hasSps(data);
  const pendingConfig = tab.pendingConfigPacket;
  tab.pendingConfigPacket = null;

  if (pendingConfig && !hasInBandConfig) {
    return prependBytes(pendingConfig, data);
  }

  if (keyFrame && tab.latestConfigPacket && !hasInBandConfig) {
    return prependBytes(tab.latestConfigPacket, data);
  }

  return data;
}

function frameDebugSummary(frame: ScrcpyFramePayload) {
  return {
    config: frame.config,
    keyFrame: frame.keyFrame,
    pts: frame.pts,
    payloadBytesBase64: frame.data.length,
  };
}

function pendingFrameStats(frames: ScrcpyFramePayload[]) {
  return {
    frames: frames.length,
    configIndexes: frames.map((frame, index) => frame.config ? index : -1).filter((index) => index >= 0),
    keyFrameIndexes: frames.map((frame, index) => frame.keyFrame ? index : -1).filter((index) => index >= 0),
    first: frames[0] ? frameDebugSummary(frames[0]) : null,
    last: frames[frames.length - 1] ? frameDebugSummary(frames[frames.length - 1]) : null,
  };
}

function nalDebugSummary(data: Uint8Array) {
  return findNalUnits(data)
    .slice(0, 8)
    .map((unit) => ({
      type: unit.length > 0 ? unit[0] & 0x1f : -1,
      bytes: unit.length,
    }));
}

function canvasDebugInfo(tab: MirrorTab) {
  const canvas = tab.canvas;
  if (!canvas) {
    return { hasCanvas: false };
  }
  const rect = canvas.getBoundingClientRect();
  const style = window.getComputedStyle(canvas);
  return {
    hasCanvas: true,
    width: canvas.width,
    height: canvas.height,
    clientWidth: canvas.clientWidth,
    clientHeight: canvas.clientHeight,
    rectWidth: Math.round(rect.width),
    rectHeight: Math.round(rect.height),
    display: style.display,
    visibility: style.visibility,
  };
}

function setMirrorCanvas(tab: MirrorTab, element: Element | null) {
  const nextCanvas = element instanceof HTMLCanvasElement ? element : null;
  if (tab.canvas === nextCanvas) {
    return;
  }
  tab.canvas = nextCanvas;
  if (nextCanvas) {
    tab.waitingForCanvas = false;
  }
  mirrorDebug("canvas-ref", {
    tabId: tab.id,
    state: tab.state,
    activeWorkspaceTabKey: activeWorkspaceTabKey.value,
    canvas: canvasDebugInfo(tab),
  });
  if (nextCanvas && tab.session && tab.state === "running") {
    const sessionId = tab.session.sessionId;
    void nextTick(() => {
      if (tab.canvas === nextCanvas && tab.session?.sessionId === sessionId && tab.state === "running") {
        flushPendingScrcpySession(tab);
      }
    });
  }
}

function isMirrorCanvasReady(tab: MirrorTab) {
  return Boolean(tab.canvas?.isConnected);
}

async function waitForMirrorCanvas(tab: MirrorTab, startToken: number) {
  for (let attempt = 0; attempt < 4; attempt++) {
    await nextTick();
    if (isMirrorCanvasReady(tab)) {
      return true;
    }
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    if (isMirrorCanvasReady(tab)) {
      return true;
    }
    if (!isMirrorStartAlive(tab, startToken)) {
      return false;
    }
  }
  return false;
}

function mirrorTabKey(tabId: number) {
  return `mirror:${tabId}`;
}

function mirrorTabStatus(tab: MirrorTab) {
  if (tab.state === "running") {
    return "运行中";
  }
  if (tab.state === "starting") {
    return "";
  }
  if (tab.state === "stopping") {
    return "";
  }
  if (tab.state === "error") {
    return "异常";
  }
  return "待启动";
}

function findMirrorTabBySessionId(sessionId: number) {
  return mirrorTabs.value.find((tab) => tab.session?.sessionId === sessionId) || null;
}

function findLastPendingFrameIndex(
  frames: ScrcpyFramePayload[],
  predicate: (frame: ScrcpyFramePayload) => boolean,
  beforeIndex = frames.length,
) {
  for (let index = Math.min(beforeIndex, frames.length) - 1; index >= 0; index--) {
    if (predicate(frames[index])) {
      return index;
    }
  }
  return -1;
}

function bufferPendingScrcpyFrame(payload: ScrcpyFramePayload, reason = "orphan") {
  const frames = pendingScrcpyFrames.get(payload.sessionId) || [];
  frames.push(payload);

  const beforeTrim = frames.length;
  const latestKeyFrameIndex = findLastPendingFrameIndex(frames, (frame) => frame.keyFrame);
  if (latestKeyFrameIndex > 0) {
    const configIndex = findLastPendingFrameIndex(frames, (frame) => frame.config, latestKeyFrameIndex);
    frames.splice(0, configIndex >= 0 ? configIndex : latestKeyFrameIndex);
  } else {
    const latestConfigIndex = findLastPendingFrameIndex(frames, (frame) => frame.config);
    if (latestConfigIndex > 0) {
      frames.splice(0, latestConfigIndex);
    }
  }

  if (frames.length !== beforeTrim) {
    mirrorDebug("pending-trim-decodable-start", {
      sessionId: payload.sessionId,
      before: beforeTrim,
      after: frames.length,
      latestFrame: frameDebugSummary(payload),
      pending: pendingFrameStats(frames),
    });
  }

  if (frames.length > MAX_PENDING_SCRCPY_FRAMES) {
    const firstDecodableFrame = frames.findIndex((frame) => frame.keyFrame || frame.config);
    if (firstDecodableFrame > 0) {
      frames.splice(0, firstDecodableFrame);
    }
    if (frames.length > MAX_PENDING_SCRCPY_FRAMES) {
      const keyFrameIndex = frames.findIndex((frame) => frame.keyFrame);
      const protectedPrefixLength = Math.max(1, keyFrameIndex + 1);
      frames.splice(
        protectedPrefixLength,
        frames.length - MAX_PENDING_SCRCPY_FRAMES,
      );
    }
    mirrorWarn("pending-trim-limit", {
      sessionId: payload.sessionId,
      maxFrames: MAX_PENDING_SCRCPY_FRAMES,
      pending: pendingFrameStats(frames),
    });
  }
  pendingScrcpyFrames.set(payload.sessionId, frames);
  if (frames.length === 1 || payload.config || payload.keyFrame) {
    mirrorDebug("frame-buffered", {
      sessionId: payload.sessionId,
      reason,
      frame: frameDebugSummary(payload),
      pending: pendingFrameStats(frames),
    });
  }
}

function pendingFramesForInitialDecode(frames: ScrcpyFramePayload[]) {
  const latestKeyFrameIndex = findLastPendingFrameIndex(frames, (frame) => frame.keyFrame);
  if (latestKeyFrameIndex < 0) {
    return frames;
  }

  const configIndex = findLastPendingFrameIndex(frames, (frame) => frame.config, latestKeyFrameIndex);
  const startIndex = configIndex >= 0 ? configIndex : latestKeyFrameIndex;
  const endIndex = Math.min(frames.length, latestKeyFrameIndex + 1 + MAX_INITIAL_SCRCPY_FLUSH_TAIL_FRAMES);
  return frames.slice(startIndex, endIndex);
}

function clearPendingScrcpySession(sessionId: number) {
  const pendingFrames = pendingScrcpyFrames.get(sessionId)?.length || 0;
  const hadStatus = pendingScrcpyStatuses.has(sessionId);
  if (pendingFrames || hadStatus) {
    mirrorDebug("pending-clear", { sessionId, pendingFrames, hadStatus });
  }
  pendingScrcpyFrames.delete(sessionId);
  pendingScrcpyStatuses.delete(sessionId);
}

function clearMirrorFirstFrameWatch(tab: MirrorTab) {
  const timer = mirrorFirstFrameTimers.get(tab.id);
  if (timer) {
    window.clearTimeout(timer);
    mirrorFirstFrameTimers.delete(tab.id);
    mirrorDebug("first-frame-watch-clear", {
      tabId: tab.id,
      sessionId: tab.session?.sessionId,
      decodedFrames: tab.decodedFrames,
      state: tab.state,
    });
  }
}

function isCurrentMirrorStart(tab: MirrorTab, token: number) {
  return tab.startToken === token && tab.state === "starting" && mirrorTabs.value.some((item) => item.id === tab.id);
}

function isMirrorStartAlive(tab: MirrorTab, token: number) {
  return tab.startToken === token && mirrorTabs.value.some((item) => item.id === tab.id);
}

async function stopLateMirrorSession(result: ScrcpyStartResult) {
  clearPendingScrcpySession(result.sessionId);
  mirrorWarn("late-session-stop", { sessionId: result.sessionId, serial: result.serial });
  try {
    await invoke("stop_scrcpy_mirror", { sessionId: result.sessionId });
  } catch (error) {
    console.warn("清理迟到的投屏会话失败", error);
  }
}

function cancelMirrorStart(tab: MirrorTab, message = "已取消启动") {
  mirrorDebug("start-cancel", { tabId: tab.id, token: tab.startToken, sessionId: tab.session?.sessionId, message });
  tab.startToken = nextMirrorStartToken++;
  clearMirrorFirstFrameWatch(tab);
  tab.autoRestarting = false;
  tab.session = null;
  tab.latestConfigPacket = null;
  tab.pendingConfigPacket = null;
  closeDecoder(tab);
  clearActiveTouches(tab);
  setState(tab, "idle", message);
}

function scrollWorkspaceTabIntoView(key = activeWorkspaceTabKey.value) {
  void nextTick(() => {
    const list = mirrorTabListRef.value;
    if (!list || !key) {
      return;
    }
    const tabElement = list.querySelector<HTMLElement>(`[data-tab-key="${key}"]`);
    tabElement?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });
}

function registerWorkspaceTab(key: string) {
  if (!workspaceTabOrder.value.includes(key)) {
    workspaceTabOrder.value = [...workspaceTabOrder.value, key];
  }
}

function unregisterWorkspaceTab(key: string) {
  workspaceTabOrder.value = workspaceTabOrder.value.filter((item) => item !== key);
}

function onMirrorTabListWheel(event: WheelEvent) {
  const list = mirrorTabListRef.value;
  if (!list || list.scrollWidth <= list.clientWidth) {
    return;
  }
  const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
  if (delta === 0) {
    return;
  }
  list.scrollLeft += delta;
  event.preventDefault();
}

function activateMirrorTab(tabId: number) {
  if (mirrorTabs.value.some((tab) => tab.id === tabId)) {
    activeMirrorTabId.value = tabId;
    activeWorkspaceTabKey.value = mirrorTabKey(tabId);
    syncMirrorToolbarState();
    scrollWorkspaceTabIntoView();
  }
}

function addMirrorTab() {
  const tab = createMirrorTab();
  mirrorTabs.value.push(tab);
  const reactiveTab = mirrorTabs.value[mirrorTabs.value.length - 1];
  registerWorkspaceTab(mirrorTabKey(reactiveTab.id));
  activateMirrorTab(reactiveTab.id);
  return reactiveTab;
}

function activateWorkspaceTab(tab: WorkspaceTab) {
  activeWorkspaceTabKey.value = tab.key;
  scrollWorkspaceTabIntoView(tab.key);
  if (tab.kind === "mirror") {
    activeMirrorTabId.value = tab.mirror.id;
    syncMirrorToolbarState();
    return;
  }
  if (tab.kind === "screenshot") {
    void nextTick(() => {
      renderImageEditor(tab.editor);
    });
    return;
  }
  if (tab.kind === "textPreview") {
    void nextTick(() => {
      const element = textPreviewBodyRef.value;
      if (element) {
        textPreviewViewportHeight.value = element.clientHeight;
      }
    });
    return;
  }
  if (tab.kind === "terminal") {
    void nextTick(() => {
      createTerminalView();
      scheduleTerminalResize();
      focusTerminalView();
    });
  }
}

function activateAdjacentWorkspaceTabAfterClose(closedKey: string, previousOrder: string[]) {
  const closedIndex = previousOrder.indexOf(closedKey);
  const orderedTabs = workspaceTabs.value;
  const activateByKey = (key: string) => {
    const tab = orderedTabs.find((item) => item.key === key);
    if (!tab) {
      return false;
    }
    activateWorkspaceTab(tab);
    return true;
  };

  if (closedIndex >= 0) {
    for (let index = closedIndex - 1; index >= 0; index--) {
      if (activateByKey(previousOrder[index])) {
        return;
      }
    }
    for (let index = closedIndex + 1; index < previousOrder.length; index++) {
      if (activateByKey(previousOrder[index])) {
        return;
      }
    }
  }

  activeWorkspaceTabKey.value = "";
}

async function closeWorkspaceTab(tab: WorkspaceTab) {
  if (tab.kind === "mirror") {
    await closeMirrorTab(tab.mirror);
    return;
  }
  if (tab.kind === "transfer") {
    closeTransferPanel();
    return;
  }
  if (tab.kind === "screenshot") {
    closeImageEditorTab(tab.editor);
    return;
  }
  if (tab.kind === "textPreview") {
    closeTextPreview();
    return;
  }
  if (tab.kind === "settings") {
    closeSettingsPanel();
    return;
  }
  await closeTerminalPanel();
}

function closeWorkspaceTabByMiddleClick(tab: WorkspaceTab, event: MouseEvent) {
  if (event.button !== 1) {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  if (!tab.closable) {
    return;
  }
  void closeWorkspaceTab(tab);
}

function closeImageEditorTab(editor: ImageEditorTab) {
  if (editor.saving) {
    return;
  }
  imageEditorCanvases.delete(editor.key);
  const wasActive = activeWorkspaceTabKey.value === editor.key;
  const previousOrder = [...workspaceTabOrder.value];
  imageEditorTabs.value = imageEditorTabs.value.filter((item) => item.id !== editor.id);
  unregisterWorkspaceTab(editor.key);
  if (wasActive) {
    activateAdjacentWorkspaceTabAfterClose(editor.key, previousOrder);
  }
}

async function closeMirrorTab(tab: MirrorTab) {
  const closedKey = mirrorTabKey(tab.id);
  const wasWorkspaceActive = activeWorkspaceTabKey.value === closedKey;
  const wasMirrorActive = activeMirrorTabId.value === tab.id;
  const previousOrder = [...workspaceTabOrder.value];
  await stopMirror(tab);
  closeDecoder(tab);
  const index = mirrorTabs.value.findIndex((item) => item.id === tab.id);
  mirrorTabs.value = mirrorTabs.value.filter((item) => item.id !== tab.id);
  unregisterWorkspaceTab(closedKey);

  const fallback = mirrorTabs.value[index] || mirrorTabs.value[index - 1] || mirrorTabs.value[0] || null;
  if (wasMirrorActive) {
    activeMirrorTabId.value = fallback?.id || 0;
  }
  if (wasWorkspaceActive) {
    activateAdjacentWorkspaceTabAfterClose(closedKey, previousOrder);
  }
}

function closeDecoder(tab: MirrorTab) {
  if (tab.decoder) {
    try {
      tab.decoder.close();
    } catch {
      // Decoder may already be closed by the browser.
    }
  }
  tab.decoder = null;
  tab.decoderReady = false;
  tab.waitingForCanvas = false;
}

function resetDecoder(tab: MirrorTab, message: string) {
  mirrorWarn("decoder-reset", {
    tabId: tab.id,
    sessionId: tab.session?.sessionId,
    token: tab.startToken,
    message,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
  });
  clearMirrorFirstFrameWatch(tab);
  closeDecoder(tab);
  setState(tab, "error", message);
}

function ensureDecoder(tab: MirrorTab) {
  if (tab.decoder) {
    return true;
  }

  const DecoderCtor = (window as any).VideoDecoder;
  if (!DecoderCtor) {
    mirrorWarn("decoder-unavailable", { tabId: tab.id, sessionId: tab.session?.sessionId });
    setState(tab, "error", "当前 WebView 不支持 WebCodecs，无法解码 scrcpy 视频流");
    return false;
  }
  if (!tab.session) {
    return false;
  }
  if (!tab.canvas) {
    if (!tab.waitingForCanvas) {
      tab.waitingForCanvas = true;
      mirrorWarn("decoder-wait-canvas", {
        tabId: tab.id,
        sessionId: tab.session.sessionId,
        token: tab.startToken,
        state: tab.state,
      });
    }
    return false;
  }

  if (tab.latestConfigPacket) {
    tab.configuredCodec = codecFromSps(tab.latestConfigPacket);
  }
  const decoderSessionId = tab.session.sessionId;
  const decoderStartToken = tab.startToken;
  mirrorDebug("decoder-configure", {
    tabId: tab.id,
    sessionId: decoderSessionId,
    token: decoderStartToken,
    codec: tab.configuredCodec,
    width: tab.session.width,
    height: tab.session.height,
    hasConfig: Boolean(tab.latestConfigPacket),
    canvas: canvasDebugInfo(tab),
  });
  tab.decoder = markRaw(new DecoderCtor({
    output: (frame: any) => {
      if (tab.session?.sessionId !== decoderSessionId || tab.startToken !== decoderStartToken) {
        mirrorWarn("decoder-output-stale", {
          tabId: tab.id,
          decoderSessionId,
          currentSessionId: tab.session?.sessionId,
          decoderStartToken,
          currentStartToken: tab.startToken,
        });
        frame.close();
        return;
      }
      const canvas = tab.canvas;
      if (!canvas) {
        mirrorWarn("decoder-output-no-canvas", {
          tabId: tab.id,
          sessionId: decoderSessionId,
          token: decoderStartToken,
          state: tab.state,
        });
        frame.close();
        return;
      }

      if (canvas.width !== frame.displayWidth || canvas.height !== frame.displayHeight) {
        canvas.width = frame.displayWidth;
        canvas.height = frame.displayHeight;
      }

      const context = canvas.getContext("2d");
      if (!context) {
        mirrorWarn("decoder-output-no-context", { tabId: tab.id, sessionId: decoderSessionId });
        frame.close();
        return;
      }
      context.drawImage(frame, 0, 0, canvas.width, canvas.height);
      frame.close();
      tab.decodedFrames++;
      if (tab.decodedFrames === 1) {
        tab.decoderReady = true;
        syncMirrorToolbarState();
        mirrorDebug("first-frame-decoded", {
          tabId: tab.id,
          sessionId: decoderSessionId,
          token: decoderStartToken,
          width: canvas.width,
          height: canvas.height,
          canvas: canvasDebugInfo(tab),
          droppedFrames: tab.droppedFrames,
        });
        clearMirrorFirstFrameWatch(tab);
      }
    },
    error: (error: Error) => {
      if (tab.session?.sessionId !== decoderSessionId || tab.startToken !== decoderStartToken) {
        mirrorWarn("decoder-error-stale", {
          tabId: tab.id,
          decoderSessionId,
          currentSessionId: tab.session?.sessionId,
          message: error.message,
        });
        return;
      }
      resetDecoder(tab, `视频解码失败: ${error.message}`);
    },
  }));

  try {
    tab.decoder.configure({
      codec: tab.configuredCodec,
      codedWidth: tab.session.width,
      codedHeight: tab.session.height,
      optimizeForLatency: true,
      avc: { format: "annexb" },
    });
  } catch (error) {
    resetDecoder(tab, `视频解码器初始化失败: ${error instanceof Error ? error.message : String(error)}`);
    return false;
  }
  tab.decoderReady = false;
  return true;
}

function handleFrame(payload: ScrcpyFramePayload) {
  const tab = findMirrorTabBySessionId(payload.sessionId);
  if (!tab) {
    mirrorWarn("frame-orphan-buffer", {
      sessionId: payload.sessionId,
      frame: frameDebugSummary(payload),
    });
    bufferPendingScrcpyFrame(payload);
    return;
  }

  const bytes = base64ToBytes(payload.data);

  if (payload.config) {
    tab.latestConfigPacket = bytes;
    tab.pendingConfigPacket = bytes;
    tab.configuredCodec = codecFromSps(bytes);
    mirrorDebug("frame-config", {
      tabId: tab.id,
      sessionId: payload.sessionId,
      codec: tab.configuredCodec,
      bytes: bytes.length,
      nalUnits: nalDebugSummary(bytes),
      droppedFrames: tab.droppedFrames,
    });
    return;
  }

  if (payload.keyFrame && hasSps(bytes)) {
    tab.configuredCodec = codecFromSps(bytes);
    mirrorDebug("frame-key-sps", {
      tabId: tab.id,
      sessionId: payload.sessionId,
      codec: tab.configuredCodec,
      bytes: bytes.length,
      nalUnits: nalDebugSummary(bytes),
      decoderReady: tab.decoderReady,
    });
  }

  if (!tab.decoder && !tab.latestConfigPacket && !hasSps(bytes)) {
    tab.droppedFrames++;
    if (tab.droppedFrames <= 3 || tab.droppedFrames % 30 === 0) {
      mirrorWarn("frame-drop-no-config", {
        tabId: tab.id,
        sessionId: payload.sessionId,
        droppedFrames: tab.droppedFrames,
        keyFrame: payload.keyFrame,
        bytes: bytes.length,
      });
    }
    return;
  }

  if (!tab.canvas) {
    bufferPendingScrcpyFrame(payload, "canvas-not-ready");
    if (!tab.waitingForCanvas) {
      tab.waitingForCanvas = true;
      mirrorWarn("frame-wait-canvas", {
        tabId: tab.id,
        sessionId: payload.sessionId,
        keyFrame: payload.keyFrame,
        decoderReady: tab.decoderReady,
        hasDecoder: Boolean(tab.decoder),
      });
    }
    return;
  }

  if (!payload.keyFrame && !tab.decoder) {
    tab.droppedFrames++;
    if (tab.droppedFrames <= 3 || tab.droppedFrames % 30 === 0) {
      mirrorWarn("frame-drop-wait-keyframe", {
        tabId: tab.id,
        sessionId: payload.sessionId,
        droppedFrames: tab.droppedFrames,
        bytes: bytes.length,
      });
    }
    return;
  }

  if (!ensureDecoder(tab)) {
    return;
  }

  const chunkData = mediaPacketForWebCodecs(tab, bytes, payload.keyFrame);
  const ChunkCtor = (window as any).EncodedVideoChunk;
  try {
    if (payload.keyFrame || tab.decodedFrames === 0) {
      mirrorDebug("decode-submit", {
        tabId: tab.id,
        sessionId: payload.sessionId,
        keyFrame: payload.keyFrame,
        payloadBytes: bytes.length,
        chunkBytes: chunkData.length,
        payloadNalUnits: nalDebugSummary(bytes),
        chunkNalUnits: nalDebugSummary(chunkData),
        hasConfig: Boolean(tab.latestConfigPacket),
        hasPendingConfig: Boolean(tab.pendingConfigPacket),
        canvas: canvasDebugInfo(tab),
        decoderState: tab.decoder?.state,
        decodedFrames: tab.decodedFrames,
        droppedFrames: tab.droppedFrames,
      });
    }
    tab.decoder.decode(
      new ChunkCtor({
        type: payload.keyFrame ? "key" : "delta",
        timestamp: Number(payload.pts || tab.decodedFrames * 16_667),
        data: chunkData,
      }),
    );
  } catch (error) {
    resetDecoder(tab, `视频解码失败: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function handleStatus(payload: ScrcpyStatusPayload) {
  const tab = findMirrorTabBySessionId(payload.sessionId);
  if (!tab) {
    if (payload.status === "running") {
      pendingScrcpyStatuses.set(payload.sessionId, payload);
      mirrorDebug("status-buffered", payload);
    } else {
      mirrorWarn("status-orphan-terminal", payload);
      clearPendingScrcpySession(payload.sessionId);
    }
    return;
  }
  mirrorDebug("status", {
    tabId: tab.id,
    sessionId: payload.sessionId,
    status: payload.status,
    message: payload.message,
    previousState: tab.state,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
  });
  tab.statusText = payload.message;
  if (payload.status === "running") {
    if (tab.state !== "stopping") {
      tab.autoRestarting = false;
      setState(tab, "running", payload.message || "视频通道已连接");
    }
    return;
  }
  if (payload.status === "stopped") {
    clearMirrorFirstFrameWatch(tab);
    stopRecordingForMirror(tab);
    tab.autoRestarting = false;
    tab.session = null;
    tab.latestConfigPacket = null;
    tab.pendingConfigPacket = null;
    clearActiveTouches(tab);
    closeDecoder(tab);
    setState(tab, "idle", payload.message || "已停止");
    return;
  }
  if (payload.status === "error") {
    clearMirrorFirstFrameWatch(tab);
    stopRecordingForMirror(tab);
    tab.autoRestarting = false;
    tab.session = null;
    tab.latestConfigPacket = null;
    tab.pendingConfigPacket = null;
    clearActiveTouches(tab);
    closeDecoder(tab);
    setState(tab, "error", payload.message || "投屏连接失败");
  }
}

function flushPendingScrcpySession(tab: MirrorTab) {
  const sessionId = tab.session?.sessionId;
  if (!sessionId) {
    return;
  }
  const status = pendingScrcpyStatuses.get(sessionId);
  if (status) {
    pendingScrcpyStatuses.delete(sessionId);
    mirrorDebug("pending-status-flush", { tabId: tab.id, sessionId, status: status.status });
    handleStatus(status);
  }
  const pendingFrames = pendingScrcpyFrames.get(sessionId);
  if (!pendingFrames?.length) {
    return;
  }
  pendingScrcpyFrames.delete(sessionId);
  const frames = tab.decodedFrames === 0 && !tab.decoder
    ? pendingFramesForInitialDecode(pendingFrames)
    : pendingFrames;
  mirrorDebug("pending-frame-flush", {
    tabId: tab.id,
    sessionId,
    pending: pendingFrameStats(pendingFrames),
    flushing: pendingFrameStats(frames),
    trimmedForInitialDecode: frames.length !== pendingFrames.length,
    canvas: canvasDebugInfo(tab),
    state: tab.state,
  });
  for (const frame of frames) {
    handleFrame(frame);
  }
  mirrorDebug("pending-frame-flush-done", {
    tabId: tab.id,
    sessionId,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
    decoderReady: tab.decoderReady,
    canvas: canvasDebugInfo(tab),
  });
}

function scheduleMirrorAutoRestart(tab: MirrorTab, startToken: number, sessionId: number) {
  clearMirrorFirstFrameWatch(tab);
  mirrorDebug("auto-restart-watch-start", {
    tabId: tab.id,
    sessionId,
    token: startToken,
    timeoutMs: MIRROR_AUTO_RESTART_DELAY_MS,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
  });
  const timer = window.setTimeout(() => {
    mirrorFirstFrameTimers.delete(tab.id);
    if (
      tab.startToken !== startToken
      || tab.state !== "running"
      || tab.session?.sessionId !== sessionId
      || tab.startRecoveryCount >= 1
      || hasMirrorVideoActivity(tab, sessionId)
    ) {
      return;
    }
    void restartMirrorOnceAfterDelay(tab, startToken, sessionId);
  }, MIRROR_AUTO_RESTART_DELAY_MS);
  mirrorFirstFrameTimers.set(tab.id, timer);
}

function hasMirrorVideoActivity(tab: MirrorTab, sessionId: number) {
  // The first WebCodecs output can be slower than the first scrcpy packets on a cold WebView.
  // Do not restart a live stream just because the decoded frame has not surfaced yet.
  return Boolean(
    tab.decoder
      || tab.latestConfigPacket
      || tab.droppedFrames > 0
      || pendingScrcpyFrames.get(sessionId)?.length,
  );
}

async function restartMirrorOnceAfterDelay(tab: MirrorTab, startToken: number, sessionId: number) {
  mirrorWarn("auto-restart-trigger", {
    tabId: tab.id,
    sessionId,
    token: startToken,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
    recoveryCount: tab.startRecoveryCount,
    hasConfig: Boolean(tab.latestConfigPacket),
    decoderReady: tab.decoderReady,
  });
  tab.startRecoveryCount += 1;
  tab.autoRestarting = true;
  syncMirrorToolbarState();
  clearPendingScrcpySession(sessionId);
  tab.session = null;
  tab.latestConfigPacket = null;
  tab.pendingConfigPacket = null;
  closeDecoder(tab);
  clearActiveTouches(tab);
  setState(tab, "starting", "正在重建投屏连接...");

  try {
    await invoke("stop_scrcpy_mirror", { sessionId });
    mirrorDebug("auto-restart-stop-ok", { tabId: tab.id, sessionId });
  } catch (error) {
    mirrorWarn("auto-restart-stop-failed", { tabId: tab.id, sessionId, error: String(error) });
  }

  if (tab.startToken !== startToken || tab.state !== "starting") {
    tab.autoRestarting = false;
    syncMirrorToolbarState();
    return;
  }
  setState(tab, "idle", "准备重建投屏连接");
  mirrorDebug("auto-restart-start", { tabId: tab.id, previousSessionId: sessionId });
  await startMirror(tab, { keepRecoveryCount: true });
}

async function startMirror(tab = activeMirrorTab.value, options: { keepRecoveryCount?: boolean } = {}) {
  if (!tab) {
    return;
  }
  if (tab.state === "starting") {
    return;
  }
  if (!selectedSerial.value) {
    adbStore.setNotice("error", "启动投屏前请先选择设备");
    return;
  }

  const startToken = nextMirrorStartToken++;
  clearMirrorFirstFrameWatch(tab);
  closeDecoder(tab);
  tab.decodedFrames = 0;
  tab.droppedFrames = 0;
  tab.startToken = startToken;
  if (!options.keepRecoveryCount) {
    tab.startRecoveryCount = 0;
    tab.autoRestarting = false;
  }
  tab.session = null;
  tab.latestConfigPacket = null;
  tab.pendingConfigPacket = null;
  tab.serial = selectedSerial.value;
  tab.title = selectedSerial.value;
  setState(tab, "starting", "正在启动 scrcpy server...");
  await waitForMirrorCanvas(tab, startToken);
  if (!isCurrentMirrorStart(tab, startToken)) {
    mirrorWarn("start-aborted-before-render", { tabId: tab.id, token: startToken, state: tab.state });
    return;
  }
  await ensureMirrorEventListenersReady();
  if (!isCurrentMirrorStart(tab, startToken)) {
    mirrorWarn("start-aborted-before-invoke", { tabId: tab.id, token: startToken, state: tab.state });
    return;
  }
  mirrorDebug("start-request", {
    tabId: tab.id,
    token: startToken,
    serial: selectedSerial.value,
    bitRate: mirrorBitRateMbps.value * 1_000_000,
    maxFps: mirrorMaxFps.value,
    recoveryCount: tab.startRecoveryCount,
    keepRecoveryCount: Boolean(options.keepRecoveryCount),
  });

  const startTimer = window.setTimeout(() => {
    if (!isCurrentMirrorStart(tab, startToken) || tab.session) {
      return;
    }
    mirrorWarn("start-timeout", {
      tabId: tab.id,
      token: startToken,
      serial: selectedSerial.value,
      timeoutMs: MIRROR_START_TIMEOUT_MS,
    });
    tab.startToken = nextMirrorStartToken++;
    tab.autoRestarting = false;
    setState(tab, "error", "启动投屏超时，请检查设备授权、USB/Wi-Fi ADB 连接后重试");
    adbStore.setNotice("error", "启动投屏超时，请检查设备连接后重试");
  }, MIRROR_START_TIMEOUT_MS);

  try {
    const result = await invoke<ScrcpyStartResult>("start_scrcpy_mirror", {
      request: {
        serial: selectedSerial.value,
        videoBitRate: mirrorBitRateMbps.value * 1_000_000,
        maxFps: mirrorMaxFps.value,
      },
    });
    if (!isCurrentMirrorStart(tab, startToken)) {
      await stopLateMirrorSession(result);
      return;
    }
    mirrorDebug("start-result", {
      tabId: tab.id,
      token: startToken,
      sessionId: result.sessionId,
      serial: result.serial,
      deviceName: result.deviceName,
      codec: result.codec,
      width: result.width,
      height: result.height,
      version: result.version,
    });
    tab.session = result;
    tab.title = result.deviceName || result.serial;
    tab.configuredCodec = "avc1.42E01F";
    tab.autoRestarting = false;
    setState(tab, "running", "视频通道已连接");
    await waitForMirrorCanvas(tab, startToken);
    if (!isMirrorStartAlive(tab, startToken) || tab.session?.sessionId !== result.sessionId || tab.state !== "running") {
      await stopLateMirrorSession(result);
      return;
    }
    await invoke("attach_scrcpy_mirror", { sessionId: result.sessionId });
    flushPendingScrcpySession(tab);
    scheduleMirrorAutoRestart(tab, startToken, result.sessionId);
  } catch (error) {
    const canReportStartError = (["starting", "running"] as MirrorState[]).includes(tab.state);
    if (!isMirrorStartAlive(tab, startToken) || !canReportStartError) {
      return;
    }
    const sessionId = tab.session?.sessionId;
    if (sessionId) {
      clearPendingScrcpySession(sessionId);
      try {
        await invoke("stop_scrcpy_mirror", { sessionId });
      } catch {
        // Keep the original startup error visible to the user.
      }
    }
    tab.session = null;
    tab.latestConfigPacket = null;
    tab.pendingConfigPacket = null;
    tab.autoRestarting = false;
    closeDecoder(tab);
    mirrorWarn("start-error", { tabId: tab.id, token: startToken, error: String(error) });
    setState(tab, "error", String(error));
    adbStore.setNotice("error", String(error));
  } finally {
    window.clearTimeout(startTimer);
  }
}

async function stopMirror(tab = activeMirrorTab.value) {
  if (!tab) {
    return;
  }
  mirrorDebug("stop-request", {
    tabId: tab.id,
    state: tab.state,
    sessionId: tab.session?.sessionId,
    decodedFrames: tab.decodedFrames,
    droppedFrames: tab.droppedFrames,
  });
  clearMirrorFirstFrameWatch(tab);
  stopRecordingForMirror(tab);
  tab.autoRestarting = false;

  if (!tab.session) {
    cancelMirrorStart(tab, "等待启动");
    return;
  }

  const sessionId = tab.session.sessionId;
  setState(tab, "stopping", "正在停止投屏...");
  try {
    await invoke("stop_scrcpy_mirror", { sessionId });
    mirrorDebug("stop-result", { tabId: tab.id, sessionId });
  } catch (error) {
    mirrorWarn("stop-error", { tabId: tab.id, sessionId, error: String(error) });
    adbStore.setNotice("error", String(error));
  } finally {
    clearPendingScrcpySession(sessionId);
    tab.session = null;
    tab.latestConfigPacket = null;
    tab.pendingConfigPacket = null;
    tab.autoRestarting = false;
    closeDecoder(tab);
    clearActiveTouches(tab);
    setState(tab, "idle", "已停止");
  }
}

async function toggleMirror() {
  mirrorDebug("toggle", {
    isActiveWorkspaceMirror: isActiveWorkspaceMirror.value,
    activeTabId: activeMirrorTab.value?.id,
    activeState: activeMirrorTab.value?.state,
    existingActiveTabId: existingActiveMirrorTab.value?.id,
    existingActiveState: existingActiveMirrorTab.value?.state,
  });
  if (!isActiveWorkspaceMirror.value) {
    const runningTab = existingActiveMirrorTab.value;
    if (runningTab) {
      activateMirrorTab(runningTab.id);
      return;
    }

    const idleTab = mirrorTabs.value.find((item) => item.state === "idle") || addMirrorTab();
    activateMirrorTab(idleTab.id);
    await startMirror(idleTab);
    return;
  }

  const tab = activeMirrorTab.value;
  if (!tab) {
    const newTab = addMirrorTab();
    await startMirror(newTab);
    return;
  }
  if (tab.state === "starting") {
    cancelMirrorStart(tab);
    return;
  }
  if (tab.state === "running" || tab.state === "error") {
    await stopMirror(tab);
    return;
  }

  if (tab.state === "idle") {
    await startMirror(tab);
  }
}

function canvasPointFromClient(tab: MirrorTab, clientX: number, clientY: number) {
  const canvas = tab.canvas;
  if (!canvas || !tab.session) {
    return null;
  }

  const rect = canvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) {
    return null;
  }

  return {
    x: Math.round(((clientX - rect.left) / rect.width) * tab.session.width),
    y: Math.round(((clientY - rect.top) / rect.height) * tab.session.height),
  };
}

function canvasPoint(tab: MirrorTab, event: PointerEvent | WheelEvent) {
  return canvasPointFromClient(tab, event.clientX, event.clientY);
}

function clampTouchPoint(tab: MirrorTab, point: TouchPoint): TouchPoint {
  const width = tab.session?.width || 0;
  const height = tab.session?.height || 0;
  return {
    x: Math.max(0, Math.min(width, Math.round(point.x))),
    y: Math.max(0, Math.min(height, Math.round(point.y))),
  };
}

async function sendTouchPoint(tab: MirrorTab, action: "down" | "move" | "up", pointerId: number, point: TouchPoint) {
  if (!tab.session || tab.state !== "running") {
    return;
  }
  const clampedPoint = clampTouchPoint(tab, point);

  await invoke("scrcpy_touch", {
    request: {
      sessionId: tab.session.sessionId,
      action,
      pointerId,
      x: clampedPoint.x,
      y: clampedPoint.y,
      videoWidth: Math.min(tab.session.width, 65535),
      videoHeight: Math.min(tab.session.height, 65535),
    },
  });
}

async function sendTouch(tab: MirrorTab, action: "down" | "move" | "up", sourcePointerId: number, event: PointerEvent) {
  const point = canvasPoint(tab, event);
  const pointerId = tab.activePointers.get(sourcePointerId);
  if (!point || pointerId === undefined) {
    return;
  }
  await sendTouchPoint(tab, action, pointerId, point);
}

function allocateTouchPointerId(tab: MirrorTab, sourcePointerId: number) {
  const existing = tab.activePointers.get(sourcePointerId);
  if (existing !== undefined) {
    return existing;
  }
  const usedPointerIds = new Set(tab.activePointers.values());
  for (let pointerId = 0; pointerId < 32; pointerId += 1) {
    if (!usedPointerIds.has(pointerId)) {
      tab.activePointers.set(sourcePointerId, pointerId);
      return pointerId;
    }
  }
  const fallbackPointerId = tab.activePointers.size + 32;
  tab.activePointers.set(sourcePointerId, fallbackPointerId);
  return fallbackPointerId;
}

function syntheticTwoFingerPoints(tab: MirrorTab, center: TouchPoint, distance: number) {
  const halfDistance = distance / 2;
  return [
    clampTouchPoint(tab, { x: center.x - halfDistance, y: center.y }),
    clampTouchPoint(tab, { x: center.x + halfDistance, y: center.y }),
  ] as const;
}

function syntheticPinchPoints(tab: MirrorTab) {
  const gesture = tab.syntheticPinch;
  return syntheticTwoFingerPoints(tab, gesture.center, gesture.distance);
}

function syntheticPanPoints(tab: MirrorTab) {
  const gesture = tab.syntheticPan;
  return syntheticTwoFingerPoints(tab, gesture.center, gesture.distance);
}

function clearSyntheticPinchTimer(tab: MirrorTab) {
  if (tab.syntheticPinch.finishTimer !== null) {
    window.clearTimeout(tab.syntheticPinch.finishTimer);
    tab.syntheticPinch.finishTimer = null;
  }
}

async function finishSyntheticPinch(tab: MirrorTab) {
  if (!tab.syntheticPinch.active) {
    return;
  }
  clearSyntheticPinchTimer(tab);
  const [firstPoint, secondPoint] = syntheticPinchPoints(tab);
  tab.syntheticPinch.active = false;
  await sendTouchPoint(tab, "up", SYNTHETIC_PINCH_POINTER_B, secondPoint);
  await sendTouchPoint(tab, "up", SYNTHETIC_PINCH_POINTER_A, firstPoint);
}

async function finishSyntheticPan(tab: MirrorTab) {
  if (!tab.syntheticPan.active) {
    return;
  }
  const [firstPoint, secondPoint] = syntheticPanPoints(tab);
  tab.syntheticPan.active = false;
  tab.syntheticPan.sourcePointerId = null;
  await sendTouchPoint(tab, "up", SYNTHETIC_PAN_POINTER_B, secondPoint);
  await sendTouchPoint(tab, "up", SYNTHETIC_PAN_POINTER_A, firstPoint);
}

function scheduleSyntheticPinchFinish(tab: MirrorTab) {
  clearSyntheticPinchTimer(tab);
  tab.syntheticPinch.finishTimer = window.setTimeout(() => {
    void finishSyntheticPinch(tab);
  }, SYNTHETIC_PINCH_END_DELAY_MS);
}

function clearActiveTouches(tab: MirrorTab) {
  tab.activePointers.clear();
  tab.lastMoveAt.clear();
  void finishSyntheticPinch(tab);
  void finishSyntheticPan(tab);
}

function syntheticGestureDistance(tab: MirrorTab) {
  if (!tab.session) {
    return 0;
  }
  return Math.min(tab.session.width, tab.session.height) * 0.22;
}

async function startSyntheticPan(tab: MirrorTab, event: PointerEvent) {
  const center = canvasPoint(tab, event);
  if (!center || !tab.session || tab.state !== "running") {
    return;
  }
  if (tab.syntheticPinch.active) {
    await finishSyntheticPinch(tab);
  }
  tab.syntheticPan.active = true;
  tab.syntheticPan.sourcePointerId = event.pointerId;
  tab.syntheticPan.center = clampTouchPoint(tab, center);
  tab.syntheticPan.distance = syntheticGestureDistance(tab);
  const [firstPoint, secondPoint] = syntheticPanPoints(tab);
  await sendTouchPoint(tab, "down", SYNTHETIC_PAN_POINTER_A, firstPoint);
  await sendTouchPoint(tab, "down", SYNTHETIC_PAN_POINTER_B, secondPoint);
}

function moveSyntheticPan(tab: MirrorTab, event: PointerEvent) {
  if (!tab.syntheticPan.active || tab.syntheticPan.sourcePointerId !== event.pointerId) {
    return false;
  }
  const center = canvasPoint(tab, event);
  if (!center) {
    return true;
  }
  const now = performance.now();
  if (now - (tab.lastMoveAt.get(event.pointerId) || 0) < 12) {
    return true;
  }
  tab.lastMoveAt.set(event.pointerId, now);
  tab.syntheticPan.center = clampTouchPoint(tab, center);
  const [firstPoint, secondPoint] = syntheticPanPoints(tab);
  void sendTouchPoint(tab, "move", SYNTHETIC_PAN_POINTER_A, firstPoint);
  void sendTouchPoint(tab, "move", SYNTHETIC_PAN_POINTER_B, secondPoint);
  return true;
}

function onPointerDown(tab: MirrorTab, event: PointerEvent) {
  if (event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  event.preventDefault();
  activeMirrorTabId.value = tab.id;
  tab.canvas?.setPointerCapture(event.pointerId);
  if (event.pointerType === "mouse" && event.altKey) {
    void startSyntheticPan(tab, event);
    return;
  }
  if (tab.syntheticPinch.active) {
    void finishSyntheticPinch(tab);
  }
  allocateTouchPointerId(tab, event.pointerId);
  void sendTouch(tab, "down", event.pointerId, event);
}

function onPointerMove(tab: MirrorTab, event: PointerEvent) {
  if (moveSyntheticPan(tab, event)) {
    event.preventDefault();
    return;
  }
  if (!tab.activePointers.has(event.pointerId)) {
    return;
  }
  const now = performance.now();
  if (now - (tab.lastMoveAt.get(event.pointerId) || 0) < 12) {
    return;
  }
  tab.lastMoveAt.set(event.pointerId, now);
  event.preventDefault();
  void sendTouch(tab, "move", event.pointerId, event);
}

function onPointerUp(tab: MirrorTab, event: PointerEvent) {
  if (tab.syntheticPan.active && tab.syntheticPan.sourcePointerId === event.pointerId) {
    event.preventDefault();
    tab.canvas?.releasePointerCapture(event.pointerId);
    void finishSyntheticPan(tab).finally(() => {
      tab.lastMoveAt.delete(event.pointerId);
    });
    return;
  }
  if (!tab.activePointers.has(event.pointerId)) {
    return;
  }
  event.preventDefault();
  tab.canvas?.releasePointerCapture(event.pointerId);
  void sendTouch(tab, "up", event.pointerId, event).finally(() => {
    tab.activePointers.delete(event.pointerId);
    tab.lastMoveAt.delete(event.pointerId);
  });
}

function onMirrorWheel(tab: MirrorTab, event: WheelEvent) {
  if (!event.ctrlKey || !tab.session || tab.state !== "running" || tab.activePointers.size > 0) {
    return;
  }

  const center = canvasPoint(tab, event);
  if (!center) {
    return;
  }

  event.preventDefault();
  activeMirrorTabId.value = tab.id;

  const minSize = Math.min(tab.session.width, tab.session.height);
  const minDistance = Math.max(24, minSize * 0.05);
  const maxDistance = Math.max(minDistance, minSize * 0.85);
  const wasActive = tab.syntheticPinch.active;
  tab.syntheticPinch.center = clampTouchPoint(tab, center);
  tab.syntheticPinch.distance = wasActive ? tab.syntheticPinch.distance : minSize * 0.22;
  tab.syntheticPinch.distance = Math.max(
    minDistance,
    Math.min(maxDistance, tab.syntheticPinch.distance - event.deltaY * 0.45),
  );
  tab.syntheticPinch.active = true;

  const [firstPoint, secondPoint] = syntheticPinchPoints(tab);
  const sendGesture = async () => {
    if (!wasActive) {
      await sendTouchPoint(tab, "down", SYNTHETIC_PINCH_POINTER_A, firstPoint);
      await sendTouchPoint(tab, "down", SYNTHETIC_PINCH_POINTER_B, secondPoint);
    }
    await sendTouchPoint(tab, "move", SYNTHETIC_PINCH_POINTER_A, firstPoint);
    await sendTouchPoint(tab, "move", SYNTHETIC_PINCH_POINTER_B, secondPoint);
    scheduleSyntheticPinchFinish(tab);
  };
  void sendGesture();
}

async function sendKey(keycode: number) {
  const tab = activeMirrorTab.value;
  if (!tab?.session || tab.state !== "running" || tab.autoRestarting) {
    return;
  }
  try {
    await invoke("scrcpy_keyevent", {
      sessionId: tab.session.sessionId,
      keycode,
    });
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

async function sendText(text: string) {
  const tab = activeMirrorTab.value;
  if (!tab?.session || tab.state !== "running" || tab.autoRestarting || !text) {
    return;
  }
  try {
    await invoke("scrcpy_text", {
      sessionId: tab.session.sessionId,
      text,
    });
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

async function pickLocalBrowserDirectory() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "选择系统目录",
  });

  if (typeof selected === "string") {
    await loadLocalEntries(selected);
  }
}

function openTransferPanel() {
  const wasOpen = transferPanelOpen.value;
  transferPanelOpen.value = true;
  if (!wasOpen) {
    registerWorkspaceTab("transfer");
  }
  activeWorkspaceTabKey.value = "transfer";
  scrollWorkspaceTabIntoView("transfer");
  void ensureLocalBrowserPath();
  void loadRemoteEntries();
}

function openTerminalPanel() {
  const wasOpen = terminalPanelOpen.value;
  terminalPanelOpen.value = true;
  if (!wasOpen) {
    registerWorkspaceTab("terminal");
  }
  activeWorkspaceTabKey.value = "terminal";
  scrollWorkspaceTabIntoView("terminal");
  void nextTick(() => {
    void ensureTerminalStarted();
    scheduleTerminalResize();
  });
}

function openSettingsPanel() {
  const wasOpen = settingsPanelOpen.value;
  settingsPanelOpen.value = true;
  if (!wasOpen) {
    registerWorkspaceTab("settings");
  }
  activeWorkspaceTabKey.value = "settings";
  scrollWorkspaceTabIntoView("settings");
}

function handleMirrorToolbarAction(event: Event) {
  const action = (event as CustomEvent<MirrorToolbarAction>).detail;
  switch (action) {
    case "toggleMirror":
      void toggleMirror();
      break;
    case "back":
      void sendKey(4);
      break;
    case "home":
      void sendKey(3);
      break;
    case "tasks":
      void sendKey(187);
      break;
    case "power":
      void sendKey(26);
      break;
    case "screenshot":
      void captureScreenshot();
      break;
    case "record":
      if (activeMirrorRecording.value) {
        void stopRecording();
      } else {
        void startRecording();
      }
      break;
    case "transfer":
      openTransferPanel();
      break;
    case "terminal":
      openTerminalPanel();
      break;
    case "settings":
      openSettingsPanel();
      break;
  }
}

function closeTransferPanel() {
  if (transferRunning.value) {
    return;
  }
  const wasActive = activeWorkspaceTabKey.value === "transfer";
  const previousOrder = [...workspaceTabOrder.value];
  transferPanelOpen.value = false;
  unregisterWorkspaceTab("transfer");
  if (wasActive) {
    activateAdjacentWorkspaceTabAfterClose("transfer", previousOrder);
  }
}

function selectQuickCommand(item: QuickCommandItem) {
  activeTerminalCommand.value = item.command;
  void writeTerminalData(item.command);
}

function toggleCommandPickerCollapsed() {
  commandPickerCollapsed.value = !commandPickerCollapsed.value;
  void nextTick(() => {
    scheduleTerminalResize();
  });
}

async function closeTerminalPanel() {
  const wasActive = activeWorkspaceTabKey.value === "terminal";
  const previousOrder = [...workspaceTabOrder.value];
  terminalPanelOpen.value = false;
  unregisterWorkspaceTab("terminal");
  await closeTerminalSession();
  disposeTerminalView();
  if (wasActive) {
    activateAdjacentWorkspaceTabAfterClose("terminal", previousOrder);
  }
}

function closeSettingsPanel() {
  const wasActive = activeWorkspaceTabKey.value === "settings";
  const previousOrder = [...workspaceTabOrder.value];
  settingsPanelOpen.value = false;
  unregisterWorkspaceTab("settings");
  if (wasActive) {
    activateAdjacentWorkspaceTabAfterClose("settings", previousOrder);
  }
}

async function ensureLocalBrowserPath() {
  if (localBrowserPath.value.trim()) {
    await loadLocalEntries(localBrowserPath.value);
    return;
  }

  const preferred = defaultLocalDir.value.trim();
  if (preferred) {
    await loadLocalEntries(preferred);
    return;
  }

  try {
    const dir = await downloadDir();
    if (dir && dir.trim()) {
      adbStore.setDefaultLocalDir(dir);
      await loadLocalEntries(dir);
      return;
    }
  } catch {
    // Try the user home directory next.
  }

  try {
    const dir = await homeDir();
    if (dir && dir.trim()) {
      await loadLocalEntries(dir);
      return;
    }
  } catch {
    // Fall back to backend default home directory.
  }

  await loadLocalEntries();
}

async function loadLocalEntries(path?: string) {
  closeTransferContextMenu();
  browsingLocal.value = true;
  localBrowseError.value = "";

  try {
    const normalized = path?.trim() || localBrowserPath.value.trim();
    localEntries.value = await invoke<LocalEntry[]>("list_local_entries", {
      path: normalized || null,
    });
    if (normalized) {
      localBrowserPath.value = normalized;
    } else if (localEntries.value[0]) {
      const first = localEntries.value[0].path.replace(/[\\/][^\\/]+$/, "");
      localBrowserPath.value = first || localBrowserPath.value;
    }
    selectedLocalPath.value = "";
  } catch (error) {
    localEntries.value = [];
    localBrowseError.value = String(error);
  } finally {
    browsingLocal.value = false;
  }
}

function goLocalParentDirectory() {
  const parent = parentLocalPath(localBrowserPath.value);
  if (!parent) {
    return;
  }

  void loadLocalEntries(parent);
}

function isWindowsDrivePath(path: string) {
  return /^[A-Za-z]:[\\/]?$/.test(path.trim());
}

function parentLocalPath(path: string) {
  const current = path.trim().replace(/[\\/]+$/, "");
  if (!current) {
    return "";
  }
  if (current === localDrivesRoot) {
    return "";
  }
  if (/^[A-Za-z]:$/.test(current)) {
    return localDrivesRoot;
  }

  const idx = Math.max(current.lastIndexOf("/"), current.lastIndexOf("\\"));
  if (idx <= 0) {
    return "";
  }

  if (/^[A-Za-z]:$/.test(current.slice(0, idx))) {
    return `${current.slice(0, idx)}\\`;
  }

  return current.slice(0, idx);
}

function selectLocalEntry(entry: LocalEntry) {
  selectedLocalPath.value = entry.path;
  if (!isWindowsDrivePath(entry.path)) {
    adbStore.setUploadLocalPath(entry.path);
  }
}

function closeTransferContextMenu(event?: Event) {
  if (transferContextMenuOpenedAt && Date.now() - transferContextMenuOpenedAt < 180) {
    return;
  }
  if (event instanceof MouseEvent && event.button !== 0) {
    return;
  }
  transferContextMenu.value = null;
}

function transferContextMenuPosition(x: number, y: number, itemCount: number) {
  const menuWidth = 188;
  const menuHeight = 14 + itemCount * 34;
  return {
    x: Math.min(x, Math.max(8, window.innerWidth - menuWidth - 8)),
    y: Math.min(y, Math.max(8, window.innerHeight - menuHeight - 8)),
  };
}

function transferContextMenuAnchor(event: MouseEvent | KeyboardEvent) {
  if (event instanceof MouseEvent) {
    return { x: event.clientX, y: event.clientY };
  }

  const target = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  const rect = target?.getBoundingClientRect();
  return {
    x: rect ? rect.right : Math.floor(window.innerWidth / 2),
    y: rect ? rect.top : Math.floor(window.innerHeight / 2),
  };
}

function openTransferContextMenu(event: MouseEvent | KeyboardEvent, scope: TransferEntryScope, entry: TransferEntry) {
  event.preventDefault();
  event.stopPropagation();

  if (scope === "local") {
    selectLocalEntry(entry as LocalEntry);
  } else {
    selectRemoteEntry(entry as RemoteEntry);
  }

  const itemCount = scope === "local" ? 5 : 4;
  const anchor = transferContextMenuAnchor(event);
  const position = transferContextMenuPosition(anchor.x, anchor.y, itemCount);
  transferContextMenuOpenedAt = Date.now();
  transferContextMenu.value = {
    scope,
    entry,
    ...position,
  };
}

function transferEntryFromEvent(event: Event, scope: TransferEntryScope) {
  const target = event.target instanceof HTMLElement ? event.target : null;
  const row = target?.closest<HTMLElement>(".file-tree-item[data-entry-path]");
  const path = row?.dataset.entryPath;
  if (!path) {
    return null;
  }

  return scope === "local"
    ? localEntries.value.find((entry) => entry.path === path) || null
    : remoteEntries.value.find((entry) => entry.path === path) || null;
}

function openTransferListContextMenu(event: MouseEvent, scope: TransferEntryScope) {
  const entry = transferEntryFromEvent(event, scope);
  if (!entry) {
    closeTransferContextMenu();
    return;
  }

  openTransferContextMenu(event, scope, entry);
}

function onTransferListPointerDown(event: PointerEvent, scope: TransferEntryScope) {
  if (event.button !== 2) {
    return;
  }

  const entry = transferEntryFromEvent(event, scope);
  if (!entry) {
    return;
  }

  openTransferContextMenu(event, scope, entry);
}

function transferPanePercentFromClientX(clientX: number) {
  const rect = transferBodyRef.value?.getBoundingClientRect();
  if (!rect || rect.width <= 0) {
    return transferPaneSplitPercent.value;
  }

  const availableWidth = Math.max(1, rect.width - transferPaneHandleWidth);
  const localPaneWidth = clientX - rect.left - transferPaneHandleWidth / 2;
  const nextPercent = (localPaneWidth / availableWidth) * 100;
  return clampSplitPanePercent(nextPercent, rect.width, transferPaneMinWidth, transferPaneHandleWidth);
}

function updateTransferPaneSplitFromClientX(clientX: number) {
  transferPaneSplitPercent.value = transferPanePercentFromClientX(clientX);
}

function startTransferPaneResize(event: PointerEvent) {
  if (event.button !== 0) {
    return;
  }

  event.preventDefault();
  event.stopPropagation();
  closeTransferContextMenu();
  transferPaneResizeActive.value = true;
  transferPaneResizePointerId = event.pointerId;
  transferPaneResizeHandle = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  transferPaneResizeHandle?.setPointerCapture(event.pointerId);
  updateTransferPaneSplitFromClientX(event.clientX);
  window.addEventListener("pointermove", updateTransferPaneResize);
  window.addEventListener("pointerup", stopTransferPaneResize);
  window.addEventListener("pointercancel", stopTransferPaneResize);
}

function updateTransferPaneResize(event: PointerEvent) {
  if (!transferPaneResizeActive.value || event.pointerId !== transferPaneResizePointerId) {
    return;
  }

  event.preventDefault();
  updateTransferPaneSplitFromClientX(event.clientX);
}

function stopTransferPaneResize(event?: PointerEvent) {
  if (event && transferPaneResizePointerId !== null && event.pointerId !== transferPaneResizePointerId) {
    return;
  }

  if (transferPaneResizePointerId !== null && transferPaneResizeHandle?.hasPointerCapture(transferPaneResizePointerId)) {
    transferPaneResizeHandle.releasePointerCapture(transferPaneResizePointerId);
  }
  transferPaneResizeActive.value = false;
  transferPaneResizePointerId = null;
  transferPaneResizeHandle = null;
  window.removeEventListener("pointermove", updateTransferPaneResize);
  window.removeEventListener("pointerup", stopTransferPaneResize);
  window.removeEventListener("pointercancel", stopTransferPaneResize);
}

function adjustTransferPaneSplit(delta: number) {
  const rect = transferBodyRef.value?.getBoundingClientRect();
  const width = rect?.width || 0;
  transferPaneSplitPercent.value = clampSplitPanePercent(
    transferPaneSplitPercent.value + delta,
    width,
    transferPaneMinWidth,
    transferPaneHandleWidth,
  );
}

function onTransferPaneResizerKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowLeft") {
    event.preventDefault();
    adjustTransferPaneSplit(-5);
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    adjustTransferPaneSplit(5);
  }
}

async function runTransferContextMenuAction(action: TransferMenuAction) {
  const menu = transferContextMenu.value;
  const item = transferContextMenuItems.value.find((current) => current.action === action);
  if (!menu || item?.disabled) {
    return;
  }

  closeTransferContextMenu();
  switch (action) {
    case "copyToDevice":
      await copyLocalToRemote();
      break;
    case "copyToComputer":
      await copyRemoteToLocal();
      break;
    case "install":
      await installSelectedApk();
      break;
    case "preview":
      await openTextPreview(menu.scope, menu.entry.path);
      break;
    case "refresh":
      if (menu.scope === "local") {
        await loadLocalEntries();
      } else {
        await loadRemoteEntries();
      }
      break;
    case "delete":
      if (menu.scope === "local") {
        requestDeleteSelectedLocalEntry();
      } else {
        requestDeleteSelectedRemoteEntry();
      }
      break;
  }
}

function handleTransferContextMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && transferContextMenu.value) {
    closeTransferContextMenu();
  }
}

function enterLocalDirectory(entry: LocalEntry) {
  if (!entry.isDir) {
    if (isPreviewablePath(entry.path)) {
      void openTextPreview("local", entry.path);
    }
    return;
  }
  void loadLocalEntries(entry.path);
}

async function loadRemoteEntries(path = remoteBrowserPath.value) {
  closeTransferContextMenu();
  if (!selectedSerial.value) {
    remoteEntries.value = [];
    remoteBrowseError.value = "请先选择设备";
    return;
  }

  browsingRemote.value = true;
  remoteBrowseError.value = "";

  try {
    const normalized = path.trim() || defaultDeviceRoot;
    remoteEntries.value = await invoke<RemoteEntry[]>("list_sdcard_entries", {
      serial: selectedSerial.value,
      path: normalized,
    });
    remoteBrowserPath.value = normalized.endsWith("/") ? normalized : `${normalized}/`;
  } catch (error) {
    remoteEntries.value = [];
    remoteBrowseError.value = String(error);
  } finally {
    browsingRemote.value = false;
  }
}

function goParentDirectory() {
  if (atSdcardRoot.value) {
    return;
  }

  const trimmed = remoteBrowserPath.value.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  let parent = idx > 0 ? trimmed.slice(0, idx) : defaultDeviceRoot.replace(/\/+$/, "");
  const root = defaultDeviceRoot.replace(/\/+$/, "");
  if (!parent.startsWith(root)) {
    parent = root;
  }
  remoteBrowserPath.value = parent === root ? defaultDeviceRoot : `${parent}/`;
  void loadRemoteEntries(remoteBrowserPath.value);
}

function selectRemoteEntry(entry: RemoteEntry) {
  selectedRemotePath.value = entry.path;
  if (entry.isDir) {
    adbStore.setDefaultDevicePath(entry.path);
  } else {
    adbStore.setDefaultDevicePath(entry.path);
  }
}

function enterRemoteDirectory(entry: RemoteEntry) {
  if (!entry.isDir) {
    if (isPreviewablePath(entry.path)) {
      void openTextPreview("remote", entry.path);
    }
    return;
  }
  selectedRemotePath.value = "";
  adbStore.setDefaultDevicePath(entry.path);
  void loadRemoteEntries(entry.path);
}

function resetTextPreviewState() {
  textPreviewLines.value = [];
  textPreviewPendingLine.value = "";
  textPreviewNextOffset.value = 0;
  textPreviewEof.value = false;
  textPreviewError.value = "";
  textPreviewAssetUrl.value = "";
  textPreviewScrollTop.value = 0;
  textPreviewBodyRef.value?.scrollTo({ top: 0 });
  resetTextPreviewSearch(true);
  resetImagePreviewView();
}

function resetTextPreviewSearch(clearQuery = false) {
  if (clearQuery) {
    textPreviewSearchQuery.value = "";
  }
  textPreviewSearchActiveLine.value = null;
  textPreviewSearchMessage.value = "";
  textPreviewSearchSearching.value = false;
}

function resetImagePreviewView() {
  imagePreviewScale.value = 1;
  imagePreviewOffset.value = { x: 0, y: 0 };
  imagePreviewDragging.value = false;
  imagePreviewDragState = null;
}

function onTextPreviewImageError() {
  textPreviewError.value = "图片加载失败，请检查文件路径、权限或图片格式";
}

function onTextPreviewVideoError() {
  textPreviewError.value = "视频加载失败，请检查文件路径、权限、格式或编码";
}

function onTextPreviewImageWheel(event: WheelEvent) {
  if (textPreviewMode.value !== "image" || !textPreviewAssetUrl.value) {
    return;
  }
  event.preventDefault();
  const oldScale = imagePreviewScale.value;
  const factor = Math.exp(-event.deltaY * 0.0012);
  const nextScale = Math.min(8, Math.max(1, oldScale * factor));
  if (nextScale === oldScale) {
    return;
  }

  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const pointerX = event.clientX - rect.left - rect.width / 2;
  const pointerY = event.clientY - rect.top - rect.height / 2;
  const ratio = nextScale / oldScale;
  const nextOffset = nextScale === 1
    ? { x: 0, y: 0 }
    : {
      x: pointerX - (pointerX - imagePreviewOffset.value.x) * ratio,
      y: pointerY - (pointerY - imagePreviewOffset.value.y) * ratio,
    };

  imagePreviewScale.value = nextScale;
  imagePreviewOffset.value = nextOffset;
}

function onTextPreviewImagePointerDown(event: PointerEvent) {
  if (textPreviewMode.value !== "image" || imagePreviewScale.value <= 1) {
    return;
  }
  event.preventDefault();
  const target = event.currentTarget as HTMLElement;
  target.setPointerCapture(event.pointerId);
  imagePreviewDragging.value = true;
  imagePreviewDragState = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    offsetX: imagePreviewOffset.value.x,
    offsetY: imagePreviewOffset.value.y,
  };
}

function onTextPreviewImagePointerMove(event: PointerEvent) {
  if (!imagePreviewDragState || imagePreviewDragState.pointerId !== event.pointerId) {
    return;
  }
  event.preventDefault();
  imagePreviewOffset.value = {
    x: imagePreviewDragState.offsetX + event.clientX - imagePreviewDragState.startX,
    y: imagePreviewDragState.offsetY + event.clientY - imagePreviewDragState.startY,
  };
}

function onTextPreviewImagePointerUp(event: PointerEvent) {
  if (!imagePreviewDragState || imagePreviewDragState.pointerId !== event.pointerId) {
    return;
  }
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) {
    target.releasePointerCapture(event.pointerId);
  }
  imagePreviewDragging.value = false;
  imagePreviewDragState = null;
}

async function openTextPreview(source: TextPreviewSource, path: string) {
  if (source === "remote" && !requireSelectedDevice()) {
    return;
  }
  if (!isPreviewablePath(path)) {
    adbStore.setNotice("info", "当前文件类型暂不支持预览");
    return;
  }
  const mode = previewModeForPath(path);
  const requestId = textPreviewRequestId.value + 1;
  const wasOpen = textPreviewOpen.value;
  textPreviewRequestId.value = requestId;
  textPreviewOpen.value = true;
  if (!wasOpen) {
    registerWorkspaceTab("text-preview");
  }
  activeWorkspaceTabKey.value = "text-preview";
  scrollWorkspaceTabIntoView("text-preview");
  textPreviewSource.value = source;
  textPreviewMode.value = mode;
  textPreviewMeta.value = null;
  resetTextPreviewState();
  textPreviewLoading.value = true;
  try {
    if (mode === "video") {
      if (source === "local") {
        const meta = await invoke<TextPreviewMeta>("get_local_text_preview_meta", { path });
        if (textPreviewRequestId.value !== requestId) {
          return;
        }
        textPreviewMeta.value = meta;
        textPreviewAssetUrl.value = convertFileSrc(meta.path);
      } else {
        const localDir = await ensureDownloadLocalDir();
        if (!localDir) {
          textPreviewLoading.value = false;
          return;
        }
        const result = await invoke<PreviewFileResult>("prepare_remote_preview_file", { serial: selectedSerial.value, path, localDir });
        if (textPreviewRequestId.value !== requestId) {
          return;
        }
        textPreviewMeta.value = result;
        textPreviewAssetUrl.value = convertFileSrc(result.path);
      }
      textPreviewLoading.value = false;
      return;
    }
    if (mode === "image") {
      let image: ImagePreviewResult;
      if (source === "local") {
        image = await invoke<ImagePreviewResult>("read_preview_image_data_url", { path });
      } else {
        const localDir = await ensureDownloadLocalDir();
        if (!localDir) {
          textPreviewLoading.value = false;
          return;
        }
        const result = await invoke<PreviewFileResult>("prepare_remote_preview_file", { serial: selectedSerial.value, path, localDir });
        image = await invoke<ImagePreviewResult>("read_preview_image_data_url", { path: result.path });
      }
      if (textPreviewRequestId.value !== requestId) {
        return;
      }
      textPreviewMeta.value = image;
      textPreviewAssetUrl.value = image.dataUrl;
      textPreviewLoading.value = false;
      return;
    }
    if (mode === "pdf") {
      if (source === "local") {
        const meta = await invoke<TextPreviewMeta>("get_local_text_preview_meta", { path });
        if (textPreviewRequestId.value !== requestId) {
          return;
        }
        textPreviewMeta.value = meta;
        textPreviewAssetUrl.value = convertFileSrc(meta.path);
      } else {
        const localDir = await ensureDownloadLocalDir();
        if (!localDir) {
          textPreviewLoading.value = false;
          return;
        }
        const result = await invoke<PreviewFileResult>("prepare_remote_preview_file", { serial: selectedSerial.value, path, localDir });
        if (textPreviewRequestId.value !== requestId) {
          return;
        }
        textPreviewMeta.value = result;
        textPreviewAssetUrl.value = convertFileSrc(result.path);
      }
      textPreviewLoading.value = false;
      return;
    }
    const meta = source === "local"
      ? await invoke<TextPreviewMeta>("get_local_text_preview_meta", { path })
      : await invoke<TextPreviewMeta>("get_remote_text_preview_meta", { serial: selectedSerial.value, path });
    if (textPreviewRequestId.value !== requestId) {
      return;
    }
    textPreviewMeta.value = meta;
    textPreviewLoading.value = false;
    await loadNextTextPreviewChunk();
  } catch (error) {
    if (textPreviewRequestId.value === requestId) {
      textPreviewError.value = String(error);
      textPreviewLoading.value = false;
    }
  }
}

function closeTextPreview() {
  const wasActive = activeWorkspaceTabKey.value === "text-preview";
  const previousOrder = [...workspaceTabOrder.value];
  textPreviewRequestId.value += 1;
  textPreviewOpen.value = false;
  textPreviewMeta.value = null;
  resetTextPreviewState();
  textPreviewLoading.value = false;
  unregisterWorkspaceTab("text-preview");
  if (wasActive) {
    activateAdjacentWorkspaceTabAfterClose("text-preview", previousOrder);
  }
}

function appendTextPreviewChunk(chunk: TextPreviewChunk) {
  const normalized = (textPreviewPendingLine.value + chunk.text).replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const parts = normalized.split("\n");
  const completed = chunk.eof ? parts : parts.slice(0, -1);
  textPreviewPendingLine.value = chunk.eof ? "" : parts[parts.length - 1] || "";
  const startId = textPreviewLines.value.length;
  textPreviewLines.value = [
    ...textPreviewLines.value,
    ...completed.map((text, index) => ({ id: startId + index, text })),
  ];
  textPreviewNextOffset.value = chunk.nextOffset;
  textPreviewEof.value = chunk.eof;
}

async function loadNextTextPreviewChunk() {
  const meta = textPreviewMeta.value;
  if (!textPreviewOpen.value || !meta || textPreviewLoading.value || textPreviewEof.value) {
    return false;
  }
  if (textPreviewMode.value === "pdf" || textPreviewMode.value === "image" || textPreviewMode.value === "video") {
    return false;
  }
  const requestId = textPreviewRequestId.value;
  textPreviewLoading.value = true;
  textPreviewError.value = "";
  let loaded = false;
  try {
    const chunk = textPreviewSource.value === "local"
      ? await invoke<TextPreviewChunk>("read_local_text_chunk", {
        path: meta.path,
        offset: textPreviewNextOffset.value,
        length: textPreviewChunkSize,
      })
      : await invoke<TextPreviewChunk>("read_remote_text_chunk", {
        serial: selectedSerial.value,
        path: meta.path,
        offset: textPreviewNextOffset.value,
        length: textPreviewChunkSize,
      });
    if (textPreviewRequestId.value !== requestId) {
      return false;
    }
    appendTextPreviewChunk(chunk);
    loaded = true;
  } catch (error) {
    if (textPreviewRequestId.value === requestId) {
      textPreviewError.value = String(error);
    }
  } finally {
    if (textPreviewRequestId.value === requestId) {
      textPreviewLoading.value = false;
    }
  }
  return loaded;
}

function onTextPreviewScroll(event: Event) {
  const element = event.target as HTMLDivElement;
  textPreviewScrollTop.value = element.scrollTop;
  textPreviewViewportHeight.value = element.clientHeight;
  if (element.scrollTop + element.clientHeight >= element.scrollHeight - 320) {
    void loadNextTextPreviewChunk();
  }
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function buildTextPreviewSearchRegex(query: string) {
  const flags = textPreviewSearchCaseSensitive.value ? "gu" : "giu";
  const source = textPreviewSearchRegex.value ? query : escapeRegExp(query);
  const boundedSource = textPreviewSearchWholeWord.value
    ? `(?<![\\p{L}\\p{N}_])(?:${source})(?![\\p{L}\\p{N}_])`
    : source;
  return new RegExp(boundedSource, flags);
}

function textPreviewLineMatches(text: string) {
  const regex = textPreviewSearchPattern.value.regex;
  if (!regex) {
    return false;
  }
  regex.lastIndex = 0;
  return regex.test(text);
}

function onTextPreviewSearchInput() {
  resetTextPreviewSearch(false);
}

function toggleTextPreviewSearchOption(option: "case" | "whole" | "regex") {
  if (option === "case") {
    textPreviewSearchCaseSensitive.value = !textPreviewSearchCaseSensitive.value;
  } else if (option === "whole") {
    textPreviewSearchWholeWord.value = !textPreviewSearchWholeWord.value;
  } else {
    textPreviewSearchRegex.value = !textPreviewSearchRegex.value;
  }
  resetTextPreviewSearch(false);
}

function onTextPreviewSearchKeydown(event: KeyboardEvent) {
  void searchTextPreview(event.shiftKey ? -1 : 1);
}

function findTextPreviewLoadedLine(startLine: number, direction: 1 | -1) {
  if (!textPreviewSearchPattern.value.regex) {
    return -1;
  }
  if (direction > 0) {
    for (let index = Math.max(0, startLine); index < textPreviewLines.value.length; index++) {
      if (textPreviewLineMatches(textPreviewLines.value[index]?.text || "")) {
        return index;
      }
    }
    return -1;
  }
  for (let index = Math.min(textPreviewLines.value.length - 1, startLine); index >= 0; index--) {
    if (textPreviewLineMatches(textPreviewLines.value[index]?.text || "")) {
      return index;
    }
  }
  return -1;
}

function focusTextPreviewLine(lineIndex: number) {
  textPreviewSearchActiveLine.value = lineIndex;
  textPreviewSearchMessage.value = "";
  const element = textPreviewBodyRef.value;
  if (!element) {
    return;
  }
  const targetTop = Math.max(0, lineIndex * textPreviewLineHeight - Math.floor(element.clientHeight * 0.35));
  element.scrollTo({ top: targetTop, behavior: "smooth" });
  textPreviewScrollTop.value = targetTop;
}

async function searchTextPreview(direction: 1 | -1) {
  if (textPreviewMode.value !== "text") {
    return;
  }
  const needle = textPreviewSearchNeedle.value;
  if (!needle || textPreviewSearchSearching.value) {
    return;
  }
  if (textPreviewSearchPattern.value.error || !textPreviewSearchPattern.value.regex) {
    textPreviewSearchMessage.value = "正则错误";
    textPreviewSearchActiveLine.value = null;
    return;
  }

  textPreviewSearchSearching.value = true;
  textPreviewSearchMessage.value = "";
  try {
    const visibleStart = Math.floor(textPreviewScrollTop.value / textPreviewLineHeight);
    const fromLine = textPreviewSearchActiveLine.value === null
      ? visibleStart
      : textPreviewSearchActiveLine.value + direction;
    let found = findTextPreviewLoadedLine(fromLine, direction);

    if (found < 0 && direction > 0) {
      while (!textPreviewEof.value) {
        const previousLength = textPreviewLines.value.length;
        const loaded = await loadNextTextPreviewChunk();
        if (!loaded || textPreviewLines.value.length <= previousLength) {
          break;
        }
        found = findTextPreviewLoadedLine(previousLength, 1);
        if (found >= 0) {
          break;
        }
      }
    }

    if (found < 0) {
      const wrapStart = direction > 0 ? 0 : textPreviewLines.value.length - 1;
      found = findTextPreviewLoadedLine(wrapStart, direction);
    }

    if (found >= 0) {
      focusTextPreviewLine(found);
      return;
    }

    textPreviewSearchActiveLine.value = null;
    textPreviewSearchMessage.value = textPreviewEof.value ? "未找到" : "未找到";
  } finally {
    textPreviewSearchSearching.value = false;
  }
}

function selectedTextInsideTextPreview() {
  const selection = window.getSelection();
  const root = textPreviewBodyRef.value;
  if (!selection || selection.isCollapsed || !root) {
    return "";
  }
  const anchorNode = selection.anchorNode;
  const focusNode = selection.focusNode;
  if ((anchorNode && !root.contains(anchorNode)) || (focusNode && !root.contains(focusNode))) {
    return "";
  }
  return selection.toString();
}

async function copyTextPreviewSelection() {
  const selectedText = selectedTextInsideTextPreview();
  const text = selectedText || (textPreviewMode.value === "pdf" || textPreviewMode.value === "image" || textPreviewMode.value === "video"
    ? ""
    : textPreviewMode.value === "html"
      ? textPreviewLines.value.map((line) => line.text).join("\n")
      : textPreviewVisibleLines.value.map((line) => line.text).join("\n"));
  if (!text) {
    adbStore.setNotice("info", "没有可复制的文本");
    return;
  }
  await copyTextToClipboard(text);
  adbStore.setNotice("success", selectedText ? "已复制选中文本" : "已复制当前可见文本");
}

function requestDeleteSelectedLocalEntry() {
  const target = selectedLocalPath.value;
  if (!target || quickBusy.value) {
    return;
  }
  deleteConfirmTarget.value = { scope: "local", path: target };
}

async function deleteSelectedLocalEntry(target: string) {
  try {
    const result = await invoke<DeleteResult>("delete_local_entry", { path: target });
    selectedLocalPath.value = "";
    adbStore.setNotice(result.success ? "success" : "error", result.message);
    await loadLocalEntries(localBrowserPath.value);
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

function requestDeleteSelectedRemoteEntry() {
  if (!requireSelectedDevice() || quickBusy.value) {
    return;
  }
  const target = selectedRemotePath.value;
  if (!target) {
    return;
  }
  deleteConfirmTarget.value = { scope: "remote", path: target };
}

async function deleteSelectedRemoteEntry(target: string) {
  try {
    const result = await invoke<DeleteResult>("delete_sdcard_entry", {
      serial: selectedSerial.value,
      path: target,
    });
    selectedRemotePath.value = "";
    adbStore.setNotice(result.success ? "success" : "error", result.message);
    await loadRemoteEntries(remoteBrowserPath.value);
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

function cancelDeleteConfirm() {
  deleteConfirmTarget.value = null;
}

async function confirmDeleteSelectedEntry() {
  const target = deleteConfirmTarget.value;
  if (!target || quickBusy.value) {
    return;
  }

  deleteConfirmTarget.value = null;
  if (target.scope === "local") {
    await deleteSelectedLocalEntry(target.path);
  } else {
    await deleteSelectedRemoteEntry(target.path);
  }
}

async function captureNativeScreenshot() {
  if (!requireSelectedDevice() || quickBusy.value) {
    return;
  }

  try {
    const result = await invoke<DeviceScreenshotResult>("capture_device_screenshot", {
      serial: selectedSerial.value,
    });
    await openScreenshotEditor(`data:image/png;base64,${result.dataBase64}`, "native");
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

async function captureMirrorScreenshot() {
  const tab = activeMirrorTab.value;
  if (!isActiveWorkspaceMirror.value || !tab || tab.state !== "running" || !tab.decoderReady || !tab.canvas) {
    adbStore.setNotice("error", "请先切换到正在运行的投屏标签页");
    return;
  }
  if (tab.canvas.width <= 0 || tab.canvas.height <= 0) {
    adbStore.setNotice("error", "当前投屏画面尚未就绪");
    return;
  }

  try {
    const blob = await canvasToPngBlob(tab.canvas);
    const dataUrl = await blobToDataUrl(blob);
    await openScreenshotEditor(dataUrl, "mirror");
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

async function captureScreenshot() {
  if (screenshotSource.value === "mirror") {
    await captureMirrorScreenshot();
    return;
  }

  await captureNativeScreenshot();
}

async function startRecording() {
  if (quickBusy.value || recording.value || savingLocalRecording.value) {
    return;
  }

  const tab = activeMirrorTab.value;
  if (!tab || tab.state !== "running" || !tab.canvas) {
    adbStore.setNotice("error", "请先启动投屏后再录屏");
    return;
  }

  if (typeof MediaRecorder === "undefined" || typeof tab.canvas.captureStream !== "function") {
    adbStore.setNotice("error", "当前系统 WebView 不支持直接录制投屏画面");
    return;
  }

  const dir = await ensureDownloadLocalDir();
  if (!dir) {
    return;
  }

  const mimeType = supportedRecordingMimeType();
  localRecordingStream = tab.canvas.captureStream(Math.max(1, mirrorMaxFps.value || 30));
  localRecordingChunks = [];
  localMediaRecorder = new MediaRecorder(localRecordingStream, mimeType ? { mimeType } : undefined);
  localRecordingMimeType = localMediaRecorder.mimeType || mimeType || "video/webm";

  localMediaRecorder.ondataavailable = (event) => {
    if (event.data.size > 0) {
      localRecordingChunks.push(event.data);
    }
  };
  localMediaRecorder.onerror = () => {
    stopRecordingTimer();
    recording.value = false;
    savingLocalRecording.value = false;
    recordingMirrorTabId.value = null;
    localRecordingStream?.getTracks().forEach((track) => track.stop());
    localRecordingStream = null;
    localMediaRecorder = null;
    adbStore.setNotice("error", "录屏失败");
  };
  localMediaRecorder.onstop = async () => {
    stopRecordingTimer();
    const chunks = localRecordingChunks;
    const finalMimeType = localRecordingMimeType || "video/webm";
    localRecordingChunks = [];
    localRecordingStream?.getTracks().forEach((track) => track.stop());
    localRecordingStream = null;
    localMediaRecorder = null;
    savingLocalRecording.value = true;

    try {
      const blob = new Blob(chunks, { type: finalMimeType });
      const dataBase64 = await blobToBase64(blob);
      const ext = recordingExtension(finalMimeType);
      const result = await invoke<AdbExecResult>("save_local_recording", {
        localDir: dir,
        fileName: `droidpilot_record.${ext}`,
        dataBase64,
      });
      adbStore.setNotice(result.success ? "success" : "error", result.stdout || result.stderr || "录屏已保存");
    } catch (error) {
      adbStore.setNotice("error", String(error));
    } finally {
      savingLocalRecording.value = false;
      recording.value = false;
      recordingMirrorTabId.value = null;
    }
  };

  localMediaRecorder.start(1000);
  recordingMirrorTabId.value = tab.id;
  recording.value = true;
  startRecordingTimer();
  adbStore.setNotice("success", "开始录制到电脑");
}

async function stopRecording() {
  if (quickBusy.value || savingLocalRecording.value) {
    return;
  }

  const tab = activeMirrorTab.value;
  if (!tab || recordingMirrorTabId.value !== tab.id) {
    return;
  }

  if (localMediaRecorder) {
    if (localMediaRecorder.state !== "inactive") {
      localMediaRecorder.stop();
    }
  }
}

function resetCopyProgress(direction: "push" | "pull") {
  copyDirection.value = direction;
  copyProgressPercent.value = null;
  copyProgressMessage.value = "准备复制";
}

function resetInstallProgress() {
  copyDirection.value = "install";
  copyProgressPercent.value = null;
  copyProgressMessage.value = "准备安装";
}

function handleTransferProgress(payload: TransferProgressPayload) {
  if (!copyDirection.value || copyDirection.value === "install" || payload.direction !== copyDirection.value) {
    return;
  }

  copyProgressPercent.value = typeof payload.percent === "number" ? payload.percent : null;
  copyProgressMessage.value = payload.message || (payload.done ? "复制完成" : "正在复制");
}

async function installApkPath(apkPath: string) {
  if (!requireSelectedDevice() || quickBusy.value) {
    return;
  }
  if (!apkPath.trim()) {
    adbStore.setNotice("error", "请先选择 APK 文件");
    return;
  }
  if (!isApkPath(apkPath)) {
    adbStore.setNotice("error", "仅支持安装 .apk 文件");
    return;
  }

  resetInstallProgress();
  adbStore.setUploadLocalPath(apkPath);
  await adbStore.quickInstallApk();
  copyProgressMessage.value = transferRunning.value ? "正在安装" : "安装完成";
}

async function installSelectedApk() {
  if (!selectedLocalPath.value) {
    adbStore.setNotice("error", "请先在系统目录中选择 APK 文件");
    return;
  }
  await installApkPath(selectedLocalPath.value);
}

async function copyLocalPathToRemote(localPath: string, targetRemotePath: string, reloadTarget = true) {
  if (!requireSelectedDevice() || quickBusy.value) {
    return;
  }
  if (!localPath.trim()) {
    adbStore.setNotice("error", "请先在系统目录中选择文件或目录");
    return;
  }

  resetCopyProgress("push");
  adbStore.setUploadLocalPath(localPath);
  adbStore.setDefaultDevicePath(targetRemotePath);
  await adbStore.runTransfer("push", localPath, targetRemotePath);
  if (!transferRunning.value && reloadTarget) {
    await loadRemoteEntries(targetRemotePath);
  }
}

async function copyLocalToRemote() {
  if (!selectedLocalPath.value) {
    adbStore.setNotice("error", "请先在系统目录中选择文件或目录");
    return;
  }

  const targetRemotePath = remoteBrowserPath.value.trim() || defaultDeviceRoot;
  await copyLocalPathToRemote(selectedLocalPath.value, targetRemotePath);
}

async function copyRemoteToLocal() {
  if (!requireSelectedDevice() || quickBusy.value) {
    return;
  }
  if (!selectedRemotePath.value) {
    adbStore.setNotice("error", "请先在设备目录中选择文件或目录");
    return;
  }
  if (!localBrowserPath.value.trim()) {
    adbStore.setNotice("error", "请先选择系统目录");
    return;
  }
  if (atLocalDrivesRoot.value) {
    adbStore.setNotice("error", "请先进入某个磁盘目录");
    return;
  }

  adbStore.setDefaultDevicePath(selectedRemotePath.value);
  adbStore.setDefaultLocalDir(localBrowserPath.value);
  resetCopyProgress("pull");
  await adbStore.runTransfer("pull", localBrowserPath.value, selectedRemotePath.value);
  if (!transferRunning.value) {
    await loadLocalEntries(localBrowserPath.value);
  }
}

function updateFileDropHover(paths: string[] = []) {
  if (fileDropBusy.value) {
    return;
  }
  fileDropActive.value = true;
  fileDropMessage.value = paths.length
    ? fileDropSummaryMessage(paths)
    : fileDropMessage.value || "释放后复制到默认设备目录，APK 将安装";
}

function clearFileDropHover() {
  fileDropActive.value = false;
  if (!fileDropBusy.value) {
    fileDropMessage.value = "";
  }
}

async function handleDroppedLocalPaths(paths: string[]) {
  const localPaths = paths.map((path) => path.trim()).filter(Boolean);
  fileDropActive.value = false;

  if (fileDropBusy.value) {
    adbStore.setNotice("info", "拖拽任务正在处理，请稍后再试");
    return;
  }
  if (localPaths.length === 0) {
    fileDropMessage.value = "";
    return;
  }
  if (!requireSelectedDevice()) {
    fileDropMessage.value = "";
    return;
  }
  if (quickBusy.value) {
    adbStore.setNotice("info", "当前有传输任务正在运行，请稍后再拖入");
    fileDropMessage.value = "";
    return;
  }

  const apkPaths = localPaths.filter(isApkPath);
  const copyPaths = localPaths.filter((path) => !isApkPath(path));
  const targetRemotePath = normalizeRemoteDropTargetPath();

  fileDropBusy.value = true;
  try {
    for (const apkPath of apkPaths) {
      fileDropMessage.value = `正在安装 ${localPathDisplayName(apkPath)}`;
      await installApkPath(apkPath);
    }

    for (const localPath of copyPaths) {
      fileDropMessage.value = `正在复制 ${localPathDisplayName(localPath)}`;
      await copyLocalPathToRemote(localPath, targetRemotePath, false);
    }

    if (copyPaths.length > 0) {
      fileDropMessage.value = "正在刷新设备目录";
      await loadRemoteEntries(targetRemotePath);
    }

    if (adbStore.notice?.type !== "error") {
      adbStore.setNotice("success", "拖拽文件处理完成");
    }
  } finally {
    fileDropBusy.value = false;
    fileDropActive.value = false;
    fileDropMessage.value = "";
  }
}

function createTerminalView() {
  if (terminal || !terminalHostRef.value) {
    return;
  }

  terminal = new Terminal({
    cursorBlink: true,
    convertEol: true,
    fontFamily: "SFMono-Regular, Consolas, 'Liberation Mono', monospace",
    fontSize: 12,
    lineHeight: 1.35,
    scrollback: 3000,
    theme: {
      background: "#0f172a",
      foreground: "#dbeafe",
      cursor: "#60a5fa",
      selectionBackground: "#1d4ed8",
      black: "#0f172a",
      blue: "#60a5fa",
      brightBlue: "#93c5fd",
      brightCyan: "#67e8f9",
      brightGreen: "#86efac",
      brightRed: "#fca5a5",
      brightYellow: "#fde68a",
      cyan: "#22d3ee",
      green: "#22c55e",
      red: "#ef4444",
      white: "#e5e7eb",
      yellow: "#eab308",
    },
  });
  terminalFitAddon = new FitAddon();
  terminal.loadAddon(terminalFitAddon);
  terminal.open(terminalHostRef.value);
  terminalDataDisposable = terminal.onData((data) => {
    void writeTerminalData(data);
  });
  terminalResizeObserver = new ResizeObserver((entries) => {
    const rect = entries[0]?.contentRect;
    if (rect) {
      const nextWidth = Math.round(rect.width);
      const nextHeight = Math.round(rect.height);
      if (nextWidth === terminalLastHostWidth && nextHeight === terminalLastHostHeight) {
        return;
      }
      terminalLastHostWidth = nextWidth;
      terminalLastHostHeight = nextHeight;
    }
    scheduleTerminalResize();
  });
  terminalResizeObserver.observe(terminalHostRef.value);
  void resizeTerminal();
}

function disposeTerminalView() {
  if (terminalResizeTimer !== null) {
    window.clearTimeout(terminalResizeTimer);
    terminalResizeTimer = null;
  }
  resetTerminalOutputQueue();
  terminalResizeObserver?.disconnect();
  terminalResizeObserver = null;
  terminalDataDisposable?.dispose();
  terminalDataDisposable = null;
  terminal?.dispose();
  terminal = null;
  terminalFitAddon = null;
  terminalLastRows = 0;
  terminalLastCols = 0;
  terminalLastHostWidth = 0;
  terminalLastHostHeight = 0;
}

function resetTerminalOutputQueue(preserveWriteActive = false) {
  terminalOutputQueue = [];
  terminalOutputQueuedBytes = 0;
  if (!preserveWriteActive) {
    terminalOutputWriteActive = false;
  }
}

function decodeTerminalOutput(dataBase64: string) {
  if (!dataBase64) {
    return new Uint8Array();
  }

  const binary = window.atob(dataBase64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function takeTerminalOutputChunk() {
  const chunkSize = Math.min(TERMINAL_OUTPUT_CHUNK_BYTES, terminalOutputQueuedBytes);
  if (chunkSize <= 0) {
    return new Uint8Array();
  }

  const chunk = new Uint8Array(chunkSize);
  let offset = 0;
  while (offset < chunkSize && terminalOutputQueue.length > 0) {
    const head = terminalOutputQueue[0];
    const remaining = chunkSize - offset;
    if (head.length <= remaining) {
      chunk.set(head, offset);
      offset += head.length;
      terminalOutputQueue.shift();
    } else {
      chunk.set(head.subarray(0, remaining), offset);
      terminalOutputQueue[0] = head.subarray(remaining);
      offset += remaining;
    }
  }

  terminalOutputQueuedBytes -= chunkSize;
  return chunk;
}

function scheduleTerminalOutputFlush() {
  if (terminalOutputWriteActive || terminalOutputQueue.length === 0) {
    return;
  }

  terminalOutputWriteActive = true;
  window.requestAnimationFrame(flushTerminalOutputQueue);
}

function flushTerminalOutputQueue() {
  if (!terminal || !terminalSessionId.value) {
    resetTerminalOutputQueue();
    return;
  }

  const chunk = takeTerminalOutputChunk();
  if (chunk.length === 0) {
    terminalOutputWriteActive = false;
    return;
  }

  terminal.write(chunk, () => {
    terminalOutputWriteActive = false;
    scheduleTerminalOutputFlush();
  });
}

function enqueueTerminalOutput(bytes: Uint8Array) {
  if (bytes.length === 0) {
    return;
  }

  if (terminalOutputQueuedBytes + bytes.length > TERMINAL_OUTPUT_MAX_QUEUE_BYTES) {
    resetTerminalOutputQueue(true);
    terminal?.writeln("\r\n[terminal output skipped: buffer limit exceeded]\r\n");
  }

  terminalOutputQueue.push(bytes);
  terminalOutputQueuedBytes += bytes.length;
  scheduleTerminalOutputFlush();
}

function focusTerminalView() {
  if (!terminal || !terminalPanelOpen.value || !terminalHostRef.value) {
    return;
  }

  try {
    terminal.focus();
  } catch {
    // The xterm textarea may be temporarily unavailable while the panel is mounting or closing.
  }
}

async function ensureTerminalStarted() {
  createTerminalView();
  if (!terminal || terminalSessionId.value || terminalStarting.value) {
    focusTerminalView();
    return;
  }

  terminalStarting.value = true;
  try {
    terminalFitAddon?.fit();
    const rows = terminal.rows;
    const cols = terminal.cols;
    const result = await invoke<TerminalSpawnResult>("terminal_spawn", {
      rows,
      cols,
    });
    if (!terminal || !terminalPanelOpen.value) {
      await invoke("terminal_close", { sessionId: result.sessionId });
      return;
    }
    terminalSessionId.value = result.sessionId;
    terminalShell.value = result.shell;
    await resizeTerminal();
    focusTerminalView();
  } catch (error) {
    adbStore.setNotice("error", String(error));
    terminal?.writeln(`\r\n${String(error)}`);
  } finally {
    terminalStarting.value = false;
  }
}

async function writeTerminalData(data: string) {
  await ensureTerminalStarted();
  const sessionId = terminalSessionId.value;
  if (!sessionId) {
    return;
  }
  try {
    await invoke("terminal_write", { sessionId, data });
    focusTerminalView();
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

function scheduleTerminalResize() {
  if (terminalResizeTimer !== null) {
    window.clearTimeout(terminalResizeTimer);
  }
  terminalResizeTimer = window.setTimeout(() => {
    terminalResizeTimer = null;
    void resizeTerminal();
  }, 80);
}

async function resizeTerminal() {
  if (!terminal || !terminalFitAddon) {
    return;
  }
  try {
    terminalFitAddon.fit();
  } catch {
    return;
  }
  const sessionId = terminalSessionId.value;
  if (!sessionId) {
    return;
  }
  if (terminal.rows === terminalLastRows && terminal.cols === terminalLastCols) {
    return;
  }
  terminalLastRows = terminal.rows;
  terminalLastCols = terminal.cols;
  try {
    await invoke("terminal_resize", {
      sessionId,
      rows: terminal.rows,
      cols: terminal.cols,
    });
  } catch {
    // The backend can already be closing while the modal is being dismissed.
  }
}

async function closeTerminalSession() {
  const sessionId = terminalSessionId.value;
  terminalSessionId.value = null;
  terminalShell.value = "";
  resetTerminalOutputQueue();
  if (!sessionId) {
    return;
  }
  try {
    await invoke("terminal_close", { sessionId });
  } catch (error) {
    adbStore.setNotice("error", String(error));
  }
}

function handleTerminalOutput(payload: TerminalOutputPayload) {
  if (payload.sessionId !== terminalSessionId.value) {
    return;
  }
  try {
    enqueueTerminalOutput(decodeTerminalOutput(payload.dataBase64));
  } catch {
    adbStore.setNotice("error", "终端输出解码失败");
  }
}

function handleTerminalExit(payload: TerminalExitPayload) {
  if (payload.sessionId !== terminalSessionId.value) {
    return;
  }
  terminalSessionId.value = null;
  resetTerminalOutputQueue();
  terminal?.writeln(`\r\n[${payload.message}]`);
}

const shortcutModifierKeys = new Set(["Control", "Shift", "Alt", "Meta"]);
const shortcutKeyLabels: Record<string, string> = {
  " ": "Space",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  ArrowUp: "Up",
  Escape: "Esc",
};
const mirrorKeyboardKeycodes: Record<string, number> = {
  ArrowDown: 20,
  ArrowLeft: 21,
  ArrowRight: 22,
  ArrowUp: 19,
  Backspace: 67,
  Delete: 112,
  End: 123,
  Enter: 66,
  Escape: 111,
  Home: 122,
  PageDown: 93,
  PageUp: 92,
  Tab: 61,
};

function normalizeShortcutPart(value: string) {
  const lower = value.trim().toLowerCase();
  if (!lower) {
    return "";
  }
  if (lower === "control" || lower === "ctrl") {
    return "Ctrl";
  }
  if (lower === "option" || lower === "alt") {
    return "Alt";
  }
  if (lower === "shift") {
    return "Shift";
  }
  if (lower === "cmd" || lower === "command" || lower === "meta") {
    return "Meta";
  }
  if (lower === "esc" || lower === "escape") {
    return "Esc";
  }
  if (lower === "space" || lower === "spacebar") {
    return "Space";
  }
  return value.length === 1 ? value.toUpperCase() : value.trim();
}

function normalizeShortcutValue(value: string) {
  const parts = value.split("+").map(normalizeShortcutPart).filter(Boolean);
  if (parts.length === 0) {
    return "";
  }
  const modifiers = [
    parts.includes("Ctrl") ? "Ctrl" : "",
    parts.includes("Alt") ? "Alt" : "",
    parts.includes("Shift") ? "Shift" : "",
    parts.includes("Meta") ? "Meta" : "",
  ].filter(Boolean);
  const key = parts.find((part) => !["Ctrl", "Alt", "Shift", "Meta"].includes(part));
  return key ? [...modifiers, key].join("+") : "";
}

function displayShortcutValue(value: string) {
  return normalizeShortcutValue(value).split("+").filter(Boolean).join(" + ") || "C";
}

function normalizeShortcutEvent(event: KeyboardEvent) {
  if (shortcutModifierKeys.has(event.key)) {
    return "";
  }
  const key = shortcutKeyLabels[event.key] ?? (event.key.length === 1 ? event.key.toUpperCase() : event.key);
  return [
    event.ctrlKey ? "Ctrl" : "",
    event.altKey ? "Alt" : "",
    event.shiftKey ? "Shift" : "",
    event.metaKey ? "Meta" : "",
    key,
  ]
    .filter(Boolean)
    .join("+");
}

function isEditableKeyboardTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  const tagName = target.tagName.toLowerCase();
  return tagName === "input" || tagName === "textarea" || tagName === "select" || target.isContentEditable;
}

function canForwardMirrorKeyboardInput(event: KeyboardEvent) {
  const tab = activeMirrorTab.value;
  return Boolean(
    isActiveWorkspaceMirror.value
      && tab?.session
      && tab.state === "running"
      && !tab.autoRestarting
      && !event.defaultPrevented
      && !isEditableKeyboardTarget(event.target),
  );
}

function mirrorKeyboardText(event: KeyboardEvent) {
  if (event.ctrlKey || event.altKey || event.metaKey || event.isComposing || event.key === "Process") {
    return "";
  }
  return event.key.length === 1 ? event.key : "";
}

function forwardMirrorKeyboardInput(event: KeyboardEvent) {
  if (!canForwardMirrorKeyboardInput(event)) {
    return false;
  }

  const text = mirrorKeyboardText(event);
  if (text) {
    event.preventDefault();
    event.stopPropagation();
    void sendText(text);
    return true;
  }

  if (event.ctrlKey || event.altKey || event.metaKey || event.isComposing) {
    return false;
  }

  const keycode = mirrorKeyboardKeycodes[event.key];
  if (!keycode) {
    return false;
  }

  event.preventDefault();
  event.stopPropagation();
  void sendKey(keycode);
  return true;
}

function activeImageEditor() {
  return imageEditorTabs.value.find((editor) => editor.key === activeWorkspaceTabKey.value) || null;
}

function pickerCopyText(editor: ImageEditorTab) {
  const preview = editor.pickerPreview;
  if (!preview) {
    return "";
  }
  if (pickerCopyFormat.value === "rgb") {
    const [red, green, blue] = preview.rgb;
    return `rgb(${red}, ${green}, ${blue})`;
  }
  return preview.color.toUpperCase();
}

function togglePickerCopyFormatByShortcut() {
  adbStore.setPickerCopyFormat(pickerCopyFormat.value === "rgb" ? "hex" : "rgb");
  adbStore.setNotice("info", `取色复制格式: ${pickerCopyFormat.value.toUpperCase()}`);
}

function copyTextFallback(text: string) {
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  textarea.remove();
}

async function copyTextToClipboard(text: string) {
  if (!navigator.clipboard?.writeText) {
    copyTextFallback(text);
    return;
  }
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    copyTextFallback(text);
  }
}

async function copyPickerColorByShortcut() {
  const editor = activeImageEditor();
  if (!editor || editor.tool !== "picker" || !editor.pickerPreview) {
    return false;
  }
  const text = pickerCopyText(editor);
  if (!text) {
    return false;
  }
  await copyTextToClipboard(text);
  adbStore.setNotice("success", `已复制 ${text}`);
  return true;
}

function blurScreenshotToolbarFocusOnEscape(event: KeyboardEvent) {
  if (event.key !== "Escape") {
    return false;
  }
  const activeElement = document.activeElement;
  if (activeElement instanceof HTMLElement && activeElement.closest(".screenshot-editor-toolbar")) {
    event.preventDefault();
    event.stopPropagation();
    activeElement.blur();
    return true;
  }
  return false;
}

function handleMirrorKeydown(event: KeyboardEvent) {
  if (blurScreenshotToolbarFocusOnEscape(event)) {
    return;
  }
  if (isEditableKeyboardTarget(event.target)) {
    return;
  }
  if (forwardMirrorKeyboardInput(event)) {
    return;
  }
  const editor = activeImageEditor();
  if (editor?.tool === "picker" && editor.pickerPreview && event.key === "Shift") {
    if (!event.repeat) {
      event.preventDefault();
      event.stopPropagation();
      togglePickerCopyFormatByShortcut();
    }
    return;
  }
  if (editor?.tool === "polyline" && editor.draft?.type === "polyline") {
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      finishImageEditorPolyline(editor);
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      cancelImageEditorPolyline(editor);
      return;
    }
  }
  const expectedShortcut = normalizeShortcutValue(pickerCopyShortcut.value);
  if (!expectedShortcut || normalizeShortcutEvent(event) !== expectedShortcut) {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  void copyPickerColorByShortcut();
}

onMounted(async () => {
  window.addEventListener("droidpilot:mirror-toolbar-action", handleMirrorToolbarAction);
  window.addEventListener("keydown", handleMirrorKeydown);
  window.addEventListener("click", closeTransferContextMenu);
  window.addEventListener("keydown", handleTransferContextMenuKeydown);
  window.setTimeout(emitMirrorToolbarState, 0);
  unlistenFrame = await listen<ScrcpyFramePayload>("scrcpy://frame", (event) => handleFrame(event.payload));
  unlistenStatus = await listen<ScrcpyStatusPayload>("scrcpy://status", (event) => handleStatus(event.payload));
  mirrorDebug("event-listeners-ready");
  resolveMirrorEventListenersReady?.();
  resolveMirrorEventListenersReady = null;
  unlistenTransferProgress = await listen<TransferProgressPayload>("adb://transfer-progress", (event) =>
    handleTransferProgress(event.payload),
  );
  unlistenTerminalOutput = await listen<TerminalOutputPayload>("terminal-output", (event) => handleTerminalOutput(event.payload));
  unlistenTerminalExit = await listen<TerminalExitPayload>("terminal-exit", (event) => handleTerminalExit(event.payload));
  unlistenFileDrop = await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter") {
      updateFileDropHover(payload.paths);
      return;
    }
    if (payload.type === "over") {
      updateFileDropHover();
      return;
    }
    if (payload.type === "drop") {
      void handleDroppedLocalPaths(payload.paths);
      return;
    }
    clearFileDropHover();
  });
  if (!defaultLocalDir.value.trim()) {
    try {
      const dir = await downloadDir();
      if (dir && dir.trim()) {
        adbStore.setDefaultLocalDir(dir);
      }
    } catch {
      // Default path can still be entered from Settings.
    }
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("droidpilot:mirror-toolbar-action", handleMirrorToolbarAction);
  window.removeEventListener("keydown", handleMirrorKeydown);
  window.removeEventListener("click", closeTransferContextMenu);
  window.removeEventListener("keydown", handleTransferContextMenuKeydown);
  stopTransferPaneResize();
  unlistenFrame?.();
  unlistenStatus?.();
  unlistenTransferProgress?.();
  unlistenTerminalOutput?.();
  unlistenTerminalExit?.();
  unlistenFileDrop?.();
  if (localMediaRecorder && localMediaRecorder.state !== "inactive") {
    stopRecordingTimer();
    localMediaRecorder.stop();
  }
  localRecordingStream?.getTracks().forEach((track) => track.stop());
  for (const tab of mirrorTabs.value) {
    clearMirrorFirstFrameWatch(tab);
    if (tab.session) {
      clearPendingScrcpySession(tab.session.sessionId);
      void invoke("stop_scrcpy_mirror", { sessionId: tab.session.sessionId });
    }
    closeDecoder(tab);
  }
  void closeTerminalSession();
  disposeTerminalView();
});
</script>

<template>
  <div class="mirror-layout">
    <div v-if="fileDropActive || fileDropBusy" class="mirror-file-drop-overlay" :class="{ busy: fileDropBusy }">
      <div class="mirror-file-drop-panel">
        <strong>{{ fileDropBusy ? "正在处理拖入文件" : "释放文件" }}</strong>
        <span>{{ fileDropMessage }}</span>
      </div>
    </div>
    <section class="mirror-stage">
      <div v-if="workspaceTabs.length > 0" class="mirror-tabbar">
        <div ref="mirrorTabListRef" class="mirror-tab-list" @wheel="onMirrorTabListWheel">
          <div
            v-for="tab in workspaceTabs"
            :key="tab.key"
            class="mirror-tab"
            :data-tab-key="tab.key"
            :class="{ active: tab.key === activeWorkspaceTabKey, running: tab.running, tool: tab.kind !== 'mirror' }"
            @mousedown.middle.prevent
            @auxclick="closeWorkspaceTabByMiddleClick(tab, $event)"
          >
            <button class="mirror-tab-main" type="button" @click="activateWorkspaceTab(tab)">
              <span>{{ tab.title }}</span>
              <small v-if="tab.status">{{ tab.status }}</small>
            </button>
            <button
              class="mirror-tab-close"
              type="button"
              :disabled="!tab.closable"
              :aria-label="`关闭 ${tab.title}`"
              :title="tab.closable ? `关闭 ${tab.title}` : '当前任务运行中'"
              @click.stop="closeWorkspaceTab(tab)"
            >
              ×
            </button>
          </div>
        </div>
      </div>

      <div class="mirror-screen-stack" :class="{ empty: workspaceTabs.length === 0 }">
        <section v-if="workspaceTabs.length === 0" class="mirror-empty-shortcuts" aria-label="快捷键">
          <div v-for="shortcut in appShortcutDefinitions" :key="shortcut.action" class="mirror-empty-shortcut-row">
            <span>{{ t(`shortcuts.${shortcut.action}.home`) }}</span>
            <div class="mirror-empty-shortcut-keys" aria-hidden="true">
              <template v-for="(part, index) in homeShortcutKeyParts(shortcut)" :key="`${shortcut.action}-${part}-${index}`">
                <kbd>{{ part }}</kbd>
                <em v-if="index < homeShortcutKeyParts(shortcut).length - 1">+</em>
              </template>
            </div>
          </div>
        </section>
        <div
          v-for="tab in mirrorTabs"
          v-show="activeWorkspaceTabKey === mirrorTabKey(tab.id)"
          :key="tab.id"
          class="screen-wrap"
          :class="{ active: tab.state === 'running' }"
        >
          <canvas
            v-show="tab.state === 'starting' || tab.state === 'running'"
            :ref="(element) => setMirrorCanvas(tab, element as Element | null)"
            class="device-canvas"
            @pointerdown="onPointerDown(tab, $event)"
            @pointermove="onPointerMove(tab, $event)"
            @pointerup="onPointerUp(tab, $event)"
            @pointercancel="onPointerUp(tab, $event)"
            @wheel="onMirrorWheel(tab, $event)"
          ></canvas>
          <div v-if="shouldShowScreenPlaceholder(tab)" class="screen-placeholder">
            <strong>{{ screenPlaceholderFor(tab).title }}</strong>
            <span>{{ screenPlaceholderFor(tab).message }}</span>
          </div>
          <div v-if="tab.id === recordingMirrorTabId && (recording || savingLocalRecording)" class="recording-float">
            <span class="recording-dot" aria-hidden="true"></span>
            <strong>{{ savingLocalRecording ? "保存中" : recordingElapsedLabel }}</strong>
          </div>
        </div>
      </div>

      <section
        v-for="editor in imageEditorTabs"
        v-show="activeWorkspaceTabKey === editor.key"
        :key="editor.key"
        class="screenshot-editor-dock"
      >
        <header class="screenshot-editor-toolbar">
          <div class="screenshot-editor-tools">
            <button
              v-for="tool in imageEditorTools"
              :key="tool.key"
              type="button"
              class="screenshot-tool-btn"
              :class="{ active: editor.tool === tool.key }"
              :title="tool.label"
              :aria-label="tool.label"
              @mousedown.prevent
              @click="updateImageEditorTool(editor, tool.key)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in imageEditorToolIconPaths[tool.key]" :key="path" :d="path" />
              </svg>
            </button>
          </div>
          <div class="screenshot-editor-controls">
            <button
              type="button"
              class="screenshot-tool-btn undo-redo"
              :disabled="editor.undoStack.length === 0"
              title="上一步"
              aria-label="上一步"
              @mousedown.prevent
              @click="undoImageEditor(editor)"
            >
              <svg viewBox="0 0 1024 1024" aria-hidden="true">
                <path v-for="path in imageEditorActionIconPaths.undo" :key="path" :d="path" />
              </svg>
            </button>
            <button
              type="button"
              class="screenshot-tool-btn undo-redo"
              :disabled="editor.redoStack.length === 0"
              title="下一步"
              aria-label="下一步"
              @mousedown.prevent
              @click="redoImageEditor(editor)"
            >
              <svg viewBox="0 0 1024 1024" aria-hidden="true">
                <path v-for="path in imageEditorActionIconPaths.undo" :key="path" :d="path" transform="translate(1024 0) scale(-1 1)" />
              </svg>
            </button>
            <label class="screenshot-color-control" title="颜色">
              <input type="color" :value="editor.color" @input="updateImageEditorColor(editor, ($event.target as HTMLInputElement).value)" />
              <span :style="{ background: editor.color }"></span>
            </label>
            <label class="screenshot-width-control">
              <span>粗细</span>
              <input
                type="range"
                min="1"
                max="12"
                :value="editor.lineWidth"
                @input="editor.lineWidth = Number(($event.target as HTMLInputElement).value)"
              />
            </label>
            <button
              type="button"
              class="screenshot-tool-btn"
              :disabled="!editor.cropRect"
              title="应用裁剪"
              aria-label="应用裁剪"
              @mousedown.prevent
              @click="applyEditorCrop(editor)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in imageEditorActionIconPaths.applyCrop" :key="path" :d="path" />
              </svg>
            </button>
            <button
              type="button"
              class="screenshot-tool-btn"
              :disabled="editor.shapes.length === 0 && !editor.cropRect"
              title="清空标注"
              aria-label="清空标注"
              @mousedown.prevent
              @click="resetImageEditor(editor)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in imageEditorActionIconPaths.clear" :key="path" :d="path" />
              </svg>
            </button>
            <button
              type="button"
              class="screenshot-tool-btn primary"
              :disabled="editor.saving"
              :title="editor.saving ? '保存中' : '保存'"
              :aria-label="editor.saving ? '保存中' : '保存'"
              @mousedown.prevent
              @click="saveImageEditor(editor)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path v-for="path in imageEditorActionIconPaths.save" :key="path" :d="path" />
              </svg>
            </button>
          </div>
        </header>
        <div class="screenshot-editor-canvas-wrap">
          <canvas
            :ref="(element) => setImageEditorCanvas(editor, element as Element | null)"
            class="screenshot-editor-canvas"
            :class="`tool-${editor.tool}`"
            :style="{ cursor: imageEditorCanvasCursor(editor) }"
            @pointerdown="onImageEditorPointerDown(editor, $event)"
            @pointermove="onImageEditorPointerMove(editor, $event)"
            @pointerup="onImageEditorPointerUp(editor, $event)"
            @pointercancel="onImageEditorPointerUp(editor, $event)"
            @pointerleave="onImageEditorPointerLeave(editor)"
            @dblclick="onImageEditorDoubleClick(editor, $event)"
          ></canvas>
          <div
            v-if="editor.tool === 'picker' && editor.pickerPreview"
            class="screenshot-picker-popover"
            :style="imageEditorPickerStyle(editor)"
          >
            <img :src="editor.pickerPreview.magnifier" alt="" />
            <div class="screenshot-picker-meta">
              <span :style="{ background: editor.pickerPreview.color }"></span>
              <code>{{ editor.pickerPreview.color.toUpperCase() }}</code>
              <code>RGB {{ editor.pickerPreview.rgb.join(", ") }}</code>
              <kbd>{{ displayShortcutValue(pickerCopyShortcut) }} 复制</kbd>
              <kbd>Shift 切换 {{ pickerCopyFormat.toUpperCase() }}</kbd>
            </div>
          </div>
          <textarea
            v-if="editor.textInput"
            v-model="editor.textInput.value"
            class="screenshot-text-input"
            :data-editor-text-input="editor.key"
            :data-editor-text-input-id="editor.textInput.id"
            :style="imageEditorTextInputStyle(editor)"
            @pointerdown.stop
            @input="resizeImageEditorTextInput"
            @keydown.esc.stop.prevent="cancelImageEditorText(editor, ($event.target as HTMLTextAreaElement).dataset.editorTextInputId)"
            @blur="commitImageEditorText(editor, ($event.target as HTMLTextAreaElement).dataset.editorTextInputId)"
          ></textarea>
        </div>
      </section>

      <div v-if="terminalPanelOpen" v-show="activeWorkspaceTabKey === 'terminal'" class="terminal-dock" :class="{ 'terminal-dock-collapsed': commandPickerCollapsed }">
        <aside v-show="!commandPickerCollapsed" class="command-picker-panel terminal-command-panel">
          <div class="command-picker-tabs">
            <button
              v-for="group in terminalQuickGroups"
              :key="group.key"
              type="button"
              :class="{ active: terminalQuickGroup === group.key }"
              @click="terminalQuickGroup = group.key"
            >
              {{ group.label }}
            </button>
          </div>
          <div class="command-picker-list">
            <button
              v-for="item in filteredQuickCommands"
              :key="`${item.group}-${item.label}`"
              type="button"
              class="command-picker-item"
              :class="{ active: activeTerminalCommand === item.command }"
              :title="item.command"
              @click="selectQuickCommand(item)"
            >
              <span class="command-picker-title">
                <strong>{{ item.label }}</strong>
                <em v-if="item.tag">{{ item.tag }}</em>
              </span>
              <span>{{ item.desc }}</span>
              <code class="mono">{{ item.command }}</code>
            </button>
            <div v-if="filteredQuickCommands.length === 0" class="command-empty">没有命令</div>
          </div>
        </aside>

        <section class="terminal-pane">
          <div class="terminal-toolbar">
            <div class="terminal-toolbar-left">
              <button
                class="terminal-icon-btn"
                type="button"
                :aria-label="commandPickerCollapsed ? '展开快捷命令' : '收起快捷命令'"
                :title="commandPickerCollapsed ? '展开快捷命令' : '收起快捷命令'"
                @click="toggleCommandPickerCollapsed"
              >
                {{ commandPickerCollapsed ? "›" : "‹" }}
              </button>
              <span>{{ terminalShell || "终端" }}</span>
            </div>
            <div class="terminal-toolbar-right">
              <span>{{ terminalStarting ? t("mirror.terminalStarting") : terminalSessionId ? t("mirror.terminalConnected") : t("mirror.terminalClosed") }}</span>
            </div>
          </div>
          <div ref="terminalHostRef" class="terminal-host"></div>
        </section>
      </div>

      <section v-if="transferPanelOpen" v-show="activeWorkspaceTabKey === 'transfer'" class="transfer-dock">
        <div
          ref="transferBodyRef"
          class="transfer-modal-body"
          :class="{ resizing: transferPaneResizeActive }"
          :style="transferPaneGridStyle"
        >
          <div class="file-pane system-pane">
            <div class="file-pane-head">
              <strong>系统目录</strong>
              <div class="file-pane-actions">
                <button type="button" class="transfer-tool-btn" :disabled="browsingLocal" title="选择系统目录" aria-label="选择系统目录" @click="pickLocalBrowserDirectory">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.chooseFolder" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="!canCopyLocalToRemote" title="复制到对方（设备）" aria-label="复制到对方（设备）" @click="copyLocalToRemote">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.copyToDevice" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="!canInstallSelectedApk" title="当前选中 APK 点击安装" aria-label="当前选中 APK 点击安装" @click="installSelectedApk">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.apk" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="!canPreviewSelectedLocalText" title="预览" aria-label="预览" @click="openTextPreview('local', selectedLocalPath)">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.preview" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="browsingLocal" title="刷新" aria-label="刷新" @click="loadLocalEntries()">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.refresh" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn danger" :disabled="!canDeleteLocal" title="删除" aria-label="删除" @click="requestDeleteSelectedLocalEntry">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.delete" :key="path" :d="path" />
                  </svg>
                </button>
              </div>
            </div>
            <div class="file-path-bar mono">{{ localBrowserPathLabel }}</div>

            <p class="helper" v-if="browsingLocal">目录读取中...</p>
            <p class="helper" v-else-if="localBrowseError">{{ localBrowseError }}</p>

            <div
              v-else
              class="file-tree-list"
              data-allow-app-context-menu="true"
              @pointerdown="onTransferListPointerDown($event, 'local')"
              @contextmenu.prevent="openTransferListContextMenu($event, 'local')"
            >
              <div class="file-tree-header">
                <button
                  type="button"
                  class="file-tree-sort-btn file-tree-sort-name"
                  :class="{ active: localEntrySort.key === 'name' }"
                  :aria-pressed="localEntrySort.key === 'name'"
                  :aria-label="fileEntrySortButtonLabel(localEntrySort, 'name', '名称')"
                  :title="fileEntrySortButtonLabel(localEntrySort, 'name', '名称')"
                  @click.stop="toggleFileEntrySort('local', 'name')"
                >
                  <span>名称</span>
                  <svg class="file-tree-sort-indicator" :class="{ visible: localEntrySort.key === 'name' }" viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(localEntrySort, 'name')]" :key="path" :d="path" />
                  </svg>
                </button>
                <span class="file-entry-meta">
                  <button
                    type="button"
                    class="file-tree-sort-btn file-entry-size"
                    :class="{ active: localEntrySort.key === 'size' }"
                    :aria-pressed="localEntrySort.key === 'size'"
                    :aria-label="fileEntrySortButtonLabel(localEntrySort, 'size', '大小')"
                    :title="fileEntrySortButtonLabel(localEntrySort, 'size', '大小')"
                    @click.stop="toggleFileEntrySort('local', 'size')"
                  >
                    <span>大小</span>
                    <svg class="file-tree-sort-indicator" :class="{ visible: localEntrySort.key === 'size' }" viewBox="0 0 24 24" aria-hidden="true">
                      <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(localEntrySort, 'size')]" :key="path" :d="path" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="file-tree-sort-btn file-entry-created"
                    :class="{ active: localEntrySort.key === 'date' }"
                    :aria-pressed="localEntrySort.key === 'date'"
                    :aria-label="fileEntrySortButtonLabel(localEntrySort, 'date', '日期')"
                    :title="fileEntrySortButtonLabel(localEntrySort, 'date', '日期')"
                    @click.stop="toggleFileEntrySort('local', 'date')"
                  >
                    <span>日期</span>
                    <svg class="file-tree-sort-indicator" :class="{ visible: localEntrySort.key === 'date' }" viewBox="0 0 24 24" aria-hidden="true">
                      <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(localEntrySort, 'date')]" :key="path" :d="path" />
                    </svg>
                  </button>
                </span>
              </div>
              <button type="button" class="file-parent-row mono" :disabled="!canGoLocalParent || browsingLocal" @dblclick="goLocalParentDirectory">
                ...
              </button>
              <div
                v-for="entry in sortedLocalEntries"
                :key="entry.path"
                class="file-tree-item"
                data-allow-app-context-menu="true"
                role="button"
                tabindex="0"
                :class="{ active: entry.path === selectedLocalPath }"
                :data-entry-path="entry.path"
                @click="selectLocalEntry(entry)"
                @dblclick="enterLocalDirectory(entry)"
                @keydown.enter.prevent="enterLocalDirectory(entry)"
              >
                <span class="file-entry-main">
                  <svg class="file-entry-icon" :class="fileEntryIconName(entry)" viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in fileEntryIconPaths[fileEntryIconName(entry)]" :key="path" :d="path" />
                  </svg>
                  <span class="file-entry-name mono">{{ entry.name }}</span>
                </span>
                <span class="file-entry-meta">
                  <span class="file-entry-size mono">{{ formatEntrySize(entry) }}</span>
                  <span class="file-entry-created mono">{{ formatEntryCreatedAt(entry) }}</span>
                </span>
              </div>
              <p class="empty-tree-state" v-if="localEntries.length === 0">当前目录为空</p>
            </div>

          </div>

          <div
            class="transfer-pane-resizer"
            role="separator"
            tabindex="0"
            aria-label="调整系统目录和设备目录宽度"
            aria-orientation="vertical"
            :aria-valuenow="Math.round(transferPaneSplitPercent)"
            aria-valuemin="0"
            aria-valuemax="100"
            title="拖拽调整左右宽度"
            @pointerdown="startTransferPaneResize"
            @keydown="onTransferPaneResizerKeydown"
          >
            <span aria-hidden="true"></span>
          </div>

          <div class="file-pane device-pane">
            <div class="file-pane-head">
              <strong>设备目录</strong>
              <div class="file-pane-actions">
                <button type="button" class="transfer-tool-btn" :disabled="!canCopyRemoteToLocal" title="复制到对方（电脑）" aria-label="复制到对方（电脑）" @click="copyRemoteToLocal">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.copyToComputer" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="!canPreviewSelectedRemoteText" title="预览" aria-label="预览" @click="openTextPreview('remote', selectedRemotePath)">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.preview" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn" :disabled="browsingRemote" title="刷新" aria-label="刷新" @click="loadRemoteEntries()">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.refresh" :key="path" :d="path" />
                  </svg>
                </button>
                <button type="button" class="transfer-tool-btn danger" :disabled="!canDeleteRemote" title="删除" aria-label="删除" @click="requestDeleteSelectedRemoteEntry">
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in transferToolIconPaths.delete" :key="path" :d="path" />
                  </svg>
                </button>
              </div>
            </div>
            <div class="file-path-bar mono">{{ remoteBrowserPath }}</div>

            <div v-if="!selectedSerial" class="file-tree-list file-tree-list-empty">
              <p class="empty-tree-state">请先选择设备后浏览目录</p>
            </div>
            <p class="helper" v-else-if="browsingRemote">目录读取中...</p>
            <p class="helper" v-else-if="remoteBrowseError">{{ remoteBrowseError }}</p>

            <div
              v-else
              class="file-tree-list"
              data-allow-app-context-menu="true"
              @pointerdown="onTransferListPointerDown($event, 'remote')"
              @contextmenu.prevent="openTransferListContextMenu($event, 'remote')"
            >
              <div class="file-tree-header">
                <button
                  type="button"
                  class="file-tree-sort-btn file-tree-sort-name"
                  :class="{ active: remoteEntrySort.key === 'name' }"
                  :aria-pressed="remoteEntrySort.key === 'name'"
                  :aria-label="fileEntrySortButtonLabel(remoteEntrySort, 'name', '名称')"
                  :title="fileEntrySortButtonLabel(remoteEntrySort, 'name', '名称')"
                  @click.stop="toggleFileEntrySort('remote', 'name')"
                >
                  <span>名称</span>
                  <svg class="file-tree-sort-indicator" :class="{ visible: remoteEntrySort.key === 'name' }" viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(remoteEntrySort, 'name')]" :key="path" :d="path" />
                  </svg>
                </button>
                <span class="file-entry-meta">
                  <button
                    type="button"
                    class="file-tree-sort-btn file-entry-size"
                    :class="{ active: remoteEntrySort.key === 'size' }"
                    :aria-pressed="remoteEntrySort.key === 'size'"
                    :aria-label="fileEntrySortButtonLabel(remoteEntrySort, 'size', '大小')"
                    :title="fileEntrySortButtonLabel(remoteEntrySort, 'size', '大小')"
                    @click.stop="toggleFileEntrySort('remote', 'size')"
                  >
                    <span>大小</span>
                    <svg class="file-tree-sort-indicator" :class="{ visible: remoteEntrySort.key === 'size' }" viewBox="0 0 24 24" aria-hidden="true">
                      <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(remoteEntrySort, 'size')]" :key="path" :d="path" />
                    </svg>
                  </button>
                  <button
                    type="button"
                    class="file-tree-sort-btn file-entry-created"
                    :class="{ active: remoteEntrySort.key === 'date' }"
                    :aria-pressed="remoteEntrySort.key === 'date'"
                    :aria-label="fileEntrySortButtonLabel(remoteEntrySort, 'date', '日期')"
                    :title="fileEntrySortButtonLabel(remoteEntrySort, 'date', '日期')"
                    @click.stop="toggleFileEntrySort('remote', 'date')"
                  >
                    <span>日期</span>
                    <svg class="file-tree-sort-indicator" :class="{ visible: remoteEntrySort.key === 'date' }" viewBox="0 0 24 24" aria-hidden="true">
                      <path v-for="path in fileEntrySortIconPaths[fileEntrySortIconDirection(remoteEntrySort, 'date')]" :key="path" :d="path" />
                    </svg>
                  </button>
                </span>
              </div>
              <button type="button" class="file-parent-row mono" :disabled="atSdcardRoot || browsingRemote" @dblclick="goParentDirectory">
                ...
              </button>
              <div
                v-for="entry in sortedRemoteEntries"
                :key="entry.path"
                class="file-tree-item"
                data-allow-app-context-menu="true"
                role="button"
                tabindex="0"
                :class="{ active: entry.path === selectedRemotePath }"
                :data-entry-path="entry.path"
                @click="selectRemoteEntry(entry)"
                @dblclick="enterRemoteDirectory(entry)"
                @keydown.enter.prevent="enterRemoteDirectory(entry)"
              >
                <span class="file-entry-main">
                  <svg class="file-entry-icon" :class="fileEntryIconName(entry)" viewBox="0 0 24 24" aria-hidden="true">
                    <path v-for="path in fileEntryIconPaths[fileEntryIconName(entry)]" :key="path" :d="path" />
                  </svg>
                  <span class="file-entry-name mono">{{ entry.name }}</span>
                </span>
                <span class="file-entry-meta">
                  <span class="file-entry-size mono">{{ formatEntrySize(entry) }}</span>
                  <span class="file-entry-created mono">{{ formatEntryCreatedAt(entry) }}</span>
                </span>
              </div>
              <p class="empty-tree-state" v-if="remoteEntries.length === 0">当前目录为空</p>
            </div>

          </div>
        </div>
      </section>

      <Teleport to="body">
        <div
          v-if="transferContextMenu"
          class="transfer-context-menu"
          data-allow-app-context-menu="true"
          :style="{ left: `${transferContextMenu.x}px`, top: `${transferContextMenu.y}px` }"
          role="menu"
          @click.stop
          @contextmenu.prevent
        >
          <button
            v-for="item in transferContextMenuItems"
            :key="item.action"
            type="button"
            class="transfer-context-menu-item"
            :class="{ danger: item.danger }"
            :disabled="item.disabled"
            role="menuitem"
            @click="runTransferContextMenuAction(item.action)"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path v-for="path in transferToolIconPaths[item.icon]" :key="path" :d="path" />
            </svg>
            <span>{{ item.label }}</span>
          </button>
        </div>
      </Teleport>

      <section v-if="settingsPanelOpen" v-show="activeWorkspaceTabKey === 'settings'" class="settings-dock">
        <SettingsPage />
      </section>

      <section
        v-if="textPreviewOpen"
        v-show="activeWorkspaceTabKey === 'text-preview'"
        class="text-preview-dock"
        :class="{ 'html-preview-dock': textPreviewMode === 'html' }"
      >
        <header class="text-preview-head">
          <div>
            <strong>{{ textPreviewMeta?.name || t("mirror.textPreview") }}</strong>
            <span>{{ textPreviewSource === "local" ? "系统文件" : "设备文件" }} · {{ textPreviewMeta ? formatBytes(textPreviewMeta.sizeBytes) : "读取中" }}</span>
          </div>
          <div class="text-preview-actions">
            <div v-if="textPreviewMode === 'text'" class="text-preview-search">
              <input
                v-model="textPreviewSearchQuery"
                type="search"
                placeholder="搜索"
                @input="onTextPreviewSearchInput"
                @keydown.enter.prevent="onTextPreviewSearchKeydown"
              />
              <button
                type="button"
                class="tiny-btn text-preview-search-toggle"
                :class="{ active: textPreviewSearchCaseSensitive }"
                title="区分大小写"
                @click="toggleTextPreviewSearchOption('case')"
              >Aa</button>
              <button
                type="button"
                class="tiny-btn text-preview-search-toggle"
                :class="{ active: textPreviewSearchWholeWord }"
                title="整词匹配"
                @click="toggleTextPreviewSearchOption('whole')"
              >
                <svg class="whole-word-icon" viewBox="0 0 28 20" aria-hidden="true">
                  <path class="whole-word-boundary" d="M3.5 5.5v9" />
                  <path class="whole-word-boundary" d="M24.5 5.5v9" />
                  <path class="whole-word-letter" d="M7.2 13.8V9.2c0-1.45 1.05-2.42 2.75-2.42 1.62 0 2.58.86 2.58 2.28v4.74" />
                  <path class="whole-word-letter" d="M12.52 10.32H9.48c-1.6 0-2.56.68-2.56 1.83 0 1.08.86 1.76 2.12 1.76 1.34 0 2.36-.76 3.48-1.93" />
                  <path class="whole-word-letter" d="M16.4 6.2v7.6" />
                  <path class="whole-word-letter" d="M16.4 9.18c.74-1.46 1.72-2.18 2.95-2.18 1.8 0 3.05 1.4 3.05 3.48 0 2.08-1.26 3.48-3.05 3.48-1.26 0-2.24-.72-2.95-2.16" />
                  <path class="whole-word-underline" d="M6.1 16.5h15.8" />
                </svg>
              </button>
              <button
                type="button"
                class="tiny-btn text-preview-search-toggle"
                :class="{ active: textPreviewSearchRegex }"
                title="正则匹配"
                @click="toggleTextPreviewSearchOption('regex')"
              >.*</button>
              <span>{{ textPreviewSearchLabel }}</span>
              <button type="button" class="tiny-btn text-preview-search-btn" :disabled="!textPreviewSearchNeedle || textPreviewSearchSearching" title="上一个" @click="searchTextPreview(-1)">↑</button>
              <button type="button" class="tiny-btn text-preview-search-btn" :disabled="!textPreviewSearchNeedle || textPreviewSearchSearching" title="下一个" @click="searchTextPreview(1)">↓</button>
            </div>
            <button type="button" class="tiny-btn" @click="copyTextPreviewSelection">复制</button>
            <button type="button" class="tiny-btn" @click="closeTextPreview">关闭</button>
          </div>
        </header>
        <div
          v-if="textPreviewMode === 'text'"
          ref="textPreviewBodyRef"
          class="text-preview-body"
          @scroll="onTextPreviewScroll"
        >
          <div class="text-preview-spacer" :style="{ height: `${textPreviewTotalHeight}px` }">
            <div
              v-for="line in textPreviewVisibleLines"
              :key="line.id"
              class="text-preview-line"
              :class="{ 'search-match': line.searchMatch, 'search-active': line.searchActive }"
              :style="{ transform: `translateY(${line.top}px)` }"
            >
              <span class="text-preview-line-number">{{ line.lineNumber }}</span>
              <code>{{ line.text || " " }}</code>
            </div>
          </div>
          <div v-if="textPreviewLoading && textPreviewLines.length === 0" class="text-preview-status">正在打开...</div>
          <div v-else-if="textPreviewError" class="text-preview-status error">{{ textPreviewError }}</div>
          <div v-else-if="textPreviewLines.length === 0" class="text-preview-status">暂无可显示文本</div>
        </div>
        <div
          v-else-if="textPreviewMode === 'html'"
          ref="textPreviewBodyRef"
          class="html-preview-body"
          @scroll="onTextPreviewScroll"
        >
          <article v-if="textPreviewLines.length > 0" class="html-preview-content" v-html="textPreviewHtmlContent"></article>
          <div v-if="textPreviewLoading && textPreviewLines.length === 0" class="text-preview-status">正在打开...</div>
          <div v-else-if="textPreviewError" class="text-preview-status error">{{ textPreviewError }}</div>
          <div v-else-if="textPreviewLines.length === 0" class="text-preview-status">暂无可显示内容</div>
        </div>
        <div
          v-else
          class="pdf-preview-body"
          :class="{ 'image-preview-body': textPreviewMode === 'image', dragging: imagePreviewDragging, zoomed: textPreviewMode === 'image' && imagePreviewScale > 1 }"
          @wheel="onTextPreviewImageWheel"
          @pointerdown="onTextPreviewImagePointerDown"
          @pointermove="onTextPreviewImagePointerMove"
          @pointerup="onTextPreviewImagePointerUp"
          @pointercancel="onTextPreviewImagePointerUp"
          @dblclick="resetImagePreviewView"
        >
          <iframe v-if="textPreviewMode === 'pdf' && textPreviewAssetUrl" :src="textPreviewAssetUrl" title="PDF 预览"></iframe>
          <video
            v-else-if="textPreviewMode === 'video' && textPreviewAssetUrl"
            :src="textPreviewAssetUrl"
            controls
            preload="metadata"
            @error="onTextPreviewVideoError"
          ></video>
          <img
            v-else-if="textPreviewMode === 'image' && textPreviewAssetUrl"
            :src="textPreviewAssetUrl"
            :style="{ transform: imagePreviewTransform }"
            alt="图片预览"
            draggable="false"
            @load="resetImagePreviewView"
            @error="onTextPreviewImageError"
          />
          <div v-if="textPreviewLoading" class="text-preview-status">正在打开...</div>
          <div v-else-if="textPreviewError" class="text-preview-status error">{{ textPreviewError }}</div>
        </div>
      </section>
    </section>

    <div v-if="deleteConfirmTarget" class="delete-confirm-layer">
      <section class="delete-confirm-dialog">
        <h3>{{ deleteConfirmTitle }}</h3>
        <p>此操作不可恢复，请确认是否删除：</p>
        <div class="delete-confirm-path mono">{{ deleteConfirmTarget.path }}</div>
        <div class="delete-confirm-actions">
          <button type="button" class="btn btn-ghost" @click="cancelDeleteConfirm">取消</button>
          <button type="button" class="btn btn-danger" :disabled="quickBusy" @click="confirmDeleteSelectedEntry">确认删除</button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.mirror-layout {
  position: relative;
  display: block;
  gap: 0;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.mirror-file-drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 60;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(15 23 42 / 0.36);
  pointer-events: none;
}

.mirror-file-drop-panel {
  display: grid;
  gap: 6px;
  width: min(360px, 100%);
  border: 1px solid var(--primary);
  border-radius: 8px;
  background: var(--surface);
  color: var(--text-main);
  padding: 16px;
  text-align: center;
  box-shadow: 0 18px 45px rgb(15 23 42 / 0.22);
}

.mirror-file-drop-panel strong {
  font-size: 15px;
}

.mirror-file-drop-panel span {
  color: var(--text-sub);
  font-size: 12px;
  line-height: 1.5;
}

.mirror-file-drop-overlay.busy .mirror-file-drop-panel {
  border-color: var(--control-border);
}

.field-row {
  display: flex;
  gap: 6px;
}

.field-row .input {
  min-width: 0;
  flex: 1;
}

.delete-confirm-layer {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: grid;
  place-items: center;
  padding: 16px;
  background: rgb(15 23 42 / 0.36);
}

.delete-confirm-dialog {
  width: min(360px, 100%);
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: var(--shadow);
  padding: 12px;
}

.delete-confirm-dialog h3 {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 15px;
}

.delete-confirm-dialog p {
  margin: 0 0 8px;
  color: var(--text-sub);
  font-size: 12px;
}

.delete-confirm-path {
  max-height: 76px;
  overflow: auto;
  border: 1px solid var(--border);
  background: #f8fafc;
  color: var(--control-text);
  padding: 7px;
  font-size: 11px;
  overflow-wrap: anywhere;
}

.delete-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 10px;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 32px;
}

.panel-head h2 {
  margin: 0;
  font-size: 16px;
}

.transfer-dock {
  position: absolute;
  inset: 50px 8px 8px;
  z-index: 5;
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  gap: 0;
  min-height: 0;
  border: 1px solid var(--dock-border);
  border-radius: 8px;
  background: var(--dock-bg);
  box-shadow: none;
  padding: 8px;
  backdrop-filter: none;
}

.settings-dock {
  position: absolute;
  inset: 50px 8px 8px;
  z-index: 5;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--dock-border);
  border-radius: 8px;
  background: var(--dock-bg);
}

.transfer-modal-body {
  --transfer-pane-columns: minmax(300px, calc(50% - 4px)) 8px minmax(300px, 1fr);
  display: grid;
  grid-template-columns: var(--transfer-pane-columns);
  min-height: 0;
  height: 100%;
  overflow: hidden;
}

.transfer-modal-body.resizing {
  cursor: col-resize;
  user-select: none;
}

.terminal-dock {
  position: absolute;
  inset: 50px 8px 8px;
  z-index: 4;
  display: grid;
  grid-template-columns: minmax(210px, 250px) minmax(0, 1fr);
  gap: 10px;
  min-height: 0;
  border: 1px solid var(--dock-border);
  border-radius: 8px;
  background: var(--dock-bg-soft);
  box-shadow: none;
  padding: 8px;
  backdrop-filter: none;
}

.terminal-dock.terminal-dock-collapsed {
  grid-template-columns: minmax(0, 1fr);
}

.screenshot-editor-dock {
  position: absolute;
  inset: 50px 8px 8px;
  z-index: 6;
  display: grid;
  grid-template-rows: max-content minmax(0, 1fr);
  min-height: 0;
  border: 1px solid var(--dock-border);
  border-radius: 8px;
  background: var(--dock-bg);
  padding: 8px;
}

.screenshot-editor-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 8px;
  min-width: 0;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--border);
}

.screenshot-editor-tools,
.screenshot-editor-controls {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.screenshot-editor-tools {
  overflow-x: auto;
  scrollbar-width: thin;
}

.screenshot-tool-btn {
  flex: 0 0 auto;
  height: 30px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--control-bg);
  color: var(--control-text);
  cursor: pointer;
}

.screenshot-tool-btn:focus,
.screenshot-tool-btn:focus-visible,
.screenshot-color-control input:focus,
.screenshot-color-control input:focus-visible,
.screenshot-width-control input:focus,
.screenshot-width-control input:focus-visible {
  outline: none;
}

.screenshot-tool-btn {
  display: grid;
  place-items: center;
  width: 30px;
  padding: 0;
}

.screenshot-tool-btn svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.screenshot-tool-btn.undo-redo svg {
  width: 18px;
  height: 18px;
  fill: currentColor;
  stroke: none;
}

.screenshot-tool-btn.active,
.screenshot-tool-btn:hover {
  border-color: var(--primary);
  background: var(--primary-soft);
  color: var(--primary);
}

.screenshot-tool-btn.primary {
  border-color: var(--primary);
  background: var(--primary);
  color: #fff;
}

.screenshot-tool-btn.primary:hover:not(:disabled) {
  border-color: var(--primary-hover);
  background: var(--primary-hover);
  color: #fff;
}

.screenshot-tool-btn:disabled {
  cursor: not-allowed;
  opacity: 0.42;
}

.screenshot-color-control {
  position: relative;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border: 1px solid #dbe3ef;
  border-radius: 6px;
  background: #fff;
  overflow: hidden;
  cursor: pointer;
}

.screenshot-color-control input {
  position: absolute;
  inset: 0;
  opacity: 0;
  cursor: pointer;
}

.screenshot-color-control span {
  width: 18px;
  height: 18px;
  border: 1px solid rgb(15 23 42 / 0.2);
  border-radius: 999px;
}

.screenshot-width-control {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #475569;
  font-size: 12px;
  white-space: nowrap;
}

.screenshot-width-control input {
  width: 86px;
}

.screenshot-editor-canvas-wrap {
  position: relative;
  display: grid;
  place-items: center;
  min-height: 0;
  overflow: auto;
  padding: 10px;
  background: #e2e8f0;
}

.screenshot-editor-canvas {
  display: block;
  max-width: 100%;
  max-height: 100%;
  width: auto;
  height: auto;
  background: #fff;
  box-shadow: 0 8px 28px rgb(15 23 42 / 0.18);
  touch-action: none;
  cursor: crosshair;
}

.screenshot-editor-canvas.tool-text {
  cursor: text;
}

.screenshot-editor-canvas.tool-eraser {
  cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='28' height='28' viewBox='0 0 28 28'%3E%3Cpath d='M5.5 18.5 15.8 8.2a2.8 2.8 0 0 1 4 0l2 2a2.8 2.8 0 0 1 0 4L13.5 22.5h-5l-3-3a1.4 1.4 0 0 1 0-1Z' fill='%23ffffff' stroke='%230f172a' stroke-width='1.6' stroke-linejoin='round'/%3E%3Cpath d='M14 10 20 16' stroke='%230f172a' stroke-width='1.4' stroke-linecap='round'/%3E%3Cpath d='M12.8 22.5H23' stroke='%230f172a' stroke-width='1.6' stroke-linecap='round'/%3E%3C/svg%3E") 8 20, cell;
}

.screenshot-editor-canvas.tool-picker {
  cursor: copy;
}

.screenshot-picker-popover {
  position: absolute;
  z-index: 3;
  display: flex;
  align-items: stretch;
  gap: 8px;
  width: 220px;
  border: 1px solid rgb(15 23 42 / 0.18);
  border-radius: 6px;
  background: rgb(255 255 255 / 0.96);
  padding: 7px;
  box-shadow: 0 8px 22px rgb(15 23 42 / 0.18);
  pointer-events: none;
}

.screenshot-picker-popover img {
  width: 88px;
  height: 88px;
  border: 1px solid rgb(15 23 42 / 0.16);
  border-radius: 4px;
  image-rendering: pixelated;
}

.screenshot-picker-meta {
  display: grid;
  align-content: center;
  gap: 5px;
  min-width: 0;
}

.screenshot-picker-meta span {
  width: 18px;
  height: 18px;
  border: 1px solid rgb(15 23 42 / 0.22);
  border-radius: 4px;
}

.screenshot-picker-meta code {
  color: #0f172a;
  font-size: 12px;
  line-height: 1.2;
  white-space: nowrap;
}

.screenshot-picker-meta kbd {
  width: max-content;
  max-width: 100%;
  border: 1px solid rgb(15 23 42 / 0.16);
  border-bottom-color: rgb(15 23 42 / 0.28);
  border-radius: 4px;
  background: #f8fafc;
  color: #475569;
  padding: 2px 5px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  line-height: 1.2;
  white-space: nowrap;
}

.screenshot-text-input {
  position: absolute;
  z-index: 2;
  min-width: 80px;
  min-height: 1.25em;
  max-width: min(360px, calc(100% - 20px));
  border: 0;
  border-radius: 0;
  background: transparent;
  padding: 0;
  font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-weight: 600;
  line-height: 1.25;
  outline: none;
  overflow: hidden;
  resize: none;
  box-shadow: none;
  white-space: pre-wrap;
  caret-color: currentColor;
}

.terminal-command-panel {
  border-right: 1px solid var(--border);
  padding-right: 10px;
}

.command-picker-panel {
  display: grid;
  grid-template-rows: max-content minmax(0, 1fr);
  gap: 7px;
  min-width: 0;
  min-height: 0;
}

.command-picker-tabs {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  border: 1px solid var(--control-border);
  border-radius: 7px;
  overflow: hidden;
}

.command-picker-tabs button {
  height: 28px;
  border: 0;
  border-right: 1px solid var(--control-border);
  background: var(--control-bg);
  color: var(--text-sub);
  font-size: 11px;
  cursor: pointer;
}

.command-picker-tabs button:last-child {
  border-right: 0;
}

.command-picker-tabs button:hover,
.command-picker-tabs button.active {
  background: var(--primary);
  color: #fff;
}

.command-picker-list {
  display: grid;
  align-content: start;
  gap: 7px;
  min-height: 0;
  overflow: auto;
  scrollbar-gutter: stable;
  padding-right: 2px;
}

.command-picker-item {
  display: grid;
  gap: 3px;
  border: 1px solid var(--control-border);
  border-radius: 7px;
  background: var(--control-bg);
  color: var(--control-text);
  padding: 8px;
  text-align: left;
  cursor: pointer;
}

.command-picker-item:hover,
.command-picker-item.active {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.command-picker-item strong {
  font-size: 12px;
}

.command-picker-item span {
  color: var(--text-sub);
  font-size: 11px;
}

.command-picker-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.command-picker-title em {
  flex: 0 0 auto;
  border-radius: 999px;
  background: #eef4ff;
  color: var(--primary);
  padding: 1px 5px;
  font-size: 10px;
  font-style: normal;
  font-weight: 700;
}

.command-picker-item code {
  min-width: 0;
  overflow: hidden;
  color: #667085;
  font-size: 10px;
  line-height: 1.35;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.command-picker-item:hover code,
.command-picker-item.active code {
  color: #2563eb;
}

.command-empty {
  display: grid;
  place-items: center;
  min-height: 120px;
  color: var(--text-sub);
  font-size: 12px;
}

.terminal-pane {
  display: grid;
  grid-template-rows: max-content minmax(0, 1fr);
  gap: 8px;
  min-width: 0;
  min-height: 0;
}

.terminal-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-height: 30px;
  color: var(--text-sub);
  font-size: 11px;
}

.terminal-toolbar-left,
.terminal-toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.terminal-toolbar-left span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-icon-btn {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border: 1px solid var(--control-border);
  border-radius: 6px;
  background: var(--control-bg);
  color: var(--control-text);
  font-size: 17px;
  line-height: 1;
  cursor: pointer;
}

.terminal-icon-btn:hover {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.terminal-icon-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.terminal-host {
  display: grid;
  min-height: 0;
  border: 1px solid #111827;
  border-radius: 8px;
  background: #0f172a;
  overflow: hidden;
  padding: 8px 6px 10px;
}

.terminal-host :deep(.xterm) {
  height: 100%;
  min-height: 0;
}

.terminal-host :deep(.xterm-viewport) {
  overflow-y: auto;
  scrollbar-gutter: stable;
  padding-bottom: 4px;
}

@media (max-width: 760px) {
  .terminal-dock {
    grid-template-columns: minmax(160px, 190px) minmax(0, 1fr);
  }

  .terminal-dock.terminal-dock-collapsed {
    grid-template-columns: minmax(0, 1fr);
  }

}

.file-pane {
  display: grid;
  grid-template-rows: max-content max-content minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  padding: 10px 0 0;
}

.system-pane {
  padding-right: 10px;
}

.device-pane {
  padding-left: 10px;
}

.transfer-pane-resizer {
  position: relative;
  display: grid;
  place-items: center;
  min-width: 8px;
  width: 8px;
  height: 100%;
  border: 0;
  background: transparent;
  color: var(--border);
  padding: 0;
  cursor: col-resize;
  touch-action: none;
}

.transfer-pane-resizer::before {
  content: "";
  width: 1px;
  height: 100%;
  background: currentColor;
}

.transfer-pane-resizer span {
  position: absolute;
  width: 4px;
  height: 42px;
  border-radius: 999px;
  background: var(--control-border);
  opacity: 0;
  transition: opacity 0.15s ease, background 0.15s ease;
}

.transfer-pane-resizer:hover,
.transfer-pane-resizer:focus-visible,
.transfer-modal-body.resizing .transfer-pane-resizer {
  color: var(--primary-border);
  outline: none;
}

.transfer-pane-resizer:hover span,
.transfer-pane-resizer:focus-visible span,
.transfer-modal-body.resizing .transfer-pane-resizer span {
  background: var(--primary);
  opacity: 1;
}

.file-pane-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 7px;
}

.file-pane-head strong {
  color: var(--text-main);
  font-size: 12px;
}

.file-path-bar {
  min-width: 0;
  margin-bottom: 7px;
  border: 1px solid var(--border);
  background: #f8fafc;
  color: var(--text-sub);
  padding: 5px 7px;
  font-size: 11px;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-path-bar:hover {
  white-space: normal;
  overflow-wrap: anywhere;
}

.file-parent-row {
  position: sticky;
  top: var(--file-tree-header-height);
  z-index: 1;
  width: 100%;
  min-width: var(--file-tree-min-width);
  min-height: 28px;
  border: 0;
  border-bottom: 1px solid var(--border);
  background: var(--surface-alt);
  color: var(--control-text);
  padding: 4px 8px;
  text-align: left;
  box-sizing: border-box;
  cursor: pointer;
}

.file-parent-row:hover:not(:disabled) {
  background: var(--primary-soft);
  color: var(--primary);
}

.file-parent-row:disabled {
  color: var(--text-sub);
  cursor: default;
  opacity: 0.55;
}

.file-pane-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 5px;
  flex-shrink: 0;
}

.transfer-tool-btn {
  display: inline-grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--control-border);
  border-radius: 6px;
  background: var(--control-bg);
  color: var(--text-sub);
  padding: 0;
  cursor: pointer;
}

.transfer-tool-btn svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.transfer-tool-btn:hover:not(:disabled) {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.transfer-tool-btn.danger:not(:disabled) {
  border-color: #fecaca;
  color: #b91c1c;
}

.transfer-tool-btn.danger:hover:not(:disabled) {
  border-color: var(--danger);
  background: #fef2f2;
}

.transfer-tool-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.tiny-btn-danger:not(:disabled) {
  border-color: #fecaca;
  color: #b91c1c;
}

.tiny-btn-danger:hover:not(:disabled) {
  border-color: var(--danger);
  background: #fef2f2;
}

.file-tree-list {
  --file-tree-min-width: 520px;
  --file-tree-header-height: 27px;
  --file-entry-size-width: 76px;
  --file-entry-date-width: 142px;
  position: relative;
  min-height: 0;
  height: 100%;
  overflow: auto;
  scrollbar-gutter: stable;
  display: grid;
  align-content: start;
  border: 1px solid var(--border);
  scrollbar-width: thin;
}

.file-tree-list::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.file-tree-list::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: #cbd5e1;
}

.file-tree-list::-webkit-scrollbar-track {
  background: #f8fafc;
}

.file-tree-list-empty {
  align-content: stretch;
  overflow: hidden;
}

.empty-tree-state {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  margin: 0;
  color: var(--text-sub);
  font-size: 12px;
}

.file-tree-header {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  min-width: var(--file-tree-min-width);
  min-height: var(--file-tree-header-height);
  border-bottom: 1px solid var(--border);
  background: var(--surface-alt);
  color: var(--text-sub);
  padding: 5px 7px;
  font-size: 10.5px;
  box-sizing: border-box;
}

.file-tree-sort-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  border: 0;
  background: transparent;
  color: inherit;
  padding: 0;
  font: inherit;
  line-height: 1.2;
  cursor: pointer;
}

.file-tree-sort-btn:hover,
.file-tree-sort-btn.active {
  color: var(--primary);
}

.file-tree-sort-name {
  min-width: 0;
  flex: 1 1 auto;
  justify-content: flex-start;
  text-align: left;
}

.file-tree-header .file-entry-meta {
  color: inherit;
}

.file-entry-meta .file-tree-sort-btn {
  width: 100%;
  justify-content: flex-end;
  text-align: right;
}

.file-tree-sort-indicator {
  width: 14px;
  height: 14px;
  flex: 0 0 14px;
  color: var(--primary);
  fill: none;
  opacity: 0;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.file-tree-sort-indicator.visible {
  opacity: 1;
}

.file-tree-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  min-width: var(--file-tree-min-width);
  min-height: 30px;
  border: 0;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  color: var(--control-text);
  padding: 5px 7px;
  text-align: left;
  cursor: pointer;
  box-sizing: border-box;
}

.file-tree-item:hover,
.file-tree-item.active {
  background: var(--primary-soft);
  color: var(--primary);
}

.file-entry-main {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
  flex: 1 1 auto;
}

.file-entry-icon {
  width: 16px;
  height: 16px;
  flex: 0 0 16px;
  color: var(--text-sub);
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.file-entry-icon.folder {
  color: #d97706;
}

.file-entry-icon.apk {
  color: #16a34a;
}

.file-entry-icon.image {
  color: #0ea5e9;
}

.file-entry-icon.video {
  color: #db2777;
}

.file-entry-icon.pdf {
  color: #dc2626;
}

.file-entry-icon.code {
  color: #7c3aed;
}

.file-entry-icon.text {
  color: #2563eb;
}

.file-entry-icon.archive {
  color: #a16207;
}

.file-entry-icon.audio {
  color: #0891b2;
}

.file-tree-item:hover .file-entry-icon,
.file-tree-item.active .file-entry-icon {
  color: var(--primary);
}

.file-entry-name {
  min-width: 0;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-entry-meta {
  display: grid;
  grid-template-columns: var(--file-entry-size-width) var(--file-entry-date-width);
  gap: 8px;
  flex: 0 0 auto;
  align-items: center;
  color: var(--text-sub);
  font-size: 10.5px;
  text-align: right;
}

.file-entry-size,
.file-entry-created {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

:global(.transfer-context-menu) {
  position: fixed;
  z-index: 2147483647;
  display: grid;
  min-width: 180px;
  padding: 6px;
  border: 1px solid var(--control-border);
  border-radius: 8px;
  background: var(--surface);
  box-shadow: 0 16px 40px rgba(15, 23, 42, 0.18);
}

:global(.transfer-context-menu-item) {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  min-height: 32px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--control-text);
  padding: 0 9px;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}

:global(.transfer-context-menu-item svg) {
  width: 15px;
  height: 15px;
  flex: 0 0 15px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

:global(.transfer-context-menu-item:hover:not(:disabled)) {
  background: var(--primary-soft);
  color: var(--primary);
}

:global(.transfer-context-menu-item.danger:not(:disabled)) {
  color: #b91c1c;
}

:global(.transfer-context-menu-item.danger:hover:not(:disabled)) {
  background: #fef2f2;
  color: var(--danger);
}

:global(.transfer-context-menu-item:disabled) {
  opacity: 0.45;
  cursor: not-allowed;
}

.text-preview-dock {
  position: absolute;
  inset: 50px 8px 8px;
  z-index: 4;
  display: grid;
  grid-template-rows: max-content minmax(0, 1fr);
  min-height: 0;
  border: 1px solid rgb(148 163 184 / 0.32);
  border-radius: 8px;
  background: rgb(248 250 252 / 0.96);
  padding: 8px;
}

.text-preview-dock.html-preview-dock {
  background: transparent;
}

.text-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 6px;
}

.text-preview-head > div:first-child {
  display: grid;
  gap: 2px;
  min-width: 0;
}

.text-preview-head strong {
  color: var(--text-main);
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.text-preview-head span {
  color: var(--text-sub);
  font-size: 11px;
}

.text-preview-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 0 0 auto;
}

.text-preview-search {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.text-preview-search input {
  width: 132px;
  height: 28px;
  border: 1px solid var(--control-border);
  border-radius: 6px;
  background: var(--control-bg);
  color: var(--control-text);
  padding: 0 8px;
  font-size: 12px;
  outline: none;
}

.text-preview-search input:focus {
  border-color: var(--primary-border);
  background: #fff;
}

.text-preview-search span {
  min-width: 42px;
  color: var(--text-sub);
  font-size: 11px;
  text-align: center;
  white-space: nowrap;
}

.text-preview-search-btn {
  width: 28px;
  padding: 0;
}

.text-preview-search-toggle {
  width: 30px;
  padding: 0;
  font-weight: 700;
}

.text-preview-search-toggle.active {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.whole-word-icon {
  width: 22px;
  height: 16px;
  display: block;
}

.whole-word-boundary,
.whole-word-letter,
.whole-word-underline {
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.whole-word-boundary {
  stroke-width: 1.45;
  opacity: 0.62;
}

.whole-word-letter {
  stroke-width: 1.55;
}

.whole-word-underline {
  stroke-width: 1.65;
}

.text-preview-body {
  position: relative;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--border);
  background: #0f172a;
  color: #dbeafe;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 20px;
  user-select: text;
}

.text-preview-spacer {
  position: relative;
  min-height: 100%;
}

.text-preview-line {
  position: absolute;
  inset: 0 0 auto;
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  height: 20px;
  min-width: max-content;
}

.text-preview-line.search-match code {
  background: rgb(250 204 21 / 0.18);
  color: #fef3c7;
}

.text-preview-line.search-active code {
  background: rgb(59 130 246 / 0.36);
  color: #ffffff;
  box-shadow: inset 3px 0 0 var(--primary);
}

.text-preview-line-number {
  position: sticky;
  left: 0;
  z-index: 1;
  border-right: 1px solid rgb(148 163 184 / 0.24);
  background: #111827;
  color: #64748b;
  padding: 0 8px;
  text-align: right;
  user-select: none;
}

.text-preview-line code {
  display: block;
  padding: 0 10px;
  color: inherit;
  white-space: pre;
  user-select: text;
}

.text-preview-status {
  position: sticky;
  left: 0;
  bottom: 0;
  width: max-content;
  min-width: 100%;
  background: rgb(15 23 42 / 0.9);
  color: #93c5fd;
  padding: 3px 8px;
  font-size: 11px;
}

.text-preview-status.error {
  color: #fecaca;
}

.html-preview-body,
.pdf-preview-body {
  position: relative;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--border);
}

.html-preview-body {
  --html-preview-radius: 6px;
  border-radius: var(--html-preview-radius);
  background: transparent;
}

.pdf-preview-body {
  background: #fff;
}

.html-preview-content {
  max-width: 960px;
  margin: 0 auto;
  background: transparent;
  padding: 18px 22px 32px;
  color: #0f172a;
  line-height: 1.65;
  user-select: text;
}

.html-preview-content :deep(h1),
.html-preview-content :deep(h2),
.html-preview-content :deep(h3) {
  margin: 1.1em 0 0.55em;
  color: #0f172a;
  line-height: 1.25;
}

.html-preview-content :deep(p) {
  margin: 0.5em 0;
}

.html-preview-content :deep(section),
.html-preview-content :deep(article),
.html-preview-content :deep(div),
.html-preview-content :deep(main),
.html-preview-content :deep(aside),
.html-preview-content :deep(header),
.html-preview-content :deep(footer),
.html-preview-content :deep(nav) {
  border-radius: var(--html-preview-radius);
}

.html-preview-content :deep(img),
.html-preview-content :deep(video),
.html-preview-content :deep(canvas),
.html-preview-content :deep(table) {
  border-radius: var(--html-preview-radius);
  overflow: hidden;
}

.html-preview-content :deep(pre) {
  overflow: auto;
  border-radius: var(--html-preview-radius);
  background: #0f172a;
  color: #dbeafe;
  padding: 10px 12px;
}

.html-preview-content :deep(code) {
  border-radius: var(--html-preview-radius);
  background: #eef2ff;
  color: #1e3a8a;
  padding: 1px 4px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.html-preview-content :deep(pre code) {
  background: transparent;
  color: inherit;
  padding: 0;
}

.html-preview-content :deep(blockquote) {
  margin: 0.8em 0;
  border-left: 3px solid var(--primary);
  border-radius: var(--html-preview-radius);
  color: #475569;
  padding: 6px 10px;
}

.html-preview-content :deep(a) {
  color: var(--primary);
  text-decoration: underline;
}

.pdf-preview-body {
  display: grid;
  place-items: center;
  min-height: 0;
  padding: 10px;
}

.pdf-preview-body iframe {
  width: 100%;
  height: 100%;
  min-height: 0;
  border: 0;
  background: #fff;
}

.pdf-preview-body video {
  width: 100%;
  height: 100%;
  min-height: 0;
  background: #020617;
  outline: none;
}

.pdf-preview-body img {
  display: block;
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  box-shadow: 0 8px 28px rgb(15 23 42 / 0.14);
}

.image-preview-body {
  display: block;
  overflow: hidden;
  padding: 0;
  cursor: zoom-in;
  touch-action: none;
  user-select: none;
}

.image-preview-body img {
  position: absolute;
  left: 50%;
  top: 50%;
  width: auto;
  height: auto;
  max-width: calc(100% - 20px);
  max-height: calc(100% - 20px);
  transform-origin: center;
  will-change: transform;
  user-select: none;
  -webkit-user-drag: none;
  transition: box-shadow 0.12s ease;
}

.image-preview-body.zoomed {
  cursor: grab;
}

.image-preview-body.dragging {
  cursor: grabbing;
}

.image-preview-body.dragging img {
  transition: none;
}

.mirror-stage {
  position: absolute;
  inset: 0;
  display: block;
  min-width: 0;
  min-height: 0;
  border: 0;
  border-radius: 0;
  background: var(--page-bg);
  padding: 0;
  overflow: hidden;
  box-shadow: none;
  user-select: none;
  -webkit-user-select: none;
}

.mirror-tabbar {
  position: absolute;
  inset: 0 0 auto;
  z-index: 6;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  height: 42px;
  min-height: 42px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-solid);
  padding: 6px 8px;
  box-shadow: 0 1px 0 rgb(15 23 42 / 0.06);
}

.mirror-tab-list {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scroll-behavior: smooth;
  scrollbar-width: none;
}

.mirror-tab-list::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.mirror-tab {
  flex: 0 0 clamp(148px, 18vw, 220px);
  display: grid;
  grid-template-columns: minmax(0, 1fr) 22px;
  align-items: center;
  gap: 4px;
  min-width: 0;
  height: 30px;
  border: 1px solid var(--control-border);
  border-radius: 7px;
  background: var(--control-bg);
  color: var(--control-text);
  padding: 0 4px 0 8px;
}

.mirror-tab.active {
  border-color: var(--primary-border);
  background: var(--primary-soft);
  color: var(--primary);
}

.mirror-tab.running small {
  color: var(--success);
}

.mirror-tab.tool small {
  color: var(--primary);
}

.mirror-tab-main {
  display: grid;
  grid-template-columns: minmax(0, 1fr) max-content;
  align-items: center;
  gap: 8px;
  min-width: 0;
  height: 100%;
  border: 0;
  background: transparent;
  color: inherit;
  padding: 0;
  cursor: pointer;
  font: inherit;
  text-align: left;
}

.mirror-tab span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 700;
}

.mirror-tab small {
  color: var(--text-sub);
  font-size: 10px;
  white-space: nowrap;
}

.mirror-tab-close {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-sub);
  cursor: pointer;
  font-size: 15px;
  line-height: 1;
}

.mirror-tab-close:hover:not(:disabled) {
  background: rgb(15 23 42 / 0.08);
  color: var(--control-text);
}

.mirror-tab-close:disabled {
  cursor: not-allowed;
  opacity: 0.35;
}

.mirror-screen-stack {
  position: absolute;
  inset: 42px 0 0;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.mirror-screen-stack.empty {
  inset: 0;
}

.mirror-empty-shortcuts {
  position: absolute;
  left: 50%;
  top: 70%;
  display: grid;
  width: min(440px, calc(100% - 48px));
  transform: translate(-50%, -50%);
  gap: 12px;
  padding: 2px;
  color: var(--control-text);
}

.mirror-empty-shortcut-row {
  display: grid;
  grid-template-columns: minmax(120px, 1fr) auto;
  align-items: center;
  gap: 28px;
  min-height: 26px;
}

.mirror-empty-shortcut-row span {
  color: var(--text-sub);
  font-size: 14px;
  line-height: 1.3;
  white-space: nowrap;
}

.mirror-empty-shortcut-keys {
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
  min-width: 176px;
}

.mirror-empty-shortcut-keys kbd {
  display: inline-grid;
  place-items: center;
  min-width: 32px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid var(--control-border);
  border-radius: 5px;
  background: var(--control-bg);
  color: var(--text-main);
  box-shadow: var(--kbd-shadow);
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace;
  font-size: 12px;
  line-height: 1;
}

.mirror-empty-shortcut-keys em {
  color: var(--text-muted);
  font-style: normal;
  font-size: 12px;
  line-height: 1;
}

.screen-wrap {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border: 0;
  border-radius: 0;
  background: var(--surface-alt);
  touch-action: none;
  cursor: default;
  user-select: none;
  -webkit-user-select: none;
}

.screen-wrap.active {
  cursor: default;
}

.device-canvas {
  display: block;
  max-width: 100%;
  max-height: 100%;
  width: auto;
  height: auto;
  background: var(--surface-alt);
  image-rendering: auto;
  user-select: none;
  -webkit-user-select: none;
  -webkit-user-drag: none;
}

.screen-placeholder {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  gap: 6px;
  color: var(--text-main);
  text-align: center;
  pointer-events: none;
  user-select: none;
  -webkit-user-select: none;
}

.screen-placeholder strong {
  font-size: 22px;
}

.screen-placeholder span {
  max-width: 520px;
  color: var(--text-sub);
  overflow-wrap: anywhere;
}

.recording-float {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 5;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-height: 30px;
  border: 1px solid rgb(248 113 113 / 0.45);
  border-radius: 999px;
  background: rgb(15 23 42 / 0.74);
  color: #fff;
  padding: 5px 10px;
  font-size: 12px;
  line-height: 1;
  pointer-events: none;
}

.recording-float strong {
  font-size: 12px;
  font-weight: 800;
  letter-spacing: 0;
}

.recording-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: #ef4444;
  box-shadow: 0 0 0 3px rgb(239 68 68 / 0.18);
}

@media (max-width: 640px) {
  .mirror-layout {
    display: block;
  }

  .mirror-stage {
    inset: 0;
  }

  .transfer-modal-body {
    grid-template-columns: 1fr;
    height: 100%;
    overflow: auto;
  }

  .transfer-pane-resizer {
    display: none;
  }

  .terminal-dock {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(128px, 0.45fr) minmax(220px, 1fr);
  }

  .terminal-dock.terminal-dock-collapsed {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
  }

  .terminal-command-panel {
    border-right: 0;
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
  }

  .system-pane {
    border-bottom: 1px solid var(--border);
    padding-right: 0;
  }

  .device-pane {
    border-left: 0;
    padding-left: 0;
  }

  .mirror-empty-shortcuts {
    top: 70%;
    width: min(340px, calc(100% - 32px));
  }

  .mirror-empty-shortcut-row {
    grid-template-columns: 1fr;
    gap: 6px;
  }

  .mirror-empty-shortcut-keys {
    justify-content: flex-start;
    min-width: 0;
  }

  .screen-wrap {
    min-height: 360px;
  }
}
</style>

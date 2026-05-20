import { computed } from "vue";
import { storeToRefs } from "pinia";
import { useAdbStore, type AppLanguage, type AppTheme } from "./stores/adb";

interface MessageTree {
  [key: string]: string | MessageTree;
}

export const appLanguageOptions: Array<{ value: AppLanguage; label: string }> = [
  { value: "zh-CN", label: "中文" },
  { value: "en-US", label: "English" },
];

export const appThemeOptions: Array<{ value: AppTheme; labelKey: string }> = [
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" },
  { value: "system", labelKey: "settings.themeSystem" },
];

const messages: Record<AppLanguage, MessageTree> = {
  "zh-CN": {
    common: {
      appName: "DroidPilot 智控助手",
      close: "关闭",
      unavailable: "不可用",
      loading: "读取中",
      settings: "设置",
      preferences: "偏好设置",
      shortcuts: "快捷键",
      about: "关于",
      ready: "就绪",
      saving: "保存中",
      running: "运行中",
      preview: "预览",
    },
    app: {
      noDevice: "请选择设备",
      loadingDevices: "加载设备中...",
      refreshDevices: "刷新设备",
      noDevices: "暂无设备，请点击刷新",
      connecting: "连接中",
      connectWifi: "连接 Wi-Fi ADB",
      disconnectingDevice: "正在断开 {serial}",
      disconnectDevice: "断开 {serial}",
      minimize: "最小化",
      maximize: "放大",
      restore: "还原",
      close: "关闭",
      back: "返回",
      tasks: "多任务",
      power: "电源",
      nativeScreenshot: "设备截图",
      mirrorScreenshot: "投屏截图",
      needRunningMirrorTab: "需要切换到正在运行的投屏标签页",
      needAuthorizedDevice: "需要已授权设备",
      saveRecording: "保存录屏",
      stopRecording: "停止录屏",
      record: "录屏",
      fileTransfer: "文件传输",
      terminal: "快捷终端",
    },
    settings: {
      language: "语言",
      theme: "主题",
      themeLight: "浅色",
      themeDark: "深色",
      themeSystem: "跟随系统",
      defaultLocalDir: "默认本地保存目录",
      defaultLocalDirPlaceholder: "截图、录屏、获取文件保存目录",
      chooseDefaultLocalDir: "选择默认本地保存目录",
      defaultLocalDirNote: "截图和视频录制会直接存储到本机此目录。",
      defaultDevicePath: "默认设备路径",
      screenshotMode: "截图处理方式",
      screenshotModeSave: "仅传输",
      screenshotModeCopy: "仅复制",
      screenshotModeSaveCopy: "传输 + 复制",
      screenshotSource: "截图来源",
      screenshotSourceNative: "按原生截图",
      screenshotSourceMirror: "按投屏截图",
      screenshotSourceNote: "原生截图通过设备 screencap 获取；投屏截图截取当前标签页中的投屏画面。",
      pickerCopy: "取色复制",
      pickerCopyNote: "截图编辑页取色器悬停时按 C 复制，按 Shift 快速切换 HEX/RGB。",
      toolbarSide: "侧边栏位置",
      toolbarRight: "右侧",
      toolbarLeft: "左侧",
      shortcutsNote: "应用窗口获得焦点时生效，不注册系统全局热键；快捷键需包含 Ctrl、Alt 或 Win。",
      shortcutCapture: "按下快捷键",
      shortcutChangeTitle: "点击后按下新的快捷键",
      shortcutCapturingTitle: "按下新的快捷键，Esc 取消",
      shortcutReset: "恢复默认快捷键",
      shortcutChangeAria: "修改{label}快捷键",
      duplicateShortcut: "快捷键已被「{label}」使用",
      pickDirTitle: "选择默认本地保存目录",
    },
    shortcuts: {
      mirror: {
        label: "投屏",
        description: "打开投屏页，未启动时直接开始投屏",
        home: "打开投屏",
      },
      transfer: {
        label: "文件传输",
        description: "打开文件传输标签页",
        home: "打开文件传输",
      },
      screenshot: {
        label: "截图",
        description: "按当前截图来源执行截图",
        home: "快速截图",
      },
      record: {
        label: "录制",
        description: "开始或停止当前投屏录制",
        home: "开始 / 停止录制",
      },
      terminal: {
        label: "快捷终端",
        description: "打开快捷终端标签页",
        home: "打开终端",
      },
    },
    about: {
      subtitle: "本地桌面端 Android 设备调试与维护工具",
      versionInfo: "版本信息",
      appVersion: "应用版本",
      tauriVersion: "Tauri 版本",
      runtime: "运行方式",
      runtimeLocal: "本地桌面应用",
      statement: "使用声明",
      statement1: "请仅连接和操作你有权管理、测试或维护的设备。",
      statement2: "ADB 命令、文件传输、截图和录屏可能改变设备状态或涉及敏感数据，请在执行前自行确认。",
      statement3: "因误操作、未授权使用、设备异常或第三方环境限制产生的后果，由使用者自行承担。",
      privacy: "隐私说明",
      privacy1: "本应用不提供账号体系，不主动上传设备列表、命令记录、截图、录屏或文件内容。",
      privacy2: "设备信息、命令历史、传输记录和偏好设置仅用于本地功能展示与操作便利。",
      privacy3: "截图、录屏和文件传输内容由用户主动触发，并保存到用户选择的位置。",
      privacy4: "在团队、客户或生产环境使用时，请先取得必要授权并按组织要求处理敏感数据。",
      oss: "开源许可",
      ossText: "本应用包含 Tauri、Vue、Pinia、xterm.js、scrcpy / scrcpy server 等第三方开源组件。各组件版权归其原作者及贡献者所有，并按各自许可证授权；列出组件名称不表示相关项目对本应用提供认可、赞助或背书。",
    },
    mirror: {
      openMirror: "打开投屏",
      startMirror: "开始投屏",
      reconnecting: "重连中",
      cancelStart: "取消启动",
      stopMirror: "停止投屏",
      stopping: "停止中",
      mirrorError: "投屏异常",
      mirrorConnectFailed: "投屏连接失败，请检查设备和解码设置",
      selectDevice: "请选择设备",
      selectDeviceMessage: "选择设备后点击开始投屏",
      waitingStart: "等待启动",
      stoppedRestart: "投屏已停止，可重新开始",
      clickStart: "点击开始投屏",
      tabSettingsStatus: "偏好",
      textPreview: "文本预览",
      opening: "打开中",
      transferInstalling: "安装中",
      transferRunning: "传输中",
      terminalStarting: "启动中",
      terminalConnected: "已连接",
      terminalClosed: "已关闭",
    },
  },
  "en-US": {
    common: {
      appName: "DroidPilot",
      close: "Close",
      unavailable: "Unavailable",
      loading: "Loading",
      settings: "Settings",
      preferences: "Preferences",
      shortcuts: "Shortcuts",
      about: "About",
      ready: "Ready",
      saving: "Saving",
      running: "Running",
      preview: "Preview",
    },
    app: {
      noDevice: "Select a device",
      loadingDevices: "Loading devices...",
      refreshDevices: "Refresh devices",
      noDevices: "No devices. Click refresh.",
      connecting: "Connecting",
      connectWifi: "Connect Wi-Fi ADB",
      disconnectingDevice: "Disconnecting {serial}",
      disconnectDevice: "Disconnect {serial}",
      minimize: "Minimize",
      maximize: "Maximize",
      restore: "Restore",
      close: "Close",
      back: "Back",
      tasks: "Recents",
      power: "Power",
      nativeScreenshot: "Device Screenshot",
      mirrorScreenshot: "Mirror Screenshot",
      needRunningMirrorTab: "Switch to a running mirror tab first",
      needAuthorizedDevice: "An authorized device is required",
      saveRecording: "Save recording",
      stopRecording: "Stop recording",
      record: "Record",
      fileTransfer: "File Transfer",
      terminal: "Terminal",
    },
    settings: {
      language: "Language",
      theme: "Theme",
      themeLight: "Light",
      themeDark: "Dark",
      themeSystem: "System",
      defaultLocalDir: "Default local save folder",
      defaultLocalDirPlaceholder: "Folder for screenshots, recordings, and pulled files",
      chooseDefaultLocalDir: "Choose default local save folder",
      defaultLocalDirNote: "Screenshots and recordings are saved directly to this folder.",
      defaultDevicePath: "Default device path",
      screenshotMode: "Screenshot handling",
      screenshotModeSave: "Transfer only",
      screenshotModeCopy: "Copy only",
      screenshotModeSaveCopy: "Transfer + Copy",
      screenshotSource: "Screenshot source",
      screenshotSourceNative: "Device screencap",
      screenshotSourceMirror: "Mirror frame",
      screenshotSourceNote: "Device screenshots use screencap; mirror screenshots capture the active mirror tab.",
      pickerCopy: "Color picker copy",
      pickerCopyNote: "In the screenshot editor, hover the picker and press C to copy. Press Shift to switch HEX/RGB.",
      toolbarSide: "Sidebar position",
      toolbarRight: "Right",
      toolbarLeft: "Left",
      shortcutsNote: "Active when the app window is focused. Shortcuts are not global and must include Ctrl, Alt, or Win.",
      shortcutCapture: "Press shortcut",
      shortcutChangeTitle: "Click, then press a new shortcut",
      shortcutCapturingTitle: "Press a new shortcut, or Esc to cancel",
      shortcutReset: "Reset shortcut",
      shortcutChangeAria: "Change {label} shortcut",
      duplicateShortcut: "Shortcut is already used by {label}",
      pickDirTitle: "Choose default local save folder",
    },
    shortcuts: {
      mirror: {
        label: "Mirror",
        description: "Open the mirror tab; start mirroring directly if idle",
        home: "Open Mirror",
      },
      transfer: {
        label: "File Transfer",
        description: "Open the file transfer tab",
        home: "Open File Transfer",
      },
      screenshot: {
        label: "Screenshot",
        description: "Capture using the current screenshot source",
        home: "Quick Screenshot",
      },
      record: {
        label: "Record",
        description: "Start or stop recording the active mirror",
        home: "Start / Stop Recording",
      },
      terminal: {
        label: "Terminal",
        description: "Open the terminal tab",
        home: "Open Terminal",
      },
    },
    about: {
      subtitle: "Local desktop tool for Android device debugging and maintenance",
      versionInfo: "Version",
      appVersion: "App version",
      tauriVersion: "Tauri version",
      runtime: "Runtime",
      runtimeLocal: "Local desktop app",
      statement: "Usage Notice",
      statement1: "Only connect to and operate devices you are authorized to manage, test, or maintain.",
      statement2: "ADB commands, file transfer, screenshots, and recordings may change device state or involve sensitive data. Review the impact before running them.",
      statement3: "You are responsible for consequences caused by misuse, unauthorized use, device issues, or third-party environment limits.",
      privacy: "Privacy",
      privacy1: "This app has no account system and does not proactively upload device lists, command history, screenshots, recordings, or file content.",
      privacy2: "Device information, command history, transfer records, and preferences are used only for local functionality and convenience.",
      privacy3: "Screenshots, recordings, and file transfers are triggered by you and saved to locations you choose.",
      privacy4: "When used in team, customer, or production environments, obtain authorization first and handle sensitive data according to your organization requirements.",
      oss: "Open Source Licenses",
      ossText: "This app includes third-party open-source components such as Tauri, Vue, Pinia, xterm.js, and scrcpy / scrcpy server. Copyright belongs to their authors and contributors, and each component is licensed under its own terms. Listing a component does not imply endorsement, sponsorship, or approval of this app.",
    },
    mirror: {
      openMirror: "Open Mirror",
      startMirror: "Start Mirror",
      reconnecting: "Reconnecting",
      cancelStart: "Cancel Start",
      stopMirror: "Stop Mirror",
      stopping: "Stopping",
      mirrorError: "Mirror Error",
      mirrorConnectFailed: "Mirror connection failed. Check the device and decoder settings.",
      selectDevice: "Select a device",
      selectDeviceMessage: "Select a device, then start mirroring.",
      waitingStart: "Waiting to Start",
      stoppedRestart: "Mirroring stopped. You can start again.",
      clickStart: "Click Start Mirror",
      tabSettingsStatus: "Prefs",
      textPreview: "Text Preview",
      opening: "Opening",
      transferInstalling: "Installing",
      transferRunning: "Transferring",
      terminalStarting: "Starting",
      terminalConnected: "Connected",
      terminalClosed: "Closed",
    },
  },
};

function formatMessage(message: string, params?: Record<string, string | number>) {
  if (!params) {
    return message;
  }
  return message.replace(/\{(\w+)\}/g, (_match, key: string) => String(params[key] ?? ""));
}

function lookup(language: AppLanguage, key: string) {
  const segments = key.split(".");
  let cursor: string | MessageTree | undefined = messages[language];
  for (const segment of segments) {
    if (!cursor || typeof cursor === "string") {
      return "";
    }
    cursor = cursor[segment];
  }
  return typeof cursor === "string" ? cursor : "";
}

export function translate(language: AppLanguage, key: string, params?: Record<string, string | number>) {
  return formatMessage(lookup(language, key) || lookup("zh-CN", key) || key, params);
}

export function useI18n() {
  const adbStore = useAdbStore();
  const { language } = storeToRefs(adbStore);
  const currentLanguage = computed(() => language.value);
  const t = (key: string, params?: Record<string, string | number>) => translate(currentLanguage.value, key, params);
  return {
    language: currentLanguage,
    t,
  };
}

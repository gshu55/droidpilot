export type AppShortcutAction = "mirror" | "transfer" | "screenshot" | "record" | "terminal";
export type AppShortcutPlatform = "macos" | "windows" | "linux" | "other";

export interface AppShortcut {
  action: AppShortcutAction;
  label: string;
  description: string;
  defaultValue: string;
}

export type AppShortcutMap = Record<AppShortcutAction, string>;

const appShortcutBaseDefinitions: Array<Omit<AppShortcut, "defaultValue">> = [
  { action: "mirror", label: "投屏", description: "打开投屏页，未启动时直接开始投屏" },
  { action: "transfer", label: "文件传输", description: "打开文件传输标签页" },
  { action: "screenshot", label: "截图", description: "按当前截图来源执行截图" },
  { action: "record", label: "录制", description: "开始或停止当前投屏录制" },
  { action: "terminal", label: "快捷终端", description: "打开快捷终端标签页" },
];

const appShortcutDefaultKeys: AppShortcutMap = {
  mirror: "M",
  transfer: "F",
  screenshot: "S",
  record: "R",
  terminal: "T",
};

const shortcutModifierDisplayLabels: Record<AppShortcutPlatform, Record<string, string>> = {
  macos: {
    Ctrl: "Control",
    Alt: "Option",
    Shift: "Shift",
    Meta: "Command",
  },
  windows: {
    Ctrl: "Ctrl",
    Alt: "Alt",
    Shift: "Shift",
    Meta: "Win",
  },
  linux: {
    Ctrl: "Ctrl",
    Alt: "Alt",
    Shift: "Shift",
    Meta: "Super",
  },
  other: {
    Ctrl: "Ctrl",
    Alt: "Alt",
    Shift: "Shift",
    Meta: "Meta",
  },
};
const shortcutModifierParts = ["Ctrl", "Alt", "Shift", "Meta"];

export function resolveAppShortcutPlatform(value?: string): AppShortcutPlatform {
  const source = (value ?? (typeof navigator === "undefined" ? "" : `${navigator.platform} ${navigator.userAgent}`)).toLowerCase();
  if (source.includes("mac")) {
    return "macos";
  }
  if (source.includes("win")) {
    return "windows";
  }
  if (source.includes("linux") || source.includes("x11")) {
    return "linux";
  }
  return "other";
}

const currentAppShortcutPlatform = resolveAppShortcutPlatform();

export function defaultAppShortcutValue(action: AppShortcutAction, platform: AppShortcutPlatform = currentAppShortcutPlatform) {
  const modifierPrefix = platform === "macos" ? "Meta+Alt" : "Ctrl+Alt";
  return `${modifierPrefix}+${appShortcutDefaultKeys[action]}`;
}

export function buildDefaultAppShortcutMap(platform: AppShortcutPlatform = currentAppShortcutPlatform): AppShortcutMap {
  return appShortcutBaseDefinitions.reduce((map, item) => {
    map[item.action] = defaultAppShortcutValue(item.action, platform);
    return map;
  }, {} as AppShortcutMap);
}

export const appShortcutDefinitions: AppShortcut[] = appShortcutBaseDefinitions.map((item) => ({
  ...item,
  defaultValue: defaultAppShortcutValue(item.action),
}));

export const defaultAppShortcutMap = appShortcutDefinitions.reduce((map, item) => {
  map[item.action] = item.defaultValue;
  return map;
}, {} as AppShortcutMap);

const legacyDefaultAppShortcutMap = buildDefaultAppShortcutMap("windows");

const shortcutModifierKeys = new Set(["Control", "Shift", "Alt", "Meta"]);
const shortcutAliases: Record<string, string> = {
  " ": "Space",
  Esc: "Escape",
  Del: "Delete",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

function normalizeShortcutPart(value: string) {
  if (value === " ") {
    return "Space";
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  const lower = trimmed.toLowerCase();
  if (lower === "ctrl" || lower === "control") {
    return "Ctrl";
  }
  if (lower === "alt" || lower === "option") {
    return "Alt";
  }
  if (lower === "shift") {
    return "Shift";
  }
  if (lower === "meta" || lower === "cmd" || lower === "command" || lower === "win") {
    return "Meta";
  }
  const alias = shortcutAliases[trimmed];
  return alias ?? (trimmed.length === 1 ? trimmed.toUpperCase() : trimmed);
}

export function normalizeAppShortcutValue(value: unknown) {
  if (typeof value !== "string") {
    return "";
  }
  const rawParts = value.split("+").map(normalizeShortcutPart).filter(Boolean);
  const key = rawParts.find((part) => !["Ctrl", "Alt", "Shift", "Meta"].includes(part));
  if (!key) {
    return "";
  }
  const parts = [
    rawParts.includes("Ctrl") ? "Ctrl" : "",
    rawParts.includes("Alt") ? "Alt" : "",
    rawParts.includes("Shift") ? "Shift" : "",
    rawParts.includes("Meta") ? "Meta" : "",
    key,
  ].filter(Boolean);
  return parts.join("+");
}

export function normalizeAppShortcutFromEvent(event: KeyboardEvent) {
  if (shortcutModifierKeys.has(event.key)) {
    return "";
  }
  if (!event.ctrlKey && !event.altKey && !event.metaKey) {
    return "";
  }
  const key = normalizeShortcutPart(event.key);
  if (!key) {
    return "";
  }
  return [
    event.ctrlKey ? "Ctrl" : "",
    event.altKey ? "Alt" : "",
    event.shiftKey ? "Shift" : "",
    event.metaKey ? "Meta" : "",
    key,
  ].filter(Boolean).join("+");
}

export function normalizeAppShortcutMap(value: unknown, platform: AppShortcutPlatform = currentAppShortcutPlatform): AppShortcutMap {
  const source = value && typeof value === "object" ? value as Partial<Record<AppShortcutAction, unknown>> : {};
  const platformDefaults = buildDefaultAppShortcutMap(platform);
  return appShortcutDefinitions.reduce((map, item) => {
    const normalized = normalizeAppShortcutValue(source[item.action]);
    const migratedDefault = platform === "macos" && normalized === legacyDefaultAppShortcutMap[item.action]
      ? platformDefaults[item.action]
      : normalized;
    map[item.action] = migratedDefault || platformDefaults[item.action];
    return map;
  }, {} as AppShortcutMap);
}

export function displayAppShortcutValue(value: string, platform: AppShortcutPlatform = currentAppShortcutPlatform) {
  const labels = shortcutModifierDisplayLabels[platform];
  const parts = normalizeAppShortcutValue(value)
    .split("+")
    .filter(Boolean);
  const key = parts.find((part) => !shortcutModifierParts.includes(part));
  const modifierOrder = parts.includes("Meta")
    ? ["Meta", "Ctrl", "Alt", "Shift"]
    : shortcutModifierParts;

  return [
    ...modifierOrder.filter((part) => parts.includes(part)),
    key || "",
  ]
    .filter(Boolean)
    .map((part) => labels[part] || part)
    .join(" + ");
}

export function appShortcutKeyParts(value: string, platform: AppShortcutPlatform = currentAppShortcutPlatform) {
  return displayAppShortcutValue(value, platform).split(" + ").filter(Boolean);
}

export function matchAppShortcut(event: KeyboardEvent, shortcutMap: AppShortcutMap) {
  if (event.repeat) {
    return null;
  }

  const pressed = normalizeAppShortcutFromEvent(event);
  if (!pressed) {
    return null;
  }
  return appShortcutDefinitions.find((shortcut) => normalizeAppShortcutValue(shortcutMap[shortcut.action]) === pressed) || null;
}

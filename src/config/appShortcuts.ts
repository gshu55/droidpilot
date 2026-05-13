export type AppShortcutAction = "mirror" | "transfer" | "screenshot" | "record" | "terminal";

export interface AppShortcut {
  action: AppShortcutAction;
  label: string;
  description: string;
  defaultValue: string;
}

export type AppShortcutMap = Record<AppShortcutAction, string>;

export const appShortcutDefinitions: AppShortcut[] = [
  { action: "mirror", label: "投屏", description: "打开投屏页，未启动时直接开始投屏", defaultValue: "Ctrl+Alt+M" },
  { action: "transfer", label: "文件传输", description: "打开文件传输标签页", defaultValue: "Ctrl+Alt+F" },
  { action: "screenshot", label: "截图", description: "按当前截图来源执行截图", defaultValue: "Ctrl+Alt+S" },
  { action: "record", label: "录制", description: "开始或停止当前投屏录制", defaultValue: "Ctrl+Alt+R" },
  { action: "terminal", label: "快捷终端", description: "打开快捷终端标签页", defaultValue: "Ctrl+Alt+T" },
];

export const defaultAppShortcutMap = appShortcutDefinitions.reduce((map, item) => {
  map[item.action] = item.defaultValue;
  return map;
}, {} as AppShortcutMap);

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

export function normalizeAppShortcutMap(value: unknown): AppShortcutMap {
  const source = value && typeof value === "object" ? value as Partial<Record<AppShortcutAction, unknown>> : {};
  return appShortcutDefinitions.reduce((map, item) => {
    map[item.action] = normalizeAppShortcutValue(source[item.action]) || item.defaultValue;
    return map;
  }, {} as AppShortcutMap);
}

export function displayAppShortcutValue(value: string) {
  return normalizeAppShortcutValue(value).split("+").filter(Boolean).join(" + ");
}

export function appShortcutKeyParts(value: string) {
  return displayAppShortcutValue(value).split(" + ").filter(Boolean);
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

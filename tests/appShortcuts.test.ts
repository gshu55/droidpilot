import assert from "node:assert/strict";
import { test } from "node:test";

import {
  buildDefaultAppShortcutMap,
  displayAppShortcutValue,
  normalizeAppShortcutMap,
} from "../src/config/appShortcuts.ts";

test("uses Command and Option labels on macOS", () => {
  assert.equal(displayAppShortcutValue("Meta+Alt+M", "macos"), "Command + Option + M");
  assert.equal(displayAppShortcutValue("Ctrl+Alt+Shift+S", "macos"), "Control + Option + Shift + S");
});

test("uses platform specific labels for the Meta modifier", () => {
  assert.equal(displayAppShortcutValue("Meta+Alt+M", "windows"), "Win + Alt + M");
  assert.equal(displayAppShortcutValue("Meta+Alt+M", "linux"), "Super + Alt + M");
  assert.equal(displayAppShortcutValue("Meta+Alt+M", "other"), "Meta + Alt + M");
});

test("builds macOS defaults with the Command modifier", () => {
  const defaults = buildDefaultAppShortcutMap("macos");

  assert.equal(defaults.mirror, "Meta+Alt+M");
  assert.equal(defaults.transfer, "Meta+Alt+F");
  assert.equal(defaults.screenshot, "Meta+Alt+S");
  assert.equal(defaults.record, "Meta+Alt+R");
  assert.equal(defaults.terminal, "Meta+Alt+T");
});

test("normalizes missing shortcuts with platform defaults", () => {
  assert.equal(normalizeAppShortcutMap({}, "macos").mirror, "Meta+Alt+M");
  assert.equal(normalizeAppShortcutMap({}, "windows").mirror, "Ctrl+Alt+M");
});

test("migrates legacy default shortcuts to macOS defaults", () => {
  const shortcuts = normalizeAppShortcutMap(
    {
      mirror: "Ctrl+Alt+M",
      transfer: "Ctrl+Alt+F",
      screenshot: "Ctrl+Alt+S",
      record: "Ctrl+Shift+R",
      terminal: "Ctrl+Alt+T",
    },
    "macos",
  );

  assert.equal(shortcuts.mirror, "Meta+Alt+M");
  assert.equal(shortcuts.transfer, "Meta+Alt+F");
  assert.equal(shortcuts.screenshot, "Meta+Alt+S");
  assert.equal(shortcuts.record, "Ctrl+Shift+R");
  assert.equal(shortcuts.terminal, "Meta+Alt+T");
});

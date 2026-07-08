import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const mirrorPage = readFileSync(new URL("../src/pages/MirrorPage.vue", import.meta.url), "utf-8");
const quickCommandsSource = mirrorPage.match(/const quickCommands = computed<QuickCommandItem\[\]>\(\(\) => \{[\s\S]*?\n\}\);/)?.[0] || "";

test("terminal command picker is collapsed by default", () => {
  const declaration = mirrorPage.match(/const commandPickerCollapsed = ref\((true|false)\);/)?.[0];
  assert.equal(declaration, "const commandPickerCollapsed = ref(true);");
});

test("terminal host owns right click copy and paste handling", () => {
  const terminalHostMarkup = mirrorPage.match(/<div\s+ref="terminalHostRef"[\s\S]*?class="terminal-host"[\s\S]*?><\/div>/)?.[0] || "";

  assert.match(terminalHostMarkup, /data-allow-app-context-menu="true"/);
  assert.match(terminalHostMarkup, /@contextmenu\.prevent\.stop="handleTerminalContextMenu"/);
});

test("terminal right click copies selection before attempting paste", () => {
  const handler = mirrorPage.match(/async function handleTerminalContextMenu\([^)]*\)\s*\{[\s\S]*?\n\}/)?.[0] || "";

  assert.match(handler, /const selection = terminal\?\.getSelection\(\)\.trim\(\) \|\| "";/);
  assert.match(handler, /if \(selection\) \{/);
  assert.match(handler, /const copied = await copyTextToClipboard\(selection\);/);
  assert.match(handler, /if \(copied\) \{/);
  assert.match(handler, /return;/);
  assert.match(handler, /await pasteClipboardTextToTerminal\(\);/);
});

test("terminal copy writes through the native clipboard before reporting success", () => {
  const copyHelper = mirrorPage.match(/async function copyTextToClipboard\(text: string\)\s*\{[\s\S]*?\n\}/)?.[0] || "";

  assert.match(copyHelper, /await invoke\("write_text_clipboard", \{ text \}\)/);
  assert.match(copyHelper, /navigator\.clipboard\?\.writeText/);
  assert.match(copyHelper, /copyTextFallback\(text\)/);
  assert.match(copyHelper, /\(\) => true/);
  assert.match(copyHelper, /\(\) => false/);
  assert.match(copyHelper, /return nativeCopied \|\| browserCopied \|\| fallbackCopied;/);
});

test("terminal copy fallback reports whether execCommand actually copied", () => {
  const fallbackHelper = mirrorPage.match(/function copyTextFallback\(text: string\)\s*\{[\s\S]*?\n\}/)?.[0] || "";

  assert.match(fallbackHelper, /return document\.execCommand\("copy"\);/);
  assert.match(fallbackHelper, /return false;/);
});

test("terminal paste reads native clipboard text and writes it to the active session", () => {
  const pasteHelper = mirrorPage.match(/async function pasteClipboardTextToTerminal\(\)\s*\{[\s\S]*?\n\}/)?.[0] || "";

  assert.match(pasteHelper, /const text = await readTerminalClipboardText\(\);/);
  assert.match(pasteHelper, /if \(!text\) \{/);
  assert.match(pasteHelper, /await writeTerminalData\(text\);/);
});

test("terminal clipboard read prefers the Tauri native command over browser clipboard permissions", () => {
  const readHelper = mirrorPage.match(/async function readTerminalClipboardText\(\)\s*\{[\s\S]*?\n\}/)?.[0] || "";

  assert.match(readHelper, /await invoke<string>\("read_text_clipboard"\)/);
  assert.match(readHelper, /navigator\.clipboard\?\.readText/);
  assert.match(readHelper, /无法读取剪贴板，请使用 Ctrl\/Command\+V 粘贴/);
});

test("terminal quick commands include a device overview for key hardware information", () => {
  assert.match(quickCommandsSource, /label:\s*"设备概览"/);
  assert.match(quickCommandsSource, /desc:\s*"汇总型号、CPU、屏幕、内存、磁盘和系统版本"/);
  assert.match(quickCommandsSource, /echo '== Android 版本 =='/);
  assert.match(quickCommandsSource, /getprop ro\.build\.version\.release/);
  assert.match(quickCommandsSource, /echo '== CPU 型号 =='/);
  assert.match(quickCommandsSource, /\/proc\/cpuinfo/);
  assert.match(quickCommandsSource, /echo '== CPU 核心 =='/);
  assert.match(quickCommandsSource, /ls -d \/sys\/devices\/system\/cpu\/cpu\[0-9\]\*/);
  assert.match(quickCommandsSource, /echo '== 分辨率 =='/);
  assert.match(quickCommandsSource, /wm size/);
  assert.match(quickCommandsSource, /echo '== DPI =='/);
  assert.match(quickCommandsSource, /wm density/);
  assert.match(quickCommandsSource, /echo '== 内存大小 =='/);
  assert.match(quickCommandsSource, /grep MemTotal \/proc\/meminfo/);
  assert.match(quickCommandsSource, /echo '== 磁盘大小 =='/);
  assert.match(quickCommandsSource, /df -h \/data/);
});

test("terminal quick commands include focused hardware info commands", () => {
  const requiredLabels = ["CPU 型号", "CPU 核心", "内存大小", "磁盘大小"];

  for (const label of requiredLabels) {
    assert.match(quickCommandsSource, new RegExp(`label:\\s*"${label}"`));
  }
  assert.match(quickCommandsSource, /label:\s*"CPU 型号"[\s\S]*?grep -m 1 -E 'Hardware|Processor|model name|cpu model' \/proc\/cpuinfo/);
  assert.match(quickCommandsSource, /label:\s*"CPU 核心"[\s\S]*?ls -d \/sys\/devices\/system\/cpu\/cpu\[0-9\]\*/);
  assert.match(quickCommandsSource, /label:\s*"内存大小"[\s\S]*?grep MemTotal \/proc\/meminfo/);
  assert.match(quickCommandsSource, /label:\s*"磁盘大小"[\s\S]*?df -h \/data/);
});

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const tauriLib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf-8");

test("Tauri exposes a native text clipboard read command", () => {
  assert.match(tauriLib, /#\[tauri::command\]\s*\nfn read_text_clipboard\(\) -> Result<String, String>/);
  assert.match(tauriLib, /read_text_from_clipboard\(\)/);
  assert.match(tauriLib, /read_text_clipboard,/);
});

test("Tauri exposes a native text clipboard write command", () => {
  assert.match(tauriLib, /#\[tauri::command\]\s*\nfn write_text_clipboard\(text: String\) -> Result<\(\), String>/);
  assert.match(tauriLib, /write_text_to_clipboard\(&text\)/);
  assert.match(tauriLib, /write_text_clipboard,/);
});

test("native text clipboard read and write have platform implementations without new dependencies", () => {
  assert.match(tauriLib, /Command::new\("\/usr\/bin\/pbpaste"\)/);
  assert.match(tauriLib, /"\/usr\/bin\/pbcopy"/);
  assert.match(tauriLib, /Command::new\("powershell(?:\.exe)?"\)/);
  assert.match(tauriLib, /Set-Clipboard/);
  assert.match(tauriLib, /Command::new\("wl-paste"\)/);
  assert.match(tauriLib, /"wl-copy"/);
  assert.match(tauriLib, /Command::new\("xclip"\)/);
});

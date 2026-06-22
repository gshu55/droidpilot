import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const mirrorPage = readFileSync(new URL("../src/pages/MirrorPage.vue", import.meta.url), "utf-8");

function paneMarkup(startMarker: string, endMarker: string) {
  const start = mirrorPage.indexOf(startMarker);
  const end = mirrorPage.indexOf(endMarker, start);
  assert.notEqual(start, -1, `missing ${startMarker}`);
  assert.notEqual(end, -1, `missing ${endMarker}`);
  return mirrorPage.slice(start, end);
}

test("parent directory row appears below sorting header in both transfer panes", () => {
  const localPane = paneMarkup('<div class="file-pane system-pane">', 'class="transfer-pane-resizer"');
  const remotePane = paneMarkup('<div class="file-pane device-pane">', '<Teleport to="body">');

  for (const pane of [localPane, remotePane]) {
    const headerIndex = pane.indexOf('class="file-tree-header"');
    const parentRowIndex = pane.indexOf('class="file-parent-row mono"');
    const itemIndex = pane.indexOf('class="file-tree-item"');
    assert.ok(headerIndex >= 0, "missing file tree sorting header");
    assert.ok(parentRowIndex > headerIndex, "parent row should appear after sorting header");
    assert.ok(itemIndex > parentRowIndex, "file entries should appear after parent row");
  }
});

test("file tree sorting header uses theme background variables", () => {
  const headerStyle = mirrorPage.match(/\.file-tree-header\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.match(headerStyle, /background:\s*var\(--surface-alt\);/);
  assert.doesNotMatch(headerStyle, /background:\s*#f8fafc;/);
});

test("parent directory row is sticky below the sorting header without double borders", () => {
  const listStyle = mirrorPage.match(/\.file-tree-list\s*\{[^}]*\}/s)?.[0] ?? "";
  const parentRowStyle = mirrorPage.match(/\.file-parent-row\s*\{[^}]*\}/s)?.[0] ?? "";

  assert.match(listStyle, /--file-tree-header-height:\s*27px;/);
  assert.match(parentRowStyle, /position:\s*sticky;/);
  assert.match(parentRowStyle, /top:\s*var\(--file-tree-header-height\);/);
  assert.match(parentRowStyle, /z-index:\s*1;/);
  assert.match(parentRowStyle, /border:\s*0;/);
  assert.match(parentRowStyle, /border-bottom:\s*1px solid var\(--border\);/);
  assert.doesNotMatch(parentRowStyle, /border:\s*1px solid var\(--border\);/);
});

test("file pane gives the file list the flexible row after moving parent row inside the list", () => {
  const paneStyle = mirrorPage.match(/\.file-pane\s*\{[^}]*\}/s)?.[0] ?? "";
  assert.match(paneStyle, /grid-template-rows:\s*max-content max-content minmax\(0,\s*1fr\);/);
  assert.doesNotMatch(paneStyle, /grid-template-rows:\s*max-content max-content max-content minmax\(0,\s*1fr\);/);
});

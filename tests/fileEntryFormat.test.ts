import assert from "node:assert/strict";
import { test } from "node:test";

import { formatEntrySize } from "../src/utils/fileEntryFormat.ts";

test("formats directory size as empty text", () => {
  assert.equal(formatEntrySize({ isDir: true }), "");
});

test("formats files with missing size as fallback text", () => {
  assert.equal(formatEntrySize({ isDir: false }), "--");
});

test("formats files with size in bytes", () => {
  assert.equal(formatEntrySize({ isDir: false, sizeBytes: 1536 }), "1.5 KB");
});

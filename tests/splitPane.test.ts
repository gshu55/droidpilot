import assert from "node:assert/strict";
import { test } from "node:test";

import { clampSplitPanePercent, splitPaneGridTemplateColumns } from "../src/utils/splitPane.ts";

test("keeps split percent inside available pane width", () => {
  assert.equal(Number(clampSplitPanePercent(10, 1000, 300, 8).toFixed(2)), 30.24);
  assert.equal(Number(clampSplitPanePercent(90, 1000, 300, 8).toFixed(2)), 69.76);
  assert.equal(clampSplitPanePercent(55, 1000, 300, 8), 55);
});

test("falls back to centered split when container cannot fit minimum pane widths", () => {
  assert.equal(clampSplitPanePercent(20, 580, 300, 8), 50);
});

test("formats grid columns with a separate resize handle column", () => {
  assert.equal(
    splitPaneGridTemplateColumns(50, 300, 8),
    "minmax(300px, calc(50% - 4px)) 8px minmax(300px, 1fr)",
  );
});

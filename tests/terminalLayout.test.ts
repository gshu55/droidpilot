import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const mirrorPage = readFileSync(new URL("../src/pages/MirrorPage.vue", import.meta.url), "utf-8");

test("terminal command picker is collapsed by default", () => {
  const declaration = mirrorPage.match(/const commandPickerCollapsed = ref\((true|false)\);/)?.[0];
  assert.equal(declaration, "const commandPickerCollapsed = ref(true);");
});

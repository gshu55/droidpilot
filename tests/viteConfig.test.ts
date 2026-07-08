import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const viteConfig = readFileSync(new URL("../vite.config.ts", import.meta.url), "utf-8");

test("vue plugin receives an explicit SFC compiler before HMR starts", () => {
  assert.match(viteConfig, /import \* as vueCompiler from "vue\/compiler-sfc";/);
  assert.match(viteConfig, /vue\(\{\s*compiler:\s*vueCompiler,?\s*\}\)/s);
});

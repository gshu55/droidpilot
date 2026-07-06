import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileViewerRenderers } from "@file-viewer/vite-plugin";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
const require = createRequire(import.meta.url);
const cargoToml = readFileSync(new URL("./src-tauri/Cargo.toml", import.meta.url), "utf-8");
const cargoPackageVersion = cargoToml.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];
const fileViewerPresetAllPackagePath = require.resolve("@file-viewer/preset-all/package.json");
const fileViewerPresetAllRequire = createRequire(fileViewerPresetAllPackagePath);
const fileViewerRendererEpubPackagePath = fileViewerPresetAllRequire.resolve("@file-viewer/renderer-epub/package.json");
const fileViewerRendererEpubRequire = createRequire(fileViewerRendererEpubPackagePath);
const epubjsPackagePath = fileViewerRendererEpubRequire.resolve("epubjs/package.json");
const epubjsRequire = createRequire(epubjsPackagePath);
const jszipPackagePath = epubjsRequire.resolve("jszip/package.json");
const fileViewerSpreadsheetWorkerSheetjsPath = require.resolve("@file-viewer/renderer-spreadsheet/worker/sheetjs");
const jszipBrowserBundlePath = join(dirname(jszipPackagePath), "dist/jszip.min.js");

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    vue(),
    fileViewerRenderers({
      preset: "all",
      inject: false,
      copyAssets: true,
      chunkStrategy: "renderer",
    }),
  ],
  define: {
    __APP_VERSION__: JSON.stringify(cargoPackageVersion ?? "unknown"),
  },
  resolve: {
    alias: {
      "@droidpilot/file-viewer-spreadsheet-worker-sheetjs": fileViewerSpreadsheetWorkerSheetjsPath,
      "jszip/dist/jszip": jszipBrowserBundlePath,
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  FILE_VIEWER_LARGE_FILE_CONFIRM_BYTES,
  filePreviewToolbarPosition,
  isFileViewerPreviewPath,
  isFileViewerSearchPath,
  isPreviewablePath,
  previewModeForPath,
  shouldConfirmLargeFileViewerPreview,
} from "../src/utils/filePreview.ts";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf-8"));
const fileViewerReadmeSupportedExtensions = [
  "docx",
  "docm",
  "dotx",
  "dotm",
  "doc",
  "dot",
  "pptx",
  "pptm",
  "potx",
  "potm",
  "ppsx",
  "ppsm",
  "rtf",
  "odt",
  "odp",
  "xlsx",
  "xltx",
  "xlsm",
  "xlsb",
  "xls",
  "xlt",
  "xltm",
  "csv",
  "ods",
  "fods",
  "numbers",
  "pdf",
  "ofd",
  "typ",
  "typst",
  "zip",
  "zipx",
  "7z",
  "rar",
  "tar",
  "gz",
  "gzip",
  "tgz",
  "bz2",
  "bzip2",
  "tbz",
  "tbz2",
  "xz",
  "txz",
  "lzma",
  "zst",
  "tzst",
  "cab",
  "ar",
  "cpio",
  "iso",
  "xar",
  "lha",
  "lzh",
  "jar",
  "war",
  "ear",
  "apk",
  "cbz",
  "cbr",
  "eml",
  "msg",
  "mbox",
  "olb",
  "dra",
  "dxf",
  "dwg",
  "dwf",
  "dwfx",
  "xps",
  "glb",
  "gltf",
  "obj",
  "stl",
  "ply",
  "fbx",
  "dae",
  "3ds",
  "3mf",
  "amf",
  "usd",
  "usda",
  "usdc",
  "usdz",
  "kmz",
  "step",
  "stp",
  "iges",
  "igs",
  "ifc",
  "3dm",
  "pcd",
  "wrl",
  "vrml",
  "xyz",
  "vtk",
  "vtp",
  "geojson",
  "kml",
  "gpx",
  "shp",
  "excalidraw",
  "drawio",
  "dio",
  "epub",
  "umd",
  "gif",
  "jpg",
  "jpeg",
  "bmp",
  "tiff",
  "tif",
  "png",
  "svg",
  "webp",
  "avif",
  "ico",
  "heic",
  "heif",
  "jxl",
  "md",
  "markdown",
  "txt",
  "json",
  "js",
  "mjs",
  "cjs",
  "css",
  "java",
  "py",
  "html",
  "htm",
  "jsx",
  "ts",
  "tsx",
  "xml",
  "log",
  "vue",
  "yaml",
  "yml",
  "ini",
  "sh",
  "bash",
  "sql",
  "go",
  "rs",
  "php",
  "c",
  "cpp",
  "cc",
  "h",
  "hpp",
  "cs",
  "diff",
  "jsonc",
  "json5",
  "ipynb",
  "toml",
  "proto",
  "hcl",
  "tex",
  "gv",
  "http",
  "react",
  "rb",
  "swift",
  "kt",
  "mp4",
  "webm",
  "m3u8",
  "mp3",
  "mpeg",
  "wav",
  "ogg",
  "oga",
  "opus",
  "m4a",
  "aac",
  "flac",
  "weba",
  "midi",
  "mid",
  "ttf",
  "otf",
  "woff",
  "woff2",
  "psd",
  "ai",
  "eps",
  "sqlite",
  "wasm",
  "parquet",
  "avro",
  "webarchive",
] as const;

test("plain text and code files use the standard Vue3 FileViewer renderer", () => {
  for (const path of ["/tmp/system.log", "/tmp/config.json", "/tmp/MainActivity.kt"]) {
    assert.equal(previewModeForPath(path), "viewer");
    assert.equal(isPreviewablePath(path), true);
    assert.equal(isFileViewerPreviewPath(path), true);
  }
});

test("rich document and media formats use the standard Vue3 FileViewer renderer", () => {
  for (const path of [
    "/tmp/report.pdf",
    "/tmp/contract.docx",
    "/tmp/sheet.xlsx",
    "/tmp/data.csv",
    "/tmp/slides.pptx",
    "/tmp/readme.md",
    "/tmp/page.html",
    "/tmp/photo.png",
    "/tmp/movie.mp4",
    "/tmp/audio.flac",
    "/tmp/archive.zip",
  ]) {
    assert.equal(previewModeForPath(path), "viewer", path);
    assert.equal(isPreviewablePath(path), true, path);
    assert.equal(isFileViewerPreviewPath(path), true, path);
  }
});

test("all extensions listed by @file-viewer/vue3 README are previewable", () => {
  const unsupported = fileViewerReadmeSupportedExtensions.filter((extension) => !isPreviewablePath(`/tmp/sample.${extension}`));
  assert.deepEqual(unsupported, []);
});

test("FileViewer search is enabled only for renderers with searchable text content", () => {
  for (const path of [
    "/tmp/report.pdf",
    "/tmp/contract.docx",
    "/tmp/sheet.xlsx",
    "/tmp/slides.pptx",
    "/tmp/readme.md",
    "/tmp/page.html",
    "/tmp/archive.zip",
    "/tmp/message.eml",
    "/tmp/map.geojson",
    "/tmp/sketch.drawio",
    "/tmp/book.epub",
    "/tmp/data.sqlite",
    "/tmp/system.log",
    "/tmp/config.json",
    "/tmp/MainActivity.kt",
  ]) {
    assert.equal(isFileViewerSearchPath(path), true, path);
  }

  for (const path of [
    "/tmp/photo.png",
    "/tmp/movie.mp4",
    "/tmp/audio.flac",
    "/tmp/model.glb",
    "/tmp/cad.dwg",
  ]) {
    assert.equal(isFileViewerSearchPath(path), false, path);
  }
});

test("unknown extension is not previewable", () => {
  assert.equal(isPreviewablePath("/tmp/blob.unknown-ext"), false);
});

test("uses the standard Vue3 FileViewer package instead of the compatibility alias", () => {
  assert.equal(packageJson.dependencies["@file-viewer/vue3"], "2.0.10");
  assert.equal(packageJson.dependencies.fileViewer3, undefined);
  assert.equal(packageJson.dependencies["file-viewer3"], undefined);
});

test("only large FileViewer files require confirmation", () => {
  assert.equal(shouldConfirmLargeFileViewerPreview("viewer", FILE_VIEWER_LARGE_FILE_CONFIRM_BYTES + 1), true);
  assert.equal(shouldConfirmLargeFileViewerPreview("viewer", FILE_VIEWER_LARGE_FILE_CONFIRM_BYTES), false);
  assert.equal(shouldConfirmLargeFileViewerPreview("viewer", null), false);
});

test("all FileViewer previews use a top toolbar so floating actions do not cover content", () => {
  for (const extension of fileViewerReadmeSupportedExtensions) {
    assert.equal(filePreviewToolbarPosition(`/tmp/sample.${extension}`), "top", extension);
  }
});

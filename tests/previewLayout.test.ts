import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

const mirrorPage = readFileSync(new URL("../src/pages/MirrorPage.vue", import.meta.url), "utf-8");
const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf-8"));
const viteConfig = readFileSync(new URL("../vite.config.ts", import.meta.url), "utf-8");
const spreadsheetWorker = readFileSync(new URL("../src/workers/fileViewerSpreadsheet.worker.ts", import.meta.url), "utf-8");

function previewHeaderMarkup() {
  const start = mirrorPage.indexOf('<header class="text-preview-head">');
  const end = mirrorPage.indexOf('<div class="text-preview-actions">', start);
  assert.notEqual(start, -1, "missing preview header");
  assert.notEqual(end, -1, "missing preview actions");
  return mirrorPage.slice(start, end);
}

function previewActionsMarkup() {
  const start = mirrorPage.indexOf('<div class="text-preview-actions">');
  const end = mirrorPage.indexOf('<button type="button" class="tiny-btn" @click="closeTextPreview">关闭</button>', start);
  assert.notEqual(start, -1, "missing preview actions");
  assert.notEqual(end, -1, "missing preview close action");
  return mirrorPage.slice(start, end);
}

test("preview header keeps source and size metadata", () => {
  const header = previewHeaderMarkup();
  assert.match(header, /class="text-preview-title-row"/);
  assert.match(header, /v-for="item in textPreviewHeaderMetaItems"/);
  assert.match(mirrorPage, /const textPreviewHeaderMetaItems = computed/);
  assert.match(mirrorPage, /textPreviewSource\.value === "local"/);
  assert.match(mirrorPage, /系统文件/);
  assert.match(mirrorPage, /设备文件/);
  assert.match(mirrorPage, /formatBytes\(meta\.sizeBytes\)/);
});

test("preview header keeps format, line count, search and actions on one row", () => {
  const header = previewHeaderMarkup();
  assert.match(header, /class="text-preview-title-row"/);
  assert.match(header, /class="text-preview-meta-inline"/);
  assert.match(mirrorPage, /const fileViewerDocumentLineCount = ref<number \| null>\(null\)/);
  assert.match(mirrorPage, /fileExtension\(name\)/);
  assert.match(mirrorPage, /fileViewerDocumentLineCount\.value !== null/);
  assert.match(mirrorPage, /`\$\{fileViewerDocumentLineCount\.value\} 行`/);
  assert.match(
    mirrorPage,
    /\.text-preview-head\s*\{[^}]*grid-template-columns:\s*minmax\(0,\s*1fr\)\s+max-content;/s,
  );
});

test("FileViewer load completion updates text preview line count", () => {
  assert.match(mirrorPage, /@load-complete="onFileViewerLoadComplete"/);
  assert.match(mirrorPage, /async function onFileViewerLoadComplete\(\)/);
  assert.match(mirrorPage, /collectDocumentAnchors\(\)/);
  assert.match(mirrorPage, /function resolveFileViewerDocumentLineCount/);
});

test("file viewer archive environment fallback notice is hidden", () => {
  const style = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.archive-info\)\s*\{[^}]*\}/s)?.[0] || "";
  assert.match(style, /display:\s*none\s*!important;/);
});

test("FileViewer code toolbar metadata is hidden because the preview header owns it", () => {
  const style = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.code-toolbar\)\s*\{[^}]*\}/s)?.[0] || "";
  assert.match(style, /display:\s*none\s*!important;/);
});

test("preview header owns zoom controls and places them before search", () => {
  const actions = previewActionsMarkup();
  const zoomIndex = actions.indexOf('class="text-preview-zoom"');
  const searchIndex = actions.indexOf('class="text-preview-search"');
  assert.notEqual(zoomIndex, -1, "missing preview zoom controls");
  assert.notEqual(searchIndex, -1, "missing preview search controls");
  assert.ok(zoomIndex < searchIndex, "zoom controls should be before search controls");
  assert.match(actions, /@click="zoomTextPreview\('out'\)"/);
  assert.match(actions, /@click="zoomTextPreview\('reset'\)"/);
  assert.match(actions, /@click="zoomTextPreview\('in'\)"/);
  assert.match(mirrorPage, /const fileViewerZoomState = ref<FileViewerZoomState \| null>\(null\)/);
  assert.match(mirrorPage, /@zoom-change="onFileViewerZoomChange"/);
  assert.match(mirrorPage, /async function zoomTextPreview\(action: "in" \| "out" \| "reset"\)/);
});

test("preview zoom controls are styled as one segmented component", () => {
  const groupStyle = mirrorPage.match(/\.text-preview-zoom\s*\{[^}]*\}/s)?.[0] || "";
  const buttonStyle = mirrorPage.match(/\.text-preview-zoom :is\(\.text-preview-zoom-btn,\s*\.text-preview-zoom-meter\)\s*\{[^}]*\}/s)?.[0] || "";
  const meterStyle = mirrorPage.match(/\.text-preview-zoom-meter\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(groupStyle, /gap:\s*0;/);
  assert.match(groupStyle, /border:\s*1px solid var\(--control-border\);/);
  assert.match(groupStyle, /border-radius:\s*6px;/);
  assert.match(groupStyle, /background:\s*var\(--control-bg\);/);
  assert.match(groupStyle, /overflow:\s*hidden;/);
  assert.match(buttonStyle, /border:\s*0;/);
  assert.match(buttonStyle, /border-radius:\s*0;/);
  assert.match(meterStyle, /border-left:\s*1px solid var\(--control-border\);/);
  assert.match(meterStyle, /border-right:\s*1px solid var\(--control-border\);/);
});

test("preview search controls are styled as one segmented component", () => {
  const groupStyle = mirrorPage.match(/\.text-preview-search\s*\{[^}]*\}/s)?.[0] || "";
  const focusStyle = mirrorPage.match(/\.text-preview-search:focus-within\s*\{[^}]*\}/s)?.[0] || "";
  const inputStyle = mirrorPage.match(/\.text-preview-search input\s*\{[^}]*\}/s)?.[0] || "";
  const actionStyle = mirrorPage.match(/\.text-preview-search :is\(\.text-preview-search-toggle,\s*\.text-preview-search-btn\)\s*\{[^}]*\}/s)?.[0] || "";
  const labelStyle = mirrorPage.match(/\.text-preview-search span\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(groupStyle, /gap:\s*0;/);
  assert.match(groupStyle, /border:\s*1px solid var\(--control-border\);/);
  assert.match(groupStyle, /border-radius:\s*6px;/);
  assert.match(groupStyle, /background:\s*var\(--control-bg\);/);
  assert.match(groupStyle, /overflow:\s*hidden;/);
  assert.match(focusStyle, /border-color:\s*var\(--primary-border\);/);
  assert.match(focusStyle, /background:\s*var\(--control-bg-hover\);/);
  assert.match(inputStyle, /border:\s*0;/);
  assert.match(inputStyle, /border-radius:\s*0;/);
  assert.match(inputStyle, /background:\s*transparent;/);
  assert.match(actionStyle, /display:\s*inline-grid;/);
  assert.match(actionStyle, /place-items:\s*center;/);
  assert.match(actionStyle, /border:\s*0;/);
  assert.match(actionStyle, /border-left:\s*1px solid var\(--control-border\);/);
  assert.match(actionStyle, /border-radius:\s*0;/);
  assert.match(actionStyle, /background:\s*transparent;/);
  assert.match(labelStyle, /border-left:\s*1px solid var\(--control-border\);/);
});

test("whole word search toggle uses the themed provided glyph icon", () => {
  const actions = previewActionsMarkup();
  const iconStyle = mirrorPage.match(/\.whole-word-icon\s*\{[^}]*\}/s)?.[0] || "";
  const glyphStyle = mirrorPage.match(/\.whole-word-icon-glyph\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(actions, /<svg class="whole-word-icon" viewBox="0 0 1024 1024" aria-hidden="true">/);
  assert.match(actions, /class="whole-word-icon-glyph"/);
  assert.match(actions, /M387\.657143 599\.771429l65\.828571 168\.228571h73\.142857/);
  assert.doesNotMatch(actions, /M0 36\.571429h1024v73\.142857H0z/);
  assert.match(actions, /M0 914\.285714h1024v73\.142857H0z/);
  assert.doesNotMatch(actions, /M943\.542857 160\.914286h73\.142857V804\.571429h-73\.142857z/);
  assert.doesNotMatch(actions, /fill="#9494A5"/);
  assert.match(iconStyle, /width:\s*13px;/);
  assert.match(iconStyle, /height:\s*13px;/);
  assert.match(iconStyle, /color:\s*currentColor;/);
  assert.match(iconStyle, /transform:\s*translate\(1px,\s*-1px\);/);
  assert.match(glyphStyle, /fill:\s*currentColor;/);
  assert.match(glyphStyle, /stroke:\s*currentColor;/);
  assert.match(glyphStyle, /stroke-width:\s*18px;/);
});

test("preview header shows spreadsheet cache summary before zoom controls", () => {
  const actions = previewActionsMarkup();
  const cacheIndex = actions.indexOf('class="text-preview-cache-summary"');
  const zoomIndex = actions.indexOf('class="text-preview-zoom"');

  assert.notEqual(cacheIndex, -1, "missing preview cache summary");
  assert.notEqual(zoomIndex, -1, "missing preview zoom controls");
  assert.ok(cacheIndex < zoomIndex, "cache summary should be before zoom controls");
  assert.match(actions, /v-if="textPreviewCacheSummary"/);
  assert.match(actions, /\{\{ textPreviewCacheSummary \}\}/);
  assert.match(mirrorPage, /const textPreviewCacheSummary = ref\(""\)/);
  assert.match(mirrorPage, /function startFileViewerCacheSummaryObserver\(\)/);
});

test("FileViewer built-in download print HTML and zoom toolbar actions are disabled", () => {
  assert.match(
    mirrorPage,
    /toolbar:\s*\{\s*position:\s*filePreviewToolbarPosition\(textPreviewMeta\.value\?\.name \|\| ""\),\s*download:\s*false,\s*print:\s*false,\s*exportHtml:\s*false,\s*zoom:\s*false,?\s*\}/s,
  );
  const viewerActionsStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.viewer-actions\)\s*\{[^}]*\}/s)?.[0] || "";
  assert.match(viewerActionsStyle, /display:\s*none\s*!important;/);
});

test("PDF previews provide a Vite-bundled PDF.js worker URL", () => {
  assert.equal(packageJson.dependencies["pdfjs-dist"], "6.0.227");
  assert.match(mirrorPage, /import pdfWorkerUrl from "pdfjs-dist\/legacy\/build\/pdf\.worker\.mjs\?worker&url";/);
  assert.match(
    mirrorPage,
    /pdf:\s*\{\s*workerUrl:\s*pdfWorkerUrl,\s*cMapUrl:\s*"\/vendor\/pdf\/cmaps\/",\s*wasmUrl:\s*"\/vendor\/pdf\/wasm\/",\s*standardFontDataUrl:\s*"\/vendor\/pdf\/standard_fonts\/",?\s*\}/s,
  );
});

test("PDF preview public assets are available for packaged Windows builds", () => {
  const pdfVendorRoot = new URL("../public/vendor/pdf/", import.meta.url);
  const cmaps = new URL("cmaps/", pdfVendorRoot);
  const wasm = new URL("wasm/", pdfVendorRoot);
  const standardFonts = new URL("standard_fonts/", pdfVendorRoot);

  assert.equal(existsSync(cmaps), true, "missing public/vendor/pdf/cmaps");
  assert.equal(existsSync(wasm), true, "missing public/vendor/pdf/wasm");
  assert.equal(existsSync(standardFonts), true, "missing public/vendor/pdf/standard_fonts");
  assert.ok(readdirSync(cmaps).some((name) => name.endsWith(".bcmap")), "missing PDF CMap files");
  assert.ok(readdirSync(wasm).some((name) => name.endsWith(".wasm")), "missing PDF wasm files");
  assert.ok(readdirSync(standardFonts).some((name) => name.endsWith(".ttf") || name.endsWith(".pfb")), "missing PDF standard font files");
});

test("FileViewer offline assets are configured for packaged Windows builds", () => {
  assert.match(
    mirrorPage,
    /archive:\s*\{\s*workerUrl:\s*"\/vendor\/libarchive\/worker-bundle\.js",\s*wasmUrl:\s*"\/vendor\/libarchive\/libarchive\.wasm",?\s*\}/s,
  );
  assert.match(
    mirrorPage,
    /docx:\s*\{\s*worker:\s*true,\s*workerUrl:\s*"\/vendor\/docx\/docx\.worker\.js",\s*workerJsZipUrl:\s*"\/vendor\/docx\/jszip\.min\.js",?\s*\}/s,
  );
  assert.match(
    mirrorPage,
    /cad:\s*\{\s*wasmPath:\s*"\/wasm\/cad\/",\s*workerUrl:\s*"\/wasm\/cad\/dwg-worker\.js",\s*dwfWasmUrl:\s*"\/wasm\/cad\/dwfv-render\.wasm",?\s*\}/s,
  );
  assert.match(
    mirrorPage,
    /typst:\s*\{\s*compilerWasmUrl:\s*"\/wasm\/typst\/typst_ts_web_compiler_bg\.wasm",\s*rendererWasmUrl:\s*"\/wasm\/typst\/typst_ts_renderer_bg\.wasm",?\s*\}/s,
  );
  assert.match(mirrorPage, /data:\s*\{\s*sqlWasmUrl:\s*"\/wasm\/data\/sql-wasm\.wasm",?\s*\}/s);
  assert.match(mirrorPage, /drawing:\s*\{\s*preferOfficial:\s*false,?\s*\}/s);

  const requiredAssets = [
    "../public/vendor/libarchive/worker-bundle.js",
    "../public/vendor/libarchive/libarchive.wasm",
    "../public/vendor/docx/docx.worker.js",
    "../public/vendor/docx/jszip.min.js",
    "../public/wasm/cad/libredwg-web.js",
    "../public/wasm/cad/libredwg-web.wasm",
    "../public/wasm/cad/dwg-worker.js",
    "../public/wasm/cad/dwfv-render.wasm",
    "../public/wasm/typst/typst_ts_web_compiler_bg.wasm",
    "../public/wasm/typst/typst_ts_renderer_bg.wasm",
    "../public/wasm/data/sql-wasm.wasm",
  ];

  for (const assetPath of requiredAssets) {
    assert.equal(existsSync(new URL(assetPath, import.meta.url)), true, `missing ${assetPath}`);
  }
});

test("PDF renderer internal zoom controls are hidden because the preview header owns zoom", () => {
  const style = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.pdf-toolbar-group--zoom\)\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(style, /display:\s*none\s*!important;/);
});

test("PDF text layer stays selectable above the rendered canvas", () => {
  const layerStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.textLayer\)\s*\{[^}]*\}/s)?.[0] || "";
  const textStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.textLayer :is\(span,\s*br\)\)\s*\{[^}]*\}/s)?.[0] || "";
  const selectionStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.textLayer ::selection\)\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(layerStyle, /z-index:\s*3\s*!important;/);
  assert.match(layerStyle, /pointer-events:\s*auto\s*!important;/);
  assert.match(textStyle, /cursor:\s*text\s*!important;/);
  assert.match(textStyle, /user-select:\s*text\s*!important;/);
  assert.match(selectionStyle, /background:\s*color-mix\(in srgb, var\(--primary\) 36%, transparent\)\s*!important;/);
});

test("PDF search falls back to extracted PDF text when FileViewer search has no matches", () => {
  assert.match(mirrorPage, /type PdfJsModule = typeof import\("pdfjs-dist\/legacy\/build\/pdf\.mjs"\);/);
  assert.match(mirrorPage, /async function loadPdfJsForSearchFallback\(\)/);
  assert.match(mirrorPage, /pdfJsModulePromise = import\("pdfjs-dist\/legacy\/build\/pdf\.mjs"\)\.then/);
  assert.match(mirrorPage, /module\.GlobalWorkerOptions\.workerSrc = pdfWorkerUrl;/);
  assert.match(mirrorPage, /interface PdfSearchMatch/);
  assert.match(mirrorPage, /const pdfSearchMatches = ref<PdfSearchMatch\[\]>\(\[\]\)/);
  assert.match(mirrorPage, /async function searchPdfPreviewFallback\(needle: string, direction: 1 \| -1\)/);
  assert.match(mirrorPage, /await buildPdfSearchMatches\(needle\)/);
  assert.match(mirrorPage, /if \(\(!state \|\| state\.total <= 0\) && isPdfPreviewPath\(textPreviewMeta\.value\?\.name \|\| ""\)\) \{/);
  assert.match(mirrorPage, /const fallbackState = await searchPdfPreviewFallback\(needle, direction\);/);
});

test("preview backgrounds use theme variables", () => {
  const dockStyle = mirrorPage.match(/\.text-preview-dock\s*\{[^}]*\}/s)?.[0] || "";
  const focusStyle = mirrorPage.match(/\.text-preview-search input:focus\s*\{[^}]*\}/s)?.[0] || "";
  const viewerBodyStyle = mirrorPage.match(/\.file-viewer-preview-body\s*\{[^}]*\}/s)?.[0] || "";
  const viewerRootStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.file-viewer\)\s*\{[^}]*\}/s)?.[0] || "";
  const viewerContentStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.content\)\s*\{[^}]*\}/s)?.[0] || "";
  const statusStyle = mirrorPage.match(/\.text-preview-status\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(dockStyle, /background:\s*var\(--dock-bg\);/);
  assert.match(dockStyle, /border:\s*1px solid var\(--dock-border\);/);
  assert.match(focusStyle, /background:\s*var\(--control-bg-hover\);/);
  assert.match(viewerBodyStyle, /background:\s*var\(--surface-solid\);/);
  assert.match(viewerRootStyle, /background:\s*var\(--surface-solid\)\s*!important;/);
  assert.match(viewerRootStyle, /color:\s*var\(--text-main\);/);
  assert.match(viewerContentStyle, /background:\s*var\(--surface-alt\)\s*!important;/);
  assert.match(statusStyle, /background:\s*var\(--surface-solid\);/);
  assert.match(statusStyle, /border-top:\s*1px solid var\(--border\);/);
});

test("preview content restores native text selection inside the mirror stage", () => {
  const stageStyle = mirrorPage.match(/\.mirror-stage\s*\{[^}]*\}/s)?.[0] || "";
  const contentStyle = mirrorPage.match(/\.file-viewer-preview-body :deep\(\.content\)\s*\{[^}]*\}/s)?.[0] || "";

  assert.match(stageStyle, /user-select:\s*none;/);
  assert.match(stageStyle, /-webkit-user-select:\s*none;/);
  assert.match(contentStyle, /user-select:\s*text\s*!important;/);
  assert.match(contentStyle, /-webkit-user-select:\s*text\s*!important;/);
});

test("preview content allows native context menu copy", () => {
  assert.match(
    mirrorPage,
    /<div\s+ref="fileViewerPreviewBodyRef"\s+class="file-viewer-preview-body"\s+data-allow-app-context-menu="true">/,
  );
});

test("preview copy event writes selected preview text to the clipboard", () => {
  assert.match(mirrorPage, /function selectedTextPreviewText\(\)/);
  assert.match(mirrorPage, /function handleTextPreviewCopy\(event: ClipboardEvent\)/);
  assert.match(mirrorPage, /event\.clipboardData\.setData\("text\/plain", text\);/);
  assert.match(mirrorPage, /window\.addEventListener\("copy", handleTextPreviewCopy\);/);
  assert.match(mirrorPage, /window\.removeEventListener\("copy", handleTextPreviewCopy\);/);
});

test("FileViewer text renderers explicitly override inherited selection suppression", () => {
  assert.match(
    mirrorPage,
    /\.file-viewer-preview-body :deep\(\.content :is\([\s\S]*\.code-viewer \*[\s\S]*\.markdown-body \*[\s\S]*\.email-text[\s\S]*\.docx-fit-viewer \*[\s\S]*\)\)\s*\{[^}]*user-select:\s*text\s*!important;[^}]*-webkit-user-select:\s*text\s*!important;/s,
  );
});

test("spreadsheet cache progress floating notice is hidden because the preview header owns it", () => {
  const style =
    mirrorPage.match(/\.file-viewer-preview-body :deep\(\.excel-wrapper \.sheet-loading\)\s*\{[^}]*\}/s)?.[0] ||
    "";
  assert.match(style, /display:\s*none\s*!important;/);
  assert.match(
    mirrorPage,
    /\.file-viewer-preview-body :deep\(\.viewer-content-shell:has\(\.excel-wrapper\)\),\s*\.file-viewer-preview-body :deep\(\.content:has\(\.excel-wrapper\)\),\s*\.file-viewer-preview-body :deep\(\.excel-wrapper \.table-wrapper\)\s*\{[^}]*overflow:\s*visible\s*!important;/s,
  );
});

test("spreadsheet visual cache refill hint is hidden for every viewport", () => {
  const mediaStart = mirrorPage.indexOf("@media (max-width: 720px)");
  assert.ok(mediaStart > 0, "expected mobile media block after global preview styles");
  const globalPreviewStyles = mirrorPage.slice(0, mediaStart);
  const hintStyle =
    globalPreviewStyles.match(/\.file-viewer-preview-body :deep\(\.excel-wrapper \.sheet-loading > span:nth-child\(2\)\)\s*\{[^}]*\}/s)?.[0] ||
    "";

  assert.match(hintStyle, /display:\s*none\s*!important;/);
});

test("FileViewer uses the 2.x preset renderer stack without core private parser aliases", () => {
  assert.equal(packageJson.dependencies["@file-viewer/vue3"], "2.1.19");
  assert.equal(packageJson.dependencies["@file-viewer/preset-all"], "2.1.19");
  assert.equal(packageJson.dependencies["@file-viewer/renderer-spreadsheet"], "2.1.19");
  assert.equal(packageJson.devDependencies["@file-viewer/vite-plugin"], "2.1.19");
  assert.match(viteConfig, /import \{ fileViewerRenderers \} from "@file-viewer\/vite-plugin";/);
  assert.match(viteConfig, /fileViewerRenderers\(\{\s*preset:\s*"all",\s*inject:\s*false,\s*copyAssets:\s*true,/s);
  assert.match(viteConfig, /require\.resolve\("@file-viewer\/renderer-spreadsheet\/worker\/sheetjs"\)/);
  assert.match(viteConfig, /"@droidpilot\/file-viewer-spreadsheet-worker-sheetjs":\s*fileViewerSpreadsheetWorkerSheetjsPath/);
  assert.match(viteConfig, /fileViewerPresetAllRequire\.resolve\("@file-viewer\/renderer-epub\/package\.json"\)/);
  assert.match(viteConfig, /epubjsRequire\.resolve\("jszip\/package\.json"\)/);
  assert.match(viteConfig, /"jszip\/dist\/jszip":\s*jszipBrowserBundlePath/);
  assert.doesNotMatch(viteConfig, /@droidpilot\/file-viewer-spreadsheet-parser/);
  assert.doesNotMatch(viteConfig, /fileViewerSpreadsheetParserPath/);
  assert.doesNotMatch(viteConfig, /dist\/renderers\/spreadsheet\/worker\/sheetjs\/parser\.js/);
  assert.match(mirrorPage, /import \{ fileViewerPresetAll \} from "@file-viewer\/preset-all";/);
  assert.match(mirrorPage, /\.\.\/workers\/fileViewerSpreadsheet\.worker\?worker&url/);
  assert.match(mirrorPage, /preset:\s*fileViewerPresetAll,\s*rendererMode:\s*"replace",\s*autoRenderers:\s*false,/s);
  assert.match(mirrorPage, /spreadsheet:\s*\{\s*worker:\s*"auto",\s*workerUrl:\s*spreadsheetWorkerUrl,?\s*\}/s);
  assert.match(spreadsheetWorker, /@droidpilot\/file-viewer-spreadsheet-worker-sheetjs/);
  assert.match(spreadsheetWorker, /handleSpreadsheetWorkerRequest/);
  assert.doesNotMatch(spreadsheetWorker, /@droidpilot\/file-viewer-spreadsheet-parser/);
  assert.doesNotMatch(spreadsheetWorker, /@file-viewer\/core/);
});

test("FileViewer text-like previews are wired to the shared search controls", () => {
  assert.match(mirrorPage, /const textPreviewSearchAvailable = computed/);
  assert.match(mirrorPage, /v-if="textPreviewSearchAvailable"\s+class="text-preview-search"/);
  assert.match(mirrorPage, /const fileViewerRef = ref<FileViewerExpose \| null>\(null\)/);
  assert.match(mirrorPage, /@search-change="onFileViewerSearchChange"/);
  assert.match(mirrorPage, /ref="fileViewerRef"/);
  assert.match(mirrorPage, /async function searchFileViewerPreview\(direction: 1 \| -1\)/);
  assert.match(mirrorPage, /return searchFileViewerPreview\(direction\);/);
});

test("FileViewer search index observes content loaded after the first search", () => {
  assert.match(mirrorPage, /async function prepareFileViewerSearchIndex\(viewer = fileViewerRef\.value\)/);
  assert.match(mirrorPage, /await viewer\.collectDocumentAnchors\(\);/);
  assert.match(
    mirrorPage,
    /const shouldCollectDocumentAnchors = isTextPreviewPath\(previewName\) \|\| isFileViewerSearchPath\(previewName\);/,
  );
  assert.match(mirrorPage, /if \(!shouldCollectDocumentAnchors\) \{\s*return;\s*\}/s);
  assert.match(
    mirrorPage,
    /if \(isTextPreviewPath\(previewName\)\) \{\s*fileViewerDocumentLineCount\.value = resolveFileViewerDocumentLineCount\(anchors\);\s*\}/s,
  );
  assert.match(
    mirrorPage,
    /: await prepareFileViewerSearchIndex\(viewer\)\.then\(\(\) => viewer\.searchDocument\(needle\)\);/s,
  );
});

test("FileViewer search uses supported options without chunked text regex controls", () => {
  assert.match(mirrorPage, /search:\s*\{\s*enabled:\s*textPreviewSearchAvailable\.value,/s);
  assert.match(mirrorPage, /caseSensitive:\s*textPreviewSearchCaseSensitive\.value/);
  assert.match(mirrorPage, /wholeWord:\s*textPreviewSearchWholeWord\.value/);
  assert.doesNotMatch(mirrorPage, /title="正则匹配"/);
});

test("self-managed chunked text preview is not used", () => {
  assert.doesNotMatch(mirrorPage, /read_local_text_chunk/);
  assert.doesNotMatch(mirrorPage, /read_remote_text_chunk/);
  assert.doesNotMatch(mirrorPage, /loadNextTextPreviewChunk/);
  assert.doesNotMatch(mirrorPage, /text-preview-body/);
  assert.doesNotMatch(mirrorPage, /textPreviewVisibleLines/);
  assert.doesNotMatch(mirrorPage, /v-if="textPreviewMode === 'text'"/);
});

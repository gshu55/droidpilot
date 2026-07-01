/// <reference types="vite/client" />

declare const __APP_VERSION__: string;

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module "@droidpilot/file-viewer-spreadsheet-parser" {
  export interface SpreadsheetParserContext {
    workbook: unknown;
    sheets: unknown[];
  }

  export interface SpreadsheetWorkerRequest {
    type: string;
    payload?: Record<string, unknown>;
  }

  export interface SpreadsheetWorkerResponse {
    type: string;
    payload?: Record<string, unknown>;
  }

  export function createSpreadsheetParserContext(): SpreadsheetParserContext;

  export function handleSpreadsheetWorkerRequest(
    context: SpreadsheetParserContext,
    request: SpreadsheetWorkerRequest,
  ): SpreadsheetWorkerResponse[];
}

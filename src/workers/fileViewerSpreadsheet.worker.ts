import {
  createSpreadsheetParserContext,
  handleSpreadsheetWorkerRequest,
  type SpreadsheetWorkerRequest,
  type SpreadsheetWorkerResponse,
} from "@droidpilot/file-viewer-spreadsheet-parser";

const ctx = self as unknown as {
  onmessage: ((message: MessageEvent<SpreadsheetWorkerRequest>) => void) | null;
  onerror: ((event: ErrorEvent) => void) | null;
  postMessage: (message: SpreadsheetWorkerResponse) => void;
};
const context = createSpreadsheetParserContext();

ctx.onmessage = (message) => {
  handleSpreadsheetWorkerRequest(context, message.data).forEach((response) => {
    ctx.postMessage(response);
  });
};

ctx.onerror = (error) => {
  console.error(error);
};

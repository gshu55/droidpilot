export interface FileEntrySizeLike {
  isDir: boolean;
  sizeBytes?: number | null;
}

export function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) {
    return "0 B";
  }
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size >= 10 || unitIndex === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unitIndex]}`;
}

export function formatEntrySize(entry: FileEntrySizeLike) {
  if (entry.isDir) {
    return "";
  }
  if (typeof entry.sizeBytes !== "number") {
    return "--";
  }
  return formatBytes(entry.sizeBytes);
}

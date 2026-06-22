export function clampSplitPanePercent(
  percent: number,
  containerWidth: number,
  minPaneWidth: number,
  handleWidth: number,
) {
  if (!Number.isFinite(percent) || !Number.isFinite(containerWidth) || !Number.isFinite(minPaneWidth) || !Number.isFinite(handleWidth)) {
    return 50;
  }

  const availableWidth = containerWidth - handleWidth;
  if (availableWidth <= minPaneWidth * 2) {
    return 50;
  }

  const minPercent = (minPaneWidth / availableWidth) * 100;
  const maxPercent = 100 - minPercent;
  return Math.min(maxPercent, Math.max(minPercent, percent));
}

export function splitPaneGridTemplateColumns(
  leftPercent: number,
  minPaneWidth: number,
  handleWidth: number,
) {
  const handleOffset = (handleWidth * leftPercent) / 100;
  return `minmax(${minPaneWidth}px, calc(${leftPercent}% - ${handleOffset}px)) ${handleWidth}px minmax(${minPaneWidth}px, 1fr)`;
}

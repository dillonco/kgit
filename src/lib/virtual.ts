// Fixed-height row windowing for the commit graph: only the rows in (or
// near) the viewport are rendered, so a 20k-commit history costs the same
// DOM as a 50-commit one.

export interface RowWindow {
  start: number;
  end: number; // exclusive
  padTop: number;
  padBottom: number;
}

export function rowWindow(
  total: number,
  rowHeight: number,
  scrollTop: number,
  viewportHeight: number,
  overscan = 12,
): RowWindow {
  if (total <= 0 || rowHeight <= 0) return { start: 0, end: 0, padTop: 0, padBottom: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + 1;
  const start = Math.max(0, Math.min(total, first - overscan));
  const end = Math.max(start, Math.min(total, first + visible + overscan));
  return { start, end, padTop: start * rowHeight, padBottom: (total - end) * rowHeight };
}

/** True once the viewport is within `threshold` rows of the end. */
export function nearEnd(
  total: number,
  rowHeight: number,
  scrollTop: number,
  viewportHeight: number,
  threshold = 40,
): boolean {
  return scrollTop + viewportHeight >= (total - threshold) * rowHeight;
}

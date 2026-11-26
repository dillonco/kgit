import { describe, expect, it } from "vitest";
import { nearEnd, rowWindow } from "./virtual";
import { refreshScope } from "./live";

describe("rowWindow", () => {
  it("renders only the viewport plus overscan", () => {
    const window = rowWindow(10_000, 28, 28 * 500, 28 * 20, 10);
    expect(window.start).toBe(490);
    expect(window.end).toBe(531);
    expect(window.padTop).toBe(490 * 28);
    expect(window.padBottom).toBe((10_000 - 531) * 28);
  });

  it("clamps at both ends", () => {
    expect(rowWindow(5, 28, 0, 1000)).toEqual({ start: 0, end: 5, padTop: 0, padBottom: 0 });
    const tail = rowWindow(100, 28, 28 * 99, 280, 4);
    expect(tail.end).toBe(100);
    expect(tail.padBottom).toBe(0);
  });

  it("is empty for no rows", () => {
    expect(rowWindow(0, 28, 0, 500)).toEqual({ start: 0, end: 0, padTop: 0, padBottom: 0 });
  });

  it("keeps padding plus rendered rows equal to the full height", () => {
    for (const top of [0, 1000, 27_000, 55_000]) {
      const w = rowWindow(2000, 28, top, 600);
      expect(w.padTop + (w.end - w.start) * 28 + w.padBottom).toBe(2000 * 28);
    }
  });
});

describe("nearEnd", () => {
  it("fires within the threshold of the last row", () => {
    expect(nearEnd(250, 28, 0, 600)).toBe(false);
    expect(nearEnd(250, 28, 250 * 28 - 600 - 28 * 30, 600)).toBe(true);
  });
});

describe("refreshScope", () => {
  it("reloads everything for ref changes and only status otherwise", () => {
    expect(refreshScope({ worktree: true, index: false, refs: true })).toBe("full");
    expect(refreshScope({ worktree: true, index: false, refs: false })).toBe("status");
    expect(refreshScope({ worktree: false, index: true, refs: false })).toBe("status");
    expect(refreshScope({ worktree: false, index: false, refs: false })).toBeNull();
  });
});

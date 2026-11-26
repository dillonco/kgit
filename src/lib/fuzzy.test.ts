import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzy", () => {
  it("requires every character in order", () => {
    expect(fuzzyScore("chk", "Checkout branch")).not.toBeNull();
    expect(fuzzyScore("tc", "Checkout")).toBeNull();
  });

  it("ranks word-start and exact matches first", () => {
    const items = ["src/lib/graph.ts", "src/lib/git.ts", "docs/getting-started.md", "src/App.svelte"];
    expect(fuzzyFilter(items, "git", (x) => x)[0]).toBe("src/lib/git.ts");
    expect(fuzzyFilter(items, "app", (x) => x)[0]).toBe("src/App.svelte");
  });

  it("returns everything for an empty query, capped", () => {
    expect(fuzzyFilter([1, 2, 3], "", String, 2)).toEqual([1, 2]);
  });
});

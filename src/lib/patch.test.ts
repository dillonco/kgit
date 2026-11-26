import { describe, expect, it } from "vitest";
import { buildLinePatch, changeLineIndexes } from "./patch";
import { conflictCount, hasMarkers, parseConflicts, resolveConflicts } from "./conflicts";

const DIFF = [
  "diff --git a/f.txt b/f.txt",
  "index 111..222 100644",
  "--- a/f.txt",
  "+++ b/f.txt",
  "@@ -1,4 +1,4 @@",
  " keep",
  "-old one",
  "-old two",
  "+new one",
  "+new two",
  " tail",
  "@@ -10,2 +10,3 @@ fn x",
  " ctx",
  "+added",
  " end",
  "",
].join("\n");

describe("buildLinePatch", () => {
  it("finds every change line", () => {
    expect(changeLineIndexes(DIFF)).toEqual([6, 7, 8, 9, 13]);
  });

  it("stages one replaced line: unselected removals become context, unselected additions vanish", () => {
    const patch = buildLinePatch(DIFF, new Set([6, 8]), false);
    expect(patch).toBe(
      [
        "diff --git a/f.txt b/f.txt",
        "index 111..222 100644",
        "--- a/f.txt",
        "+++ b/f.txt",
        "@@ -1,4 +1,4 @@",
        " keep",
        "-old one",
        " old two",
        "+new one",
        " tail",
        "",
      ].join("\n"),
    );
  });

  it("shifts later hunks by the lines earlier hunks added", () => {
    const patch = buildLinePatch(DIFF, new Set([8, 13]), false);
    expect(patch).toContain("@@ -1,4 +1,5 @@");
    expect(patch).toContain("@@ -10,2 +11,3 @@ fn x");
  });

  it("reverse patches keep unselected additions as context and drop unselected removals", () => {
    const patch = buildLinePatch(DIFF, new Set([9]), true);
    expect(patch).toContain(["@@ -1,3 +1,4 @@", " keep", " new one", "+new two", " tail"].join("\n"));
  });

  it("returns nothing when no change line is selected", () => {
    expect(buildLinePatch(DIFF, new Set([5, 10]), false)).toBe("");
  });

  it("drops a no-newline marker whose line was dropped", () => {
    const diff = ["--- a/x", "+++ b/x", "@@ -1 +1 @@", "-a", "\\ No newline at end of file", "+b", ""].join("\n");
    const patch = buildLinePatch(diff, new Set([5]), false);
    expect(patch).toBe(["--- a/x", "+++ b/x", "@@ -1,1 +1,2 @@", " a", "\\ No newline at end of file", "+b", ""].join("\n"));
  });
});

describe("conflicts", () => {
  const FILE = "top\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nmid\n<<<<<<< HEAD\na\n||||||| base\nb\n=======\nc\n>>>>>>> feature\nend\n";

  it("splits text and conflict regions with diff3 bases", () => {
    const segments = parseConflicts(FILE);
    expect(conflictCount(segments)).toBe(2);
    const second = segments.find((s) => s.kind === "conflict" && s.index === 1);
    expect(second).toMatchObject({ ours: "a\n", base: "b\n", theirs: "c\n", theirsLabel: "feature" });
  });

  it("resolves each conflict on its own and keeps unresolved markers", () => {
    const segments = parseConflicts(FILE);
    const partial = resolveConflicts(segments, { 0: "theirs" });
    expect(partial.startsWith("top\ntheirs\nmid\n<<<<<<< HEAD\na\n")).toBe(true);
    expect(hasMarkers(partial)).toBe(true);
    const done = resolveConflicts(segments, { 0: "ours-theirs", 1: "base" });
    expect(done).toBe("top\nours\ntheirs\nmid\nb\nend\n");
    expect(hasMarkers(done)).toBe(false);
  });

  it("uses custom (AI) text for a region", () => {
    const segments = parseConflicts(FILE);
    const done = resolveConflicts(segments, { 0: { text: "merged\n", confidence: 0.9 }, 1: "ours" });
    expect(done).toBe("top\nmerged\nmid\na\nend\n");
  });

  it("round-trips a file with no choices", () => {
    expect(resolveConflicts(parseConflicts(FILE), {})).toBe(FILE);
  });

  it("leaves an unterminated marker as text", () => {
    const segments = parseConflicts("x\n<<<<<<< HEAD\nnope\n");
    expect(conflictCount(segments)).toBe(0);
  });
});

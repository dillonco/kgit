// End-to-end: the line patches must apply with the real `git apply`, the
// same way the backend's apply_hunk runs them.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { buildLinePatch, changeLineIndexes } from "./patch";

let dir = "";
const git = (args: string[], input?: string) =>
  execFileSync("git", args, { cwd: dir, input, encoding: "utf8", env: { ...process.env, GIT_CONFIG_NOSYSTEM: "1" } });

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "kgit-patch-"));
  git(["init", "-q", "-b", "main"]);
  git(["config", "user.email", "t@kgit.dev"]);
  git(["config", "user.name", "T"]);
  writeFileSync(join(dir, "f.txt"), ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"].join("\n") + "\n");
  git(["add", "f.txt"]);
  git(["commit", "-qm", "init"]);
  // Two separate hunks, each replacing a line and adding one.
  writeFileSync(join(dir, "f.txt"), ["a", "B", "B2", "c", "d", "e", "f", "g", "h", "i", "J", "J2", "k", "l"].join("\n") + "\n");
});

afterEach(() => rmSync(dir, { recursive: true, force: true }));

describe("line patches against real git", () => {
  it("stages only the selected lines, in either hunk", () => {
    const diff = git(["diff", "--", "f.txt"]);
    const lines = diff.split("\n");
    const pick = new Set(changeLineIndexes(diff).filter((i) => lines[i] === "+J2" || lines[i] === "-b" || lines[i] === "+B"));
    git(["apply", "--cached", "-"], buildLinePatch(diff, pick, false));
    const staged = git(["show", ":f.txt"]);
    expect(staged).toBe(["a", "B", "c", "d", "e", "f", "g", "h", "i", "j", "J2", "k", "l"].join("\n") + "\n");
    // The working tree is untouched.
    expect(readFileSync(join(dir, "f.txt"), "utf8")).toContain("B2");
  });

  it("unstages selected lines from the index with a reverse patch", () => {
    git(["add", "f.txt"]);
    const diff = git(["diff", "--cached", "--", "f.txt"]);
    const lines = diff.split("\n");
    const pick = new Set(changeLineIndexes(diff).filter((i) => lines[i] === "+B2"));
    git(["apply", "--cached", "--reverse", "-"], buildLinePatch(diff, pick, true));
    expect(git(["show", ":f.txt"])).toBe(["a", "B", "c", "d", "e", "f", "g", "h", "i", "J", "J2", "k", "l"].join("\n") + "\n");
  });

  it("discards selected lines from the working tree", () => {
    const diff = git(["diff", "--", "f.txt"]);
    const lines = diff.split("\n");
    const pick = new Set(changeLineIndexes(diff).filter((i) => lines[i] === "-j" || lines[i] === "+J"));
    git(["apply", "--reverse", "-"], buildLinePatch(diff, pick, true));
    expect(readFileSync(join(dir, "f.txt"), "utf8")).toBe(
      ["a", "B", "B2", "c", "d", "e", "f", "g", "h", "i", "j", "J2", "k", "l"].join("\n") + "\n",
    );
  });
});

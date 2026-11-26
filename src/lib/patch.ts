// Builds a patch that applies only the selected +/- lines of a diff, for
// line-level stage / unstage / discard (`git apply [--cached] [--reverse]`).
//
// Forward (stage): an unselected "-" stays as context and an unselected "+"
// is dropped, so the patch's pre-image still matches the index.
// Reverse (unstage, discard): applied with --reverse, so the post-image must
// match what is there now: an unselected "+" becomes context and an
// unselected "-" is dropped.

interface Hunk {
  oldStart: number;
  newStart: number;
  suffix: string;
  /** [raw line index, text] for each body line. */
  body: [number, string][];
}

const HUNK_HEADER = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/;

function splitDiff(diff: string): { header: string[]; hunks: Hunk[] } {
  const header: string[] = [];
  const hunks: Hunk[] = [];
  diff.split("\n").forEach((line, index) => {
    const match = line.match(HUNK_HEADER);
    if (match) {
      hunks.push({ oldStart: Number(match[1]), newStart: Number(match[2]), suffix: match[3], body: [] });
    } else if (hunks.length) {
      if (line !== "" || index < diff.split("\n").length - 1) hunks[hunks.length - 1].body.push([index, line]);
    } else {
      header.push(line);
    }
  });
  // A trailing empty line is the diff's final newline, not a context line.
  for (const hunk of hunks) {
    while (hunk.body.length && hunk.body[hunk.body.length - 1][1] === "") hunk.body.pop();
  }
  return { header, hunks };
}

/** Raw line indexes (into `diff.split("\n")`) of every +/- line. */
export function changeLineIndexes(diff: string): number[] {
  return diff
    .split("\n")
    .map((line, index) => ({ line, index }))
    .filter(({ line }) => (line.startsWith("+") && !line.startsWith("+++")) || (line.startsWith("-") && !line.startsWith("---")))
    .map(({ index }) => index);
}

/**
 * Returns a patch containing only `selected` change lines (raw line indexes
 * into the diff), or "" when none of the selected lines is a change.
 */
export function buildLinePatch(diff: string, selected: Set<number>, reverse: boolean): string {
  const { header, hunks } = splitDiff(diff);
  const out: string[] = [...header];
  let delta = 0;
  let emitted = 0;
  for (const hunk of hunks) {
    const lines: string[] = [];
    let changed = false;
    let lastKept = true;
    for (const [index, line] of hunk.body) {
      const kind = line[0];
      const picked = selected.has(index);
      if (kind === "\\") {
        // "\ No newline at end of file" belongs to the line before it.
        if (lastKept) lines.push(line);
        continue;
      }
      if (kind === "+") {
        if (picked) {
          lines.push(line);
          changed = true;
          lastKept = true;
        } else if (reverse) {
          lines.push(` ${line.slice(1)}`);
          lastKept = true;
        } else {
          lastKept = false;
        }
      } else if (kind === "-") {
        if (picked) {
          lines.push(line);
          changed = true;
          lastKept = true;
        } else if (!reverse) {
          lines.push(` ${line.slice(1)}`);
          lastKept = true;
        } else {
          lastKept = false;
        }
      } else {
        lines.push(line === "" ? " " : line);
        lastKept = true;
      }
    }
    if (!changed) continue;
    const oldCount = lines.filter((line) => line[0] === " " || line[0] === "-").length;
    const newCount = lines.filter((line) => line[0] === " " || line[0] === "+").length;
    // Forward patches keep the old side's positions; reverse patches keep
    // the new side's (that is what has to match the target).
    const oldStart = reverse ? hunk.newStart - delta : hunk.oldStart;
    const newStart = reverse ? hunk.newStart : hunk.oldStart + delta;
    out.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@${hunk.suffix}`, ...lines);
    delta += newCount - oldCount;
    emitted += 1;
  }
  return emitted ? `${out.join("\n")}\n` : "";
}

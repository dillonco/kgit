// Splits a file with conflict markers into plain text and conflict regions,
// so each conflict can be resolved on its own (ours, theirs, both) instead
// of taking one side for the whole file.

export type ConflictChoice =
  | "ours"
  | "theirs"
  | "ours-theirs"
  | "theirs-ours"
  | "base"
  /** Text written by hand or proposed by AI, replacing the whole region. */
  | { text: string; confidence?: number; explanation?: string };

export type Segment =
  | { kind: "text"; text: string }
  | {
      kind: "conflict";
      index: number;
      ours: string;
      base: string | null;
      theirs: string;
      oursLabel: string;
      theirsLabel: string;
    };

export function parseConflicts(content: string): Segment[] {
  const lines = content.split(/(?<=\n)/);
  const segments: Segment[] = [];
  let text = "";
  let index = 0;
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.startsWith("<<<<<<<")) {
      text += line;
      i += 1;
      continue;
    }
    // Find the whole region before committing to it; an unterminated
    // marker is left as plain text.
    let j = i + 1;
    let ours = "";
    let base: string | null = null;
    let theirs = "";
    let section: "ours" | "base" | "theirs" = "ours";
    let theirsLabel = "";
    let closed = false;
    for (; j < lines.length; j += 1) {
      const current = lines[j];
      if (section === "ours" && current.startsWith("|||||||")) {
        section = "base";
        base = "";
      } else if ((section === "ours" || section === "base") && current.startsWith("=======")) {
        section = "theirs";
      } else if (section === "theirs" && current.startsWith(">>>>>>>")) {
        theirsLabel = current.slice(7).trim();
        closed = true;
        break;
      } else if (section === "ours") ours += current;
      else if (section === "base") base += current;
      else theirs += current;
    }
    if (!closed) {
      text += line;
      i += 1;
      continue;
    }
    if (text) segments.push({ kind: "text", text });
    text = "";
    segments.push({ kind: "conflict", index, ours, base, theirs, oursLabel: line.slice(7).trim(), theirsLabel });
    index += 1;
    i = j + 1;
  }
  if (text) segments.push({ kind: "text", text });
  return segments;
}

export function conflictCount(segments: Segment[]): number {
  return segments.filter((segment) => segment.kind === "conflict").length;
}

function pick(segment: Extract<Segment, { kind: "conflict" }>, choice: ConflictChoice): string {
  if (typeof choice === "object") return choice.text;
  switch (choice) {
    case "ours":
      return segment.ours;
    case "theirs":
      return segment.theirs;
    case "base":
      return segment.base ?? "";
    case "ours-theirs":
      return segment.ours + segment.theirs;
    case "theirs-ours":
      return segment.theirs + segment.ours;
  }
}

/** Rebuilds the file. Conflicts without a choice keep their markers. */
export function resolveConflicts(segments: Segment[], choices: Record<number, ConflictChoice | undefined>): string {
  return segments
    .map((segment) => {
      if (segment.kind === "text") return segment.text;
      const choice = choices[segment.index];
      if (choice) return pick(segment, choice);
      const base = segment.base != null ? `||||||| base\n${segment.base}` : "";
      return `<<<<<<< ${segment.oursLabel}\n${segment.ours}${base}=======\n${segment.theirs}>>>>>>> ${segment.theirsLabel}\n`;
    })
    .join("");
}

/** True while any conflict marker is still in the text. */
export function hasMarkers(content: string): boolean {
  return /^(<<<<<<<|>>>>>>>)( |$)/m.test(content) || /^=======$/m.test(content);
}

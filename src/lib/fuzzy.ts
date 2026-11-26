// Small fuzzy matcher for the command palette and quick open: every query
// character must appear in order; matches at word starts and runs of
// consecutive characters score higher, and shorter targets win ties.

export function fuzzyScore(query: string, target: string): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const t = target.toLowerCase();
  let score = 0;
  let ti = 0;
  let run = 0;
  for (const ch of q) {
    if (ch === " ") continue;
    const found = t.indexOf(ch, ti);
    if (found < 0) return null;
    const boundary = found === 0 || /[\s/_.\-:]/.test(t[found - 1]);
    run = found === ti ? run + 1 : 0;
    score += 1 + (boundary ? 6 : 0) + run * 3 - Math.min(found - ti, 10) * 0.2;
    ti = found + 1;
  }
  // An exact substring is the strongest signal.
  if (t.includes(q)) score += 15;
  return score - t.length * 0.01;
}

export function fuzzyFilter<T>(items: T[], query: string, text: (item: T) => string, limit = 200): T[] {
  if (!query.trim()) return items.slice(0, limit);
  return items
    .map((item) => ({ item, score: fuzzyScore(query, text(item)) }))
    .filter((entry): entry is { item: T; score: number } => entry.score != null)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((entry) => entry.item);
}

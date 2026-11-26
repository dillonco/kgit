// Live repository updates from the backend file watcher (src-tauri/src/watch.rs).
import { isDemo, call } from "./git";

export interface RepoChange {
  root: string;
  worktree: boolean;
  index: boolean;
  refs: boolean;
}

/** Starts watching the open repository and calls `onChange` for each burst
 * of changes. Resolves to an unsubscribe function; a no-op in the demo. */
export async function watchRepository(onChange: (change: RepoChange) => void): Promise<() => void> {
  if (isDemo) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = await listen<RepoChange>("repo-changed", (event) => onChange(event.payload));
  await call("watch_repository");
  return unlisten;
}

/** What a change needs: a full state + graph reload, a status-only reload, or nothing. */
export function refreshScope(change: Pick<RepoChange, "worktree" | "index" | "refs">): "full" | "status" | null {
  if (change.refs) return "full";
  if (change.worktree || change.index) return "status";
  return null;
}

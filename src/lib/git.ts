import type {
  AiStatus,
  CommitExplanation,
  ConflictProposal,
  Issue,
  PullRequest,
  RepoSummary,
  WorkspaceResult,
  BlameLine,
  BranchCleanupReport,
  ConflictCheck,
  FileHistoryEntry,
  ReflogEntry,
  RepoConfig,
  Signature,
  CloneProgress,
  CloneRequest,
  CloneTarget,
  CommitDetail,
  CommitGraph,
  ConflictFile,
  FileDiff,
  GhRepo,
  GhStatus,
  GitAction,
  GitResult,
  RebasePlan,
  RemoteBranches,
  RebaseStep,
  RefCompare,
  RepositoryState,
  WorktreeStatus,
} from "./types";

// When the UI runs in a plain browser (`npm run dev:ui`) there is no Tauri
// backend, so route every call to the in-memory demo repository instead.
// This keeps the full UI testable for design work without a Rust build.
const hasTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** True in the browser-only demo build, where every call hits the fake repo. */
export const isDemo = !hasTauri;

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (hasTauri) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }
  const { demoInvoke } = await import("./demo");
  return demoInvoke<T>(command, args ?? {});
}

export function getRepositoryState(): Promise<RepositoryState> {
  return call("get_repository_state");
}

export function getWorktreeStatus(): Promise<WorktreeStatus> {
  return call("get_worktree_status");
}

export function homeDir(): Promise<string | null> {
  return call("home_dir");
}

/** `limit` commits of the graph after skipping the newest `skip`. */
export function getCommitGraph(limit = 500, skip = 0): Promise<CommitGraph> {
  return call("get_commit_graph", { limit, skip });
}

export function getCommitDetail(hash: string): Promise<CommitDetail> {
  return call("get_commit_detail", { hash });
}

export function getCommitTree(hash: string): Promise<string[]> {
  return call("get_commit_tree", { hash });
}

export function getCommitFileDiff(hash: string, path: string): Promise<FileDiff> {
  return call("get_commit_file_diff", { hash, path });
}

export function runGitAction(action: GitAction): Promise<GitResult> {
  return call("run_git_action", { action });
}

export function setRepositoryPath(path: string): Promise<RepositoryState> {
  return call("set_repository_path", { path });
}

export function openTerminal(): Promise<GitResult> {
  return call("open_terminal");
}

export function pickRepositoryFolder(prompt: string | null = null): Promise<string | null> {
  return call<string | null>("pick_repository_folder", { prompt });
}

export function createRepository(path: string): Promise<RepositoryState> {
  return call("create_repository", { path });
}

/** Streams `git clone --progress` updates to `onProgress` while it runs. */
export async function cloneRepository(
  request: CloneRequest,
  onProgress?: (progress: CloneProgress) => void,
): Promise<RepositoryState> {
  if (hasTauri) {
    const { invoke, Channel } = await import("@tauri-apps/api/core");
    const channel = new Channel<CloneProgress>();
    if (onProgress) channel.onmessage = onProgress;
    return invoke<RepositoryState>("clone_repository", { request, onProgress: channel });
  }
  const { demoClone } = await import("./demo");
  return demoClone(request, onProgress);
}

/** Stops the running clone; resolves false when none is running. */
export function cancelClone(): Promise<boolean> {
  return call("cancel_clone");
}

export function inspectCloneTarget(path: string): Promise<CloneTarget> {
  return call("inspect_clone_target", { path });
}

/** The subset of `paths` that already hold a git checkout. */
export function existingCheckouts(paths: string[]): Promise<string[]> {
  return call("existing_checkouts", { paths });
}

export function applyHunk(patch: string, mode: "stage" | "unstage" | "discard"): Promise<GitResult> {
  return call("apply_hunk", { patch, mode });
}

export function getConflictFile(path: string): Promise<ConflictFile> {
  return call("get_conflict_file", { path });
}

export function getFileDiff(path: string, staged: boolean): Promise<FileDiff> {
  return call("get_file_diff", { path, staged });
}

export function getFileContent(path: string, staged: boolean): Promise<string> {
  return call("get_file_content", { path, staged });
}

export function getFileBlame(path: string): Promise<string> {
  return call("get_file_blame", { path });
}

export function getFileHistory(path: string): Promise<string> {
  return call("get_file_history", { path });
}

export function saveConflictResolution(path: string, content: string): Promise<GitResult> {
  return call("save_conflict_resolution", { path, content });
}

export function getBranchCleanup(base: string | null, staleDays: number | null): Promise<BranchCleanupReport> {
  return call("get_branch_cleanup", { base, staleDays });
}

export function getRefCompare(base: string | null, head: string, threeDot: boolean): Promise<RefCompare> {
  return call("get_ref_compare", { base, head, threeDot });
}

export function getRefFileDiff(
  base: string | null,
  head: string,
  path: string,
  threeDot: boolean,
): Promise<FileDiff> {
  return call("get_ref_file_diff", { base, head, path, threeDot });
}

export function ghStatus(): Promise<GhStatus> {
  return call("gh_status");
}

/** Every repository the `gh` account can reach, across all owners. */
export function ghRepoList(): Promise<GhRepo[]> {
  return call("gh_repo_list");
}

/** `owner/name` a fork was made from, or null. */
export function ghRepoParent(nameWithOwner: string): Promise<string | null> {
  return call("gh_repo_parent", { nameWithOwner });
}

export function listRemoteBranches(url: string): Promise<RemoteBranches> {
  return call("list_remote_branches", { url });
}

export function getRebasePlan(base: string | null): Promise<RebasePlan> {
  return call("get_rebase_plan", { base });
}

export function runInteractiveRebase(base: string, steps: RebaseStep[]): Promise<GitResult> {
  return call("run_interactive_rebase", { base, steps });
}

/** Test-merges `target` into HEAD without touching the tree. */
export function checkConflicts(target: string): Promise<ConflictCheck> {
  return call("check_conflicts", { target });
}

/** The branch `branch` was created from (per its reflog), else the repository default. */
export function getBaseBranch(branch?: string | null): Promise<string> {
  return call("get_base_branch", { branch: branch ?? null });
}

export function getStashDetail(name: string): Promise<CommitDetail> {
  return call("get_stash_detail", { name });
}

export function getStashFileDiff(name: string, path: string): Promise<FileDiff> {
  return call("get_stash_file_diff", { name, path });
}

export function getReflog(limit = 200): Promise<ReflogEntry[]> {
  return call("get_reflog", { limit });
}

export function getFileHistoryEntries(path: string): Promise<FileHistoryEntry[]> {
  return call("get_file_history_entries", { path });
}

export function getBlameLines(path: string): Promise<BlameLine[]> {
  return call("get_blame_lines", { path });
}

export function listFiles(): Promise<string[]> {
  return call("list_files");
}

/** Checks out `branch`, stashing local changes first and restoring them after. */
export function checkoutWithAutostash(branch: string): Promise<GitResult> {
  return call("checkout_with_autostash", { branch });
}

export function getRepoConfig(): Promise<RepoConfig> {
  return call("get_repo_config");
}

/** Writes (or, with an empty value, unsets) an allow-listed git config key. */
export function setConfig(key: string, value: string | null, global = false): Promise<GitResult> {
  return call("set_config", { key, value, global });
}

export function getCommitSignature(hash: string): Promise<Signature> {
  return call("get_commit_signature", { hash });
}

/** source: "commit" | "range" | "staged" | "worktree" */
export function makePatch(source: string, target: string | null = null): Promise<string> {
  return call("make_patch", { source, target });
}

export function applyPatch(patch: string, index = false): Promise<GitResult> {
  return call("apply_patch", { patch, index });
}

export function writeTextFile(path: string, content: string): Promise<void> {
  return call("write_text_file", { path, content });
}

export function readTextFile(path: string): Promise<string> {
  return call("read_text_file", { path });
}

/** Opens the repo (or a file in it) in `app`, or reveals it in the file manager. */
export function openExternal(app: string | null, path: string | null = null, reveal = false): Promise<GitResult> {
  return call("open_external", { app, path, reveal });
}

export function launchTool(kind: "diff" | "merge", path: string, tool: string | null = null): Promise<GitResult> {
  return call("launch_tool", { kind, path, tool });
}

export function pickSavePath(prompt: string, defaultName: string): Promise<string | null> {
  return call("pick_save_path", { prompt, defaultName });
}

export function pickOpenFile(prompt: string): Promise<string | null> {
  return call("pick_open_file", { prompt });
}

// ---- GitHub: pull requests, issues, shared patches (through gh) ----

export function ghPrList(): Promise<PullRequest[]> {
  return call("gh_pr_list");
}

/** [opened by you, awaiting your review], across every repository. */
export function ghMyPrs(): Promise<[PullRequest[], PullRequest[]]> {
  return call("gh_my_prs");
}

export function ghPrCreate(title: string, body: string, base: string, head: string, draft: boolean): Promise<string> {
  return call("gh_pr_create", { title, body, base, head, draft });
}

export function ghPrCheckout(number: number): Promise<GitResult> {
  return call("gh_pr_checkout", { number });
}

export function ghIssueList(mine: boolean): Promise<Issue[]> {
  return call("gh_issue_list", { mine });
}

export function issueBranch(number: number, title: string): Promise<string> {
  return call("issue_branch", { number, title });
}

/** Uploads a patch as a gist (secret unless `isPublic`) and returns its URL. */
export function sharePatch(patch: string, name: string, description: string, isPublic = false): Promise<string> {
  return call("share_patch", { patch, name, description, public: isPublic });
}

export function fetchSharedPatch(link: string): Promise<string> {
  return call("fetch_shared_patch", { link });
}

export function openUrl(url: string): Promise<void> {
  return call("open_url", { url });
}

// ---- AI (Claude API) ----

export function aiStatus(): Promise<AiStatus> {
  return call("ai_status");
}

/** Saves the key in the keychain; an empty key removes it. */
export function aiSetKey(key: string): Promise<void> {
  return call("ai_set_key", { key });
}

export function aiCommitMessage(model: string | null): Promise<{ subject: string; body: string }> {
  return call("ai_commit_message", { model });
}

export function aiExplainCommit(hash: string, model: string | null): Promise<CommitExplanation> {
  return call("ai_explain_commit", { hash, model });
}

export function aiResolveConflicts(
  path: string,
  file: string,
  regions: { index: number; ours: string; theirs: string; base: string | null }[],
  model: string | null,
): Promise<ConflictProposal> {
  return call("ai_resolve_conflicts", { path, file, regions, model });
}

// ---- workspaces ----

export function workspaceStatus(paths: string[]): Promise<RepoSummary[]> {
  return call("workspace_status", { paths });
}

export function workspaceRun(paths: string[], kind: "fetch" | "pull"): Promise<WorkspaceResult[]> {
  return call("workspace_run", { paths, kind });
}

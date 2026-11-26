<script lang="ts">
  import {
    aiCommitMessage,
    aiExplainCommit,
    aiResolveConflicts,
    aiSetKey,
    aiStatus,
    fetchSharedPatch,
    ghPrCheckout,
    issueBranch,
    openUrl,
    sharePatch,
    applyHunk,
    applyPatch,
    checkConflicts,
    checkoutWithAutostash,
    getCommitSignature,
    launchTool,
    listFiles,
    makePatch,
    openExternal,
    pickOpenFile,
    pickSavePath,
    readTextFile,
    writeTextFile,
    createRepository,
    getBaseBranch,
    getBlameLines,
    getFileHistoryEntries,
    getStashDetail,
    getStashFileDiff,
    getCommitDetail,
    getCommitFileDiff,
    getCommitGraph,
    getCommitTree,
    getConflictFile,
    getFileBlame,
    getFileContent,
    getFileDiff,
    getFileHistory,
    getRepositoryState,
    getWorktreeStatus,
    homeDir,
    isDemo,
    openTerminal,
    pickRepositoryFolder,
    runGitAction,
    saveConflictResolution,
    setRepositoryPath,
  } from "./lib/git";
  import DiffTable from "./lib/DiffTable.svelte";
  import FileGroup from "./lib/FileGroup.svelte";
  import ReadonlyPane from "./lib/ReadonlyPane.svelte";
  import ConflictRegions from "./lib/ConflictRegions.svelte";
  import BlameView from "./lib/BlameView.svelte";
  import HistoryView from "./lib/HistoryView.svelte";
  import CommandPalette from "./lib/CommandPalette.svelte";
  import ContextMenu from "./lib/ContextMenu.svelte";
  import ShortcutsDialog from "./lib/ShortcutsDialog.svelte";
  import { buildLinePatch, changeLineIndexes } from "./lib/patch";
  import { hasMarkers, parseConflicts, resolveConflicts, type ConflictChoice } from "./lib/conflicts";
  import { parseDiffRows } from "./lib/diffRows";
  import { avatarUrl, coAuthorsOf } from "./lib/avatar";
  import { hoverExpand } from "./lib/hoverExpand";
  import { buildGraphRows, githubOwnerAvatar, graphPaths, isGithubUrl, type GraphRow } from "./lib/graph";
  import { blockedReason, undoableKinds, undoEntryFor, undoItem as toUndoItem, type UndoItem } from "./lib/undo";
  import { checkForUpdate, installUpdate } from "./lib/updates";
  import { refreshScope, watchRepository, type RepoChange } from "./lib/live";
  import { nearEnd, rowWindow } from "./lib/virtual";
  import type { Update } from "@tauri-apps/plugin-updater";
  import type {
    AiStatus,
    CommitExplanation,
    Issue,
    PullRequest,
    Workspace,
    MenuItem,
    PaletteItem,
    Profile,
    Signature,
    BlameLine,
    Branch,
    ConflictCheck,
    FileHistoryEntry,
    StashEntry,
    CommitDetail,
    CommitFileChange,
    CommitNode,
    ConflictFile,
    FileDiff,
    FileStatus,
    GitAction,
    GitResult,
    RepositoryState,
    Worktree,
  } from "./lib/types";

  type AppTab = {
    id: string;
    kind: "launchpad" | "repo";
    label: string;
    path?: string;
  };

  type RecentRepo = { name: string; path: string };

  type Settings = {
    confirmRisky: boolean;
    clonePath: string;
    graphLimit: number;
    staleDays: number;
    showAvatars: boolean;
    pullMode: string;
    /** Minutes between background `fetch --all --prune`; 0 turns it off. */
    autoFetchMinutes: number;
    refreshOnFocus: boolean;
    detectConflicts: boolean;
    /** Interface zoom, 0.7–1.6. */
    zoom: number;
    showLeftPanel: boolean;
    showRightPanel: boolean;
    /** App names for "Open in editor" / "Open terminal" (macOS `open -a`). */
    editorApp: string;
    terminalApp: string;
    /** Repository paths pinned to ⌥⌘1–9 and the top of the launchpad. */
    favorites: string[];
    profiles: Profile[];
    /** AI features send diffs and file contents to the Claude API; off until the user turns them on. */
    aiEnabled: boolean;
    aiModel: string;
    workspaces: Workspace[];
  };

  const defaultSettings: Settings = {
    confirmRisky: true,
    // Filled in from the home directory on first launch (see initClonePath).
    clonePath: "",
    // Commits per page: the graph loads this many, then pages in as you scroll.
    graphLimit: 500,
    staleDays: 30,
    showAvatars: true,
    pullMode: "pull",
    autoFetchMinutes: 5,
    refreshOnFocus: true,
    detectConflicts: true,
    zoom: 1,
    showLeftPanel: true,
    showRightPanel: true,
    editorApp: "",
    terminalApp: "Terminal",
    favorites: [],
    profiles: [],
    aiEnabled: false,
    aiModel: "claude-opus-5-5",
    workspaces: [],
  };

  let state: RepositoryState | null = null;
  let tabs: AppTab[] = [{ id: "launchpad", kind: "launchpad", label: "Launchpad" }];
  let activeTabId = "launchpad";
  let commits: CommitNode[] = [];
  let selectedCommit: CommitNode | null = null;
  let commitDetail: CommitDetail | null = null;
  let commitDetailBusy = false;
  let selectedFile: FileStatus | null = null;
  let selectedDiff: FileDiff | null = null;
  let diffContext: "worktree" | "commit" = "worktree";
  let commitFilePath = "";
  let centerMode: "graph" | "file" | "launchpad" | "compare" | "reflog" | "prs" = "graph";
  let fileViewMode: "diff" | "file" | "blame" | "history" = "diff";
  let fileText = "";
  let historyEntries: FileHistoryEntry[] = [];
  let blameLines: BlameLine[] = [];
  let splitDiff = false;
  let selectedHunk = 0;
  let conflict: ConflictFile | null = null;
  let compareBase: string | null = null;
  let compareHead = "";
  let resolvedContent = "";
  let commitMessage = "";
  let commitDescription = "";
  let amendCommit = false;
  let commandTarget = "";
  let branchName = "";
  let resetMode = "mixed";
  let rebaseOpen = false;
  let rebaseMode: "interactive" | "plain" = "interactive";
  let rebaseBase: string | null = null;
  let rightTab: "path" | "tree" = "path";
  let searchOpen = false;
  let searchQuery = "";
  let sortAsc = true;
  let localOpen = false;
  let remoteOpen = false;
  let stashesOpen = false;
  let tagsOpen = false;
  let worktreesOpen = false;
  let collapsedLocalDirs = new Set<string>();
  let collapsedRemoteDirs = new Set<string>();
  let collapsedCommitDirs = new Set<string>();
  let cleanupOpen = false;
  let unstagedOpen = true;
  let stagedOpen = true;
  let actionsOpen = false;
  let settingsOpen = false;
  let settings: Settings = loadSettings();
  let recentRepos: RecentRepo[] = loadRecentRepos();
  let busy = false;
  let error = "";
  let notice = "";
  let noticeTimer: ReturnType<typeof setTimeout> | null = null;
  let availableUpdate: Update | null = null;
  let installingUpdate = false;
  let filterInput: HTMLInputElement | null = null;
  let cloneOpen = false;
  let pullMenuOpen = false;
  let branchMenuOpen = false;

  const pullOptions = [
    { kind: "pullMerge", label: "Pull (fast-forward if possible)" },
    { kind: "pull", label: "Pull (fast-forward only)" },
    { kind: "pullRebase", label: "Pull (rebase)" },
  ];

  // Undo and redo stacks per repository, kept for the session.
  let undoHistory: Record<string, { undo: UndoItem[]; redo: UndoItem[] }> = {};
  $: repoHistory = (state && undoHistory[state.root]) || { undo: [], redo: [] };
  $: undoItem = repoHistory.undo.at(-1) ?? null;
  $: redoItem = repoHistory.redo.at(-1) ?? null;

  function recordUndo(action: GitAction, before: RepositoryState | null, result: GitResult) {
    if (!before || !state || before.root !== state.root) return;
    const entry = undoEntryFor(action, before, state, result);
    if (!entry) return;
    const current = undoHistory[state.root] ?? { undo: [], redo: [] };
    undoHistory = {
      ...undoHistory,
      [state.root]: { undo: [...current.undo, toUndoItem(entry, state)].slice(-50), redo: [] },
    };
  }

  async function replay(direction: "undo" | "redo") {
    if (!state || busy) return;
    const root = state.root;
    const current = undoHistory[root] ?? { undo: [], redo: [] };
    const item = current[direction].at(-1);
    if (!item) return;
    const verb = direction === "undo" ? "Undo" : "Redo";
    busy = true;
    error = "";
    notice = "";
    let failure = "";
    try {
      const blocked = blockedReason(item, await getRepositoryState());
      if (blocked === "dirty") {
        failure = `${verb} ${item.entry.label} would discard uncommitted changes. Commit or stash them first.`;
      } else if (blocked === "moved") {
        // Every entry beneath it is older still, so none of them can apply either.
        undoHistory = { ...undoHistory, [root]: { ...current, [direction]: [] } };
        failure = `Can't ${direction} ${item.entry.label}: the repository has changed since.`;
      } else {
        for (const step of item.entry[direction]) {
          const result = await runGitAction(step);
          if (!result.ok) {
            failure = result.stderr || result.stdout || `${verb} ${item.entry.label} failed`;
            break;
          }
        }
        await refresh();
        const next = { undo: [...current.undo], redo: [...current.redo] };
        next[direction].pop();
        if (!failure && state) {
          next[direction === "undo" ? "redo" : "undo"].push(toUndoItem(item.entry, state));
          notice = `${direction === "undo" ? "Undid" : "Redid"} ${item.entry.label}`;
        }
        undoHistory = { ...undoHistory, [root]: next };
      }
    } catch (err) {
      failure = String(err);
    } finally {
      busy = false;
    }
    if (failure) error = failure;
  }

  function setPullMode(kind: string) {
    settings = { ...settings, pullMode: kind };
    try {
      localStorage.setItem("kgit:settings", JSON.stringify(settings));
    } catch {
      /* localStorage unavailable */
    }
  }

  function runPullMenu(action: GitAction, label: string) {
    pullMenuOpen = false;
    void execute(action, label);
  }

  $: if (typeof document !== "undefined") {
    (document.documentElement.style as CSSStyleDeclaration & { zoom: string }).zoom = String(settings.zoom || 1);
  }

  function setZoom(next: number) {
    settings = { ...settings, zoom: Math.round(Math.min(1.6, Math.max(0.7, next)) * 10) / 10 };
    persistSettings();
  }

  function togglePanel(which: "showLeftPanel" | "showRightPanel") {
    settings = { ...settings, [which]: !settings[which] };
    persistSettings();
  }

  // Success toasts dismiss themselves; errors stay until addressed.
  $: if (notice) {
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 3200);
  }

  const riskyActions = new Set([
    "discard",
    "reset",
    "forcePush",
    "stashDrop",
    "deleteBranch",
    "deleteBranchForce",
    "cleanUntracked",
    "rebase",
    "merge",
  ]);

  $: staged = grouped(state, "staged");
  $: unstaged = grouped(state, "unstaged");
  $: untracked = grouped(state, "untracked");
  $: conflicted = grouped(state, "conflicted");
  $: currentBranch = state?.currentBranch || "detached";
  $: totalChanges = state?.files.length ?? 0;
  $: wipModified = unstaged.length;
  $: wipAdded = untracked.length + staged.filter((file) => file.index === "A").length;
  $: repoName = state?.root.split("/").filter(Boolean).at(-1) ?? "kGit";
  $: accountName = state?.userName?.trim() || "Local";
  $: fullCommitMessage = commitDescription.trim()
    ? `${commitMessage.trim()}\n\n${commitDescription.trim()}`
    : commitMessage.trim();
  $: visibleUnstaged = sortFiles([...unstaged, ...untracked, ...conflicted]);
  $: visibleStaged = sortFiles(staged);
  $: diffRows = parseDiffRows(selectedDiff?.diff ?? "");
  $: hunkRows = diffRows.map((row, index) => ({ row, index })).filter((item) => item.row.kind === "hunk");
  $: graphRows = buildGraphRows(commits, { hasWip: totalChanges > 0, remotes: state?.remotes ?? [] });
  $: visibleGraphRows = filterGraphRows(graphRows, searchOpen ? searchQuery : "");
  // The column grows past its 130px floor (styles.css, --graph-column) only
  // when the history actually has the lanes for it.
  $: graphLaneCount = Math.max(1, ...graphRows.flatMap((row) => row.lanes.map((lane) => lane.index + 1)));
  $: filteredBranches = (state?.branches ?? []).filter(
    (branch) => !searchQuery.trim() || branch.name.toLowerCase().includes(searchQuery.trim().toLowerCase()),
  );
  $: commitFileSummaryParts = commitDetail
    ? (
        [
          { key: "modified", cls: "st-mod", glyph: "✎", count: commitDetail.files.filter((f) => !/^[AD?]/.test(f.status)).length },
          { key: "added", cls: "st-add", glyph: "+", count: commitDetail.files.filter((f) => /^[A?]/.test(f.status)).length },
          { key: "deleted", cls: "st-del", glyph: "−", count: commitDetail.files.filter((f) => /^D/.test(f.status)).length },
        ] as const
      ).filter((part) => part.count > 0)
    : [];
  $: commitFileEntries = commitDetail
    ? withUnchangedFiles(commitDetail.files, viewAllFiles && commitTree?.hash === commitDetail.hash ? commitTree.paths : null)
    : [];
  $: sortedCommitFiles = sortByPath(commitFileEntries, sortAsc);
  $: coAuthors = commitDetail ? coAuthorsOf(commitDetail.body) : [];
  $: commitFileTree = commitDetail ? buildRefTree(sortedCommitFiles, (file) => file.path, collapsedCommitDirs) : [];
  $: localTree = buildRefTree(filteredBranches, (branch) => branch.name, collapsedLocalDirs);
  $: remoteTree = buildRefTree(filteredRemoteBranches, (name) => name, collapsedRemoteDirs);
  // Sections after the last expanded one stack at the sidebar's bottom edge
  // (matching the reference), so the empty space collects between the open
  // section's content and the collapsed remainder instead of below everything.
  $: lastOpenNavIndex = [localOpen, remoteOpen, worktreesOpen, stashesOpen, tagsOpen].reduce(
    (last, open, index) => (open ? index : last),
    -1,
  );
  $: filteredRemoteBranches = (state?.remoteBranches ?? []).filter(
    (branch) => !searchQuery.trim() || branch.toLowerCase().includes(searchQuery.trim().toLowerCase()),
  );

  function loadSettings(): Settings {
    try {
      return { ...defaultSettings, ...JSON.parse(localStorage.getItem("kgit:settings") ?? "{}") };
    } catch {
      return { ...defaultSettings };
    }
  }

  /** Persists settings changed by a shortcut or menu, without closing anything. */
  function persistSettings() {
    try {
      localStorage.setItem("kgit:settings", JSON.stringify(settings));
    } catch {
      /* localStorage unavailable */
    }
  }

  function saveSettings() {
    settings.graphLimit = Math.min(5000, Math.max(25, Math.round(Number(settings.graphLimit)) || 500));
    settings.autoFetchMinutes = Math.min(1440, Math.max(0, Math.round(Number(settings.autoFetchMinutes)) || 0));
    settings.staleDays = Math.min(3650, Math.max(1, Math.round(Number(settings.staleDays)) || 30));
    try {
      localStorage.setItem("kgit:settings", JSON.stringify(settings));
    } catch {
      /* localStorage unavailable */
    }
    settingsOpen = false;
    notice = "Settings saved";
    void refresh();
  }

  function loadRecentRepos(): RecentRepo[] {
    try {
      const stored = JSON.parse(localStorage.getItem("kgit:recentRepos") ?? "null");
      if (Array.isArray(stored) && stored.length) return stored;
    } catch {
      /* fall through to seeds */
    }
    return [];
  }

  function rememberRepo(path: string, name: string) {
    recentRepos = [{ name, path }, ...recentRepos.filter((repo) => repo.path !== path)].slice(0, 12);
    try {
      localStorage.setItem("kgit:recentRepos", JSON.stringify(recentRepos));
    } catch {
      /* localStorage unavailable */
    }
  }

  // ---- graph paging ----
  let graphExhausted = false;
  let loadingMore = false;
  let graphScroller: HTMLDivElement | null = null;
  let graphScrollTop = 0;
  let graphViewport = 600;
  const ROW_HEIGHT = 28;

  /** Re-reads every commit already loaded, so a refresh never drops the
   * history you scrolled into. */
  async function loadGraph(): Promise<CommitNode[]> {
    const limit = Math.max(settings.graphLimit, commits.length);
    const graph = await getCommitGraph(limit, 0);
    graphExhausted = graph.commits.length < limit;
    return graph.commits;
  }

  /** First page of a repository that was just opened, cloned or created. */
  async function loadFreshGraph(): Promise<CommitNode[]> {
    commits = [];
    graphExhausted = false;
    graphScrollTop = 0;
    if (graphScroller) graphScroller.scrollTop = 0;
    return loadGraph();
  }

  async function loadMoreCommits() {
    if (loadingMore || graphExhausted || !state) return;
    loadingMore = true;
    const root = state.root;
    try {
      const page = await getCommitGraph(settings.graphLimit, commits.length);
      if (state?.root !== root) return;
      const seen = new Set(commits.map((commit) => commit.hash));
      commits = [...commits, ...page.commits.filter((commit) => !seen.has(commit.hash))];
      graphExhausted = page.commits.length < settings.graphLimit;
    } catch (err) {
      error = String(err);
    } finally {
      loadingMore = false;
    }
  }

  function onGraphScroll() {
    if (!graphScroller) return;
    graphScrollTop = graphScroller.scrollTop;
    if (searchOpen && searchQuery.trim()) return;
    if (nearEnd(visibleGraphRows.length, ROW_HEIGHT, graphScrollTop - wipOffset, graphViewport)) void loadMoreCommits();
  }

  $: wipOffset = totalChanges > 0 ? ROW_HEIGHT : 0;
  $: graphWindow = rowWindow(visibleGraphRows.length, ROW_HEIGHT, graphScrollTop - wipOffset, graphViewport);
  $: windowRows = visibleGraphRows.slice(graphWindow.start, graphWindow.end);
  // A short first page (or a filter) can leave the viewport unfilled with no
  // scroll to trigger paging; keep loading until it fills or history ends.
  $: if (state && !graphExhausted && !loadingMore && !(searchOpen && searchQuery.trim()) && visibleGraphRows.length * ROW_HEIGHT < graphViewport) {
    void loadMoreCommits();
  }

  /** Reloads state and graph. `quiet` is for background refreshes (file
   * watcher, focus, auto-fetch): no busy bar, errors are not surfaced, and
   * the open commit's detail is kept rather than re-read. */
  async function refresh(options: { quiet?: boolean } = {}) {
    const quiet = options.quiet ?? false;
    if (!quiet) {
      busy = true;
      error = "";
    }
    try {
      const before = state?.root;
      const [nextState, nextCommits] = await Promise.all([getRepositoryState(), loadGraph()]);
      if (quiet && before && nextState.root !== before) return;
      state = nextState;
      syncRepoTab(nextState.root, nextState.root.split("/").filter(Boolean).at(-1) ?? "repo");
      commits = nextCommits;
      const keptStash =
        selectedCommit && isStash(selectedCommit.hash) && nextState.stashes.some((stash) => stash.name === selectedCommit?.hash)
          ? selectedCommit
          : null;
      const kept = selectedCommit
        ? keptStash ?? nextCommits.find((commit) => commit.hash === selectedCommit?.hash) ?? null
        : null;
      const shown = kept ?? cleanHeadCommit(nextState, nextCommits);
      selectedCommit = shown;
      if (shown) {
        if (!quiet || commitDetail?.hash !== shown.hash) await loadCommitDetail(shown.hash);
      } else {
        commitDetail = null;
      }
    } catch (err) {
      if (!quiet) error = String(err);
    } finally {
      if (!quiet) busy = false;
    }
  }

  /** Status-only reload for a working-tree or index change. */
  async function refreshStatus() {
    if (!state) return;
    const root = state.root;
    try {
      const status = await getWorktreeStatus();
      if (!state || state.root !== root) return;
      state = { ...state, ...status };
      // Keep an open working-tree diff in step with the file on disk.
      if (centerMode === "file" && diffContext === "worktree" && selectedFile && !conflict && fileViewMode === "diff") {
        const file = selectedFile;
        if (state.files.some((entry) => entry.path === file.path && normalizedGroup(entry) === file.group)) {
          selectedDiff = await getFileDiff(file.path, file.group === "staged");
        }
      }
    } catch {
      /* a background refresh never interrupts the user */
    }
  }

  // ---- early conflict detection ----
  // After each refresh, test-merge the branch's merge target(s) into HEAD
  // (git merge-tree: nothing is touched) and warn before conflicts bite.
  let conflictWarnings: ConflictCheck[] = [];
  let dismissedConflictKey = "";
  // Where the current branch was created from (its reflog), else the repo default.
  let baseBranchFor: { root: string; branch: string; base: string | null } = { root: "", branch: "", base: null };
  let conflictTimer: ReturnType<typeof setTimeout> | null = null;
  // Bumped after every refresh caused by a ref change (a commit, checkout or
  // fetch, here or in a terminal), so the check reruns even when a fetch
  // moves a remote branch without changing anything else the key covers.
  let refsVersion = 0;
  $: currentTracking = state?.branches.find((branch) => branch.current);
  $: conflictKey = state && settings.detectConflicts
    ? [
        state.root,
        state.head,
        state.currentBranch ?? "",
        commits[0]?.hash ?? "",
        state.remoteBranches.length,
        currentTracking?.ahead ?? 0,
        currentTracking?.behind ?? 0,
        refsVersion,
      ].join("|")
    : "";
  $: if (conflictKey) scheduleConflictCheck(conflictKey);
  $: visibleConflictWarnings = conflictKey && conflictKey === dismissedConflictKey ? [] : conflictWarnings;

  function scheduleConflictCheck(key: string) {
    if (conflictTimer) clearTimeout(conflictTimer);
    conflictTimer = setTimeout(() => void runConflictCheck(key), 800);
  }

  async function conflictTargets(): Promise<string[]> {
    if (!state?.currentBranch || state.merging || state.rebasing) return [];
    if (baseBranchFor.root !== state.root || baseBranchFor.branch !== (state.currentBranch ?? "")) {
      const branch = state.currentBranch;
      baseBranchFor = { root: state.root, branch: branch ?? "", base: await getBaseBranch(branch).catch(() => null) };
    }
    const targets: string[] = [];
    const base = baseBranchFor.base?.replace(/^origin\//, "") ?? null;
    if (base && base !== state.currentBranch) {
      // What a pull request merges into is the remote copy, when there is one.
      targets.push(state.remoteBranches.includes(`origin/${base}`) ? `origin/${base}` : base);
    }
    const current = state.branches.find((branch) => branch.current);
    if (current?.upstream && current.behind > 0 && !targets.includes(current.upstream)) targets.push(current.upstream);
    return targets;
  }

  async function runConflictCheck(key: string) {
    try {
      const targets = await conflictTargets();
      const results = await Promise.all(targets.map((target) => checkConflicts(target).catch(() => null)));
      if (key !== conflictKey) return;
      conflictWarnings = results.filter((result): result is ConflictCheck => !!result && result.conflicts);
    } catch {
      conflictWarnings = [];
    }
  }

  // ---- live updates: file watcher, window focus, auto-fetch ----
  let liveQueued: "full" | "status" | null = null;
  let liveRunning = false;
  let watchedRoot = "";
  let stopWatching: (() => void) | null = null;
  let lastLiveRefresh = 0;
  let lastFetchAt: number | null = null;

  async function onRepoChange(change: Pick<RepoChange, "worktree" | "index" | "refs">) {
    const scope = refreshScope(change);
    if (!scope || !state) return;
    liveQueued = liveQueued === "full" || scope === "full" ? "full" : "status";
    if (liveRunning) return;
    liveRunning = true;
    try {
      while (liveQueued) {
        // A user action is running and refreshes when it finishes.
        if (busy) {
          liveQueued = null;
          break;
        }
        const next = liveQueued;
        liveQueued = null;
        if (next === "full") {
          await refresh({ quiet: true });
          refsVersion += 1;
        } else await refreshStatus();
        lastLiveRefresh = Date.now();
      }
    } finally {
      liveRunning = false;
    }
  }

  async function startWatching() {
    stopWatching?.();
    stopWatching = null;
    try {
      stopWatching = await watchRepository((change) => {
        if (change.root === state?.root) void onRepoChange(change);
      });
    } catch (err) {
      // Watching is best effort: focus refresh and manual refresh still work.
      console.warn("kGit: file watching unavailable", err);
    }
  }

  $: if (state?.root && state.root !== watchedRoot) {
    watchedRoot = state.root;
    void startWatching();
    // Fetch once soon after opening, so ahead/behind and the conflict check
    // start from the remote's current state rather than the last fetch.
    if (settings.autoFetchMinutes && !isDemo) setTimeout(() => void backgroundFetch(), 2000);
  }

  function onWindowFocus() {
    if (!settings.refreshOnFocus || !state || busy || Date.now() - lastLiveRefresh < 3000) return;
    void onRepoChange({ worktree: false, index: false, refs: true });
  }

  let autoFetchTimer: ReturnType<typeof setInterval> | null = null;
  $: scheduleAutoFetch(settings.autoFetchMinutes, state?.root ?? "");

  function scheduleAutoFetch(minutes: number, root: string) {
    if (autoFetchTimer) clearInterval(autoFetchTimer);
    autoFetchTimer = null;
    if (!root || !minutes || isDemo) return;
    autoFetchTimer = setInterval(() => void backgroundFetch(), minutes * 60_000);
  }

  async function backgroundFetch() {
    if (busy || !state?.remotes.length) return;
    const result = await runGitAction({ kind: "fetchAll" }).catch(() => null);
    if (!result?.ok) return;
    lastFetchAt = Date.now();
    // The watcher sees the ref updates too; this covers remotes it can't.
    await onRepoChange({ worktree: false, index: false, refs: true });
  }

  function normalizedGroup(file: FileStatus): FileStatus["group"] {
    const rawGroup = String(file.group).toLowerCase();
    if (rawGroup === "staged" || rawGroup === "unstaged" || rawGroup === "untracked" || rawGroup === "conflicted") {
      return rawGroup;
    }
    if (file.index === "U" || file.worktree === "U") return "conflicted";
    if (file.index === "?" || file.worktree === "?") return "untracked";
    if (file.index && file.index !== ".") return "staged";
    return "unstaged";
  }

  function grouped(repositoryState: RepositoryState | null, group: FileStatus["group"]) {
    return repositoryState?.files.filter((file) => normalizedGroup(file) === group) ?? [];
  }

  function syncRepoTab(path: string, label: string) {
    rememberRepo(path, label);
    const id = `repo:${path}`;
    const existing = tabs.find((tab) => tab.id === id);
    if (existing) {
      tabs = tabs.map((tab) => (tab.id === id ? { ...tab, label } : tab));
      activeTabId = id;
      return;
    }
    tabs = [...tabs, { id, kind: "repo", label, path }];
    activeTabId = id;
  }

  function switchToTab(tab: AppTab) {
    activeTabId = tab.id;
    if (tab.kind === "launchpad") {
      centerMode = "launchpad";
      return;
    }
    if (tab.path) {
      centerMode = "graph";
      void openRepositoryPath(tab.path, true);
    }
  }

  let closedTabs: AppTab[] = [];

  function reopenClosedTab() {
    const tab = closedTabs.at(-1);
    if (!tab?.path) return;
    closedTabs = closedTabs.slice(0, -1);
    void openRepositoryPath(tab.path);
  }

  function closeTab(tab: AppTab) {
    if (tab.kind === "launchpad") return;
    closedTabs = [...closedTabs.filter((item) => item.id !== tab.id), tab].slice(-20);
    const remaining = tabs.filter((item) => item.id !== tab.id);
    tabs = remaining.length ? remaining : [{ id: "launchpad", kind: "launchpad", label: "Launchpad" }];
    if (activeTabId === tab.id) {
      const nextActive = tabs.find((item) => item.kind === "repo") ?? tabs[0];
      activeTabId = nextActive.id;
      switchToTab(nextActive);
    }
  }

  /** Runs a git action; resolves true only when it succeeded. */
  async function execute(action: GitAction, label: string): Promise<boolean> {
    if (
      riskyActions.has(action.kind) &&
      settings.confirmRisky &&
      !confirm(`${label} can rewrite or discard repository state. Continue?`)
    ) {
      return false;
    }

    actionsOpen = false;
    pullMenuOpen = false;
    branchMenuOpen = false;
    busy = true;
    error = "";
    notice = "";
    try {
      // Read fresh rather than trusting the last refresh: undo returns to this HEAD.
      const before = undoableKinds.has(action.kind) ? await getRepositoryState().catch(() => null) : null;
      const result = await runGitAction(action);
      // refresh() unconditionally clears `error` at its start, so a failure
      // message would otherwise be wiped the instant it's set. Re-apply it
      // after refresh() runs so the toast is actually shown.
      let failure = "";
      if (
        !result.ok &&
        action.kind === "checkoutBranch" &&
        action.branch &&
        /would be overwritten by checkout|commit your changes or stash them/i.test(result.stderr) &&
        confirm(`Your local changes would be overwritten by checking out ${action.branch}.\n\nStash them, check out ${action.branch}, and re-apply them there?`)
      ) {
        const carried = await checkoutWithAutostash(action.branch);
        Object.assign(result, carried);
      }
      if (!result.ok) {
        failure = result.stderr || result.stdout || `${label} failed`;
      } else {
        notice = `${label} complete`;
      }
      if (result.refresh) {
        await refresh();
        recordUndo(action, before, result);
      }
      if (failure) error = failure;
      if (selectedFile && action.path === selectedFile.path && diffContext === "worktree") {
        const stillThere = state?.files.some(
          (file) => file.path === selectedFile?.path && normalizedGroup(file) === selectedFile?.group,
        );
        if (stillThere) await openFile(selectedFile);
        else closeFileView();
      }
      return result.ok;
    } catch (err) {
      error = String(err);
      return false;
    } finally {
      busy = false;
    }
  }

  // Amend: ticking the box loads the previous commit's summary and
  // description into the composer, keeping whatever was typed so unticking
  // can put it back (unless the loaded message has since been edited).
  let amendDraft: { message: string; description: string } | null = null;
  let amendLoaded: { message: string; description: string } | null = null;
  let amendLoading = false;

  async function setAmend(checked: boolean) {
    amendCommit = checked;
    if (checked) {
      amendDraft = { message: commitMessage, description: commitDescription };
      amendLoaded = null;
      amendLoading = true;
      try {
        const head = await getCommitDetail("HEAD");
        if (!amendCommit) return;
        commitMessage = head.subject;
        commitDescription = head.body.trim();
        amendLoaded = { message: commitMessage, description: commitDescription };
      } catch (err) {
        error = `Could not load the previous commit: ${String(err)}`;
      } finally {
        amendLoading = false;
      }
      return;
    }
    const untouched =
      !amendLoaded || (commitMessage === amendLoaded.message && commitDescription === amendLoaded.description);
    if (amendDraft && untouched) {
      commitMessage = amendDraft.message;
      commitDescription = amendDraft.description;
    }
    amendDraft = null;
    amendLoaded = null;
  }

  async function commitChanges() {
    const amending = amendCommit;
    const done = await execute(
      { kind: amending ? "commitAmend" : "commit", message: fullCommitMessage },
      amending ? "Amend commit" : "Commit",
    );
    if (!done) return;
    commitMessage = "";
    commitDescription = "";
    amendCommit = false;
    amendDraft = null;
    amendLoaded = null;
  }

  async function loadCommitDetail(hash: string) {
    commitDetailBusy = true;
    signature = null;
    try {
      commitDetail = isStash(hash) ? await getStashDetail(hash) : await getCommitDetail(hash);
      if (!isStash(hash) && commitDetail) {
        const forHash = commitDetail.hash;
        void getCommitSignature(forHash)
          .then((value) => {
            if (commitDetail?.hash === forHash) signature = value.status === "N" ? null : value;
          })
          .catch(() => {});
      }
    } catch (err) {
      commitDetail = null;
      error = String(err);
    } finally {
      commitDetailBusy = false;
    }
  }

  async function selectCommit(commit: CommitNode | null) {
    selectedCommit = commit;
    commitDetail = null;
    if (!commit) return;
    await loadCommitDetail(commit.hash);
  }

  async function openCommitFile(change: CommitFileChange) {
    if (!commitDetail) return;
    busy = true;
    error = "";
    try {
      selectedDiff = isStash(selectedCommit?.hash)
        ? await getStashFileDiff(selectedCommit!.hash, change.path)
        : await getCommitFileDiff(commitDetail.hash, change.path);
      diffContext = "commit";
      commitFilePath = change.path;
      selectedFile = null;
      selectedHunk = 0;
      fileViewMode = "diff";
      centerMode = "file";
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  /** Shows any commit (blame, history, reflog) in the detail panel, even one
   * outside the loaded graph. */
  async function showCommitByHash(hash: string) {
    const loaded = commits.find((commit) => commit.hash === hash);
    selectedCommit = loaded ?? {
      hash,
      shortHash: hash.slice(0, 8),
      parents: [],
      refs: [],
      author: "",
      email: "",
      timestamp: 0,
      relativeDate: "",
      subject: "",
      bodySummary: "",
    };
    await loadCommitDetail(hash);
  }

  async function openHistoryEntry(entry: FileHistoryEntry) {
    await showCommitByHash(entry.hash);
    if (!commitDetail) return;
    await openCommitFile({ status: entry.status, path: entry.path });
  }

  const isStash = (hash: string | undefined) => !!hash && hash.startsWith("stash@{");

  async function openStash(stash: StashEntry) {
    selectedCommit = {
      hash: stash.name,
      shortHash: stash.name,
      parents: [],
      refs: [],
      author: "",
      email: "",
      timestamp: 0,
      relativeDate: "",
      subject: stash.message,
      bodySummary: "",
    };
    await loadCommitDetail(stash.name);
  }

  function closeFileView() {
    centerMode = "graph";
    selectedFile = null;
    selectedDiff = null;
    diffContext = "worktree";
    commitFilePath = "";
    compareHead = "";
  }

  async function openConflict(file: FileStatus) {
    selectedFile = file;
    selectedDiff = null;
    conflict = null;
    resolvedContent = "";
    if (file.group !== "conflicted") return;

    busy = true;
    error = "";
    try {
      conflict = await getConflictFile(file.path);
      resolvedContent = conflict.working;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function openFile(file: FileStatus) {
    selectedFile = file;
    selectedDiff = null;
    selectedHunk = 0;
    diffContext = "worktree";
    commitFilePath = "";
    centerMode = "file";
    if (file.group === "conflicted") {
      await openConflict(file);
      return;
    }

    busy = true;
    error = "";
    try {
      selectedDiff = await getFileDiff(file.path, file.group === "staged");
      if (fileViewMode !== "diff") await loadFileAuxiliary(file);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function saveResolution() {
    if (!conflict) return;
    busy = true;
    error = "";
    try {
      const result = await saveConflictResolution(conflict.path, resolvedContent);
      if (!result.ok) error = result.stderr || "Unable to save resolution";
      await refresh();
      notice = `Saved ${conflict.path}`;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function useVersion(content?: string | null) {
    if (content != null) resolvedContent = content;
  }

  // ---- per-conflict resolution ----
  let conflictChoices: Record<number, ConflictChoice | undefined> = {};
  let conflictSource: string | null = null;
  $: conflictSegments = conflict ? parseConflicts(conflict.working) : [];
  $: if ((conflict?.working ?? null) !== conflictSource) {
    conflictSource = conflict?.working ?? null;
    conflictChoices = {};
  }

  function chooseConflict(index: number, choice: ConflictChoice | undefined) {
    conflictChoices = { ...conflictChoices, [index]: choice };
    resolvedContent = resolveConflicts(conflictSegments, conflictChoices);
  }

  async function saveAndMarkResolved() {
    if (!conflict) return;
    if (hasMarkers(resolvedContent) && !confirm("The result still has conflict markers. Mark it resolved anyway?")) return;
    const path = conflict.path;
    await saveResolution();
    if (error) return;
    const done = await execute({ kind: "markResolved", path }, `Resolve ${path}`);
    if (done) conflict = null;
  }

  function fileAction(file: FileStatus, kind: string, label: string) {
    execute({ kind, path: file.path }, label);
  }

  async function switchRepository() {
    const path = await pickRepositoryFolder();
    if (!path) return;
    await openRepositoryPath(path);
  }

  async function openCreatePrompt() {
    const path = prompt("Create repository at path", `${settings.clonePath.replace(/\/$/, "")}/new-repo`);
    if (!path?.trim()) return;
    busy = true;
    error = "";
    try {
      state = await createRepository(path.trim());
      commits = await loadFreshGraph();
      selectedFile = null;
      selectedDiff = null;
      await selectCommit(cleanHeadCommit(state, commits));
      centerMode = "graph";
      syncRepoTab(state.root, state.root.split("/").filter(Boolean).at(-1) ?? "repo");
      notice = `Created ${state.root}`;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
  async function afterClone(next: RepositoryState) {
    // Close first: once `cloneRepository` has succeeded the dialog's job is
    // done, and any failure below (e.g. the graph reload) should surface on
    // App's own error banner rather than reopen a "clone failed" state in a
    // dialog that already reported success.
    cloneOpen = false;
    state = next;
    try {
      commits = await loadFreshGraph();
      selectedFile = null;
      selectedDiff = null;
      await selectCommit(cleanHeadCommit(state, commits));
      centerMode = "graph";
      syncRepoTab(state.root, state.root.split("/").filter(Boolean).at(-1) ?? "repo");
      notice = `Cloned ${state.root}`;
    } catch (err) {
      error = String(err);
    }
  }

  // Commit messages are typed verbatim: without this the macOS web view turns
  // "--" into an em dash and straight quotes into curly ones.
  const plainText: Record<string, string> = { spellcheck: "false", autocorrect: "off", autocapitalize: "off" };

  async function openRepositoryPath(path: string, silent = false): Promise<boolean> {
    busy = true;
    error = "";
    notice = "";
    try {
      state = await setRepositoryPath(path);
      commits = await loadFreshGraph();
      selectedFile = null;
      selectedDiff = null;
      await selectCommit(cleanHeadCommit(state, commits));
      centerMode = "graph";
      syncRepoTab(state.root, state.root.split("/").filter(Boolean).at(-1) ?? "repo");
      if (!silent) notice = `Opened ${state.root}`;
      return true;
    } catch (err) {
      error = String(err);
      return false;
    } finally {
      busy = false;
    }
  }

  function sortFiles(files: FileStatus[]) {
    return files
      .filter((file) => !searchQuery.trim() || file.path.toLowerCase().includes(searchQuery.trim().toLowerCase()))
      .sort((a, b) => (sortAsc ? a.path.localeCompare(b.path) : b.path.localeCompare(a.path)));
  }

  async function loadFileAuxiliary(file = selectedFile) {
    if (!file) return;
    busy = true;
    error = "";
    try {
      if (fileViewMode === "file") fileText = await getFileContent(file.path, file.group === "staged");
      if (fileViewMode === "blame") blameLines = await getBlameLines(file.path);
      if (fileViewMode === "history") historyEntries = await getFileHistoryEntries(file.path);
    } catch (err) {
      fileText = "";
      blameLines = [];
      historyEntries = [];
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function setFileViewMode(mode: "diff" | "file" | "blame" | "history") {
    fileViewMode = mode;
    if (mode === "diff") return;
    await loadFileAuxiliary();
  }

  // An avatar that fails to load (no Gravatar, offline) stays on initials for
  // the session rather than being re-requested every time the rows render.
  let failedAvatars = new Set<string>();
  $: detailAvatar = commitDetail ? avatarFor(commitDetail.email, settings.showAvatars, failedAvatars) : null;

  function avatarFor(email: string, enabled: boolean, failed: Set<string>) {
    if (!enabled) return null;
    const url = avatarUrl(email);
    return url && !failed.has(url) ? url : null;
  }

  // The pill for a branch on a GitHub remote carries the remote account's
  // avatar; it falls back to the GitHub mark when that can't be loaded.
  function remoteAvatarFor(url: string | null | undefined, enabled: boolean, failed: Set<string>) {
    if (!enabled) return null;
    const avatar = githubOwnerAvatar(url);
    return avatar && !failed.has(avatar) ? avatar : null;
  }

  function avatarFailed(url: string) {
    failedAvatars = new Set(failedAvatars).add(url);
  }

  function authorInitials(author: string) {
    const parts = author.trim().split(/\s+/).filter(Boolean);
    if (parts.length >= 2) {
      return parts
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase())
        .join("");
    }
    const word = parts[0];
    if (!word) return "G";
    // A single-word author (the common case for a solo repo) still gets two
    // letters, e.g. "Dillon" -> "Di", instead of a single dot-everywhere "D".
    return word.length > 1 ? word[0].toUpperCase() + word[1].toLowerCase() : word[0].toUpperCase();
  }

  // Git wraps a manually-authored commit body at ~72 columns for terminal
  // reading; rendering those hard breaks verbatim inside a ~320px-wide panel
  // re-wraps them again and shreds the paragraph into short fragments. This
  // reflows each paragraph's wrapped lines back together (a line is a
  // continuation unless it starts a bullet/numbered item or looks like a
  // trailer such as "Co-Authored-By: ..."), so the browser can wrap the
  // result naturally at the panel's real width. Blank lines still separate
  // paragraphs; bullets and trailers still get their own line.
  function reflowCommitBody(raw: string) {
    if (!raw) return "";
    return raw
      .replace(/\r\n/g, "\n")
      .split(/\n{2,}/)
      .map((paragraph) => {
        const groups: string[] = [];
        for (const rawLine of paragraph.split("\n")) {
          const line = rawLine.trim();
          if (!line) continue;
          const isBullet = /^([-*•]|\d+[.)])\s+/.test(line);
          const isTrailer = /^[A-Za-z][\w .()/'-]*:\s/.test(line);
          if (groups.length && !isBullet && !isTrailer) {
            groups[groups.length - 1] += ` ${line}`;
          } else {
            groups.push(line);
          }
        }
        return groups.join("\n");
      })
      .join("\n\n");
  }

  type RefTreeDirRow = { key: string; depth: number; kind: "dir"; label: string };
  type RefTreeLeafRow<T> = { key: string; depth: number; kind: "leaf"; label: string; item: T };
  type RefTreeRow<T> = RefTreeDirRow | RefTreeLeafRow<T>;

  // Branch names like "style/graph-sidebar-match" render as a folder tree
  // (split on "/"), matching the reference — not a flat list of full names.
  function buildRefTree<T>(items: T[], pathOf: (item: T) => string, collapsed: Set<string>): RefTreeRow<T>[] {
    type Node = { dirs: Map<string, Node>; leaves: T[] };
    const root: Node = { dirs: new Map(), leaves: [] };
    for (const item of items) {
      const parts = pathOf(item).split("/");
      let node = root;
      for (const part of parts.slice(0, -1)) {
        if (!node.dirs.has(part)) node.dirs.set(part, { dirs: new Map(), leaves: [] });
        node = node.dirs.get(part)!;
      }
      node.leaves.push(item);
    }
    const rows: RefTreeRow<T>[] = [];
    const walk = (node: Node, prefix: string, depth: number) => {
      for (const [segment, child] of [...node.dirs.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
        const key = prefix ? `${prefix}/${segment}` : segment;
        rows.push({ key, depth, kind: "dir", label: segment });
        if (!collapsed.has(key)) walk(child, key, depth + 1);
      }
      for (const item of [...node.leaves].sort((a, b) => pathOf(a).localeCompare(pathOf(b)))) {
        const path = pathOf(item);
        const segment = path.split("/").at(-1) ?? path;
        rows.push({ key: `${path}:leaf`, depth, kind: "leaf", label: segment, item });
      }
    };
    walk(root, "", 0);
    return rows;
  }

  function toggleRefDir(collapsed: Set<string>, key: string) {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    return next;
  }

  function commitStatusGlyph(status: string): { glyph: string; cls: string } {
    if (!status) return { glyph: "", cls: "" };
    const code = status[0]?.toUpperCase();
    if (code === "A" || code === "?") return { glyph: "+", cls: "st-add" };
    if (code === "D") return { glyph: "−", cls: "st-del" };
    if (code === "R" || code === "C") return { glyph: "⇝", cls: "st-mod" };
    return { glyph: "✎", cls: "st-mod" };
  }

  function sortByPath<T extends { path: string }>(items: T[], asc: boolean) {
    const sorted = [...items].sort((a, b) => a.path.localeCompare(b.path));
    return asc ? sorted : sorted.reverse();
  }

  // Upstream/ahead-behind detail moved off the row (it was rendering as a
  // second line, making tree rows ragged and uneven height) and into this
  // tooltip instead — every row stays a single, uniform-height line.
  function branchTooltip(branch: Branch) {
    const parts = [branch.current ? `${branch.name} (checked out)` : `Checkout ${branch.name}`];
    if (branch.upstreamGone) parts.push(`${branch.upstream ?? "upstream"} · gone`);
    else if (branch.upstream) parts.push(branch.upstream);
    if (branch.ahead || branch.behind) parts.push(`↑${branch.ahead} ↓${branch.behind}`);
    return parts.join(" — ");
  }

  // With nothing to commit there is no WIP to show, so the right panel opens
  // on the checked-out commit instead of an empty change list.
  function cleanHeadCommit(repo: RepositoryState | null, nodes: CommitNode[]) {
    if (!repo || repo.files.length > 0) return null;
    return nodes.find((commit) => commit.refs.some((ref) => ref === "HEAD" || ref.startsWith("HEAD -> "))) ?? null;
  }

  // "View all files": the commit's whole tree, with its changes marked.
  let viewAllFiles = false;
  let commitTree: { hash: string; paths: string[] } | null = null;
  let commitTreePending = "";
  $: if (viewAllFiles && commitDetail && commitTree?.hash !== commitDetail.hash && commitTreePending !== commitDetail.hash) {
    void loadCommitTree(commitDetail.hash);
  }

  async function loadCommitTree(hash: string) {
    commitTreePending = hash;
    try {
      const paths = await getCommitTree(hash);
      commitTree = { hash, paths };
    } catch (err) {
      error = String(err);
      viewAllFiles = false;
    } finally {
      commitTreePending = "";
    }
  }

  function withUnchangedFiles(changes: CommitFileChange[], paths: string[] | null): CommitFileChange[] {
    if (!paths) return changes;
    const byPath = new Map(changes.map((change) => [change.path, change]));
    const inTree = new Set(paths);
    // Deleted files are gone from the tree but are still part of the change.
    return [
      ...paths.map((path) => byPath.get(path) ?? { status: "", path }),
      ...changes.filter((change) => !inTree.has(change.path)),
    ];
  }

  function splitPath(path: string) {
    const cut = path.lastIndexOf("/") + 1;
    return { dir: path.slice(0, cut), name: path.slice(cut) };
  }

  // Checked out in some other worktree. The current checkout's own branch is
  // just a local branch (the HEAD pill already says it is checked out here).
  function isWorktreeBranch(name: string) {
    return (state?.worktrees ?? []).some((worktree) => worktree.branch === name && !worktree.current);
  }

  function filterGraphRows(rows: GraphRow[], query: string) {
    const trimmed = query.trim().toLowerCase();
    if (!trimmed) return rows;
    return rows.filter((row) => {
      const haystack = [
        row.commit.subject,
        row.commit.bodySummary,
        row.commit.author,
        row.commit.hash,
        row.commit.shortHash,
        ...row.labels,
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(trimmed);
    });
  }

  function fileStageAction() {
    if (!selectedFile) return;
    if (selectedFile.group === "staged") {
      execute({ kind: "unstage", path: selectedFile.path }, "Unstage file");
    } else {
      execute({ kind: "stage", path: selectedFile.path }, "Stage file");
    }
  }

  function createBranchFromToolbar() {
    const name = prompt("New branch name");
    if (name?.trim()) execute({ kind: "createBranch", branch: name.trim() }, `Create ${name.trim()}`);
  }

  function createTagPrompt(target?: string) {
    const name = prompt("Tag name");
    if (!name?.trim()) return;
    execute({ kind: "createTag", branch: name.trim(), target: target ?? null }, `Tag ${name.trim()}`);
  }

  function openCompare(base: string | null, head: string) {
    compareBase = base;
    compareHead = head;
    centerMode = "compare";
  }

  function commitRowTitle(row: GraphRow): string {
    if (selectedCommit && selectedCommit.hash !== row.commit.hash) {
      return `Shift-click to compare with ${selectedCommit.shortHash}`;
    }
    return row.labels.join("  ");
  }

  function handleCommitRowClick(event: MouseEvent, row: GraphRow, rowIndex: number) {
    if (event.shiftKey && selectedCommit && selectedCommit.hash !== row.commit.hash) {
      const selectedIndex = visibleGraphRows.findIndex((item) => item.commit.hash === selectedCommit?.hash);
      // Rows are newest-first, so a larger index means an older commit; the
      // older commit always becomes the base or every diff comes out reversed.
      const older = selectedIndex > rowIndex ? selectedCommit : row.commit;
      const newer = selectedIndex > rowIndex ? row.commit : selectedCommit;
      openCompare(older.hash, newer.hash);
      return;
    }
    selectCommit(row.commit);
  }

  function createBranchAtCommit(hash: string) {
    const name = prompt("Branch name for this commit");
    if (!name?.trim()) return;
    execute({ kind: "createBranch", branch: name.trim(), target: hash }, `Create ${name.trim()} at ${hash.slice(0, 8)}`);
  }

  function shortWorktreePath(path: string) {
    const parts = path.split("/").filter(Boolean);
    return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : path;
  }

  function worktreeLabel(worktree: Worktree) {
    return worktree.branch ?? `${worktree.head} (detached)`;
  }

  function addWorktreePrompt() {
    const branch = prompt("Branch for the new worktree (an existing name checks it out, a new name creates it)");
    if (!branch?.trim()) return;
    const name = branch.trim();
    const rootParts = (state?.root ?? "").split("/").filter(Boolean);
    const repoName = rootParts.at(-1) ?? "repo";
    const parent = rootParts.slice(0, -1).join("/");
    const suggested = `/${parent ? `${parent}/` : ""}${repoName}-${name.replace(/[^\w.-]+/g, "-")}`;
    const path = prompt("Create worktree at path", suggested);
    if (!path?.trim()) return;
    const exists = state?.branches.some((entry) => entry.name === name) ?? false;
    execute(
      { kind: "worktreeAdd", path: path.trim(), branch: name, mode: exists ? "checkout" : "new" },
      `Add worktree for ${name}`,
    );
  }

  async function removeWorktree(worktree: Worktree) {
    if (
      settings.confirmRisky &&
      !confirm(`Remove the worktree at ${worktree.path}? Its checkout directory will be deleted.`)
    ) {
      return;
    }
    busy = true;
    error = "";
    notice = "";
    try {
      let result = await runGitAction({ kind: "worktreeRemove", path: worktree.path });
      if (
        !result.ok &&
        /modified or untracked files|locked working tree|use --force/i.test(result.stderr) &&
        confirm(`${result.stderr.trim()}\n\nForce remove ${worktree.path}?`)
      ) {
        result = await runGitAction({ kind: "worktreeRemoveForce", path: worktree.path });
      }
      if (!result.ok) {
        error = result.stderr || result.stdout || "Remove worktree failed";
      } else {
        notice = `Removed worktree ${worktree.path}`;
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
    await refresh();
  }

  async function deleteBranch(name: string) {
    busy = true;
    error = "";
    notice = "";
    let action: GitAction = { kind: "deleteBranch", branch: name };
    let before: RepositoryState | null = null;
    let result: GitResult | null = null;
    try {
      before = await getRepositoryState().catch(() => null);
      result = await runGitAction(action);
      if (
        !result.ok &&
        /not fully merged/i.test(result.stderr) &&
        confirm(`${result.stderr.trim()}\n\nDelete ${name} anyway? Its tip stays recoverable for about two weeks.`)
      ) {
        action = { kind: "deleteBranchForce", branch: name };
        result = await runGitAction(action);
      }
      if (!result.ok) {
        error = result.stderr || result.stdout || "Delete branch failed";
      } else {
        notice = `Deleted ${name}`;
      }
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
    // refresh() clears `error`, which used to swallow a failed delete's message.
    const failure = error;
    await refresh();
    if (failure) error = failure;
    if (result) recordUndo(action, before, result);
  }

  // post-merge: open RebasePanel in plain mode
  function rebaseOnto(target: string) {
    if (settings.confirmRisky && !confirm(`Rebase ${currentBranch} onto ${target}?`)) return;
    void execute({ kind: "rebase", target }, `Rebase onto ${target}`);
  }

  async function copyHash(hash: string) {
    try {
      await navigator.clipboard.writeText(hash);
      notice = `Copied ${hash.slice(0, 8)}`;
    } catch {
      notice = hash;
    }
  }
  function openRebasePanel(rebaseModeArg: "interactive" | "plain", base: string | null = null) {
    rebaseMode = rebaseModeArg;
    rebaseBase = base;
    rebaseOpen = true;
    actionsOpen = false;
  }

  function showAddRepoNotice() {
    activeTabId = "launchpad";
    centerMode = "launchpad";
  }

  // ---- line-level staging ----
  let selectedLines = new Set<number>();
  let lastPickedLine: number | null = null;
  let linesForDiff: string | undefined;
  // A new diff (another file, or the same file after a change) clears the selection.
  $: if (selectedDiff?.diff !== linesForDiff) {
    linesForDiff = selectedDiff?.diff;
    selectedLines = new Set();
    lastPickedLine = null;
  }

  function toggleLine(line: number, extend: boolean) {
    const next = new Set(selectedLines);
    if (extend && lastPickedLine != null && selectedDiff) {
      const [from, to] = [Math.min(lastPickedLine, line), Math.max(lastPickedLine, line)];
      for (const index of changeLineIndexes(selectedDiff.diff)) if (index >= from && index <= to) next.add(index);
    } else if (next.has(line)) next.delete(line);
    else next.add(line);
    lastPickedLine = line;
    selectedLines = next;
  }

  async function applySelectedLines(mode: "stage" | "unstage" | "discard") {
    if (!selectedFile || !selectedDiff || selectedLines.size === 0) return;
    const count = selectedLines.size;
    if (mode === "discard" && settings.confirmRisky && !confirm(`Discard ${count} selected line${count === 1 ? "" : "s"}? This cannot be undone.`)) return;
    const patch = buildLinePatch(selectedDiff.diff, selectedLines, mode !== "stage");
    if (!patch) return;
    busy = true;
    error = "";
    notice = "";
    try {
      const result = await applyHunk(patch, mode);
      if (!result.ok) error = result.stderr || result.stdout || "Unable to apply the selected lines";
      else notice = `${mode === "stage" ? "Staged" : mode === "unstage" ? "Unstaged" : "Discarded"} ${count} line${count === 1 ? "" : "s"}`;
      const failure = error;
      await refresh();
      if (failure) error = failure;
      const still = state?.files.find((file) => file.path === selectedFile?.path && normalizedGroup(file) === selectedFile?.group);
      if (still) await openFile(selectedFile);
      else closeFileView();
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function moveHunk(direction: 1 | -1) {
    if (hunkRows.length === 0) return;
    selectedHunk = (selectedHunk + direction + hunkRows.length) % hunkRows.length;
  }

  function selectedHunkPatch() {
    if (!selectedDiff?.diff || hunkRows.length === 0) return "";
    const lines = selectedDiff.diff.split("\n");
    const header: string[] = [];
    let hunkNumber = -1;
    const hunk: string[] = [];

    for (const line of lines) {
      if (line.startsWith("@@ ")) {
        hunkNumber += 1;
        if (hunkNumber > selectedHunk) break;
      }
      if (hunkNumber < 0) header.push(line);
      if (hunkNumber === selectedHunk) hunk.push(line);
    }

    return [...header, ...hunk, ""].join("\n");
  }

  async function applySelectedHunk(mode: "stage" | "unstage" | "discard") {
    if (!selectedFile) return;
    if (mode === "discard" && settings.confirmRisky && !confirm("Discard hunk? This cannot be undone.")) return;
    const patch = selectedHunkPatch();
    if (!patch.trim()) {
      error = "No hunk selected";
      return;
    }
    busy = true;
    error = "";
    notice = "";
    try {
      const result = await applyHunk(patch, mode);
      if (!result.ok) error = result.stderr || result.stdout || "Unable to apply hunk";
      else notice = `${mode === "stage" ? "Staged" : mode === "unstage" ? "Unstaged" : "Discarded"} hunk`;
      await refresh();
      await openFile(selectedFile);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  // ---- GitHub pull requests and issues ----
  let knownPrs: PullRequest[] = [];
  let createPrFor: { head: string; base: string } | null = null;
  $: githubRemote = !!state && Object.values(state.remoteUrls ?? {}).some((url) => isGithubUrl(url));

  function prBaseOptions(): string[] {
    const locals = state?.branches.map((branch) => branch.name) ?? [];
    const preferred = ["main", "master", "develop"].filter((name) => locals.includes(name) || state?.remoteBranches.includes(`origin/${name}`));
    return [...new Set([...preferred, ...locals])];
  }

  function openCreatePr(head = state?.currentBranch ?? "") {
    if (!head) {
      error = "Check out a branch to open a pull request from it.";
      return;
    }
    const base = baseBranchFor.base?.replace(/^origin\//, "") || prBaseOptions().find((name) => name !== head) || "main";
    createPrFor = { head, base };
  }

  /** Pushes the PR's branch (and sets its upstream) when GitHub doesn't have it yet. */
  async function ensurePushed(head: string): Promise<boolean> {
    const branch = state?.branches.find((entry) => entry.name === head);
    if (branch?.upstream && !branch.upstreamGone && branch.ahead === 0) return true;
    return execute({ kind: "pushBranch", branch: head, remote: "origin" }, `Push ${head}`);
  }

  async function checkoutPr(pr: PullRequest) {
    busy = true;
    error = "";
    try {
      await ghPrCheckout(pr.number);
      notice = `Checked out #${pr.number}`;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
    const failure = error;
    await refresh();
    if (failure) error = failure;
  }

  async function branchFromIssue(issue: Issue) {
    const suggested = await issueBranch(issue.number, issue.title).catch(() => `issue-${issue.number}`);
    const name = prompt(`Branch for #${issue.number} ${issue.title}`, suggested);
    if (name?.trim()) await execute({ kind: "createBranch", branch: name.trim() }, `Create ${name.trim()}`);
  }

  function prMenuItems(branch: string): MenuItem[] {
    if (!githubRemote || !branch) return [];
    const existing = knownPrs.find((pr) => pr.head === branch);
    return existing
      ? [{ label: `Open pull request #${existing.number}`, run: () => openUrl(existing.url) }]
      : [{ label: "Create pull request…", run: () => openCreatePr(branch) }];
  }

  // ---- sharing patches as links (secret gists) ----
  async function sharePatchLink(source: string, target: string | null, name: string, description: string) {
    try {
      const text = await makePatch(source, target);
      if (!text.trim()) {
        notice = "Nothing to share";
        return;
      }
      if (!confirm(`Upload this patch as a secret GitHub gist? Anyone with the link can read it.`)) return;
      busy = true;
      const url = await sharePatch(text, name, description);
      await navigator.clipboard.writeText(url).catch(() => {});
      notice = `Shared: ${url} (link copied)`;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function applySharedPatch() {
    const link = prompt("Gist link or id of the shared patch");
    if (!link?.trim()) return;
    try {
      await applyPatchText(await fetchSharedPatch(link.trim()), "the shared link");
    } catch (err) {
      error = String(err);
    }
  }

  // ---- AI (Claude API) ----
  let aiInfo: AiStatus | null = null;
  let aiKeyDraft = "";
  let aiWorking = "";
  let explanation: { hash: string; value: CommitExplanation } | null = null;
  $: aiReady = settings.aiEnabled && !!aiInfo?.hasKey;
  $: if (settings.aiEnabled && !aiInfo) void loadAiStatus();

  async function loadAiStatus() {
    aiInfo = await aiStatus().catch(() => null);
  }

  async function saveAiKey() {
    try {
      await aiSetKey(aiKeyDraft);
      aiKeyDraft = "";
      await loadAiStatus();
      notice = aiInfo?.hasKey ? "API key saved in the keychain" : "API key removed";
    } catch (err) {
      error = String(err);
    }
  }

  function requireAi(): boolean {
    if (aiReady) return true;
    settingsOpen = true;
    error = settings.aiEnabled ? "Add a Claude API key in Settings › AI first." : "Turn on AI features in Settings › AI first.";
    return false;
  }

  async function writeCommitMessageWithAi() {
    if (!requireAi() || aiWorking) return;
    aiWorking = "commit";
    error = "";
    try {
      const suggestion = await aiCommitMessage(settings.aiModel || null);
      commitMessage = suggestion.subject;
      commitDescription = suggestion.body;
    } catch (err) {
      error = String(err);
    } finally {
      aiWorking = "";
    }
  }

  async function explainCommit(hash: string) {
    if (!requireAi() || aiWorking) return;
    aiWorking = "explain";
    error = "";
    try {
      if (selectedCommit?.hash !== hash) await showCommitByHash(hash);
      explanation = { hash, value: await aiExplainCommit(hash, settings.aiModel || null) };
    } catch (err) {
      error = String(err);
    } finally {
      aiWorking = "";
    }
  }

  async function resolveConflictsWithAi() {
    if (!conflict || !requireAi() || aiWorking) return;
    const regions = conflictSegments.flatMap((segment) =>
      segment.kind === "conflict" ? [{ index: segment.index, ours: segment.ours, theirs: segment.theirs, base: segment.base }] : [],
    );
    aiWorking = "conflicts";
    error = "";
    try {
      const proposal = await aiResolveConflicts(conflict.path, conflict.working, regions, settings.aiModel || null);
      for (const item of proposal.resolutions) {
        conflictChoices = { ...conflictChoices, [item.index]: { text: item.resolution, confidence: item.confidence, explanation: item.explanation } };
      }
      resolvedContent = resolveConflicts(conflictSegments, conflictChoices);
      const low = proposal.resolutions.filter((item) => item.confidence < 0.5).length;
      notice = `AI proposed ${proposal.resolutions.length} resolution${proposal.resolutions.length === 1 ? "" : "s"}${low ? `; ${low} with low confidence, review those closely` : ""}. Review, then Save & Mark Resolved.`;
    } catch (err) {
      error = String(err);
    } finally {
      aiWorking = "";
    }
  }

  function aiCommitMenuItems(commit: CommitNode): MenuItem[] {
    return [
      { separator: true },
      { label: "Share as link (secret gist)…", run: () => sharePatchLink("commit", commit.hash, `${commit.shortHash}.patch`, commit.subject) },
      { label: "Explain this commit with AI", run: () => explainCommit(commit.hash) },
    ];
  }

  function extraPaletteItems(repo: RepositoryState | null): PaletteItem[] {
    const items: PaletteItem[] = [];
    const add = (group: string, title: string, run: () => unknown, subtitle?: string) =>
      items.push({ id: `${group}:${title}`, group, title, subtitle, run: () => void run() });
    if (repo) {
      add("GitHub", "Pull requests", () => (centerMode = "prs"));
      add("GitHub", "Create pull request from this branch…", () => openCreatePr());
      add("Patches", "Share uncommitted changes as a link…", () => sharePatchLink("worktree", null, `${repoName}-changes.patch`, `Changes on ${currentBranch}`));
      add("Patches", "Share staged changes as a link…", () => sharePatchLink("staged", null, `${repoName}-staged.patch`, `Staged changes on ${currentBranch}`));
      add("Patches", "Apply a shared patch link…", applySharedPatch);
      add("AI", "Write the commit message from staged changes", writeCommitMessageWithAi, settings.aiEnabled ? undefined : "turn on in Settings");
      if (selectedCommit && !isStash(selectedCommit.hash)) add("AI", `Explain commit ${selectedCommit.shortHash}`, () => explainCommit(selectedCommit!.hash));
      if (conflict) add("AI", "Auto-resolve this file's conflicts", resolveConflictsWithAi);
    }
    add("Open", "Workspaces and your pull requests (launchpad)", () => { activeTabId = "launchpad"; centerMode = "launchpad"; });
    return items;
  }

  // ---- signature badge ----
  let signature: Signature | null = null;
  const signatureLabels: Record<string, string> = {
    G: "Verified signature",
    U: "Signed (key validity unknown)",
    X: "Signed (signature expired)",
    Y: "Signed (key expired)",
    R: "Signed (key revoked)",
    B: "Bad signature",
    E: "Signed (cannot verify: key missing)",
  };

  // ---- external tools ----
  async function openInEditor(path: string | null = null) {
    if (!settings.editorApp.trim()) {
      const app = prompt("Which editor should kGit open? (application name, e.g. Visual Studio Code, Cursor, Zed, Xcode)");
      if (!app?.trim()) return;
      settings = { ...settings, editorApp: app.trim() };
      persistSettings();
    }
    const result = await openExternal(settings.editorApp, path).catch((err) => ({ ok: false, stderr: String(err) }) as GitResult);
    if (!result.ok) error = result.stderr || `Unable to open ${settings.editorApp}`;
  }

  async function revealInFinder(path: string | null = null) {
    const result = await openExternal(null, path, true).catch((err) => ({ ok: false, stderr: String(err) }) as GitResult);
    if (!result.ok) error = result.stderr || "Unable to reveal the file";
  }

  async function externalTool(kind: "diff" | "merge", path: string) {
    const result = await launchTool(kind, path).catch((err) => ({ ok: false, stderr: String(err) }) as GitResult);
    if (!result.ok) error = result.stderr || `Unable to start the ${kind} tool`;
    else notice = `Opened ${path} in the ${kind} tool`;
  }

  // ---- patches ----
  async function savePatch(source: string, target: string | null, defaultName: string) {
    try {
      const text = await makePatch(source, target);
      if (!text.trim()) {
        notice = "Nothing to put in a patch";
        return;
      }
      const path = await pickSavePath("Save patch", defaultName);
      if (!path) return;
      await writeTextFile(path, text);
      notice = `Saved ${path}`;
    } catch (err) {
      error = String(err);
    }
  }

  async function copyPatch(source: string, target: string | null) {
    try {
      const text = await makePatch(source, target);
      await navigator.clipboard.writeText(text);
      notice = "Patch copied to the clipboard";
    } catch (err) {
      error = String(err);
    }
  }

  async function applyPatchText(text: string, origin: string) {
    if (!text.trim()) {
      error = `${origin} is empty`;
      return;
    }
    busy = true;
    error = "";
    try {
      const result = await applyPatch(text);
      const failure = result.ok ? "" : result.stderr || result.stdout || "The patch did not apply";
      await refresh();
      if (failure) error = failure;
      else notice = `Applied patch from ${origin}`;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function applyPatchFromFile() {
    const path = await pickOpenFile("Choose a patch file").catch(() => null);
    if (!path) return;
    try {
      await applyPatchText(await readTextFile(path), path.split("/").at(-1) ?? "file");
    } catch (err) {
      error = String(err);
    }
  }

  async function applyPatchFromClipboard() {
    try {
      await applyPatchText(await navigator.clipboard.readText(), "the clipboard");
    } catch (err) {
      error = String(err);
    }
  }

  // ---- repository settings panel ----
  type RepoSettingsTab = "remotes" | "identity" | "lfs" | "sparse" | "submodules" | "tools";
  let repoSettingsTab: RepoSettingsTab | null = null;

  function openRepoSettings(tab: RepoSettingsTab) {
    actionsOpen = false;
    repoSettingsTab = tab;
  }

  // ---- context menus ----
  let menuState: { x: number; y: number; title: string; items: MenuItem[] } | null = null;

  function showMenu(event: MouseEvent, title: string, items: MenuItem[]) {
    event.preventDefault();
    event.stopPropagation();
    menuState = { x: event.clientX, y: event.clientY, title, items };
  }

  const sep: MenuItem = { separator: true };

  function promptAction(question: string, initial: string, build: (value: string) => GitAction, label: (value: string) => string) {
    const value = prompt(question, initial);
    if (value?.trim()) void execute(build(value.trim()), label(value.trim()));
  }

  function mergeItems(target: string): MenuItem[] {
    return [
      { label: `Merge ${target} into ${currentBranch}`, run: () => execute({ kind: "merge", target }, `Merge ${target}`) },
      { label: "Merge (always create a merge commit)", run: () => execute({ kind: "mergeNoFf", target }, `Merge ${target} --no-ff`) },
      { label: "Squash merge (stage the changes, no commit)", run: () => execute({ kind: "mergeSquash", target }, `Squash ${target}`) },
      { label: `Fast-forward ${currentBranch} to ${target}`, run: () => execute({ kind: "mergeFfOnly", target }, `Fast-forward to ${target}`) },
    ];
  }

  function commitMenu(event: MouseEvent, commit: CommitNode) {
    const hash = commit.hash;
    const short = commit.shortHash;
    showMenu(event, `${short} ${commit.subject}`, [
      { label: "Check out (detached)", run: () => execute({ kind: "checkoutCommit", target: hash }, `Checkout ${short}`) },
      { label: "Create branch here…", run: () => createBranchAtCommit(hash) },
      { label: "Create tag here…", run: () => createTagPrompt(hash) },
      { label: "Create annotated tag here…", run: () => createAnnotatedTagPrompt(hash) },
      sep,
      { label: `Cherry-pick onto ${currentBranch}`, run: () => execute({ kind: "cherryPick", target: hash }, `Cherry-pick ${short}`) },
      { label: "Revert this commit", run: () => execute({ kind: "revert", target: hash }, `Revert ${short}`) },
      { label: `Rebase ${currentBranch} onto this commit`, run: () => openRebasePanel("plain", hash) },
      { label: "Interactive rebase from here…", run: () => openRebasePanel("interactive", hash) },
      sep,
      { label: `Reset ${currentBranch} here (soft: keep changes staged)`, run: () => execute({ kind: "reset", target: hash, mode: "soft" }, `Soft reset to ${short}`) },
      { label: `Reset ${currentBranch} here (mixed: keep changes unstaged)`, run: () => execute({ kind: "reset", target: hash, mode: "mixed" }, `Mixed reset to ${short}`) },
      { label: `Reset ${currentBranch} here (hard: discard changes)`, danger: true, run: () => execute({ kind: "reset", target: hash, mode: "hard" }, `Hard reset to ${short}`) },
      sep,
      { label: "Compare with working tree head", run: () => openCompare(hash, "HEAD") },
      { label: "Save as patch…", run: () => savePatch("commit", hash, `${short}.patch`) },
      { label: "Copy as patch", run: () => copyPatch("commit", hash) },
      { label: "Copy commit SHA", run: () => copyHash(hash) },
      { label: "Copy commit message", run: () => navigator.clipboard.writeText(commit.subject).then(() => (notice = "Copied message")) },
      ...aiCommitMenuItems(commit),
    ]);
  }

  function branchMenu(event: MouseEvent, branch: Branch) {
    const name = branch.name;
    const notCurrent = !branch.current;
    showMenu(event, name, [
      ...(notCurrent
        ? [
            { label: `Check out ${name}`, run: () => execute({ kind: "checkoutBranch", branch: name }, `Checkout ${name}`) },
            sep,
            ...mergeItems(name),
            { label: `Rebase ${currentBranch} onto ${name}`, run: () => rebaseOnto(name) },
            { label: `Interactive rebase ${currentBranch} onto ${name}…`, run: () => openRebasePanel("interactive", name) },
            sep,
          ]
        : []),
      { label: `Push ${name}${branch.upstream ? ` to ${branch.upstream}` : " (and track it)"}`, run: () => execute({ kind: "pushBranch", branch: name, remote: branch.upstream?.split("/")[0] ?? "origin" }, `Push ${name}`) },
      {
        label: "Set upstream…",
        run: () =>
          promptAction(`Upstream for ${name} (remote/branch)`, branch.upstream ?? `origin/${name}`, (value) => ({ kind: "setUpstream", branch: name, target: value }), (value) => `Track ${value}`),
      },
      ...(branch.upstream ? [{ label: "Unset upstream", run: () => execute({ kind: "unsetUpstream", branch: name }, `Untrack ${name}`) }] : []),
      sep,
      { label: "Rename…", run: () => promptAction(`Rename ${name} to`, name, (value) => ({ kind: "renameBranch", branch: name, target: value }), (value) => `Rename to ${value}`) },
      { label: "Create branch from here…", run: () => promptAction(`New branch from ${name}`, "", (value) => ({ kind: "createBranch", branch: value, target: name }), (value) => `Create ${value}`) },
      { label: "Create tag here…", run: () => createTagPrompt(name) },
      ...(notCurrent ? [{ label: `Compare with ${currentBranch}`, run: () => openCompare(currentBranch, name) }] : []),
      ...prMenuItems(name),
      { label: "Copy branch name", run: () => navigator.clipboard.writeText(name).then(() => (notice = `Copied ${name}`)) },
      ...(notCurrent ? [sep, { label: `Delete ${name}`, danger: true, run: () => deleteBranch(name) }] : []),
    ]);
  }

  function remoteBranchMenu(event: MouseEvent, name: string) {
    showMenu(event, name, [
      { label: `Check out ${name} (tracking branch)`, run: () => execute({ kind: "checkoutRemote", target: name }, `Checkout ${name}`) },
      sep,
      ...mergeItems(name),
      { label: `Rebase ${currentBranch} onto ${name}`, run: () => rebaseOnto(name) },
      { label: `Compare with ${currentBranch}`, run: () => openCompare(currentBranch, name) },
      { label: "Create branch from here…", run: () => promptAction(`New branch from ${name}`, name.split("/").slice(1).join("/"), (value) => ({ kind: "createBranch", branch: value, target: name }), (value) => `Create ${value}`) },
      ...prMenuItems(name.split("/").slice(1).join("/")),
      sep,
      {
        label: `Delete ${name} from the remote`,
        danger: true,
        run: () => {
          if (confirm(`Delete ${name} from the remote? This affects everyone using it.`)) void execute({ kind: "deleteRemoteBranch", target: name }, `Delete ${name}`);
        },
      },
    ]);
  }

  function tagMenu(event: MouseEvent, tag: string) {
    const remote = state?.remotes.includes("origin") ? "origin" : state?.remotes[0] ?? "origin";
    showMenu(event, tag, [
      { label: "Check out (detached)", run: () => execute({ kind: "checkoutCommit", target: tag }, `Checkout ${tag}`) },
      { label: "Create branch here…", run: () => promptAction(`New branch at ${tag}`, "", (value) => ({ kind: "createBranch", branch: value, target: tag }), (value) => `Create ${value}`) },
      sep,
      { label: `Push ${tag} to ${remote}`, run: () => execute({ kind: "pushTag", branch: tag, remote }, `Push ${tag}`) },
      { label: `Push all tags to ${remote}`, run: () => execute({ kind: "pushAllTags", remote }, "Push tags") },
      { label: "Copy tag name", run: () => navigator.clipboard.writeText(tag).then(() => (notice = `Copied ${tag}`)) },
      sep,
      { label: "Delete locally", danger: true, run: () => execute({ kind: "deleteTag", branch: tag }, `Delete tag ${tag}`) },
      {
        label: `Delete from ${remote}`,
        danger: true,
        run: () => {
          if (confirm(`Delete ${tag} from ${remote}?`)) void execute({ kind: "deleteRemoteTag", branch: tag, remote }, `Delete ${tag} from ${remote}`);
        },
      },
    ]);
  }

  function stashMenu(event: MouseEvent, stash: StashEntry) {
    showMenu(event, `${stash.name} ${stash.message}`, [
      { label: "Show changes", run: () => openStash(stash) },
      { label: "Apply", run: () => execute({ kind: "stashApply", target: stash.name }, `Apply ${stash.name}`) },
      { label: "Pop", run: () => execute({ kind: "stashPop", target: stash.name }, `Pop ${stash.name}`) },
      { label: "Create branch from stash…", run: () => promptAction("Branch name", "", (value) => ({ kind: "stashBranch", branch: value, target: stash.name }), (value) => `Branch ${value} from ${stash.name}`) },
      sep,
      { label: "Drop", danger: true, run: () => execute({ kind: "stashDrop", target: stash.name }, `Drop ${stash.name}`) },
    ]);
  }

  function worktreeMenu(event: MouseEvent, worktree: Worktree) {
    showMenu(event, worktree.path, [
      { label: "Open", disabled: worktree.current, run: () => openRepositoryPath(worktree.path) },
      worktree.locked
        ? { label: "Unlock", run: () => execute({ kind: "worktreeUnlock", path: worktree.path }, "Unlock worktree") }
        : { label: "Lock (protect from prune and removal)…", run: () => { const reason = prompt("Lock reason (optional)", ""); if (reason !== null) void execute({ kind: "worktreeLock", path: worktree.path, message: reason }, "Lock worktree"); } },
      { label: "Copy path", run: () => navigator.clipboard.writeText(worktree.path).then(() => (notice = "Copied path")) },
      ...(!worktree.main && !worktree.current ? [sep, { label: "Remove", danger: true, run: () => removeWorktree(worktree) }] : []),
    ]);
  }

  function fileMenu(event: MouseEvent, file: FileStatus) {
    const path = file.path;
    const staged = file.group === "staged";
    const items: MenuItem[] = [];
    if (file.group === "conflicted") {
      items.push(
        { label: "Resolve in kGit", run: () => openConflict(file) },
        { label: "Open in external merge tool", run: () => externalTool("merge", path) },
        { label: "Mark resolved", run: () => execute({ kind: "markResolved", path }, `Resolve ${path}`) },
        sep,
      );
    } else {
      items.push(
        staged
          ? { label: "Unstage", run: () => execute({ kind: "unstage", path }, `Unstage ${path}`) }
          : { label: "Stage", run: () => execute({ kind: "stage", path }, `Stage ${path}`) },
        ...(file.group === "unstaged" ? [{ label: "Stash this file", run: () => execute({ kind: "stashPush", path, mode: "all", message: `kGit: ${path}` }, `Stash ${path}`) }] : []),
        { label: "Open in external diff tool", disabled: file.group === "untracked", run: () => externalTool("diff", path) },
        sep,
      );
    }
    items.push(
      { label: "File history", disabled: file.group === "untracked", run: () => openFileInMode(file, "history") },
      { label: "Blame", disabled: file.group === "untracked", run: () => openFileInMode(file, "blame") },
      { label: "Open in editor", run: () => openInEditor(path) },
      { label: "Reveal in Finder", run: () => revealInFinder(path) },
      { label: "Copy path", run: () => navigator.clipboard.writeText(path).then(() => (notice = "Copied path")) },
    );
    if (file.group === "untracked") items.push({ label: "Ignore (add to .gitignore)", run: () => execute({ kind: "ignorePath", path }, `Ignore ${path}`) });
    if (!staged && file.group !== "conflicted") {
      items.push(sep, {
        label: file.group === "untracked" ? "Delete file" : "Discard changes",
        danger: true,
        run: () => execute({ kind: file.group === "untracked" ? "cleanUntracked" : "discard", path }, `Discard ${path}`),
      });
    }
    showMenu(event, path, items);
  }

  async function openFileInMode(file: FileStatus, mode: "history" | "blame" | "file") {
    fileViewMode = mode;
    await openFile(file);
  }

  // ---- drag a branch onto another to merge, rebase or push ----
  let dragRef: { name: string; remote: boolean } | null = null;
  let dropTarget = "";

  function startRefDrag(event: DragEvent, name: string, remote: boolean) {
    dragRef = { name, remote };
    event.dataTransfer?.setData("text/plain", name);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function allowRefDrop(event: DragEvent, target: string) {
    if (!dragRef || dragRef.name === target) return;
    event.preventDefault();
    dropTarget = target;
  }

  function endRefDrag() {
    dragRef = null;
    dropTarget = "";
  }

  function dropRef(event: DragEvent, target: string, targetIsRemote: boolean) {
    event.preventDefault();
    const source = dragRef;
    endRefDrag();
    if (!source || source.name === target) return;
    const items: MenuItem[] = [];
    const targetIsCurrent = !targetIsRemote && target === state?.currentBranch;
    const sourceIsCurrent = !source.remote && source.name === state?.currentBranch;
    if (targetIsRemote && !source.remote) {
      items.push(
        { label: `Push ${source.name} to ${target}`, run: () => execute({ kind: "pushTo", branch: source.name, target }, `Push ${source.name} to ${target}`) },
        { label: `Set ${source.name} to track ${target}`, run: () => execute({ kind: "setUpstream", branch: source.name, target }, `Track ${target}`) },
      );
    } else if (targetIsCurrent) {
      items.push(...mergeItems(source.name), { label: `Rebase ${target} onto ${source.name}`, run: () => rebaseOnto(source.name) });
    } else if (sourceIsCurrent) {
      items.push(
        { label: `Rebase ${source.name} onto ${target}`, run: () => rebaseOnto(target) },
        { label: `Merge ${target} into ${source.name}`, run: () => execute({ kind: "merge", target }, `Merge ${target}`) },
      );
    } else if (!targetIsRemote) {
      items.push({
        label: `Check out ${target} and merge ${source.name} into it`,
        run: async () => {
          if (await execute({ kind: "checkoutBranch", branch: target }, `Checkout ${target}`)) await execute({ kind: "merge", target: source.name }, `Merge ${source.name}`);
        },
      });
    }
    items.push(sep, { label: `Compare ${source.name} with ${target}`, run: () => openCompare(target, source.name) });
    menuState = { x: event.clientX, y: event.clientY, title: `${source.name} → ${target}`, items };
  }

  /** The first local branch on a graph row, as a drop target. */
  function rowBranch(row: GraphRow): string | null {
    return row.refs.find((ref) => ref.local && !ref.tag)?.name ?? null;
  }

  // ---- annotated tags ----
  function createAnnotatedTagPrompt(target?: string) {
    const name = prompt("Annotated tag name");
    if (!name?.trim()) return;
    const message = prompt(`Message for ${name.trim()}`, name.trim());
    if (message == null) return;
    void execute({ kind: "createAnnotatedTag", branch: name.trim(), message: message.trim() || name.trim(), target: target ?? null }, `Tag ${name.trim()}`);
  }

  // ---- graph keyboard navigation ----
  function moveGraphSelection(delta: number) {
    if (!visibleGraphRows.length) return;
    const current = selectedCommit ? visibleGraphRows.findIndex((row) => row.commit.hash === selectedCommit?.hash) : -1;
    const next = Math.max(0, Math.min(visibleGraphRows.length - 1, current < 0 ? 0 : current + delta));
    void selectCommit(visibleGraphRows[next].commit);
    if (graphScroller) {
      const top = wipOffset + next * ROW_HEIGHT;
      if (top < graphScroller.scrollTop) graphScroller.scrollTop = top;
      else if (top + ROW_HEIGHT > graphScroller.scrollTop + graphViewport) graphScroller.scrollTop = top + ROW_HEIGHT - graphViewport;
    }
  }

  // ---- favorites ----
  $: favoriteRepos = settings.favorites.map((path) => recentRepos.find((repo) => repo.path === path) ?? { name: path.split("/").filter(Boolean).at(-1) ?? path, path });

  function toggleFavorite(path: string) {
    const favorites = settings.favorites.includes(path) ? settings.favorites.filter((entry) => entry !== path) : [...settings.favorites, path];
    settings = { ...settings, favorites };
    persistSettings();
  }

  // ---- command palette and quick open ----
  let paletteMode: "commands" | "files" | null = null;
  let paletteFiles: string[] = [];
  let shortcutsOpen = false;

  async function openQuickOpen() {
    if (!state) return;
    paletteFiles = [];
    paletteMode = "files";
    try {
      paletteFiles = await listFiles();
    } catch (err) {
      error = String(err);
    }
  }

  function openPathInMode(path: string, mode: "history" | "blame" | "file") {
    const changed = state?.files.find((file) => file.path === path && normalizedGroup(file) !== "staged");
    void openFileInMode(changed ?? { path, index: ".", worktree: ".", group: "unstaged" }, mode);
  }

  $: fileItems = paletteFiles.map(
    (path): PaletteItem => ({ id: `file:${path}`, group: "Files", title: path, subtitle: "history", run: () => openPathInMode(path, "history") }),
  );

  const shortcuts = [
    { group: "General", keys: "⌘K or ⇧⌘P", action: "Command palette" },
    { group: "General", keys: "⌘P", action: "Quick open a file (history / blame)" },
    { group: "General", keys: "⌘/", action: "Keyboard shortcuts" },
    { group: "General", keys: "⌘,", action: "Settings" },
    { group: "General", keys: "⌘R", action: "Refresh" },
    { group: "General", keys: "⌘Z / ⇧⌘Z", action: "Undo / redo the last git action" },
    { group: "Tabs", keys: "⌘1 – ⌘9", action: "Switch to tab" },
    { group: "Tabs", keys: "⌥⌘1 – ⌥⌘9", action: "Open favorite repository" },
    { group: "Tabs", keys: "⌘W", action: "Close tab" },
    { group: "Tabs", keys: "⇧⌘T", action: "Reopen closed tab" },
    { group: "Tabs", keys: "⌃Tab / ⌃⇧Tab", action: "Next / previous tab" },
    { group: "View", keys: "⌘= / ⌘- / ⌘0", action: "Zoom in / out / reset" },
    { group: "View", keys: "⌘J", action: "Toggle the left panel" },
    { group: "View", keys: "⌘L", action: "Toggle the commit panel" },
    { group: "View", keys: "⌥⌘F", action: "Filter branches and files" },
    { group: "View", keys: "⌘F", action: "Search the graph" },
    { group: "Graph", keys: "↑ ↓ or K J", action: "Previous / next commit" },
    { group: "Graph", keys: "Right-click", action: "Commit, branch, tag, stash and file actions" },
    { group: "Graph", keys: "Drag a branch", action: "Onto another to merge, rebase or push" },
    { group: "Changes", keys: "⇧⌘S / ⇧⌘U", action: "Stage all / unstage all" },
    { group: "Changes", keys: "⌘↵", action: "Commit (from the message box)" },
    { group: "Changes", keys: "Click / ⇧-click line numbers", action: "Pick lines to stage" },
    { group: "Remote", keys: "⇧⌘L / ⇧⌘K", action: "Pull / push" },
    { group: "Remote", keys: "⌥⌘L", action: "Fetch all" },
    { group: "Branches", keys: "⌘B", action: "New branch" },
  ];

  $: paletteCommands = buildPaletteCommands(state, recentRepos, tabs, settings, undoItem, redoItem, closedTabs);

  function buildPaletteCommands(
    repo: RepositoryState | null,
    recent: RecentRepo[],
    openTabs: AppTab[],
    prefs: Settings,
    undo: UndoItem | null,
    redo: UndoItem | null,
    closed: AppTab[],
  ): PaletteItem[] {
    const items: PaletteItem[] = [];
    const add = (group: string, title: string, run: () => unknown, shortcut?: string, subtitle?: string) =>
      items.push({ id: `${group}:${title}`, group, title, run: () => void run(), shortcut, subtitle });
    if (repo) {
      add("Repository", "Fetch all remotes", () => execute({ kind: "fetchAll" }, "Fetch all remotes"), "⌥⌘L");
      for (const option of pullOptions) add("Repository", option.label, () => execute({ kind: option.kind }, option.label), option.kind === prefs.pullMode ? "⇧⌘L" : undefined);
      add("Repository", "Push", () => execute({ kind: "push" }, "Push"), "⇧⌘K");
      add("Repository", "Force push (with lease)", () => execute({ kind: "forcePush" }, "Force push"));
      add("Repository", "Push all tags", () => execute({ kind: "pushAllTags" }, "Push tags"));
      add("Repository", "Refresh", () => refresh(), "⌘R");
      if (undo) add("Repository", `Undo ${undo.entry.label}`, () => replay("undo"), "⌘Z");
      if (redo) add("Repository", `Redo ${redo.entry.label}`, () => replay("redo"), "⇧⌘Z");
      add("Branches", "New branch…", createBranchFromToolbar, "⌘B");
      add("Branches", "New tag at HEAD…", () => createTagPrompt());
      add("Branches", "New annotated tag at HEAD…", () => createAnnotatedTagPrompt());
      add("Branches", "Compare refs…", () => openCompare(null, currentBranch));
      add("Branches", "Interactive rebase…", () => openRebasePanel("interactive"));
      add("Branches", "Clean up branches…", () => (cleanupOpen = true));
      add("Branches", "Reflog: recover lost commits…", () => (centerMode = "reflog"));
      for (const branch of repo.branches.filter((entry) => !entry.current)) {
        add("Check out", branch.name, () => execute({ kind: "checkoutBranch", branch: branch.name }, `Checkout ${branch.name}`), undefined, branch.lastCommitRelative);
      }
      for (const name of repo.remoteBranches.filter((remote) => !repo.branches.some((branch) => remote.endsWith(`/${branch.name}`)))) {
        add("Check out", name, () => execute({ kind: "checkoutRemote", target: name }, `Checkout ${name}`), undefined, "remote");
      }
      add("Changes", "Stage all changes", () => execute({ kind: "stage", path: "." }, "Stage all changes"), "⇧⌘S");
      add("Changes", "Unstage all changes", () => execute({ kind: "unstage", path: "." }, "Unstage all changes"), "⇧⌘U");
      add("Changes", "Stash all changes", () => execute({ kind: "stashPush", mode: "all", message: "kGit stash" }, "Create stash"));
      add("Changes", "Stash staged changes only", () => execute({ kind: "stashPush", mode: "staged", message: "kGit stash (staged)" }, "Stash staged changes"));
      add("Changes", "Stash, keeping the index", () => execute({ kind: "stashPush", mode: "keepIndex", message: "kGit stash" }, "Create stash"));
      add("Changes", "Discard all changes…", () => { if (confirm("Discard every uncommitted change to tracked files? Untracked files stay.")) void execute({ kind: "discardAll" }, "Discard all changes"); });
      add("Patches", "Save uncommitted changes as a patch…", () => savePatch("worktree", null, `${repoName}-changes.patch`));
      add("Patches", "Save staged changes as a patch…", () => savePatch("staged", null, `${repoName}-staged.patch`));
      add("Patches", "Copy uncommitted changes as a patch", () => copyPatch("worktree", null));
      add("Patches", "Apply a patch file…", applyPatchFromFile);
      add("Patches", "Apply a patch from the clipboard", applyPatchFromClipboard);
      add("Repository settings", "Remotes…", () => openRepoSettings("remotes"));
      add("Repository settings", "Author identity & commit signing…", () => openRepoSettings("identity"));
      add("Repository settings", "Git LFS…", () => openRepoSettings("lfs"));
      add("Repository settings", "Sparse checkout…", () => openRepoSettings("sparse"));
      add("Repository settings", "Submodules…", () => openRepoSettings("submodules"));
      add("Repository settings", "External diff & merge tools…", () => openRepoSettings("tools"));
      add("Open", "Open repository in editor", () => openInEditor());
      add("Open", "Reveal repository in Finder", () => revealInFinder());
      add("Open", "Open terminal here", openRepoTerminal);
      add("Open", "Quick open a file…", openQuickOpen, "⌘P");
      add(
        "Open",
        repo && prefs.favorites.includes(repo.root) ? "Remove this repository from favorites" : "Add this repository to favorites",
        () => toggleFavorite(repo.root),
      );
    }
    add("Open", "Open repository…", switchRepository);
    add("Open", "Clone repository…", () => (cloneOpen = true));
    add("Open", "Create repository…", openCreatePrompt);
    for (const recentRepo of recent) add("Recent", recentRepo.name, () => openRepositoryPath(recentRepo.path), undefined, recentRepo.path.replace(/^\/Users\/[^/]+/, "~"));
    openTabs.forEach((tab, index) => add("Tabs", `Switch to ${tab.label}`, () => switchToTab(tab), index < 9 ? `⌘${index + 1}` : undefined));
    if (closed.length) add("Tabs", `Reopen ${closed.at(-1)?.label}`, reopenClosedTab, "⇧⌘T");
    add("View", prefs.showLeftPanel ? "Hide the left panel" : "Show the left panel", () => togglePanel("showLeftPanel"), "⌘J");
    add("View", prefs.showRightPanel ? "Hide the commit panel" : "Show the commit panel", () => togglePanel("showRightPanel"), "⌘L");
    add("View", "Zoom in", () => setZoom(prefs.zoom + 0.1), "⌘=");
    add("View", "Zoom out", () => setZoom(prefs.zoom - 0.1), "⌘-");
    add("View", "Reset zoom", () => setZoom(1), "⌘0");
    add("Help", "Keyboard shortcuts", () => (shortcutsOpen = true), "⌘/");
    add("Help", "Settings", () => (settingsOpen = true), "⌘,");
    items.push(...extraPaletteItems(repo));
    return items;
  }

  /** Global shortcuts. Returns true when the event was handled. */
  function handleShortcut(event: KeyboardEvent, typing: boolean): boolean {
    const mod = event.metaKey || event.ctrlKey;
    const key = event.key.toLowerCase();
    const run = (fn: () => unknown) => {
      event.preventDefault();
      void fn();
      return true;
    };
    if (mod && !event.altKey && (key === "k" && !event.shiftKey || key === "p" && event.shiftKey)) return run(() => (paletteMode = "commands"));
    if (mod && !event.altKey && !event.shiftKey && key === "p") return run(openQuickOpen);
    if (mod && key === "/") return run(() => (shortcutsOpen = true));
    if (mod && key === ",") return run(() => (settingsOpen = true));
    if (mod && !event.shiftKey && (key === "=" || key === "+")) return run(() => setZoom(settings.zoom + 0.1));
    if (mod && !event.shiftKey && key === "-") return run(() => setZoom(settings.zoom - 0.1));
    if (mod && !event.shiftKey && key === "0") return run(() => setZoom(1));
    if (mod && !event.shiftKey && !event.altKey && key === "j") return run(() => togglePanel("showLeftPanel"));
    if (mod && !event.shiftKey && !event.altKey && key === "l") return run(() => togglePanel("showRightPanel"));
    if (mod && event.shiftKey && key === "t") return run(reopenClosedTab);
    if (mod && !event.shiftKey && key === "w") {
      const tab = tabs.find((item) => item.id === activeTabId);
      return tab && tab.kind === "repo" ? run(() => closeTab(tab)) : false;
    }
    if (event.ctrlKey && event.key === "Tab") {
      const index = tabs.findIndex((tab) => tab.id === activeTabId);
      const next = tabs[(index + (event.shiftKey ? -1 : 1) + tabs.length) % tabs.length];
      return next ? run(() => switchToTab(next)) : false;
    }
    if (mod && /^Digit[1-9]$/.test(event.code)) {
      const index = Number(event.code.slice(5)) - 1;
      if (event.altKey) {
        const favorite = favoriteRepos[index];
        return favorite ? run(() => openRepositoryPath(favorite.path)) : false;
      }
      const tab = tabs[index];
      return tab ? run(() => switchToTab(tab)) : false;
    }
    if (!state || busy) return false;
    if (mod && !event.shiftKey && key === "r") return run(() => refresh());
    if (mod && !event.shiftKey && !event.altKey && key === "f") return run(() => (searchOpen = true));
    if (mod && !event.shiftKey && key === "b" && !typing) return run(createBranchFromToolbar);
    if (mod && event.shiftKey && key === "s") return run(() => execute({ kind: "stage", path: "." }, "Stage all changes"));
    if (mod && event.shiftKey && key === "u") return run(() => execute({ kind: "unstage", path: "." }, "Unstage all changes"));
    if (mod && event.shiftKey && key === "l") return run(() => execute({ kind: settings.pullMode }, "Pull"));
    if (mod && event.shiftKey && key === "k") return run(() => execute({ kind: "push" }, "Push"));
    if (mod && event.altKey && event.code === "KeyL") return run(() => execute({ kind: "fetchAll" }, "Fetch all remotes"));
    if (!typing && !mod && centerMode === "graph" && (key === "arrowdown" || key === "j")) return run(() => moveGraphSelection(1));
    if (!typing && !mod && centerMode === "graph" && (key === "arrowup" || key === "k")) return run(() => moveGraphSelection(-1));
    return false;
  }

  async function openRepoTerminal() {
    actionsOpen = false;
    busy = true;
    error = "";
    try {
      const result = settings.terminalApp.trim() && !isDemo ? await openExternal(settings.terminalApp.trim()) : await openTerminal();
      if (!result.ok) error = result.stderr || "Unable to open terminal";
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function statusLabel(status: string) {
    const map: Record<string, string> = { A: "added", M: "modified", D: "deleted", R: "renamed", C: "copied" };
    return map[status] ?? status;
  }

  async function restartToUpdate() {
    if (!availableUpdate) return;
    installingUpdate = true;
    try {
      await installUpdate(availableUpdate);
    } catch (err) {
      error = `Update failed: ${err}`;
      installingUpdate = false;
    }
  }

  async function initClonePath() {
    if (settings.clonePath.trim()) return;
    const home = await homeDir().catch(() => null);
    if (!home) return;
    settings = { ...settings, clonePath: `${home.replace(/\/$/, "")}/Documents/dev` };
  }

  // Reopen the last repository; with none (a fresh install) start on the
  // launchpad rather than guessing a path. The browser demo always has one.
  async function startup() {
    await initClonePath();
    const last = recentRepos[0];
    if (isDemo) await refresh();
    else if (last && (await openRepositoryPath(last.path, true))) return;
    else {
      // A last repository that was moved or deleted lands on the launchpad,
      // not on an empty repository view.
      if (last) error = `Couldn't reopen ${last.path}: ${error}`;
      activeTabId = "launchpad";
      centerMode = "launchpad";
    }
  }

  void startup();
  void checkForUpdate().then((update) => (availableUpdate = update));
</script>

<svelte:window
  on:focus={onWindowFocus}
  on:keydown={(event) => {
    if (event.key === "Escape") {
      settingsOpen = false;
      actionsOpen = false;
      pullMenuOpen = false;
      branchMenuOpen = false;
      shortcutsOpen = false;
    }
    // ⌘Z / ⇧⌘Z (or ⌘Y) undo and redo git actions, but never while typing,
    // where they belong to the text field.
    const typing =
      event.target instanceof HTMLElement &&
      (event.target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName));
    if (paletteMode || menuState || repoSettingsTab) return;
    if (handleShortcut(event, typing)) return;
    const key = event.key.toLowerCase();
    if ((event.metaKey || event.ctrlKey) && !event.altKey && !typing && (key === "z" || key === "y")) {
      if (centerMode !== "launchpad" && !settingsOpen && !conflict) {
        event.preventDefault();
        void replay(key === "z" && !event.shiftKey ? "undo" : "redo");
      }
    }
    if ((event.metaKey || event.ctrlKey) && event.altKey && event.code === "KeyF") {
      event.preventDefault();
      filterInput?.focus();
      filterInput?.select();
    }
  }}
  on:click={(event) => {
    if (actionsOpen && !(event.target instanceof Element && event.target.closest(".search-actions"))) {
      actionsOpen = false;
    }
    if (pullMenuOpen && !(event.target instanceof Element && event.target.closest(".pull-split"))) {
      pullMenuOpen = false;
    }
    if (branchMenuOpen && !(event.target instanceof Element && event.target.closest(".branch-picker"))) {
      branchMenuOpen = false;
    }
  }}
/>

<main
  class="shell"
  class:launchpad-mode={centerMode === "launchpad"}
  class:hide-left={!settings.showLeftPanel}
  class:hide-right={!settings.showRightPanel}
>
  <!-- Dragging the empty chrome moves the window: with an overlay title
       bar there is no system title bar left to grab. Tauri checks the
       event target itself, not its ancestors, so every container that
       covers empty header space carries the attribute; the tabs and
       buttons inside still take clicks. -->
  <header class="app-header" data-tauri-drag-region>
    <nav class="tabs" aria-label="Open repositories" data-tauri-drag-region>
      {#each tabs as tab}
        <div class="tab-wrap">
          <button class="tab {tab.id === activeTabId ? 'active' : ''}" on:click={() => switchToTab(tab)}>
            {#if tab.kind === "repo"}
              <svg class="tab-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></svg>
            {/if}
            <span>{tab.label}</span>
          </button>
          {#if tab.kind === "repo"}
            <button class="tab-close" on:click={() => closeTab(tab)} aria-label={`Close ${tab.label}`}>×</button>
          {/if}
        </div>
      {/each}
      <button class="tab add-tab" title="Open repository" on:click={showAddRepoNotice}>+</button>
    </nav>
    <div class="account-strip" data-tauri-drag-region>
      <button title="Notifications" on:click={() => (notice = "No notifications")}>◔</button>
      <button title="Settings" on:click={() => (settingsOpen = true)}>⚙</button>
      <strong data-tauri-drag-region>{accountName}</strong>
    </div>
  </header>

  {#if centerMode !== "launchpad"}
    <div class="repo-bar">
      <div class="repo-group">
        <button class="repo-select" on:click={switchRepository}>
          <span>repository</span>
          <strong>{repoName}</strong>
        </button>
        <div class="branch-picker">
          <button
            class="branch-select"
            title="Switch branch"
            aria-haspopup="menu"
            aria-expanded={branchMenuOpen}
            class:active={branchMenuOpen}
            on:click={() => {
              actionsOpen = false;
              pullMenuOpen = false;
              branchMenuOpen = !branchMenuOpen;
            }}
            disabled={busy}
          >
            <span>branch</span>
            <strong>{currentBranch}</strong>
          </button>
          {#if branchMenuOpen}
            <div class="dropdown-menu branch-menu" role="menu">
              {#each state?.branches ?? [] as branch (branch.name)}
                <button
                  role="menuitemradio"
                  aria-checked={branch.current}
                  class:current={branch.current}
                  title={branchTooltip(branch)}
                  on:click={() => (branch.current ? (branchMenuOpen = false) : execute({ kind: "checkoutBranch", branch: branch.name }, `Checkout ${branch.name}`))}
                >
                  <i class="branch-check" aria-hidden="true">{branch.current ? "✓" : ""}</i>
                  <span>{branch.name}</span>
                </button>
              {:else}
                <p class="empty">No local branches</p>
              {/each}
            </div>
          {/if}
        </div>
      </div>
      <div class="top-actions">
        <button title={undoItem ? `Undo ${undoItem.entry.label} (⌘Z)` : "Nothing to undo"} on:click={() => replay("undo")} disabled={busy || !undoItem}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /></svg>
          <span>Undo</span>
        </button>
        <button title={redoItem ? `Redo ${redoItem.entry.label} (⇧⌘Z)` : "Nothing to redo"} on:click={() => replay("redo")} disabled={busy || !redoItem}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" /><path d="M21 3v5h-5" /></svg>
          <span>Redo</span>
        </button>
        <div class="pull-split">
          <button title={pullOptions.find((option) => option.kind === settings.pullMode)?.label ?? "Pull"} on:click={() => execute({ kind: settings.pullMode }, "Pull")} disabled={busy}>
            <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v13" /><path d="m6 10 6 6 6-6" /><path d="M5 21h14" /></svg>
            <span>Pull</span>
          </button>
          <button class="pull-caret" title="Pull options" aria-haspopup="menu" class:active={pullMenuOpen} on:click={() => (pullMenuOpen = !pullMenuOpen)} disabled={busy}>
            <svg class="caret-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M5 9h14l-7 8z" /></svg>
          </button>
          {#if pullMenuOpen}
            <div class="dropdown-menu pull-menu" role="menu">
              <button class="menu-plain" on:click={() => runPullMenu({ kind: "fetchAll" }, "Fetch all remotes")}>Fetch All</button>
              {#each pullOptions as option (option.kind)}
                <div class="menu-row">
                  <button
                    class="menu-default"
                    class:checked={settings.pullMode === option.kind}
                    title="Use for the Pull button"
                    aria-label={`Use ${option.label} for the Pull button`}
                    on:click={() => setPullMode(option.kind)}
                  ></button>
                  <button on:click={() => runPullMenu({ kind: option.kind }, option.label)}>{option.label}</button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
        <button title="Push" on:click={() => execute({ kind: "push" }, "Push")} disabled={busy}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 16V3" /><path d="m6 9 6-6 6 6" /><path d="M5 21h14" /></svg>
          <span>Push</span>
        </button>
        <button title="Branch" on:click={createBranchFromToolbar} disabled={busy}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></svg>
          <span>Branch</span>
        </button>
        <button title={totalChanges ? "Stash all changes" : "Nothing to stash"} on:click={() => execute({ kind: "stashCreate", message: "kGit stash" }, "Create stash")} disabled={busy || totalChanges === 0}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v9" /><path d="m8 8 4 4 4-4" /><path d="M3 14h5l1.5 2.5h5L16 14h5v7H3z" /></svg>
          <span>Stash</span>
        </button>
        <button title={state?.stashes.length ? `Pop ${state.stashes[0].message}` : "No stashes"} on:click={() => execute({ kind: "stashPop", target: "stash@{0}" }, "Pop stash")} disabled={busy || !state?.stashes.length}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 12V3" /><path d="m8 7 4-4 4 4" /><path d="M3 14h5l1.5 2.5h5L16 14h5v7H3z" /></svg>
          <span>Pop</span>
        </button>
        <span class="toolbar-sep" aria-hidden="true"></span>
        <button title="Terminal" on:click={openRepoTerminal} disabled={busy}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>
          <span>Terminal</span>
        </button>
      </div>
      <div class="search-actions">
        <button title="Actions" class:active={actionsOpen} on:click={() => (actionsOpen = !actionsOpen)}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="4" y1="21" x2="4" y2="14" /><line x1="4" y1="10" x2="4" y2="3" /><line x1="12" y1="21" x2="12" y2="12" /><line x1="12" y1="8" x2="12" y2="3" /><line x1="20" y1="21" x2="20" y2="16" /><line x1="20" y1="12" x2="20" y2="3" /><line x1="1" y1="14" x2="7" y2="14" /><line x1="9" y1="8" x2="15" y2="8" /><line x1="17" y1="16" x2="23" y2="16" /></svg>
          <span>Actions</span>
        </button>
        <button title="Search" on:click={() => (searchOpen = !searchOpen)}>
          <svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>
          <span>Search</span>
        </button>
        {#if actionsOpen}
          <div class="dropdown-menu" role="menu">
            <button on:click={() => { actionsOpen = false; void refresh(); }} disabled={busy}>Refresh</button>
            <button on:click={() => { actionsOpen = false; openCompare(null, currentBranch); }}>Compare Refs…</button>
            <button on:click={() => { actionsOpen = false; centerMode = "reflog"; }}>Reflog (recover lost commits)…</button>
            {#if githubRemote}
              <button on:click={() => { actionsOpen = false; centerMode = "prs"; }}>Pull Requests &amp; Issues…</button>
              <button on:click={() => { actionsOpen = false; openCreatePr(); }}>Create Pull Request…</button>
            {/if}
            <button on:click={() => execute({ kind: "fetchAll" }, "Fetch all remotes")}>Fetch All &amp; Prune</button>
            <button on:click={() => execute({ kind: "forcePush" }, "Force push")}>Force Push (with lease)</button>
            <button on:click={() => openRebasePanel("interactive")}>Interactive Rebase…</button>
            <button on:click={() => openRebasePanel("plain")}>Rebase onto…</button>
            <button on:click={() => createTagPrompt()}>Create Tag at HEAD…</button>
            <button
              on:click={() => {
                actionsOpen = false;
                const message = prompt("Stash message", "kGit stash");
                if (message != null) execute({ kind: "stashCreate", message: message.trim() || "kGit stash" }, "Create stash");
              }}
            >
              Stash With Message…
            </button>
            <button on:click={openRepoTerminal}>Open Terminal Here</button>
            <button on:click={() => { actionsOpen = false; void openInEditor(); }}>Open in Editor</button>
            <button on:click={() => { actionsOpen = false; void revealInFinder(); }}>Reveal in Finder</button>
            <button on:click={() => { actionsOpen = false; void savePatch("worktree", null, `${repoName}-changes.patch`); }}>Save Changes as Patch…</button>
            <button on:click={() => { actionsOpen = false; void applyPatchFromFile(); }}>Apply Patch…</button>
            <button on:click={() => openRepoSettings("remotes")}>Repository Settings…</button>
            <button on:click={() => { actionsOpen = false; paletteMode = "commands"; }}>All Commands… <kbd>⌘K</kbd></button>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if centerMode !== "launchpad"}
    <aside class="left-panel">
      <div class="viewing-row">Viewing <strong>{filteredBranches.length + filteredRemoteBranches.length}</strong></div>
      <div class="filter-block">
        <div class="filter-input-wrap">
          <input
            aria-label="Filter refs"
            bind:this={filterInput}
            bind:value={searchQuery}
            placeholder="Filter (⌘ + Option + f)"
          />
          <span class="filter-icon">⌕</span>
        </div>
      </div>

      <div class="nav-scroll">
        <section class="nav-section">
          <div class="section-head">
            <button class="nav-row" on:click={() => (localOpen = !localOpen)}>
              <span class="nav-chevron">{localOpen ? "⌄" : "›"}</span>
              <i class="nav-icon">⌂</i>
              <span class="nav-label">Local</span>
              <strong>{state?.branches.length ?? 0}</strong>
            </button>
            <span class="section-actions">
              <button
                class="section-action"
                title="Delete merged, squash-merged and gone branches…"
                on:click={() => (cleanupOpen = true)}
                disabled={busy}
              >clean up</button>
            </span>
          </div>
          {#if localOpen}
            <div class="branch-list">
              {#each localTree as row (row.key)}
                {#if row.kind === "dir"}
                  <button class="ref-tree-dir" style={`--depth:${row.depth}`} on:click={() => (collapsedLocalDirs = toggleRefDir(collapsedLocalDirs, row.key))}>
                    <svg class="tree-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
                    <span>{row.label}</span>
                  </button>
                {:else}
                  <div
                    class="branch-row"
                    class:active={row.item.current}
                    class:drop-target={dropTarget === row.item.name}
                    style={`--depth:${row.depth}`}
                    draggable="true"
                    role="listitem"
                    on:dragstart={(event) => startRefDrag(event, row.item.name, false)}
                    on:dragend={endRefDrag}
                    on:dragover={(event) => allowRefDrop(event, row.item.name)}
                    on:dragleave={() => (dropTarget = "")}
                    on:drop={(event) => dropRef(event, row.item.name, false)}
                    on:contextmenu={(event) => branchMenu(event, row.item)}
                  >
                    <button
                      class="branch-name"
                      title={branchTooltip(row.item)}
                      on:click={() => execute({ kind: "checkoutBranch", branch: row.item.name }, `Checkout ${row.item.name}`)}
                      disabled={busy || row.item.current}
                    >
                      <span class="branch-name-line">
                        {#if row.item.current}<i class="branch-check">✓</i>{/if}
                        <svg class="tree-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></svg>
                        <span use:hoverExpand={{ text: row.label }}>{row.label}</span>
                      </span>
                    </button>
                    {#if !row.item.current}
                      <button
                        class="row-action"
                        title={`Rebase ${currentBranch} onto ${row.item.name}`}
                        on:click={() => rebaseOnto(row.item.name)}
                        disabled={busy}
                      >⤴</button>
                      <button
                        class="row-action danger"
                        title={`Delete ${row.item.name}`}
                        on:click={() => deleteBranch(row.item.name)}
                        disabled={busy}
                      >×</button>
                    {/if}
                  </div>
                {/if}
              {:else}
                <p class="empty">No local branches</p>
              {/each}
            </div>
          {/if}
        </section>
        {#if lastOpenNavIndex === 0}<div class="nav-spacer"></div>{/if}

        <section class="nav-section">
          <button class="nav-row" on:click={() => (remoteOpen = !remoteOpen)}>
            <span class="nav-chevron">{remoteOpen ? "⌄" : "›"}</span>
            <i class="nav-icon">☁</i>
            <span class="nav-label">Remote</span>
            <strong>{state?.remoteBranches.length ?? 0}</strong>
          </button>
          {#if remoteOpen}
            <div class="branch-list">
              {#each remoteTree as row (row.key)}
                {#if row.kind === "dir"}
                  <button class="ref-tree-dir" style={`--depth:${row.depth}`} on:click={() => (collapsedRemoteDirs = toggleRefDir(collapsedRemoteDirs, row.key))}>
                    <svg class="tree-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
                    <span>{row.label}</span>
                  </button>
                {:else}
                  <button
                    class="branch-name tree-leaf"
                    class:drop-target={dropTarget === row.item}
                    style={`--depth:${row.depth}`}
                    title={`Checkout tracking branch for ${row.item}`}
                    draggable="true"
                    on:dragstart={(event) => startRefDrag(event, row.item, true)}
                    on:dragend={endRefDrag}
                    on:dragover={(event) => allowRefDrop(event, row.item)}
                    on:dragleave={() => (dropTarget = "")}
                    on:drop={(event) => dropRef(event, row.item, true)}
                    on:contextmenu={(event) => remoteBranchMenu(event, row.item)}
                    on:click={() => execute({ kind: "checkoutRemote", target: row.item }, `Checkout ${row.item}`)}
                    disabled={busy}
                  >
                    <span class="branch-name-line"><svg class="tree-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" /><path d="M18 9a9 9 0 0 1-9 9" /></svg> <span use:hoverExpand={{ text: row.label }}>{row.label}</span></span>
                  </button>
                {/if}
              {:else}
                <p class="empty">No remote branches</p>
              {/each}
            </div>
          {/if}
        </section>
        {#if lastOpenNavIndex === 1}<div class="nav-spacer"></div>{/if}

        <section class="nav-section">
          <div class="section-head">
            <button class="nav-row" on:click={() => (worktreesOpen = !worktreesOpen)}>
              <span class="nav-chevron">{worktreesOpen ? "⌄" : "›"}</span>
              <i class="nav-icon">⧉</i>
              <span class="nav-label">Worktrees</span>
              <strong>{state?.worktrees.length ?? 0}</strong>
            </button>
            <span class="section-actions">
              {#if (state?.worktrees ?? []).some((entry) => entry.prunable)}
                <button
                  class="section-action"
                  title="Prune worktrees whose directories are gone"
                  on:click={() => execute({ kind: "worktreePrune" }, "Prune worktrees")}
                  disabled={busy}
                >prune</button>
              {/if}
              <button class="section-action" title="Add worktree…" on:click={addWorktreePrompt} disabled={busy}>+</button>
            </span>
          </div>
          {#if worktreesOpen}
            <div class="branch-list">
              {#each state?.worktrees ?? [] as worktree (worktree.path)}
                <div class="worktree-row" class:active={worktree.current} class:stale={worktree.prunable} role="listitem" on:contextmenu={(event) => worktreeMenu(event, worktree)}>
                  <button
                    class="worktree-main"
                    title={worktree.current ? worktree.path : `Switch to ${worktree.path}`}
                    on:click={() => openRepositoryPath(worktree.path)}
                    disabled={busy || worktree.current}
                  >
                    <span class="wt-dot"></span>
                    <span class="wt-label">
                      <span class="wt-name">
                        <span class="wt-branch" use:hoverExpand={{ text: worktreeLabel(worktree) }}>{worktreeLabel(worktree)}</span>
                        {#if worktree.main}<em>main</em>{/if}
                        {#if worktree.locked}<em title={worktree.lockReason ?? "locked"}>locked</em>{/if}
                        {#if worktree.prunable}<em>stale</em>{/if}
                      </span>
                      <small class="wt-path">{shortWorktreePath(worktree.path)}</small>
                    </span>
                  </button>
                  {#if !worktree.main && !worktree.current}
                    <button
                      class="row-action danger"
                      title={`Remove worktree ${worktree.path}`}
                      on:click={() => removeWorktree(worktree)}
                      disabled={busy}
                    >×</button>
                  {/if}
                </div>
              {:else}
                <p class="empty">No worktrees</p>
              {/each}
            </div>
          {/if}
        </section>
        {#if lastOpenNavIndex === 2}<div class="nav-spacer"></div>{/if}

        <section class="nav-section">
          <button class="nav-row" on:click={() => (stashesOpen = !stashesOpen)}>
            <span class="nav-chevron">{stashesOpen ? "⌄" : "›"}</span>
            <i class="nav-icon">▤</i>
            <span class="nav-label">Stashes</span>
            <strong>{state?.stashes.length ?? 0}</strong>
          </button>
          {#if stashesOpen}
            <div class="branch-list">
              {#each state?.stashes ?? [] as stash}
                <div class="stash-row" role="listitem" on:contextmenu={(event) => stashMenu(event, stash)}>
                  <button class="stash-info" title={`Show ${stash.name}: ${stash.message}`} on:click={() => openStash(stash)}>
                    <span>{stash.name}</span>
                    <small use:hoverExpand={{ text: stash.message }}>{stash.message}</small>
                  </button>
                  <div class="stash-actions">
                    <button title="Apply stash" on:click={() => execute({ kind: "stashApply", target: stash.name }, `Apply ${stash.name}`)} disabled={busy}>Apply</button>
                    <button title="Pop stash" on:click={() => execute({ kind: "stashPop", target: stash.name }, `Pop ${stash.name}`)} disabled={busy}>Pop</button>
                    <button class="danger" title="Drop stash" on:click={() => execute({ kind: "stashDrop", target: stash.name }, `Drop ${stash.name}`)} disabled={busy}>×</button>
                  </div>
                </div>
              {:else}
                <p class="empty">No stashes</p>
              {/each}
            </div>
          {/if}
        </section>
        {#if lastOpenNavIndex === 3}<div class="nav-spacer"></div>{/if}

        <section class="nav-section">
          <button class="nav-row" on:click={() => (tagsOpen = !tagsOpen)}>
            <span class="nav-chevron">{tagsOpen ? "⌄" : "›"}</span>
            <i class="nav-icon">⌖</i>
            <span class="nav-label">Tags</span>
            <strong>{state?.tags.length ?? 0}</strong>
          </button>
          {#if tagsOpen}
            <div class="branch-list">
              {#each state?.tags ?? [] as tag}
                <div class="branch-row" role="listitem" on:contextmenu={(event) => tagMenu(event, tag)}>
                  <button
                    class="branch-name"
                    title={`Checkout ${tag} (detached)`}
                    on:click={() => execute({ kind: "checkoutCommit", target: tag }, `Checkout ${tag}`)}
                    disabled={busy}
                  >
                    <span use:hoverExpand={{ text: `⌖ ${tag}` }}>⌖ {tag}</span>
                  </button>
                  <button
                    class="row-action danger"
                    title={`Delete tag ${tag}`}
                    on:click={() => execute({ kind: "deleteTag", branch: tag }, `Delete tag ${tag}`)}
                    disabled={busy}
                  >×</button>
                </div>
              {:else}
                <p class="empty">No tags</p>
              {/each}
            </div>
          {/if}
        </section>

      </div>
    </aside>
  {/if}

  <section class="graph-area" style={`--lane-count:${graphLaneCount}`}>
    {#if busy}<div class="busy-bar" aria-hidden="true"></div>{/if}
    {#if error}
      <div class="message error" role="alert">
        <span>{error}</span>
        <button class="msg-close" title="Dismiss" on:click={() => (error = "")}>×</button>
      </div>
    {/if}
    {#if notice}
      <div class="message ok" role="status">
        <span>{notice}</span>
        <button class="msg-close" title="Dismiss" on:click={() => (notice = "")}>×</button>
      </div>
    {/if}
    {#if availableUpdate}
      <div class="message update-banner">
        <span>kGit {availableUpdate.version} is available</span>
        <div class="banner-actions">
          <button on:click={restartToUpdate} disabled={installingUpdate}>
            {installingUpdate ? "Updating…" : "Restart to update"}
          </button>
          <button on:click={() => (availableUpdate = null)} disabled={installingUpdate}>Later</button>
        </div>
      </div>
    {/if}
    {#if state?.merging || state?.rebasing}
      <div class="message conflict-banner">
        <span>
          {state.merging ? "Merge" : "Rebase"} in progress
          {#if conflicted.length}· {conflicted.length} conflicted {conflicted.length === 1 ? "file" : "files"} to resolve{/if}
        </span>
        <div class="banner-actions">
          <button
            on:click={() => execute({ kind: state?.merging ? "mergeContinue" : "rebaseContinue" }, "Continue")}
            disabled={busy || conflicted.length > 0}
          >
            Continue
          </button>
          <button
            class="danger"
            on:click={() => execute({ kind: state?.merging ? "mergeAbort" : "rebaseAbort" }, "Abort")}
            disabled={busy}
          >
            Abort
          </button>
        </div>
      </div>
    {/if}
    {#each centerMode === "launchpad" ? [] : visibleConflictWarnings as warning (warning.target)}
      <div class="message conflict-warning" role="status">
        <span>
          ⚠ {currentBranch} will conflict with <strong>{warning.target}</strong>
          <small>{warning.files.length} file{warning.files.length === 1 ? "" : "s"}: {warning.files.slice(0, 4).join(", ")}{warning.files.length > 4 ? ", …" : ""}</small>
        </span>
        <div class="banner-actions">
          <button on:click={() => execute({ kind: "merge", target: warning.target }, `Merge ${warning.target}`)} disabled={busy}>Merge now</button>
          <button on:click={() => rebaseOnto(warning.target)} disabled={busy}>Rebase now</button>
          <button title="Hide until something changes" on:click={() => (dismissedConflictKey = conflictKey)}>Dismiss</button>
        </div>
      </div>
    {/each}
    {#if searchOpen}
      <div class="message search-banner">
        <span>Search commits, files, and refs</span>
        <input aria-label="Search commits, files, and refs" bind:value={searchQuery} placeholder="Filter the graph by message, author, hash, or ref" />
      </div>
    {/if}

    {#if centerMode === "launchpad"}
      <div class="launchpad">
        <h1>Repositories</h1>
        <div class="launch-actions">
          <button on:click={switchRepository}>Open</button>
          <button on:click={() => (cloneOpen = true)}>Clone</button>
          <button on:click={openCreatePrompt}>Create</button>
        </div>
        {#if favoriteRepos.length}
          <section class="recent-repos favorites">
            <h2>Favorites</h2>
            {#each favoriteRepos as repo, index (repo.path)}
              <div class="recent-row">
                <button on:click={() => openRepositoryPath(repo.path)}>
                  <strong>{repo.name}</strong>
                  <span>{repo.path.replace(/^\/Users\/[^/]+/, "~")}</span>
                  {#if index < 9}<kbd>⌥⌘{index + 1}</kbd>{/if}
                </button>
                <button class="star on" title="Remove from favorites" on:click={() => toggleFavorite(repo.path)}>★</button>
              </div>
            {/each}
          </section>
        {/if}
        {#await import("./lib/LaunchpadHub.svelte") then module}
          <svelte:component
            this={module.default}
            workspaces={settings.workspaces}
            candidates={[...favoriteRepos, ...recentRepos.filter((repo) => !settings.favorites.includes(repo.path))]}
            onOpen={(path) => openRepositoryPath(path)}
            onSaveWorkspaces={(next) => {
              settings = { ...settings, workspaces: next };
              persistSettings();
            }}
          />
        {/await}
        <section class="recent-repos">
          <h2>Recent</h2>
          {#each recentRepos as repo}
            <div class="recent-row">
              <button on:click={() => openRepositoryPath(repo.path)}>
                <strong>{repo.name}</strong>
                <span>{repo.path.replace(/^\/Users\/[^/]+/, "~")}</span>
              </button>
              <button
                class="star"
                class:on={settings.favorites.includes(repo.path)}
                title={settings.favorites.includes(repo.path) ? "Remove from favorites" : "Add to favorites"}
                on:click={() => toggleFavorite(repo.path)}
              >{settings.favorites.includes(repo.path) ? "★" : "☆"}</button>
            </div>
          {:else}
            <p class="empty">No repositories yet. Open, clone or create one to get started.</p>
          {/each}
        </section>
      </div>
    {:else if centerMode === "prs"}
      {#await import("./lib/PullRequestsView.svelte") then module}
        <svelte:component
          this={module.default}
          {currentBranch}
          onCheckout={(pr) => void checkoutPr(pr)}
          onBranchFromIssue={(issue) => void branchFromIssue(issue)}
          onCreatePr={() => openCreatePr()}
          onLoaded={(prs) => (knownPrs = prs)}
          onClose={closeFileView}
        />
      {/await}
    {:else if centerMode === "reflog"}
      {#await import("./lib/ReflogView.svelte") then module}
        <svelte:component
          this={module.default}
          onSelect={(hash) => void showCommitByHash(hash)}
          onBranch={(hash) => createBranchAtCommit(hash)}
          onCheckout={(hash) => execute({ kind: "checkoutCommit", target: hash }, `Checkout ${hash.slice(0, 8)}`)}
          onClose={closeFileView}
        />
      {/await}
    {:else if centerMode === "compare"}
      {#await import("./lib/CompareView.svelte") then module}
        <svelte:component
          this={module.default}
          base={compareBase}
          head={compareHead}
          branches={state?.branches ?? []}
          remoteBranches={state?.remoteBranches ?? []}
          tags={state?.tags ?? []}
          onClose={closeFileView}
        />
      {/await}
    {:else if centerMode === "file" && diffContext === "commit"}
      <div class="file-diff-shell commit-context">
        <header class="file-diff-header">
          <div class="file-diff-name">
            <span>◉</span>
            <strong>{commitFilePath}</strong>
            <em class="at-commit">at {commitDetail?.shortHash ?? selectedDiff?.path ?? ""}</em>
          </div>
          <div class="file-diff-actions">
            <button title="Unified" class:active={!splitDiff} on:click={() => (splitDiff = false)}>▣</button>
            <button title="Split" class:active={splitDiff} on:click={() => (splitDiff = true)}>▥</button>
            <button title="Close commit diff" on:click={closeFileView}>×</button>
          </div>
        </header>
        {#if selectedDiff?.diff}
          <DiffTable rows={diffRows} split={splitDiff} />
        {:else}
          <p class="diff-empty main-empty">No text diff available.</p>
        {/if}
      </div>
    {:else if centerMode === "file" && selectedFile}
      <div class="file-diff-shell">
        <header class="file-diff-header">
          <div class="file-diff-name">
            <span>✎</span>
            <strong>{selectedFile.path}</strong>
          </div>
          <div class="file-diff-actions">
            <span>UTF-8</span>
            <button class="stage-file" on:click={fileStageAction}>
              {selectedFile.group === "staged" ? "Unstage File" : "Stage File"}
            </button>
            <button title="Close file diff" on:click={closeFileView}>×</button>
          </div>
        </header>

        <div class="file-diff-toolbar">
          <div class="segmented">
            <button class:active={selectedFile.group !== "staged"} on:click={() => selectedFile && openFile({ ...selectedFile, group: "unstaged" })}>
              Unstaged
            </button>
            <button class:active={selectedFile.group === "staged"} on:click={() => selectedFile && openFile({ ...selectedFile, group: "staged" })}>
              Staged
            </button>
          </div>
          <div class="segmented">
            <button class:active={fileViewMode === "diff"} on:click={() => setFileViewMode("diff")}>Diff</button>
            <button class:active={fileViewMode === "file"} on:click={() => setFileViewMode("file")}>File</button>
            <button class:active={fileViewMode === "blame"} on:click={() => setFileViewMode("blame")}>Blame</button>
            <button class:active={fileViewMode === "history"} on:click={() => setFileViewMode("history")}>History</button>
          </div>
          <div class="diff-tool-group">
            <button title="Previous hunk" on:click={() => moveHunk(-1)}>↑</button>
            <button title="Next hunk" on:click={() => moveHunk(1)}>↓</button>
            <button title="Unified" class:active={!splitDiff} on:click={() => (splitDiff = false)}>▣</button>
            <button title="Split" class:active={splitDiff} on:click={() => (splitDiff = true)}>▥</button>
          </div>
        </div>

        <div class="hunk-toolbar">
          <span class="hunk-count">{hunkRows.length ? `Hunk ${selectedHunk + 1} of ${hunkRows.length}` : "No hunks"}</span>
          <button class="danger" on:click={() => applySelectedHunk("discard")} disabled={hunkRows.length === 0}>
            Discard Hunk
          </button>
          <button
            class="stage-file"
            on:click={() => applySelectedHunk(selectedFile?.group === "staged" ? "unstage" : "stage")}
            disabled={hunkRows.length === 0}
          >
            {selectedFile.group === "staged" ? "Unstage Hunk" : "Stage Hunk"}
          </button>
          <div class="line-selection-bar">
            {#if selectedLines.size}
              <span>{selectedLines.size} line{selectedLines.size === 1 ? "" : "s"} selected</span>
              {#if selectedFile.group !== "staged"}
                <button class="danger" on:click={() => applySelectedLines("discard")} disabled={busy}>Discard Lines</button>
                <button class="stage-file" on:click={() => applySelectedLines("stage")} disabled={busy}>Stage Lines</button>
              {:else}
                <button class="stage-file" on:click={() => applySelectedLines("unstage")} disabled={busy}>Unstage Lines</button>
              {/if}
              <button on:click={() => (selectedLines = new Set())}>Clear</button>
            {:else if fileViewMode === "diff" && !splitDiff && hunkRows.length}
              <span>Click line numbers to pick lines</span>
            {/if}
          </div>
        </div>

        {#if fileViewMode === "diff" && selectedDiff?.diff}
          <DiffTable
            rows={diffRows}
            split={splitDiff}
            selectedHunkRow={hunkRows[selectedHunk]?.index ?? null}
            selectable={!splitDiff && !selectedDiff.binary}
            {selectedLines}
            onToggleLine={toggleLine}
          />
        {:else if fileViewMode === "blame"}
          <BlameView lines={blameLines} onOpenCommit={(hash) => void showCommitByHash(hash)} />
        {:else if fileViewMode === "history"}
          <HistoryView entries={historyEntries} currentPath={selectedFile.path} onOpen={(entry) => void openHistoryEntry(entry)} />
        {:else if fileViewMode !== "diff"}
          <pre class="file-text-view">{fileText || "No content available."}</pre>
        {:else}
          <p class="diff-empty main-empty">No text diff available.</p>
        {/if}
      </div>
    {:else}
      <div class="graph-head">
        <span>Branch / Tag</span>
        <span>Graph</span>
        <span>
          Commit Message
          <button class="graph-settings" title="Settings" on:click={() => (settingsOpen = true)}>⚙</button>
        </span>
      </div>
      <div class="graph-scroll" bind:this={graphScroller} bind:clientHeight={graphViewport} on:scroll={onGraphScroll}>
        {#if totalChanges > 0}
          <div class="wip-row">
            <div class="branch-cell"></div>
            <div class="wip-graph">
              <span class="wip-rail"></span>
              <svg class="wip-node" viewBox="0 0 22 22" aria-hidden="true">
                <circle cx="11" cy="11" r="9.6" pathLength="128" />
              </svg>
            </div>
            <div class="wip-summary">
              <button class="wip-message" on:click={() => selectCommit(null)}>
                <strong>// WIP</strong>
              </button>
              {#if wipModified || wipAdded}
                <span class="wip-count" title={`${totalChanges} WIP file changes`}>
                  {#if wipModified}
                    <span class="wip-pencil">✎</span>
                    <strong>{wipModified}</strong>
                  {/if}
                  {#if wipAdded}
                    <span class="wip-added">+</span>
                    <strong>{wipAdded}</strong>
                  {/if}
                </span>
              {/if}
            </div>
          </div>
        {/if}
        <div class="graph-pad" style={`height:${graphWindow.padTop}px`} aria-hidden="true"></div>
        {#each windowRows as row, windowIndex (row.commit.hash)}
          {@const rowIndex = graphWindow.start + windowIndex}
          <button
            class="commit-row"
            class:active={selectedCommit?.hash === row.commit.hash}
            class:head={row.head}
            class:date-break={rowIndex > 0 && visibleGraphRows[rowIndex - 1].dateBucket !== row.dateBucket}
            class:drop-target={dropTarget !== "" && dropTarget === rowBranch(row)}
            style={`--row-color:${row.color}`}
            title={commitRowTitle(row)}
            on:click={(event) => handleCommitRowClick(event, row, rowIndex)}
            on:contextmenu={(event) => commitMenu(event, row.commit)}
            on:dragover={(event) => { const branch = rowBranch(row); if (branch) allowRefDrop(event, branch); }}
            on:dragleave={() => (dropTarget = "")}
            on:drop={(event) => { const branch = rowBranch(row); if (branch) dropRef(event, branch, false); }}
          >
            <span
              class="branch-cell"
              class:linked={row.refs.length > 0}
              class:head={row.head}
              style={`--ref-color:${row.color}`}
              title={row.labels.join("  ")}
            >
              {#if row.refs.length}
                {@const ref = row.refs[0]}
                {@const githubRemote = ref.remotes.find((remote) => isGithubUrl(state?.remoteUrls?.[remote]))}
                <span
                  class="ref-pill"
                  class:head={ref.head}
                  style={`--ref-color:${row.color}`}
                  draggable={ref.local && !ref.tag ? "true" : "false"}
                  role="button"
                  tabindex="-1"
                  title={ref.local && !ref.tag ? `Drag ${ref.name} onto another branch to merge, rebase or push` : ref.name}
                  on:dragstart={(event) => ref.local && !ref.tag && startRefDrag(event, ref.name, false)}
                  on:dragend={endRefDrag}
                  on:contextmenu={(event) => {
                    const branch = state?.branches.find((entry) => entry.name === ref.name);
                    if (branch && ref.local && !ref.tag) branchMenu(event, branch);
                    else if (ref.tag) tagMenu(event, ref.name);
                  }}
                >
                  {#if ref.head}<i class="pill-check">✓</i>{/if}
                  <span class="pill-label" use:hoverExpand={{ text: ref.name }}>{ref.name}</span>
                  {#if ref.tag}
                    <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label="tag"><path d="M12.6 2.6A2 2 0 0 0 11.2 2H4a2 2 0 0 0-2 2v7.2a2 2 0 0 0 .6 1.4l8.7 8.7a2.4 2.4 0 0 0 3.4 0l6.6-6.6a2.4 2.4 0 0 0 0-3.4z" /><circle cx="7.5" cy="7.5" r="1" fill="currentColor" /></svg>
                  {:else if ref.local && isWorktreeBranch(ref.name)}
                    <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label="checked out in another worktree"><path d="m17 14 3 3.3a1 1 0 0 1-.7 1.7H4.7a1 1 0 0 1-.7-1.7L7 14h-.3a1 1 0 0 1-.7-1.7L9 9h-.2A1 1 0 0 1 8 7.3L12 3l4 4.3a1 1 0 0 1-.8 1.7H15l3 3.3a1 1 0 0 1-.7 1.7H17Z" /><path d="M12 22v-3" /></svg>
                  {:else if ref.local}
                    <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label="local"><rect x="4" y="5" width="16" height="11" rx="1.5" /><path d="M2 19.5h20" /></svg>
                  {/if}
                  {#if githubRemote}
                    {@const ownerAvatar = remoteAvatarFor(state?.remoteUrls?.[githubRemote], settings.showAvatars, failedAvatars)}
                    {#if ownerAvatar}
                      <img class="pill-avatar" src={ownerAvatar} alt="GitHub" loading="lazy" on:error={() => avatarFailed(ownerAvatar)} />
                    {:else}
                      <svg class="pill-icon" viewBox="0 0 16 16" fill="currentColor" aria-label="GitHub"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z" /></svg>
                    {/if}
                  {/if}
                  {#if ref.remotes.some((remote) => !isGithubUrl(state?.remoteUrls?.[remote]))}
                    <svg class="pill-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label={ref.remotes.join(", ")}><path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9Z" /></svg>
                  {/if}
                </span>
                {#if row.refs.length > 1}
                  <span class="ref-pill more-pill" style={`--ref-color:${row.color}`}>+{row.refs.length - 1}</span>
                {/if}
              {/if}
            </span>
            <span class="graph-cell" style={`--lane-count:${graphLaneCount}`}>
              <span class="graph-tint" style={`--lane:${row.lane}; --lane-color:${row.color}`}></span>
              {#if row.refs.length}
                <span class="ref-link" class:head={row.head} style={`--lane:${row.lane}; --lane-color:${row.color}`}></span>
              {/if}
              <svg class="graph-lines" aria-hidden="true">
                {#each graphPaths(row) as path}
                  <path d={path.d} stroke={path.color} />
                {/each}
              </svg>
              {#if row.commit.parents.length > 1}
                <span class="commit-dot merge" style={`--lane:${row.lane}; --lane-color:${row.color}`}></span>
              {:else}
                {@const avatar = avatarFor(row.commit.email, settings.showAvatars, failedAvatars)}
                <span class="commit-dot" style={`--lane:${row.lane}; --lane-color:${row.color}`}>
                  {authorInitials(row.commit.author)}
                  {#if avatar}<img src={avatar} alt="" loading="lazy" on:error={() => avatarFailed(avatar)} />{/if}
                </span>
              {/if}
            </span>
            <span class="commit-main">
              <strong>
                <span use:hoverExpand={{ text: row.commit.subject }}>{row.commit.subject}</span>
                {#if row.commit.bodySummary}
                  <span class="commit-summary">
                    <em use:hoverExpand={{ text: row.commit.bodySummary }}>{row.commit.bodySummary}</em>
                  </span>
                {/if}
              </strong>
              {#if rowIndex > 0 && visibleGraphRows[rowIndex - 1].dateBucket !== row.dateBucket}
                <span class="date-marker">{row.dateBucket}</span>
              {/if}
            </span>
          </button>
        {:else}
          <p class="empty centered">{searchOpen && searchQuery.trim() ? "No commits match the search" : "No commits yet"}</p>
        {/each}
        <div class="graph-pad" style={`height:${graphWindow.padBottom}px`} aria-hidden="true"></div>
        {#if loadingMore}<p class="empty centered graph-loading">Loading more history…</p>{/if}
      </div>
    {/if}
  </section>

  {#if centerMode !== "launchpad"}
    <aside class="right-panel">
      {#if selectedCommit}
        <div class="commit-detail">
          <div class="commit-detail-head">
            {#if isStash(selectedCommit.hash)}
              {@const stashName = selectedCommit.hash}
              <span class="commit-hash-btn">stash: <strong>{stashName}</strong></span>
              <span class="stash-detail-actions">
                <button on:click={() => execute({ kind: "stashApply", target: stashName }, `Apply ${stashName}`)} disabled={busy}>Apply</button>
                <button on:click={() => execute({ kind: "stashPop", target: stashName }, `Pop ${stashName}`)} disabled={busy}>Pop</button>
                <button class="danger" on:click={() => execute({ kind: "stashDrop", target: stashName }, `Drop ${stashName}`)} disabled={busy}>Drop</button>
              </span>
            {:else}
              <button class="commit-hash-btn" title="Copy hash" on:click={() => selectedCommit && copyHash(selectedCommit.hash)}>
                commit: <strong>{selectedCommit.hash.slice(0, 6)}</strong>
              </button>
            {/if}
          </div>
          {#if commitDetailBusy}
            <p class="empty centered">Loading commit…</p>
          {:else if commitDetail}
            <div class="commit-detail-scroll">
              <div class="commit-message-card">
                <h2>{commitDetail.subject}</h2>
                {#if commitDetail.body}
                  <div class="commit-body-scroll">
                    <p class="commit-body">{reflowCommitBody(commitDetail.body)}</p>
                  </div>
                {/if}
              </div>
              <div class="split-handle"></div>

              <div class="commit-author-row">
                <span class="author-badge">
                  {authorInitials(commitDetail.author)}
                  {#if detailAvatar}<img src={detailAvatar} alt="" on:error={() => detailAvatar && avatarFailed(detailAvatar)} />{/if}
                </span>
                <div class="commit-author-main">
                  <strong>{commitDetail.author}</strong>
                  <small>authored {commitDetail.date}</small>
                  {#if signature}
                    <span
                      class="signature-badge"
                      class:verified={signature.status === "G"}
                      class:bad={signature.status === "B" || signature.status === "R"}
                      title={`${signatureLabels[signature.status] ?? "Signed"}${signature.signer ? ` by ${signature.signer}` : ""}${signature.key ? ` (key ${signature.key})` : ""}`}
                    >{signature.status === "G" ? "✓ Verified" : signature.status === "B" ? "✗ Bad signature" : "Signed"}</span>
                  {/if}
                </div>
                {#if commitDetail.parents.length}
                  <div class="commit-parent">
                    <span>{commitDetail.parents.length === 1 ? "parent" : "parents"}:</span>
                    <strong>{commitDetail.parents.map((parent) => parent.slice(0, 6)).join(", ")}</strong>
                  </div>
                {/if}
              </div>

              {#if explanation && explanation.hash === commitDetail.hash}
                <div class="ai-explanation">
                  <strong>✦ AI explanation</strong>
                  <p>{explanation.value.summary}</p>
                  {#if explanation.value.details.length}<ul>{#each explanation.value.details as line}<li>{line}</li>{/each}</ul>{/if}
                  {#if explanation.value.risks.length}<ul>{#each explanation.value.risks as line}<li class="risk">⚠ {line}</li>{/each}</ul>{/if}
                </div>
              {:else if aiWorking === "explain"}
                <p class="hint">✦ Explaining…</p>
              {/if}
              {#if coAuthors.length}
                <div class="commit-coauthors">
                  <span>Co-authors:</span>
                  {#each coAuthors as person (person.email)}
                    {@const url = avatarFor(person.email, settings.showAvatars, failedAvatars)}
                    <span class="author-badge coauthor-badge" title={`${person.name} <${person.email}>`}>
                      {authorInitials(person.name)}
                      {#if url}<img src={url} alt="" on:error={() => avatarFailed(url)} />{/if}
                    </span>
                  {/each}
                </div>
              {/if}

              {#if commitFileSummaryParts.length}
                <div class="commit-change-summary">{#each commitFileSummaryParts as part, index (part.key)}{#if index > 0}<span class="summary-sep">,</span>{/if}<span class={part.cls}>{part.glyph}</span> {part.count} {part.key}{/each}</div>
              {/if}

              <div class="changes-tools">
                <button class="sort-btn" title={sortAsc ? "Sorted A to Z" : "Sorted Z to A"} on:click={() => (sortAsc = !sortAsc)}>⇅<i>{sortAsc ? "AZ" : "ZA"}</i></button>
                <div class="segmented">
                  <button class:active={rightTab === "path"} on:click={() => (rightTab = "path")}>☰ Path</button>
                  <button class:active={rightTab === "tree"} on:click={() => (rightTab = "tree")}>⌘ Tree</button>
                </div>
                <label class="view-all-files">
                  <input type="checkbox" bind:checked={viewAllFiles} />
                  View all files
                </label>
              </div>

              <div class="commit-files">
                {#if rightTab === "tree"}
                  {#each commitFileTree as row (row.key)}
                    {#if row.kind === "dir"}
                      <button class="tree-dir" style={`--depth:${row.depth}`} on:click={() => (collapsedCommitDirs = toggleRefDir(collapsedCommitDirs, row.key))}>
                        <span>{collapsedCommitDirs.has(row.key) ? "›" : "⌄"}</span>
                        <strong>{row.label}</strong>
                      </button>
                    {:else}
                      <button
                        class="commit-file-row tree-row"
                        class:active={commitFilePath === row.item.path && diffContext === "commit"}
                        class:unchanged={!row.item.status}
                        style={`--depth:${row.depth}`}
                        title={row.item.status ? `${statusLabel(row.item.status)}: ${row.item.path}` : row.item.path}
                        disabled={!row.item.status}
                        on:click={() => openCommitFile(row.item)}
                      >
                        <span class={commitStatusGlyph(row.item.status).cls}>{commitStatusGlyph(row.item.status).glyph}</span>
                        <strong>{row.label}</strong>
                      </button>
                    {/if}
                  {:else}
                    <p class="empty">No file changes recorded</p>
                  {/each}
                {:else}
                  {#each sortedCommitFiles as change (change.path)}
                    {@const parts = splitPath(change.path)}
                    <button
                      class="commit-file-row"
                      class:active={commitFilePath === change.path && diffContext === "commit"}
                      class:unchanged={!change.status}
                      title={change.status ? `${statusLabel(change.status)}: ${change.path}` : change.path}
                      disabled={!change.status}
                      on:click={() => openCommitFile(change)}
                    >
                      <span class={commitStatusGlyph(change.status).cls}>{commitStatusGlyph(change.status).glyph}</span>
                      <strong>{#if parts.dir}<em class="path-dir">{parts.dir}</em>{/if}{parts.name}</strong>
                    </button>
                  {:else}
                    <p class="empty">No file changes recorded</p>
                  {/each}
                {/if}
              </div>

              <details class="commit-actions-disclosure">
                <summary>
                  <span class="options-toggle"><i>›</i> Commit actions</span>
                </summary>
                <div class="commit-actions">
                  <button on:click={() => commitDetail && execute({ kind: "checkoutCommit", target: commitDetail.hash }, `Checkout ${commitDetail.shortHash}`)} disabled={busy}>Checkout</button>
                  <button on:click={() => commitDetail && createBranchAtCommit(commitDetail.hash)} disabled={busy}>Branch</button>
                  <button on:click={() => commitDetail && createTagPrompt(commitDetail.hash)} disabled={busy}>Tag</button>
                  <button on:click={() => commitDetail && execute({ kind: "cherryPick", target: commitDetail.hash }, `Cherry-pick ${commitDetail.shortHash}`)} disabled={busy}>Cherry-pick</button>
                  <button on:click={() => commitDetail && execute({ kind: "revert", target: commitDetail.hash }, `Revert ${commitDetail.shortHash}`)} disabled={busy}>Revert</button>
                  <button on:click={() => commitDetail && openRebasePanel("plain", commitDetail.hash)} disabled={busy}>Rebase onto this</button>
                </div>
                <div class="reset-mode-row">
                  <label for="detail-reset-mode">reset mode</label>
                  <select id="detail-reset-mode" bind:value={resetMode}>
                    <option value="soft">soft</option>
                    <option value="mixed">mixed</option>
                    <option value="hard">hard</option>
                  </select>
                  <button class="danger" on:click={() => commitDetail && execute({ kind: "reset", target: commitDetail.hash, mode: resetMode }, `Reset to ${commitDetail.shortHash}`)} disabled={busy}>
                    Reset here
                  </button>
                </div>
              </details>
            </div>
          {:else}
            <p class="empty centered">Unable to load commit detail.</p>
          {/if}
        </div>
      {:else}
        <div class="changes-title">
          <button
            class="trash danger"
            title="Discard selected"
            on:click={() => selectedFile && execute({ kind: selectedFile.group === "untracked" ? "cleanUntracked" : "discard", path: selectedFile.path }, "Discard selected file")}
            disabled={!selectedFile}
          >⌫</button>
          <strong><span class="changes-title-label">{totalChanges} file changes on</span> <span class="changes-title-branch">{currentBranch}</span></strong>
          <button class="refresh-btn" title="Refresh" on:click={() => refresh()} disabled={busy}>↻</button>
        </div>
        <div class="changes-tools">
          <button class="sort-btn" title={sortAsc ? "Sorted A to Z" : "Sorted Z to A"} on:click={() => (sortAsc = !sortAsc)}>⇅<i>{sortAsc ? "AZ" : "ZA"}</i></button>
          <div class="segmented">
            <button class:active={rightTab === "path"} on:click={() => (rightTab = "path")}>☰ Path</button>
            <button class:active={rightTab === "tree"} on:click={() => (rightTab = "tree")}>⌘ Tree</button>
          </div>
        </div>

        <div class="change-list">
          <div class="change-group-head">
            <button class="section-toggle" on:click={() => (unstagedOpen = !unstagedOpen)}>
              <span class="chevron">{unstagedOpen ? "▾" : "▸"}</span>
              <span>Unstaged Files ({unstaged.length + untracked.length + conflicted.length})</span>
            </button>
            <button on:click={() => execute({ kind: "stage", path: "." }, "Stage all changes")} disabled={busy || totalChanges === staged.length}>
              Stage All Changes
            </button>
          </div>
          {#if unstagedOpen}
            <FileGroup title="Conflicted" files={visibleUnstaged.filter((file) => file.group === "conflicted")} open={openConflict} action={fileAction} menu={fileMenu} tree={rightTab === "tree"} hideWhenEmpty />
            <FileGroup title="Unstaged" files={visibleUnstaged.filter((file) => file.group === "unstaged")} open={openFile} action={fileAction} menu={fileMenu} selectedPath={selectedFile?.path} tree={rightTab === "tree"} hideWhenEmpty={untracked.length + conflicted.length > 0} />
            <FileGroup title="Untracked" files={visibleUnstaged.filter((file) => file.group === "untracked")} open={openFile} action={fileAction} menu={fileMenu} selectedPath={selectedFile?.path} tree={rightTab === "tree"} hideWhenEmpty />
          {/if}
          <div class="change-group-head compact">
            <button class="section-toggle" on:click={() => (stagedOpen = !stagedOpen)}>
              <span class="chevron">{stagedOpen ? "▾" : "▸"}</span>
              <span>Staged Files ({staged.length})</span>
            </button>
            <button on:click={() => execute({ kind: "unstage", path: "." }, "Unstage all changes")} disabled={busy || staged.length === 0}>
              Unstage All Changes
            </button>
          </div>
          {#if stagedOpen}
            <FileGroup title="Staged" files={visibleStaged} open={openFile} action={fileAction} menu={fileMenu} selectedPath={selectedFile?.path} tree={rightTab === "tree"} />
          {/if}
        </div>

        <div class="commit-panel">
          <div class="split-handle"></div>
          <div class="commit-tabs">
            <button class="commit-tab">-o- Commit</button>
          </div>
          <label class="checkbox">
            <input
              type="checkbox"
              checked={amendCommit}
              disabled={busy || !state || state.head === "unborn"}
              on:change={(event) => setAmend(event.currentTarget.checked)}
            />
            Amend previous commit{amendLoading ? "…" : ""}
          </label>
          {#if settings.aiEnabled}
            <button class="ai-button" title="Write the message from the staged diff with Claude" on:click={writeCommitMessageWithAi} disabled={busy || !!aiWorking || staged.length === 0}>
              {aiWorking === "commit" ? "Writing…" : "✦ Write message with AI"}
            </button>
          {/if}
          <div class="commit-box">
            <label class="commit-input" for="commit-message">
              <input
                id="commit-message"
                bind:value={commitMessage}
                maxlength="72"
                placeholder="Commit summary"
                {...plainText}
                on:keydown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && commitMessage.trim()) {
                    event.preventDefault();
                    void commitChanges();
                  }
                }}
              />
              <small>{72 - commitMessage.length}</small>
            </label>
            <textarea
              class="description"
              bind:value={commitDescription}
              placeholder="Description"
              {...plainText}
              on:keydown={(event) => {
                if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && commitMessage.trim()) {
                  event.preventDefault();
                  void commitChanges();
                }
              }}
            ></textarea>
          </div>
          <details>
            <summary>
              <span class="options-toggle"><i>›</i> Commit options</span>
            </summary>
            <div class="field">
              <label for="branch-name">Branch</label>
              <input id="branch-name" bind:value={branchName} placeholder="feature/name" />
            </div>
            <button on:click={() => execute({ kind: "createBranch", branch: branchName }, "Create branch")} disabled={busy || !branchName.trim()}>
              Create Branch
            </button>
            <div class="field">
              <label for="command-target">Target</label>
              <input id="command-target" bind:value={commandTarget} placeholder="branch, commit, stash@{0}" />
            </div>
            <select bind:value={resetMode} aria-label="Reset mode">
              <option value="soft">soft</option>
              <option value="mixed">mixed</option>
              <option value="hard">hard</option>
            </select>
            <div class="mini-actions">
              <button on:click={() => execute({ kind: "merge", target: commandTarget }, "Merge")} disabled={busy || !commandTarget.trim()}>Merge</button>
              <button on:click={() => execute({ kind: "cherryPick", target: commandTarget }, "Cherry-pick")} disabled={busy || !commandTarget.trim()}>
                Cherry-pick
              </button>
              <button class="danger" on:click={() => execute({ kind: "reset", target: commandTarget, mode: resetMode }, "Reset")} disabled={busy || !commandTarget.trim()}>
                Reset
              </button>
            </div>
          </details>
          <button
            class="commit-button"
            on:click={commitChanges}
            disabled={busy || amendLoading || !commitMessage.trim() || (!amendCommit && staged.length === 0)}
          >
            {commitMessage.trim() ? (amendCommit ? "Amend Previous Commit" : "Commit Staged Changes") : "-o- Type a Message to Commit"}
          </button>
        </div>
      {/if}
    </aside>
  {/if}
  {#if rebaseOpen && state}
    {#await import("./lib/RebasePanel.svelte") then module}
      <svelte:component
        this={module.default}
        currentBranch={currentBranch}
        branches={state.branches}
        mode={rebaseMode}
        initialBase={rebaseBase}
        confirmRisky={settings.confirmRisky}
        onClose={() => (rebaseOpen = false)}
        onDone={async (result, label) => {
          rebaseOpen = false;
          if (result.ok) notice = label;
          if (result.refresh) await refresh();
        }}
      />
    {/await}
  {/if}

  {#if conflict}
    <section class="merge-editor">
      <div class="panel-title">
        <h1>Merge Editor: {conflict.path}</h1>
        <div class="button-row">
          <button on:click={() => useVersion(conflict?.ours)}>Use Ours</button>
          <button on:click={() => useVersion(conflict?.theirs)}>Use Theirs</button>
          <button on:click={() => useVersion(conflict?.base)}>Use Base</button>
          {#if settings.aiEnabled}
            <button class="ai-button" on:click={resolveConflictsWithAi} disabled={busy || !!aiWorking}>{aiWorking === "conflicts" ? "Resolving…" : "✦ Auto-resolve with AI"}</button>
          {/if}
          <button title="Open in the configured git mergetool" on:click={() => conflict && externalTool("merge", conflict.path)}>External Tool</button>
          <button on:click={saveResolution} disabled={busy}>Save Resolution</button>
          <button class="stage-file" on:click={saveAndMarkResolved} disabled={busy}>Save &amp; Mark Resolved</button>
          <button title="Close merge editor" on:click={() => (conflict = null)}>×</button>
        </div>
      </div>
      <ConflictRegions segments={conflictSegments} choices={conflictChoices} onChoose={chooseConflict} />
      <div class="merge-grid">
        <ReadonlyPane title="Base" content={conflict.base ?? ""} />
        <ReadonlyPane title="Ours" content={conflict.ours ?? ""} />
        <ReadonlyPane title="Theirs" content={conflict.theirs ?? ""} />
        <div class="merge-pane resolved">
          <h2>Resolved</h2>
          <textarea bind:value={resolvedContent} spellcheck="false"></textarea>
        </div>
      </div>
    </section>
  {/if}
  {#if cloneOpen}
    {#await import("./lib/CloneDialog.svelte") then module}
      <svelte:component
        this={module.default}
        clonePath={settings.clonePath}
        onClose={() => (cloneOpen = false)}
        onCloned={afterClone}
        showAvatars={settings.showAvatars}
        onOpenExisting={async (path) => {
          cloneOpen = false;
          await openRepositoryPath(path);
        }}
      />
    {/await}
  {/if}

  {#if settingsOpen}
    <div
      class="modal-backdrop"
      role="presentation"
      on:click={(event) => event.target === event.currentTarget && (settingsOpen = false)}
    >
      <div class="modal" role="dialog" aria-label="Settings" tabindex="-1">
        <div class="panel-title">
          <h1>Settings</h1>
          <button title="Close settings" on:click={() => (settingsOpen = false)}>×</button>
        </div>
        <div class="modal-body">
          <label class="checkbox">
            <input type="checkbox" bind:checked={settings.confirmRisky} />
            Confirm destructive actions (discard, reset, force push, drop stash, remove worktree, delete branches)
          </label>
          <div class="field">
            <label for="settings-clone-path">Default clone / create directory</label>
            <input id="settings-clone-path" bind:value={settings.clonePath} placeholder="~/dev" />
          </div>
          <div class="field">
            <label for="settings-graph-limit">Commits per graph page (25–5000; more load as you scroll)</label>
            <input id="settings-graph-limit" type="number" min="25" max="5000" bind:value={settings.graphLimit} />
          </div>
          <div class="field">
            <label for="settings-auto-fetch">Auto-fetch every N minutes (0 = off)</label>
            <input id="settings-auto-fetch" type="number" min="0" max="1440" bind:value={settings.autoFetchMinutes} />
          </div>
          <label class="checkbox">
            <input type="checkbox" bind:checked={settings.detectConflicts} />
            Warn when the current branch would conflict with its merge target (checked with git merge-tree; nothing is changed)
          </label>
          <label class="checkbox">
            <input type="checkbox" bind:checked={settings.refreshOnFocus} />
            Refresh when kGit regains focus (changes on disk are picked up live either way)
          </label>
          <div class="field">
            <label for="settings-stale-days">Branch cleanup: stale after (1–3650 days)</label>
            <input id="settings-stale-days" type="number" min="1" max="3650" bind:value={settings.staleDays} />
          </div>
          <div class="field">
            <label for="settings-editor">External editor (application name)</label>
            <input id="settings-editor" bind:value={settings.editorApp} placeholder="e.g. Visual Studio Code, Cursor, Zed" />
          </div>
          <div class="field">
            <label for="settings-terminal">External terminal (application name)</label>
            <input id="settings-terminal" bind:value={settings.terminalApp} placeholder="Terminal, iTerm, Warp, Ghostty" />
          </div>
          <div class="field">
            <span class="field-label">Author profiles (apply one to a repository from Repository Settings › Identity)</span>
            {#each settings.profiles as profile, index (profile.id)}
              <div class="profile-row">
                <input bind:value={profile.label} placeholder="Label (Work, Personal)" aria-label="Profile label" />
                <input bind:value={profile.name} placeholder="Name" aria-label="Author name" />
                <input bind:value={profile.email} placeholder="Email" aria-label="Author email" />
                <input bind:value={profile.signingKey} placeholder="Signing key (optional)" aria-label="Signing key" />
                <label class="checkbox"><input type="checkbox" bind:checked={profile.signCommits} /> Sign</label>
                <button class="danger" title="Delete profile" on:click={() => (settings.profiles = settings.profiles.filter((_, i) => i !== index))}>×</button>
              </div>
            {/each}
            <button
              on:click={() =>
                (settings.profiles = [
                  ...settings.profiles,
                  { id: crypto.randomUUID(), label: "Profile", name: state?.userName ?? "", email: state?.userEmail ?? "", signingKey: "", signingFormat: "", signCommits: false },
                ])}
            >Add Profile</button>
          </div>
          <div class="field ai-settings">
            <span class="field-label">AI (Claude)</span>
            <label class="checkbox">
              <input type="checkbox" bind:checked={settings.aiEnabled} />
              Turn on AI features: commit messages, commit explanations, conflict auto-resolve. Diffs and file contents you run them on are sent to the Claude API.
            </label>
            {#if settings.aiEnabled}
              <p class="hint">
                {#if aiInfo?.hasKey}API key: saved ({aiInfo.source === "environment" ? "from ANTHROPIC_API_KEY" : "in the keychain"}).{:else}No API key yet.{/if}
              </p>
              <div class="inline-form">
                <input type="password" class="grow" placeholder="sk-ant-… (stored in the macOS keychain)" bind:value={aiKeyDraft} aria-label="Claude API key" autocomplete="off" />
                <button on:click={saveAiKey} disabled={!aiKeyDraft.trim()}>Save Key</button>
                {#if aiInfo?.source === "keychain"}<button class="danger" on:click={() => { aiKeyDraft = ""; void saveAiKey(); }}>Remove</button>{/if}
              </div>
              <label for="settings-ai-model">Model</label>
              <input id="settings-ai-model" bind:value={settings.aiModel} placeholder="claude-opus-5-5" />
            {/if}
          </div>
          <label class="checkbox">
            <input type="checkbox" bind:checked={settings.showAvatars} />
            Show author avatars (loaded from GitHub and Gravatar)
          </label>
        </div>
        <div class="modal-footer">
          <button on:click={() => (settingsOpen = false)}>Cancel</button>
          <button class="stage-file" on:click={saveSettings}>Save Settings</button>
        </div>
      </div>
    </div>
  {/if}
  {#if cleanupOpen && state}
    {#await import("./lib/CleanupPanel.svelte") then module}
      <svelte:component
        this={module.default}
        {state}
        staleDays={settings.staleDays}
        confirmRisky={settings.confirmRisky}
        onClose={() => (cleanupOpen = false)}
        onDone={async (s) => {
          notice = s;
          await refresh();
        }}
      />
    {/await}
  {/if}

  {#if createPrFor}
    {#await import("./lib/CreatePrDialog.svelte") then module}
      <svelte:component
        this={module.default}
        head={createPrFor.head}
        base={createPrFor.base}
        baseOptions={prBaseOptions().filter((name) => name !== createPrFor?.head)}
        ensurePushed={() => ensurePushed(createPrFor?.head ?? "")}
        onClose={() => (createPrFor = null)}
        onCreated={(url) => {
          createPrFor = null;
          notice = `Opened ${url}`;
          knownPrs = [];
        }}
      />
    {/await}
  {/if}
  {#if paletteMode === "commands"}
    <CommandPalette items={paletteCommands} placeholder="Run a command, check out a branch, open a repository…" onClose={() => (paletteMode = null)} />
  {:else if paletteMode === "files"}
    <CommandPalette items={fileItems} placeholder={paletteFiles.length ? `Find a file (${paletteFiles.length}) to see its history` : "Loading files…"} onClose={() => (paletteMode = null)} />
  {/if}
  {#if menuState}
    <ContextMenu x={menuState.x} y={menuState.y} title={menuState.title} items={menuState.items} onClose={() => (menuState = null)} />
  {/if}
  {#if shortcutsOpen}
    <ShortcutsDialog {shortcuts} onClose={() => (shortcutsOpen = false)} />
  {/if}
  {#if repoSettingsTab}
    {#await import("./lib/RepoSettings.svelte") then module}
      <svelte:component
        this={module.default}
        initialTab={repoSettingsTab}
        profiles={settings.profiles}
        onRun={(action, label) => execute(action, label)}
        onClose={() => (repoSettingsTab = null)}
      />
    {/await}
  {/if}

  <footer class="status-bar">
    <span>⌁ {repoName}</span>
    <span>
      {busy
        ? "Running git command..."
        : state?.merging
          ? "Merge in progress"
          : state?.rebasing
            ? "Rebase in progress"
            : "Ready"}
    </span>
    <span title={lastFetchAt ? `Last auto-fetch ${new Date(lastFetchAt).toLocaleTimeString()}` : "Auto-fetch has not run yet"}>
      {commits.length}{graphExhausted ? "" : "+"} commits
    </span>
    <span>{currentBranch}</span>
  </footer>
</main>

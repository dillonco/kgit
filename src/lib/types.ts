export type FileGroup = "staged" | "unstaged" | "untracked" | "conflicted";

export interface FileStatus {
  path: string;
  index: string;
  worktree: string;
  group: FileGroup;
}

export interface Branch {
  name: string;
  current: boolean;
  upstream?: string | null;
  upstreamGone: boolean;
  ahead: number;
  behind: number;
  lastCommitUnix: number;
  lastCommitRelative: string;
}

export interface StashEntry {
  name: string;
  message: string;
}

export interface Worktree {
  path: string;
  head: string;
  branch?: string | null;
  detached: boolean;
  bare: boolean;
  current: boolean;
  main: boolean;
  locked: boolean;
  lockReason?: string | null;
  prunable: boolean;
}

export interface RepositoryState {
  root: string;
  /** Absolute git dir; in a linked worktree this is not `<root>/.git`. */
  gitDir: string;
  currentBranch?: string | null;
  head: string;
  merging: boolean;
  rebasing: boolean;
  cherryPicking: boolean;
  reverting: boolean;
  files: FileStatus[];
  branches: Branch[];
  remotes: string[];
  // Fetch URL per remote name.
  remoteUrls: Record<string, string>;
  remoteBranches: string[];
  tags: string[];
  worktrees: Worktree[];
  stashes: StashEntry[];
  userName?: string | null;
  userEmail?: string | null;
}

/** The fast-changing slice of RepositoryState, refreshed on its own. */
export interface WorktreeStatus {
  currentBranch?: string | null;
  head: string;
  merging: boolean;
  rebasing: boolean;
  cherryPicking: boolean;
  reverting: boolean;
  files: FileStatus[];
}

export interface CommitNode {
  hash: string;
  shortHash: string;
  parents: string[];
  refs: string[];
  author: string;
  email: string;
  // Committer date, unix seconds: what --date-order sorts by.
  timestamp: number;
  relativeDate: string;
  subject: string;
  bodySummary: string;
}

export interface CommitGraph {
  commits: CommitNode[];
}

export interface CommitFileChange {
  status: string;
  path: string;
}

export interface CommitDetail {
  hash: string;
  shortHash: string;
  parents: string[];
  refs: string[];
  author: string;
  email: string;
  date: string;
  relativeDate: string;
  subject: string;
  body: string;
  files: CommitFileChange[];
}

export interface GitAction {
  kind: string;
  path?: string | null;
  message?: string | null;
  branch?: string | null;
  target?: string | null;
  remote?: string | null;
  mode?: string | null;
}

export interface GitResult {
  ok: boolean;
  stdout: string;
  stderr: string;
  code: number;
  refresh: boolean;
}

export interface ConflictFile {
  path: string;
  base?: string | null;
  ours?: string | null;
  theirs?: string | null;
  working: string;
}

export interface FileDiff {
  path: string;
  staged: boolean;
  diff: string;
  binary: boolean;
}

export interface BranchAudit {
  name: string;
  current: boolean;
  isBase: boolean;
  head: string;
  shortHead: string;
  upstream?: string | null;
  upstreamGone: boolean;
  ahead: number;
  behind: number;
  aheadOfBase: number;
  behindBase: number;
  merged: boolean;
  squashMerged: boolean;
  stale: boolean;
  lastCommitUnix: number;
  lastCommitRelative: string;
  worktreePath?: string | null;
  classification: string; // "current" | "base" | "merged" | "squashMerged" | "gone" | "stale" | "active"
}

export interface BranchCleanupReport {
  base: string;
  staleDays: number;
  branches: BranchAudit[];
}


export interface RefCompare {
  base: string;
  head: string;
  mergeBase?: string | null;
  threeDot: boolean;
  ahead: number;
  behind: number;
  files: CommitFileChange[];
  commits: CommitNode[];
  commitsTruncated: boolean;
}


export interface GhStatus {
  installed: boolean;
  authenticated: boolean;
  login?: string | null;
  host: string;
  protocol: string; // "https" | "ssh"
  message?: string | null;
}

export interface GhRepo {
  name: string;
  nameWithOwner: string;
  owner: string;
  ownerIsOrg?: boolean;
  description?: string | null;
  isPrivate: boolean;
  isFork: boolean;
  isArchived: boolean;
  pushedAt?: string | null;
  url: string;
  sshUrl: string;
  language?: string | null;
  defaultBranch?: string | null;
  /** `owner/name` of the repository this fork came from. */
  parent?: string | null;
}

export interface CloneRequest {
  url: string;
  path: string;
  branch?: string | null;
  depth?: number | null;
  recurseSubmodules?: boolean;
  upstreamUrl?: string | null;
}

export interface CloneProgress {
  phase: string;
  percent?: number | null;
  line: string;
}

export interface RemoteBranches {
  defaultBranch?: string | null;
  branches: string[];
}

export interface CloneTarget {
  path: string;
  exists: boolean;
  isDir: boolean;
  isEmpty: boolean;
  isRepo: boolean;
  originUrl?: string | null;
}


export interface RebaseStep {
  action: string;
  hash: string;
  message?: string | null;
}

export interface RebasePlan {
  base: string;
  mergeBase?: string | null;
  commits: CommitNode[];
  clean: boolean;
  inProgress: boolean;
  currentBranch?: string | null;
  upstream?: string | null;
}

export interface ConflictCheck {
  target: string;
  supported: boolean;
  upToDate: boolean;
  conflicts: boolean;
  files: string[];
}

export interface ReflogEntry {
  hash: string;
  shortHash: string;
  selector: string;
  action: string;
  message: string;
  relativeDate: string;
}

export interface FileHistoryEntry {
  hash: string;
  shortHash: string;
  author: string;
  email: string;
  relativeDate: string;
  subject: string;
  /** The file's path in that commit (differs from today's after a rename). */
  path: string;
  status: string;
}

export interface BlameLine {
  hash: string;
  shortHash: string;
  author: string;
  summary: string;
  time: number;
  line: number;
  text: string;
}

export interface Submodule {
  path: string;
  commit: string;
  /** "current" | "uninitialized" | "modified" | "conflict" */
  status: string;
  describe?: string | null;
}

export interface RemoteInfo {
  name: string;
  fetchUrl: string;
  pushUrl: string;
}

export interface RepoConfig {
  remotes: RemoteInfo[];
  submodules: Submodule[];
  lfsAvailable: boolean;
  lfsInitialized: boolean;
  lfsPatterns: string[];
  sparseEnabled: boolean;
  sparseCone: boolean;
  sparsePaths: string[];
  userName?: string | null;
  userEmail?: string | null;
  identityScope?: string | null;
  signingKey?: string | null;
  gpgFormat?: string | null;
  gpgProgram?: string | null;
  commitGpgsign: boolean;
  tagGpgsign: boolean;
  mergeTool?: string | null;
  diffTool?: string | null;
}

export interface Signature {
  /** git's %G?: G good, B bad, U unknown validity, X/Y expired, R revoked, E can't check, N none. */
  status: string;
  signer: string;
  key: string;
}

/** An author identity the user can apply to a repository. */
export interface Profile {
  id: string;
  label: string;
  name: string;
  email: string;
  signingKey?: string;
  signingFormat?: "" | "openpgp" | "ssh" | "x509";
  signCommits?: boolean;
}

export interface PaletteItem {
  id: string;
  title: string;
  group: string;
  subtitle?: string;
  shortcut?: string;
  run: () => unknown;
}

export interface MenuItem {
  label?: string;
  run?: () => unknown;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  hint?: string;
}

export interface PullRequest {
  number: number;
  title: string;
  url: string;
  head: string;
  base: string;
  author: string;
  draft: boolean;
  state: string;
  /** "APPROVED" | "CHANGES_REQUESTED" | "REVIEW_REQUIRED" | "" */
  review: string;
  /** "success" | "failure" | "pending" | "" */
  checks: string;
  updatedAt: string;
  repository: string;
}

export interface Issue {
  number: number;
  title: string;
  url: string;
  labels: string[];
  assignees: string[];
  updatedAt: string;
}

export interface AiStatus {
  hasKey: boolean;
  source?: "keychain" | "environment" | null;
  model: string;
}

export interface CommitExplanation {
  summary: string;
  details: string[];
  risks: string[];
}

export interface AiConflictResolution {
  index: number;
  resolution: string;
  confidence: number;
  explanation: string;
}

export interface ConflictProposal {
  summary: string;
  resolutions: AiConflictResolution[];
}

export interface RepoSummary {
  path: string;
  ok: boolean;
  error?: string | null;
  branch?: string | null;
  upstream?: string | null;
  ahead: number;
  behind: number;
  changes: number;
  conflicts: number;
}

export interface WorkspaceResult {
  path: string;
  ok: boolean;
  message: string;
}

/** A named group of repositories (kept in settings). */
export interface Workspace {
  id: string;
  name: string;
  paths: string[];
}

// Browser-demo responses for commands added after demo.ts: they return
// plausible data so the UI can be exercised without a Rust build.
import type { GitResult, RepoConfig } from "./types";

export const UNHANDLED = Symbol("unhandled");

const ok = (stdout = ""): GitResult => ({ ok: true, stdout, stderr: "", code: 0, refresh: true });
const unavailable = (what: string): GitResult => ({ ok: false, stdout: "", stderr: `${what} is unavailable in the browser demo`, code: 1, refresh: false });

const demoConfig: RepoConfig = {
  remotes: [
    { name: "origin", fetchUrl: "git@github.com:wareness/kgit.git", pushUrl: "git@github.com:wareness/kgit.git" },
    { name: "upstream", fetchUrl: "https://github.com/kgit-org/kgit.git", pushUrl: "https://github.com/kgit-org/kgit.git" },
  ],
  submodules: [{ path: "vendor/icons", commit: "4f2a9c1e", status: "current", describe: "v2.1.0" }],
  lfsAvailable: true,
  lfsInitialized: true,
  lfsPatterns: ["*.psd", "assets/video/**"],
  sparseEnabled: false,
  sparseCone: true,
  sparsePaths: [],
  userName: "Christine Ware",
  userEmail: "christine@wareness.dev",
  identityScope: "global",
  signingKey: "~/.ssh/id_ed25519.pub",
  gpgFormat: "ssh",
  gpgProgram: null,
  commitGpgsign: true,
  tagGpgsign: false,
  mergeTool: "opendiff",
  diffTool: "opendiff",
};

const demoFiles = [
  ".gitignore",
  "README.md",
  "docs/roadmap.md",
  "package.json",
  "src/App.svelte",
  "src/lib/CommitDetail.svelte",
  "src/lib/demo-notes.md",
  "src/lib/git.ts",
  "src/lib/graph.ts",
  "src/main.ts",
  "src/styles.css",
  "src-tauri/src/lib.rs",
];

export async function demoExtraInvoke(command: string, args: Record<string, unknown>): Promise<unknown> {
  switch (command) {
    case "watch_repository":
      return null;
    case "list_files":
      return demoFiles;
    case "get_repo_config":
      return structuredClone(demoConfig);
    case "set_config":
      return ok();
    case "get_commit_signature":
      // Every other demo commit is signed, so the badge shows up while browsing.
      return String(args.hash).charCodeAt(0) % 2
        ? { status: "G", signer: "Christine Ware <christine@wareness.dev>", key: "SHA256:k9demo" }
        : { status: "N", signer: "", key: "" };
    case "make_patch":
      return "diff --git a/src/App.svelte b/src/App.svelte\n--- a/src/App.svelte\n+++ b/src/App.svelte\n@@ -1 +1 @@\n-old\n+new\n";
    case "apply_patch":
      return ok("Applied patch");
    case "checkout_with_autostash":
      return ok(`Checked out ${String(args.branch)} and restored your changes`);
    case "write_text_file":
    case "read_text_file":
      throw new Error("file access is unavailable in the browser demo");
    case "open_external":
      return unavailable("Opening other apps");
    case "launch_tool":
      return unavailable("External tools");
    case "pick_save_path":
    case "pick_open_file":
      return null;
    case "gh_pr_list":
      return [
        { number: 42, title: "Commit detail panel with file diffs", url: "https://github.com/wareness/kgit/pull/42", head: "feature/commit-details", base: "main", author: "christine", draft: false, state: "OPEN", review: "REVIEW_REQUIRED", checks: "pending", updatedAt: "", repository: "" },
        { number: 39, title: "Keep graph lanes stable across merges", url: "https://github.com/wareness/kgit/pull/39", head: "fix/lanes", base: "main", author: "priya", draft: false, state: "OPEN", review: "APPROVED", checks: "success", updatedAt: "", repository: "" },
        { number: 37, title: "Hunk staging", url: "https://github.com/wareness/kgit/pull/37", head: "feature/hunk-staging", base: "main", author: "dillon", draft: true, state: "OPEN", review: "", checks: "failure", updatedAt: "", repository: "" },
      ];
    case "gh_my_prs":
      return [
        [{ number: 42, title: "Commit detail panel with file diffs", url: "https://github.com/wareness/kgit/pull/42", head: "", base: "", author: "christine", draft: false, state: "OPEN", review: "", checks: "", updatedAt: "", repository: "wareness/kgit" }],
        [{ number: 311, title: "Add retry to the sync worker", url: "https://github.com/wareness/data-layer/pull/311", head: "", base: "", author: "priya", draft: false, state: "OPEN", review: "", checks: "", updatedAt: "", repository: "wareness/data-layer" }],
      ];
    case "gh_issue_list":
      return [
        { number: 58, title: "Graph flickers when fetching", url: "https://github.com/wareness/kgit/issues/58", labels: ["bug"], assignees: ["christine"], updatedAt: "" },
        { number: 61, title: "Support worktree locking", url: "https://github.com/wareness/kgit/issues/61", labels: ["enhancement"], assignees: [], updatedAt: "" },
      ];
    case "issue_branch":
      return `${String(args.number)}-${String(args.title).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
    case "gh_pr_create":
      return "https://github.com/wareness/kgit/pull/43";
    case "gh_pr_checkout":
      return ok(`Switched to branch for #${String(args.number)}`);
    case "share_patch":
      return "https://gist.github.com/christine/0123456789abcdef0123";
    case "fetch_shared_patch":
      return "diff --git a/a b/a\n";
    case "open_url":
      return null;
    case "ai_status":
      return { hasKey: true, source: "keychain", model: "claude-opus-5-5" };
    case "ai_set_key":
      return null;
    case "ai_commit_message":
      return { subject: "feat: show commit details beside the graph", body: "Selecting a commit loads its metadata and changed files into the\nright panel, with per-file diffs one click away." };
    case "ai_explain_commit":
      return {
        summary: "Adds a stash section to the left panel so stashes can be applied, popped and dropped without leaving the graph.",
        details: ["New Stashes section lists `git stash list` entries", "Apply, pop and drop run through the action map"],
        risks: ["Dropping a stash has no confirmation when risky-action prompts are off"],
      };
    case "ai_resolve_conflicts":
      return {
        summary: "Kept both sides' additions.",
        resolutions: (args.regions as { index: number; ours: string; theirs: string }[]).map((region) => ({
          index: region.index,
          resolution: region.ours + region.theirs,
          confidence: region.index === 0 ? 0.92 : 0.41,
          explanation: "Both sides added independent lines; kept ours first, then theirs.",
        })),
      };
    case "workspace_status":
      return (args.paths as string[]).map((path, index) => ({ path, ok: true, branch: index ? "develop" : "main", upstream: "origin/main", ahead: index, behind: 2 - index, changes: index * 3, conflicts: 0 }));
    case "workspace_run":
      return (args.paths as string[]).map((path) => ({ path, ok: true, message: "" }));
    default:
      return UNHANDLED;
  }
}

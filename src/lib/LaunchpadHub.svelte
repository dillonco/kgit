<script lang="ts">
  // Launchpad sections: your pull requests across GitHub, and workspaces
  // (groups of repositories you fetch, pull and check on together).
  import { onMount } from "svelte";
  import { ghMyPrs, openUrl, workspaceRun, workspaceStatus } from "./git";
  import type { PullRequest, RepoSummary, Workspace } from "./types";

  export let workspaces: Workspace[];
  export let candidates: { name: string; path: string }[];
  export let onOpen: (path: string) => void;
  export let onSaveWorkspaces: (next: Workspace[]) => void;

  let mine: PullRequest[] = [];
  let review: PullRequest[] = [];
  let prError = "";
  let prLoading = true;
  let summaries: Record<string, RepoSummary> = {};
  let working = "";
  let results = "";
  let editing: Workspace | null = null;

  onMount(async () => {
    try {
      [mine, review] = await ghMyPrs();
    } catch (err) {
      prError = String(err);
    } finally {
      prLoading = false;
    }
    await refreshSummaries();
  });

  async function refreshSummaries() {
    const paths = [...new Set(workspaces.flatMap((workspace) => workspace.paths))];
    if (!paths.length) return;
    const list = await workspaceStatus(paths).catch(() => []);
    summaries = Object.fromEntries(list.map((entry) => [entry.path, entry]));
  }

  async function run(workspace: Workspace, kind: "fetch" | "pull") {
    working = `${workspace.id}:${kind}`;
    results = "";
    try {
      const outcome = await workspaceRun(workspace.paths, kind);
      const failed = outcome.filter((entry) => !entry.ok);
      results = failed.length
        ? `${kind === "fetch" ? "Fetch" : "Pull"} failed in ${failed.length} of ${outcome.length}: ${failed.map((entry) => `${entry.path.split("/").at(-1)} (${entry.message.split("\n")[0]})`).join("; ")}`
        : `${kind === "fetch" ? "Fetched" : "Pulled"} ${outcome.length} repositories`;
      await refreshSummaries();
    } finally {
      working = "";
    }
  }

  function startNew() {
    editing = { id: crypto.randomUUID(), name: "New workspace", paths: [] };
  }

  function saveEditing() {
    if (!editing) return;
    const next = workspaces.some((workspace) => workspace.id === editing?.id)
      ? workspaces.map((workspace) => (workspace.id === editing?.id ? editing! : workspace))
      : [...workspaces, editing];
    onSaveWorkspaces(next);
    editing = null;
    void refreshSummaries();
  }

  function togglePath(path: string) {
    if (!editing) return;
    editing = {
      ...editing,
      paths: editing.paths.includes(path) ? editing.paths.filter((entry) => entry !== path) : [...editing.paths, path],
    };
  }

  const short = (path: string) => path.split("/").filter(Boolean).at(-1) ?? path;
</script>

<section class="hub-section">
  <h2>Pull Requests</h2>
  {#if prLoading}
    <p class="empty">Loading from GitHub…</p>
  {:else if prError}
    <p class="empty" title={prError}>Pull requests need the GitHub CLI signed in (gh auth login).</p>
  {:else}
    {#each [{ label: "Waiting for your review", items: review }, { label: "Opened by you", items: mine }] as group}
      <h3>{group.label} ({group.items.length})</h3>
      {#each group.items as pr (pr.url)}
        <button class="hub-pr" on:click={() => openUrl(pr.url)}>
          <span class="history-hash">{pr.repository}#{pr.number}</span>
          <span>{pr.draft ? "[draft] " : ""}{pr.title}</span>
        </button>
      {:else}
        <p class="empty">None</p>
      {/each}
    {/each}
  {/if}
</section>

<section class="hub-section">
  <h2>Workspaces <button class="section-action" on:click={startNew}>+ New</button></h2>
  {#if results}<p class="hint">{results}</p>{/if}
  {#each workspaces as workspace (workspace.id)}
    <div class="workspace-card">
      <div class="workspace-head">
        <strong>{workspace.name}</strong>
        <button on:click={() => run(workspace, "fetch")} disabled={!!working}>{working === `${workspace.id}:fetch` ? "Fetching…" : "Fetch all"}</button>
        <button on:click={() => run(workspace, "pull")} disabled={!!working}>{working === `${workspace.id}:pull` ? "Pulling…" : "Pull all"}</button>
        <button on:click={() => (editing = { ...workspace, paths: [...workspace.paths] })}>Edit</button>
        <button class="danger" title="Delete workspace" on:click={() => onSaveWorkspaces(workspaces.filter((entry) => entry.id !== workspace.id))}>×</button>
      </div>
      {#each workspace.paths as path}
        {@const summary = summaries[path]}
        <button class="workspace-repo" on:click={() => onOpen(path)}>
          <strong>{short(path)}</strong>
          {#if summary?.ok}
            <span>{summary.branch ?? "detached"}</span>
            {#if summary.ahead}<span class="ahead">↑{summary.ahead}</span>{/if}
            {#if summary.behind}<span class="behind">↓{summary.behind}</span>{/if}
            {#if summary.changes}<span class="dirty" title="Uncommitted changes">✎{summary.changes}</span>{/if}
            {#if summary.conflicts}<span class="behind">{summary.conflicts} conflicts</span>{/if}
          {:else if summary}
            <span class="behind" title={summary.error ?? ""}>unavailable</span>
          {/if}
        </button>
      {/each}
    </div>
  {:else}
    <p class="empty">Group repositories to fetch, pull and check on them together.</p>
  {/each}
  {#if editing}
    <div class="workspace-card editing">
      <input bind:value={editing.name} aria-label="Workspace name" />
      {#each candidates as repo (repo.path)}
        <label class="checkbox"><input type="checkbox" checked={editing.paths.includes(repo.path)} on:change={() => togglePath(repo.path)} /> {repo.name} <small>{repo.path}</small></label>
      {:else}
        <p class="empty">Open some repositories first; they'll be listed here.</p>
      {/each}
      <div class="modal-actions">
        <button on:click={() => (editing = null)}>Cancel</button>
        <button class="stage-file" on:click={saveEditing} disabled={!editing.name.trim() || !editing.paths.length}>Save Workspace</button>
      </div>
    </div>
  {/if}
</section>

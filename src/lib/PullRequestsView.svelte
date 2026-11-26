<script lang="ts">
  // The open repository's pull requests and issues on GitHub (through gh).
  import { onMount } from "svelte";
  import { ghIssueList, ghPrList, openUrl } from "./git";
  import type { Issue, PullRequest } from "./types";

  export let currentBranch: string;
  export let onCheckout: (pr: PullRequest) => void;
  export let onBranchFromIssue: (issue: Issue) => void;
  export let onCreatePr: () => void;
  export let onClose: () => void;
  export let onLoaded: (prs: PullRequest[]) => void = () => {};

  let tab: "prs" | "issues" = "prs";
  let prs: PullRequest[] = [];
  let issues: Issue[] = [];
  let mineOnly = false;
  let loading = true;
  let error = "";

  async function load() {
    loading = true;
    error = "";
    try {
      if (tab === "prs") {
        prs = await ghPrList();
        onLoaded(prs);
      } else {
        issues = await ghIssueList(mineOnly);
      }
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function select(next: "prs" | "issues") {
    tab = next;
    void load();
  }

  const reviewLabel: Record<string, string> = {
    APPROVED: "approved",
    CHANGES_REQUESTED: "changes requested",
    REVIEW_REQUIRED: "review required",
  };
</script>

<div class="reflog-view hub-view">
  <header class="file-diff-header">
    <div class="file-diff-name">
      <div class="segmented">
        <button class:active={tab === "prs"} on:click={() => select("prs")}>Pull Requests</button>
        <button class:active={tab === "issues"} on:click={() => select("issues")}>Issues</button>
      </div>
    </div>
    <div class="file-diff-actions">
      {#if tab === "issues"}
        <label class="checkbox"><input type="checkbox" bind:checked={mineOnly} on:change={load} /> Assigned to me</label>
      {:else}
        <button class="stage-file" on:click={onCreatePr}>New Pull Request from {currentBranch}</button>
      {/if}
      <button title="Reload" on:click={load}>↻</button>
      <button title="Close" on:click={onClose}>×</button>
    </div>
  </header>
  {#if loading}
    <p class="empty centered">Loading from GitHub…</p>
  {:else if error}
    <p class="empty centered">{error}</p>
  {:else if tab === "prs"}
    <div class="reflog-list">
      {#each prs as pr (pr.number)}
        <div class="reflog-row">
          <button class="hub-main" on:click={() => openUrl(pr.url)} title="Open on GitHub">
            <span class="history-hash">#{pr.number}</span>
            <span class="history-subject">{pr.draft ? "[draft] " : ""}{pr.title}</span>
            <span class="hub-branch" title={`${pr.head} → ${pr.base}`}>{pr.head} → {pr.base}</span>
            <span class={`ci ci-${pr.checks || "none"}`} title={`Checks: ${pr.checks || "none"}`}>●</span>
            <span class="history-author">{pr.author}{pr.review ? ` · ${reviewLabel[pr.review] ?? pr.review.toLowerCase()}` : ""}</span>
          </button>
          <button class="row-action" title={`Check out #${pr.number} locally`} on:click={() => onCheckout(pr)}>⇥</button>
        </div>
      {:else}
        <p class="empty centered">No open pull requests.</p>
      {/each}
    </div>
  {:else}
    <div class="reflog-list">
      {#each issues as issue (issue.number)}
        <div class="reflog-row">
          <button class="hub-main" on:click={() => openUrl(issue.url)} title="Open on GitHub">
            <span class="history-hash">#{issue.number}</span>
            <span class="history-subject">{issue.title}</span>
            <span class="hub-labels">{issue.labels.join(", ")}</span>
            <span class="history-author">{issue.assignees.join(", ")}</span>
          </button>
          <button class="row-action" title="Create a branch for this issue" on:click={() => onBranchFromIssue(issue)}>⑂</button>
        </div>
      {:else}
        <p class="empty centered">No open issues{mineOnly ? " assigned to you" : ""}.</p>
      {/each}
    </div>
  {/if}
</div>

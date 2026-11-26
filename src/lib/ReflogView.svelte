<script lang="ts">
  // HEAD's reflog: every commit, checkout, reset and rebase, including
  // commits no branch points at any more, so lost work can be recovered.
  import { onMount } from "svelte";
  import { getReflog } from "./git";
  import type { ReflogEntry } from "./types";

  export let onSelect: (hash: string) => void;
  export let onBranch: (hash: string) => void;
  export let onCheckout: (hash: string) => void;
  export let onClose: () => void;

  let entries: ReflogEntry[] = [];
  let error = "";
  let loading = true;
  let filter = "";

  $: shown = entries.filter((entry) =>
    !filter.trim() || `${entry.action} ${entry.message} ${entry.hash}`.toLowerCase().includes(filter.trim().toLowerCase()),
  );

  onMount(async () => {
    try {
      entries = await getReflog(500);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  });
</script>

<div class="reflog-view">
  <header class="file-diff-header">
    <div class="file-diff-name"><span>↺</span><strong>Reflog</strong><em class="at-commit">where HEAD has been</em></div>
    <div class="file-diff-actions">
      <input class="reflog-filter" placeholder="Filter" bind:value={filter} aria-label="Filter reflog" />
      <button title="Close reflog" on:click={onClose}>×</button>
    </div>
  </header>
  {#if loading}
    <p class="empty centered">Loading reflog…</p>
  {:else if error}
    <p class="empty centered">{error}</p>
  {:else}
    <div class="reflog-list">
      {#each shown as entry (entry.selector)}
        <div class="reflog-row">
          <button class="reflog-main" on:click={() => onSelect(entry.hash)}>
            <span class="reflog-selector">{entry.selector}</span>
            <span class="reflog-action">{entry.action}</span>
            <span class="reflog-message">{entry.message}</span>
            <span class="reflog-date">{entry.relativeDate}</span>
          </button>
          <button class="row-action" title="Create a branch here (recovers lost commits)" on:click={() => onBranch(entry.hash)}>⑂</button>
          <button class="row-action" title="Check out this commit (detached)" on:click={() => onCheckout(entry.hash)}>⇥</button>
        </div>
      {:else}
        <p class="empty centered">No reflog entries.</p>
      {/each}
    </div>
  {/if}
</div>

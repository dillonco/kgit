<script lang="ts">
  import type { FileHistoryEntry } from "./types";

  export let entries: FileHistoryEntry[];
  export let currentPath: string;
  export let onOpen: (entry: FileHistoryEntry) => void;
</script>

<div class="file-history">
  {#each entries as entry (entry.hash)}
    <button class="history-row" on:click={() => onOpen(entry)} title="Show this commit's change to the file">
      <span class="history-hash">{entry.shortHash}</span>
      <span class="history-subject">{entry.subject}</span>
      {#if entry.path !== currentPath}<em class="history-path" title="The file's name in this commit">{entry.path}</em>{/if}
      <span class="history-author">{entry.author}</span>
      <span class="history-date">{entry.relativeDate}</span>
    </button>
  {:else}
    <p class="empty centered">No history (the file may be untracked).</p>
  {/each}
</div>

<script lang="ts">
  import type { BlameLine } from "./types";

  export let lines: BlameLine[];
  export let onOpenCommit: (hash: string) => void;

  // Consecutive lines from one commit share a single gutter label.
  $: rows = lines.map((line, index) => ({ line, first: index === 0 || lines[index - 1].hash !== line.hash }));
  // Older commits fade so the recent churn stands out.
  $: times = lines.map((line) => line.time).filter(Boolean);
  $: newest = Math.max(0, ...times);
  $: oldest = Math.min(newest, ...times);

  function age(time: number) {
    if (newest === oldest) return 1;
    return 0.35 + (0.65 * (time - oldest)) / (newest - oldest);
  }

  function date(time: number) {
    return time ? new Date(time * 1000).toLocaleDateString() : "";
  }
</script>

<div class="blame-view">
  {#each rows as { line, first } (line.line)}
    <div class="blame-row" class:group-start={first}>
      <button class="blame-gutter" style={`--age:${age(line.time)}`} title={`${line.shortHash} · ${line.summary}`} on:click={() => onOpenCommit(line.hash)}>
        {#if first}
          <span class="blame-author">{line.author}</span>
          <span class="blame-meta">{date(line.time)} · {line.shortHash}</span>
        {/if}
      </button>
      <span class="line-no">{line.line}</span>
      <code>{line.text}</code>
    </div>
  {:else}
    <p class="empty centered">No blame information (the file may be untracked).</p>
  {/each}
</div>

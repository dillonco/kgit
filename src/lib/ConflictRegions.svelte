<script lang="ts">
  // Per-conflict resolution: each region in the file gets its own choice
  // instead of taking one side for the whole file.
  import type { ConflictChoice, Segment } from "./conflicts";

  export let segments: Segment[];
  export let choices: Record<number, ConflictChoice | undefined>;
  export let onChoose: (index: number, choice: ConflictChoice | undefined) => void;

  $: regions = segments.filter((segment): segment is Extract<Segment, { kind: "conflict" }> => segment.kind === "conflict");
  $: resolved = regions.filter((region) => choices[region.index]).length;

  function custom(choice: ConflictChoice | undefined) {
    return typeof choice === "object" ? choice : null;
  }

  function confidenceClass(value: number | undefined) {
    if (value == null) return "";
    return value >= 0.8 ? "high" : value >= 0.5 ? "medium" : "low";
  }

  const options: { choice: ConflictChoice; label: string }[] = [
    { choice: "ours", label: "Ours" },
    { choice: "theirs", label: "Theirs" },
    { choice: "ours-theirs", label: "Both" },
    { choice: "theirs-ours", label: "Both (theirs first)" },
  ];

  function preview(text: string) {
    const lines = text.split("\n").filter(Boolean);
    return lines.length > 6 ? [...lines.slice(0, 6), `… ${lines.length - 6} more lines`].join("\n") : lines.join("\n");
  }
</script>

{#if regions.length}
  <div class="conflict-regions">
    <div class="conflict-regions-head">
      <strong>{resolved} of {regions.length} conflicts resolved</strong>
      <span>Pick a side per conflict; the result updates below.</span>
    </div>
    {#each regions as region (region.index)}
      <div class="conflict-region" class:done={choices[region.index]}>
        <span class="conflict-number">#{region.index + 1}</span>
        <pre class="conflict-side ours" title="Ours ({region.oursLabel})">{preview(region.ours) || "(empty)"}</pre>
        <pre class="conflict-side theirs" title="Theirs ({region.theirsLabel})">{preview(region.theirs) || "(empty)"}</pre>
        <div class="conflict-choices" role="group" aria-label={`Resolve conflict ${region.index + 1}`}>
          {#each options as option}
            <button class:active={choices[region.index] === option.choice} on:click={() => onChoose(region.index, option.choice)}>
              {option.label}
            </button>
          {/each}
          {#if region.base != null}
            <button class:active={choices[region.index] === "base"} on:click={() => onChoose(region.index, "base")}>Base</button>
          {/if}
          {#if choices[region.index]}
            <button class="ghost" title="Put the markers back" on:click={() => onChoose(region.index, undefined)}>Reset</button>
          {/if}
        </div>
        {#if custom(choices[region.index])}
          {@const proposal = custom(choices[region.index])}
          <div class="conflict-proposal">
            <span class={`confidence ${confidenceClass(proposal?.confidence)}`}>
              {proposal?.confidence != null ? `AI · ${Math.round(proposal.confidence * 100)}% confident` : "Edited"}
            </span>
            {#if proposal?.explanation}<span class="proposal-why">{proposal.explanation}</span>{/if}
            <pre class="conflict-side merged">{preview(proposal?.text ?? "") || "(empty)"}</pre>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

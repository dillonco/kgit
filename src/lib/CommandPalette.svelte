<script lang="ts">
  import type { PaletteItem } from "./types";
  // ⌘K command palette and ⌘P quick open share this list: type to fuzzy
  // filter, arrows to move, Enter to run, Esc to close.
  import { tick } from "svelte";
  import { fuzzyFilter } from "./fuzzy";
  import { trapFocus } from "./modal";


  export let items: PaletteItem[];
  export let placeholder = "Type a command";
  export let onClose: () => void;

  let query = "";
  let active = 0;
  let list: HTMLDivElement | null = null;

  $: shown = fuzzyFilter(items, query, (item) => `${item.group} ${item.title} ${item.subtitle ?? ""}`, 120);
  $: if (active >= shown.length) active = Math.max(0, shown.length - 1);
  $: query, (active = 0);

  async function choose(item: PaletteItem | undefined) {
    if (!item) return;
    onClose();
    await item.run();
  }

  async function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown" || (event.ctrlKey && event.key === "n")) {
      event.preventDefault();
      active = Math.min(shown.length - 1, active + 1);
    } else if (event.key === "ArrowUp" || (event.ctrlKey && event.key === "p")) {
      event.preventDefault();
      active = Math.max(0, active - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      void choose(shown[active]);
      return;
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    } else return;
    await tick();
    list?.querySelector<HTMLElement>(".palette-item.active")?.scrollIntoView({ block: "nearest" });
  }
</script>

<div class="modal-backdrop palette-backdrop" role="presentation" on:click={(event) => event.target === event.currentTarget && onClose()}>
  <div class="palette" role="dialog" aria-label={placeholder} use:trapFocus={{ initial: ".palette-input" }}>
    <input class="palette-input" {placeholder} bind:value={query} on:keydown={onKeydown} aria-label={placeholder} />
    <div class="palette-list" role="listbox" bind:this={list}>
      {#each shown as item, index (item.id)}
        {#if index === 0 || shown[index - 1].group !== item.group}
          <div class="palette-group">{item.group}</div>
        {/if}
        <button
          class="palette-item"
          class:active={index === active}
          role="option"
          aria-selected={index === active}
          on:mousemove={() => (active = index)}
          on:click={() => choose(item)}
        >
          <span class="palette-title">{item.title}</span>
          {#if item.subtitle}<span class="palette-subtitle">{item.subtitle}</span>{/if}
          {#if item.shortcut}<kbd>{item.shortcut}</kbd>{/if}
        </button>
      {:else}
        <p class="empty centered">No matches</p>
      {/each}
    </div>
  </div>
</div>

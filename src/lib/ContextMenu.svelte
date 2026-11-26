<script lang="ts">
  import type { MenuItem } from "./types";
  // Right-click menu anchored at the pointer and kept inside the window.
  import { onMount, tick } from "svelte";


  export let x: number;
  export let y: number;
  export let title = "";
  export let items: MenuItem[];
  export let onClose: () => void;

  let menu: HTMLDivElement | null = null;
  let left = x;
  let top = y;

  onMount(async () => {
    await tick();
    if (!menu) return;
    const rect = menu.getBoundingClientRect();
    left = Math.max(4, Math.min(x, window.innerWidth - rect.width - 4));
    top = Math.max(4, Math.min(y, window.innerHeight - rect.height - 4));
    menu.querySelector<HTMLElement>("button:not([disabled])")?.focus();
  });

  function choose(item: MenuItem) {
    if (item.disabled || !item.run) return;
    onClose();
    void item.run();
  }

  function onKeydown(event: KeyboardEvent) {
    const buttons = Array.from(menu?.querySelectorAll<HTMLButtonElement>("button:not([disabled])") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") onClose();
    else if (event.key === "ArrowDown") buttons[(index + 1) % buttons.length]?.focus();
    else if (event.key === "ArrowUp") buttons[(index - 1 + buttons.length) % buttons.length]?.focus();
    else return;
    event.preventDefault();
  }
</script>

<svelte:window on:blur={onClose} on:resize={onClose} />
<div class="context-backdrop" role="presentation" on:click={onClose} on:contextmenu|preventDefault={onClose}></div>
<div class="context-menu" role="menu" tabindex="-1" bind:this={menu} style={`left:${left}px; top:${top}px`} on:keydown={onKeydown}>
  {#if title}<div class="context-title">{title}</div>{/if}
  {#each items as item}
    {#if item.separator}
      <div class="context-separator" role="separator"></div>
    {:else}
      <button role="menuitem" class:danger={item.danger} disabled={item.disabled} on:click={() => choose(item)}>
        <span>{item.label}</span>
        {#if item.hint}<kbd>{item.hint}</kbd>{/if}
      </button>
    {/if}
  {/each}
</div>

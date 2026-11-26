<script lang="ts">
  import { trapFocus } from "./modal";

  export let shortcuts: { keys: string; action: string; group: string }[];
  export let onClose: () => void;

  $: groups = [...new Set(shortcuts.map((shortcut) => shortcut.group))];
</script>

<div class="modal-backdrop" role="presentation" on:click={(event) => event.target === event.currentTarget && onClose()}>
  <div class="modal shortcuts-modal" role="dialog" aria-label="Keyboard shortcuts" tabindex="-1" use:trapFocus>
    <div class="panel-title">
      <h1>Keyboard Shortcuts</h1>
      <button title="Close" on:click={onClose}>×</button>
    </div>
    <div class="modal-body shortcuts-grid">
      {#each groups as group}
        <section>
          <h2>{group}</h2>
          {#each shortcuts.filter((shortcut) => shortcut.group === group) as shortcut}
            <div class="shortcut-row"><span>{shortcut.action}</span><kbd>{shortcut.keys}</kbd></div>
          {/each}
        </section>
      {/each}
    </div>
  </div>
</div>

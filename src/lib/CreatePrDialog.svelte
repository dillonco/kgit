<script lang="ts">
  // Opens a pull request for a branch with gh. Title and body start from the
  // branch's commits; the branch is pushed first when it has no upstream.
  import { onMount } from "svelte";
  import { getRefCompare, ghPrCreate, openUrl } from "./git";
  import { trapFocus } from "./modal";

  export let head: string;
  export let base: string;
  export let baseOptions: string[];
  export let ensurePushed: () => Promise<boolean>;
  export let onClose: () => void;
  export let onCreated: (url: string) => void;

  let title = "";
  let body = "";
  let draft = false;
  let working = false;
  let error = "";

  onMount(async () => {
    try {
      const compare = await getRefCompare(base, head, true);
      const subjects = compare.commits.map((commit) => commit.subject);
      title = subjects.length === 1 ? subjects[0] : head.replace(/[-_/]+/g, " ").trim();
      body = subjects.length > 1 ? subjects.map((subject) => `- ${subject}`).join("\n") : compare.commits[0]?.bodySummary ?? "";
    } catch {
      title = head;
    }
  });

  async function create() {
    working = true;
    error = "";
    try {
      if (!(await ensurePushed())) {
        error = "Push the branch first, then try again.";
        return;
      }
      const url = await ghPrCreate(title, body, base, head, draft);
      onCreated(url);
      if (url.startsWith("https://")) void openUrl(url);
    } catch (err) {
      error = String(err);
    } finally {
      working = false;
    }
  }
</script>

<div class="modal-backdrop" role="presentation" on:click={(event) => event.target === event.currentTarget && onClose()}>
  <div class="modal repo-settings" role="dialog" aria-label="New pull request" tabindex="-1" use:trapFocus={{ initial: "#pr-title" }}>
    <div class="panel-title">
      <h1>New Pull Request</h1>
      <button title="Close" on:click={onClose}>×</button>
    </div>
    <div class="modal-body">
      {#if error}<p class="form-error">{error}</p>{/if}
      <div class="form-grid">
        <span class="field-label">From</span><strong class="mono">{head}</strong>
        <label for="pr-base">Into</label>
        <select id="pr-base" bind:value={base}>
          {#each baseOptions as option}<option value={option}>{option}</option>{/each}
        </select>
        <label for="pr-title">Title</label><input id="pr-title" bind:value={title} />
      </div>
      <label class="field-label" for="pr-body">Description</label>
      <textarea id="pr-body" rows="8" bind:value={body}></textarea>
      <label class="checkbox"><input type="checkbox" bind:checked={draft} /> Open as a draft</label>
      <div class="modal-actions">
        <button on:click={onClose}>Cancel</button>
        <button class="stage-file" on:click={create} disabled={working || !title.trim()}>{working ? "Creating…" : "Create Pull Request"}</button>
      </div>
    </div>
  </div>
</div>

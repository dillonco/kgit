<script lang="ts">
  // Per-repository management: remotes, author identity and commit signing,
  // LFS, sparse checkout, submodules, and external diff/merge tools.
  import { onMount } from "svelte";
  import { getRepoConfig, setConfig } from "./git";
  import { trapFocus } from "./modal";
  import type { GitAction, Profile, RepoConfig } from "./types";

  export let initialTab: Tab = "remotes";
  export let profiles: Profile[] = [];
  export let onRun: (action: GitAction, label: string) => Promise<boolean>;
  export let onClose: () => void;

  type Tab = "remotes" | "identity" | "lfs" | "sparse" | "submodules" | "tools";
  const tabs: { id: Tab; label: string }[] = [
    { id: "remotes", label: "Remotes" },
    { id: "identity", label: "Identity & Signing" },
    { id: "lfs", label: "LFS" },
    { id: "sparse", label: "Sparse Checkout" },
    { id: "submodules", label: "Submodules" },
    { id: "tools", label: "Diff & Merge Tools" },
  ];

  let tab: Tab = initialTab;
  let config: RepoConfig | null = null;
  let error = "";
  let working = false;

  // form state
  let newRemoteName = "";
  let newRemoteUrl = "";
  let identity = { name: "", email: "", signingKey: "", gpgFormat: "", commitGpgsign: false, tagGpgsign: false, global: false };
  let lfsPattern = "";
  let sparsePaths = "";
  let sparseCone = true;
  let submoduleUrl = "";
  let submodulePath = "";
  let mergeTool = "";
  let diffTool = "";

  async function load() {
    try {
      config = await getRepoConfig();
      identity = {
        name: config.userName ?? "",
        email: config.userEmail ?? "",
        signingKey: config.signingKey ?? "",
        gpgFormat: config.gpgFormat ?? "",
        commitGpgsign: config.commitGpgsign,
        tagGpgsign: config.tagGpgsign,
        global: config.identityScope === "global",
      };
      sparsePaths = config.sparsePaths.join("\n");
      sparseCone = config.sparseEnabled ? config.sparseCone : true;
      mergeTool = config.mergeTool ?? "";
      diffTool = config.diffTool ?? "";
    } catch (err) {
      error = String(err);
    }
  }

  onMount(load);

  async function run(action: GitAction, label: string) {
    working = true;
    error = "";
    try {
      if (await onRun(action, label)) await load();
    } finally {
      working = false;
    }
  }

  async function writeConfig(entries: [string, string | null][], global: boolean) {
    working = true;
    error = "";
    try {
      for (const [key, value] of entries) {
        const result = await setConfig(key, value, global);
        if (!result.ok) throw new Error(result.stderr || `Could not set ${key}`);
      }
      await load();
    } catch (err) {
      error = String(err);
    } finally {
      working = false;
    }
  }

  function saveIdentity() {
    void writeConfig(
      [
        ["user.name", identity.name],
        ["user.email", identity.email],
        ["user.signingkey", identity.signingKey || null],
        ["gpg.format", identity.gpgFormat || null],
        ["commit.gpgsign", identity.commitGpgsign ? "true" : null],
        ["tag.gpgsign", identity.tagGpgsign ? "true" : null],
      ],
      identity.global,
    );
  }

  function applyProfile(profile: Profile) {
    identity = {
      ...identity,
      name: profile.name,
      email: profile.email,
      signingKey: profile.signingKey ?? "",
      gpgFormat: profile.signingFormat ?? "",
      commitGpgsign: !!profile.signCommits,
      global: false,
    };
    saveIdentity();
  }

  function editRemote(name: string, field: "url" | "name", current: string) {
    const value = prompt(field === "url" ? `New URL for ${name}` : `Rename remote ${name} to`, current);
    if (!value?.trim() || value.trim() === current) return;
    void run(
      field === "url"
        ? { kind: "setRemoteUrl", remote: name, target: value.trim() }
        : { kind: "renameRemote", remote: name, target: value.trim() },
      field === "url" ? `Update ${name}` : `Rename ${name}`,
    );
  }
</script>

<div class="modal-backdrop" role="presentation" on:click={(event) => event.target === event.currentTarget && onClose()}>
  <div class="modal repo-settings" role="dialog" aria-label="Repository settings" tabindex="-1" use:trapFocus>
    <div class="panel-title">
      <h1>Repository</h1>
      <button title="Close" on:click={onClose}>×</button>
    </div>
    <nav class="repo-settings-tabs" aria-label="Repository settings sections">
      {#each tabs as item}
        <button class:active={tab === item.id} on:click={() => (tab = item.id)}>{item.label}</button>
      {/each}
    </nav>
    <div class="modal-body repo-settings-body">
      {#if error}<p class="form-error">{error}</p>{/if}
      {#if !config}
        <p class="empty centered">Loading…</p>
      {:else if tab === "remotes"}
        <table class="settings-table">
          <thead><tr><th>Name</th><th>Fetch URL</th><th></th></tr></thead>
          <tbody>
            {#each config.remotes as remote (remote.name)}
              <tr>
                <td><strong>{remote.name}</strong></td>
                <td class="mono" title={remote.pushUrl !== remote.fetchUrl ? `push: ${remote.pushUrl}` : ""}>{remote.fetchUrl}</td>
                <td class="row-buttons">
                  <button on:click={() => run({ kind: "fetchRemote", remote: remote.name }, `Fetch ${remote.name}`)} disabled={working}>Fetch</button>
                  <button on:click={() => editRemote(remote.name, "url", remote.fetchUrl)} disabled={working}>Edit URL</button>
                  <button on:click={() => editRemote(remote.name, "name", remote.name)} disabled={working}>Rename</button>
                  <button class="danger" on:click={() => confirm(`Remove remote ${remote.name}? Its remote-tracking branches go too.`) && run({ kind: "removeRemote", remote: remote.name }, `Remove ${remote.name}`)} disabled={working}>Remove</button>
                </td>
              </tr>
            {:else}
              <tr><td colspan="3" class="empty">No remotes</td></tr>
            {/each}
          </tbody>
        </table>
        <form class="inline-form" on:submit|preventDefault={() => run({ kind: "addRemote", remote: newRemoteName.trim(), target: newRemoteUrl.trim() }, `Add ${newRemoteName.trim()}`).then(() => { newRemoteName = ""; newRemoteUrl = ""; })}>
          <input placeholder="name (e.g. upstream)" bind:value={newRemoteName} aria-label="Remote name" />
          <input placeholder="URL" bind:value={newRemoteUrl} aria-label="Remote URL" class="grow" />
          <button class="stage-file" disabled={working || !newRemoteName.trim() || !newRemoteUrl.trim()}>Add Remote</button>
        </form>
      {:else if tab === "identity"}
        {#if profiles.length}
          <div class="field">
            <span class="field-label">Apply a profile to this repository</span>
            <div class="chip-row">
              {#each profiles as profile (profile.id)}
                <button on:click={() => applyProfile(profile)} disabled={working} title={`${profile.name} <${profile.email}>`}>{profile.label}</button>
              {/each}
            </div>
          </div>
        {/if}
        <p class="hint">
          Currently committing as <strong>{config.userName ?? "(unset)"} &lt;{config.userEmail ?? "unset"}&gt;</strong>
          {#if config.identityScope}from the {config.identityScope} config{/if}.
        </p>
        <div class="form-grid">
          <label for="id-name">Author name</label><input id="id-name" bind:value={identity.name} />
          <label for="id-email">Author email</label><input id="id-email" bind:value={identity.email} />
          <label for="id-key">Signing key</label><input id="id-key" bind:value={identity.signingKey} placeholder="GPG key id, or path to an SSH public key" />
          <label for="id-format">Signature format</label>
          <select id="id-format" bind:value={identity.gpgFormat}>
            <option value="">GPG (default)</option>
            <option value="openpgp">OpenPGP</option>
            <option value="ssh">SSH</option>
            <option value="x509">X.509 (S/MIME)</option>
          </select>
        </div>
        <label class="checkbox"><input type="checkbox" bind:checked={identity.commitGpgsign} /> Sign commits</label>
        <label class="checkbox"><input type="checkbox" bind:checked={identity.tagGpgsign} /> Sign annotated tags</label>
        <label class="checkbox"><input type="checkbox" bind:checked={identity.global} /> Save to the global config (every repository) instead of just this one</label>
        <div class="modal-actions"><button class="stage-file" on:click={saveIdentity} disabled={working || !identity.name.trim() || !identity.email.trim()}>Save Identity</button></div>
      {:else if tab === "lfs"}
        {#if !config.lfsAvailable}
          <p class="hint">Git LFS is not installed. Install it (for example <code>brew install git-lfs</code>) to manage large files.</p>
        {:else}
          <p class="hint">LFS is {config.lfsInitialized ? "initialized" : "not initialized"} in this repository.</p>
          {#if !config.lfsInitialized}
            <button class="stage-file" on:click={() => run({ kind: "lfsInstall" }, "Initialize LFS")} disabled={working}>Initialize LFS</button>
          {/if}
          <h2 class="settings-subhead">Tracked patterns</h2>
          <div class="chip-row">
            {#each config.lfsPatterns as pattern}
              <span class="chip">{pattern}<button title={`Stop tracking ${pattern}`} on:click={() => run({ kind: "lfsUntrack", target: pattern }, `Untrack ${pattern}`)} disabled={working}>×</button></span>
            {:else}
              <span class="empty">No patterns tracked</span>
            {/each}
          </div>
          <form class="inline-form" on:submit|preventDefault={() => run({ kind: "lfsTrack", target: lfsPattern.trim() }, `Track ${lfsPattern.trim()}`).then(() => (lfsPattern = ""))}>
            <input placeholder="Pattern, e.g. *.psd or assets/**" bind:value={lfsPattern} class="grow" aria-label="LFS pattern" />
            <button class="stage-file" disabled={working || !lfsPattern.trim()}>Track</button>
          </form>
          <div class="button-row">
            <button on:click={() => run({ kind: "lfsPull" }, "LFS pull")} disabled={working}>Pull LFS files</button>
            <button on:click={() => run({ kind: "lfsFetch" }, "LFS fetch")} disabled={working}>Fetch all LFS files</button>
            <button on:click={() => run({ kind: "lfsPushAll" }, "LFS push")} disabled={working}>Push all LFS files</button>
            <button on:click={() => run({ kind: "lfsPrune" }, "LFS prune")} disabled={working}>Prune local LFS cache</button>
          </div>
        {/if}
      {:else if tab === "sparse"}
        <p class="hint">
          {config.sparseEnabled ? "Sparse checkout is on: only the paths below are in the working tree." : "Sparse checkout is off: the whole tree is checked out."}
        </p>
        <label class="field-label" for="sparse-paths">Paths to include, one per line</label>
        <textarea id="sparse-paths" class="mono" rows="6" bind:value={sparsePaths} placeholder={"src/\ndocs/\nREADME.md"}></textarea>
        <label class="checkbox"><input type="checkbox" bind:checked={sparseCone} /> Cone mode (whole directories; faster, recommended)</label>
        <div class="button-row">
          <button class="stage-file" on:click={() => run({ kind: "sparseSet", message: sparsePaths, mode: sparseCone ? "cone" : "nonCone" }, config?.sparseEnabled ? "Update sparse checkout" : "Enable sparse checkout")} disabled={working || !sparsePaths.trim()}>
            {config.sparseEnabled ? "Update Paths" : "Enable Sparse Checkout"}
          </button>
          {#if config.sparseEnabled}
            <button on:click={() => run({ kind: "sparseReapply" }, "Reapply sparse checkout")} disabled={working}>Reapply</button>
            <button class="danger" on:click={() => run({ kind: "sparseDisable" }, "Disable sparse checkout")} disabled={working}>Disable</button>
          {/if}
        </div>
      {:else if tab === "submodules"}
        <table class="settings-table">
          <thead><tr><th>Path</th><th>Commit</th><th>Status</th><th></th></tr></thead>
          <tbody>
            {#each config.submodules as submodule (submodule.path)}
              <tr>
                <td class="mono">{submodule.path}</td>
                <td class="mono">{submodule.commit}{submodule.describe ? ` (${submodule.describe})` : ""}</td>
                <td>{submodule.status}</td>
                <td class="row-buttons">
                  <button class="danger" on:click={() => confirm(`Remove submodule ${submodule.path}?`) && run({ kind: "submoduleRemove", path: submodule.path }, `Remove ${submodule.path}`)} disabled={working}>Remove</button>
                </td>
              </tr>
            {:else}
              <tr><td colspan="4" class="empty">No submodules</td></tr>
            {/each}
          </tbody>
        </table>
        <div class="button-row">
          <button on:click={() => run({ kind: "submoduleUpdate" }, "Update submodules")} disabled={working}>Init &amp; Update All</button>
          <button on:click={() => run({ kind: "submoduleSync" }, "Sync submodule URLs")} disabled={working}>Sync URLs</button>
        </div>
        <form class="inline-form" on:submit|preventDefault={() => run({ kind: "submoduleAdd", target: submoduleUrl.trim(), path: submodulePath.trim() || null }, "Add submodule").then(() => { submoduleUrl = ""; submodulePath = ""; })}>
          <input placeholder="Repository URL" bind:value={submoduleUrl} class="grow" aria-label="Submodule URL" />
          <input placeholder="path (optional)" bind:value={submodulePath} aria-label="Submodule path" />
          <button class="stage-file" disabled={working || !submoduleUrl.trim()}>Add Submodule</button>
        </form>
      {:else if tab === "tools"}
        <p class="hint">kGit launches these through <code>git difftool</code> / <code>git mergetool</code>, so any tool git knows works (opendiff, vscode, kdiff3, meld, bc, p4merge, …).</p>
        <div class="form-grid">
          <label for="merge-tool">Merge tool</label><input id="merge-tool" bind:value={mergeTool} placeholder="e.g. opendiff" />
          <label for="diff-tool">Diff tool</label><input id="diff-tool" bind:value={diffTool} placeholder="e.g. opendiff" />
        </div>
        <div class="modal-actions">
          <button class="stage-file" on:click={() => writeConfig([["merge.tool", mergeTool || null], ["diff.tool", diffTool || null]], true)} disabled={working}>Save (global)</button>
        </div>
      {/if}
    </div>
  </div>
</div>

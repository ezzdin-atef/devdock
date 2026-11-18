<script lang="ts">
  import { onMount } from "svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Editor from "$lib/editor/Editor.svelte";
  import { files } from "$lib/files.svelte";
  import type { Project } from "$lib/types";
  import FileTree from "./files/FileTree.svelte";
  import SearchPanel from "./files/SearchPanel.svelte";
  import { fileIcon } from "./files/fileIcon";

  // Browse, edit and search the project's files. One file open at a time;
  // its unsaved edits survive switching tabs or projects.
  let { project }: { project: Project } = $props();

  // Remounted per project (ProjectDetail keys the tab body), so the id is fixed here.
  // svelte-ignore state_referenced_locally
  files.ensure(project.id);
  const id = $derived(project.id);
  const pf = $derived(files.of(id));
  const open = $derived(pf.open);
  let searchPanel = $state<SearchPanel | null>(null);

  onMount(() => {
    if (!pf.listings[""]) files.loadDir(id, "");
    files.checkDisk(id);
    const onFocus = () => files.checkDisk(id);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  function onKey(e: KeyboardEvent) {
    if (!e.ctrlKey || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === "f" && e.shiftKey) {
      e.preventDefault();
      if (pf.panel === "search") searchPanel?.focus();
      else pf.panel = "search";
    } else if (key === "s" && !e.shiftKey && open?.dirty) {
      // In the editor its own key handler saves; this covers focus elsewhere in the tab.
      e.preventDefault();
      files.save(id);
    }
  }

  const crumbs = $derived(open?.path.split("/") ?? pf.failed?.path.split("/") ?? []);
</script>

<svelte:window onkeydown={onKey} />

<div class="files">
  <aside>
    <div class="switch" role="tablist">
      <button role="tab" aria-selected={pf.panel === "tree"} class:on={pf.panel === "tree"} onclick={() => (pf.panel = "tree")}>
        <Icon name="folder" size={14} /> Files
      </button>
      <button
        role="tab"
        aria-selected={pf.panel === "search"}
        class:on={pf.panel === "search"}
        title="Search in files (Ctrl+Shift+F)"
        onclick={() => (pf.panel = "search")}
      >
        <Icon name="search" size={14} /> Search
      </button>
      <button
        class="icon-btn hidden-toggle"
        class:on={files.showHidden}
        aria-pressed={files.showHidden}
        title={files.showHidden ? "Hide hidden files and folders (dotfiles, .git)" : "Show hidden files and folders (dotfiles, .git)"}
        aria-label="Show hidden files and folders"
        onclick={() => files.setShowHidden(id, !files.showHidden)}
      >
        <Icon name={files.showHidden ? "eye" : "eyeOff"} size={14} />
      </button>
      {#if pf.panel === "tree"}
        <button class="icon-btn refresh" title="Refresh the file list" aria-label="Refresh the file list" onclick={() => files.refreshTree(id)}>
          <Icon name="refresh" size={14} />
        </button>
      {/if}
    </div>
    {#if pf.panel === "tree"}
      <div class="tree" role="tree">
        <FileTree projectId={id} dir="" />
      </div>
    {:else}
      <SearchPanel bind:this={searchPanel} projectId={id} />
    {/if}
  </aside>

  <section class="pane">
    {#if open || pf.failed}
      <header>
        <span class="crumb-icon" style:color={fileIcon(crumbs.at(-1) ?? "").tint}><Icon name={fileIcon(crumbs.at(-1) ?? "").icon} size={14} /></span>
        <div class="crumbs mono" title={open?.path ?? pf.failed?.path}>
          {#each crumbs as c, i (i)}
            {#if i > 0}<span class="sep">/</span>{/if}
            <span class:last={i === crumbs.length - 1}>{c}</span>
          {/each}
          {#if open?.dirty}<span class="dirty" title="Unsaved changes">●</span>{/if}
        </div>
        {#if open}
          <span class="meta mono" title="Line endings and encoding are kept when saving">
            {open.format.eol.toUpperCase()} · UTF-8{open.format.bom ? " BOM" : ""}
          </span>
          <Button variant="primary" disabled={!open.dirty || open.saving} onclick={() => files.save(id)}>
            {open.saving ? "Saving…" : "Save"}
          </Button>
        {/if}
        <button
          class="icon-btn"
          title={open?.dirty ? "Save or discard your changes first" : "Close the file"}
          aria-label="Close the file"
          disabled={open?.dirty}
          onclick={() => files.close(id)}
        >
          <Icon name="x" size={14} />
        </button>
      </header>
    {/if}

    {#if pf.pending && open}
      <div class="bar warn" role="alertdialog" aria-label="Unsaved changes">
        <span><strong>{open.path.split("/").pop()}</strong> has unsaved changes.</span>
        <div class="bar-actions">
          <Button variant="primary" onclick={() => files.resolvePending(id, "save")}>Save</Button>
          <Button variant="secondary" onclick={() => files.resolvePending(id, "discard")}>Discard</Button>
          <Button variant="secondary" onclick={() => files.resolvePending(id, "cancel")}>Cancel</Button>
        </div>
      </div>
    {/if}

    {#if open?.conflict === "changed"}
      <div class="bar warn" role="alert">
        <span>This file changed on disk{open.dirty ? " and you have unsaved edits" : ""}.</span>
        <div class="bar-actions">
          <Button variant="secondary" onclick={() => files.reload(id)}>Reload from disk</Button>
          {#if open.dirty}
            <Button variant="secondary" onclick={() => files.save(id, true)}>Overwrite with mine</Button>
          {/if}
        </div>
      </div>
    {:else if open?.conflict === "deleted"}
      <div class="bar warn" role="alert">
        <span>This file was deleted or moved on disk. Your text is still here.</span>
        <div class="bar-actions">
          {#if open.dirty}<Button variant="secondary" onclick={() => (files.of(id).open!.dirty = false)}>Discard</Button>{/if}
        </div>
      </div>
    {/if}

    {#if pf.error && pf.panel === "tree"}
      <div class="bar bad" role="alert">
        <span>{pf.error.message} {pf.error.fix ?? ""}</span>
        <div class="bar-actions">
          <Button variant="secondary" onclick={() => (pf.error = null)}>Dismiss</Button>
        </div>
      </div>
    {/if}

    {#if open}
      {#key id}
        <Editor projectId={id} />
      {/key}
    {:else if pf.failed}
      <div class="empty">
        <Icon name="alert" size={18} />
        <p>{pf.failed.error.message}</p>
        {#if pf.failed.error.fix}<p class="faint">{pf.failed.error.fix}</p>{/if}
      </div>
    {:else}
      <div class="empty">
        <Icon name="code" size={20} />
        <p>Pick a file to view or edit it.</p>
        <p class="faint">
          <span class="kbd mono">Ctrl S</span> saves · <span class="kbd mono">Ctrl F</span> finds in the file ·
          <span class="kbd mono">Ctrl Shift F</span> searches every file
        </p>
      </div>
    {/if}
  </section>
</div>

<style>
  .files {
    display: grid;
    grid-template-columns: minmax(220px, 280px) 1fr;
    flex: 1;
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    background: var(--bg-panel);
    overflow: hidden;
  }
  aside {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--line);
  }
  .switch {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-3);
    border-bottom: 1px solid var(--line);
  }
  .switch button[role="tab"] {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 28px;
    padding: 0 var(--space-4);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }
  .switch button[role="tab"]:hover {
    color: var(--text);
  }
  .switch button.on {
    background: var(--bg-raised);
    color: var(--text);
    font-weight: 600;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-left: auto;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text);
  }
  .switch .refresh {
    margin-left: 0;
  }
  .icon-btn.on {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .icon-btn.on:hover {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-2);
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--line);
  }
  .crumb-icon {
    display: grid;
    flex-shrink: 0;
    margin-right: calc(-1 * var(--space-2));
  }
  .crumbs {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    min-width: 0;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 12.5px;
    white-space: nowrap;
  }
  .crumbs .last {
    color: var(--text);
    font-weight: 600;
  }
  .sep {
    color: var(--text-faint);
  }
  .dirty {
    margin-left: var(--space-2);
    color: var(--accent-fg);
  }
  .meta {
    margin-left: auto;
    color: var(--text-faint);
    font-size: 11.5px;
    white-space: nowrap;
  }
  header .icon-btn {
    margin-left: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-6);
    padding: var(--space-3) var(--space-6);
    border-bottom: 1px solid var(--warning-border);
    background: var(--warning-panel);
    font-size: 13px;
  }
  .bar.bad {
    border-color: var(--error-border);
    background: var(--error-panel);
    color: var(--error-text);
  }
  .bar-actions {
    display: flex;
    gap: var(--space-3);
    flex-shrink: 0;
  }
  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-10);
    color: var(--text-muted);
    text-align: center;
  }
  .empty p {
    margin: 0;
  }
  .faint {
    color: var(--text-faint);
    font-size: 12.5px;
  }
  .kbd {
    display: inline-block;
    padding: 0 5px;
    border: 1px solid var(--line-strong);
    border-bottom-width: 2px;
    border-radius: var(--radius-badge);
    font-size: 11px;
  }
</style>

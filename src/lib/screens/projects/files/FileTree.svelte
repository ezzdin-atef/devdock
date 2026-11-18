<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { files } from "$lib/files.svelte";
  import type { ProjectId } from "$lib/types";
  import Self from "./FileTree.svelte";
  import { fileIcon } from "./fileIcon";

  // One folder's entries; folders expand in place and load on first open.
  let { projectId, dir, depth = 0 }: { projectId: ProjectId; dir: string; depth?: number } = $props();

  const pf = $derived(files.of(projectId));
  const listing = $derived(pf.listings[dir]);
</script>

{#if listing}
  {#each listing.entries as e (e.path)}
    {#if e.dir}
      <button
        class="row"
        class:ignored={e.ignored || e.hidden}
        style:--depth={depth}
        aria-expanded={!!pf.expanded[e.path]}
        title={e.path}
        onclick={() => files.toggleDir(projectId, e.path)}
      >
        <span class="caret" class:open={pf.expanded[e.path]}><Icon name="chevron" size={12} /></span>
        <span class="kind folder"><Icon name={pf.expanded[e.path] ? "folderOpen" : "folder"} size={14} /></span>
        <span class="name">{e.name}</span>
      </button>
      {#if pf.expanded[e.path]}
        <Self {projectId} dir={e.path} depth={depth + 1} />
      {/if}
    {:else}
      <button
        class="row file"
        class:ignored={e.ignored || e.hidden}
        class:active={pf.open?.path === e.path || pf.failed?.path === e.path}
        style:--depth={depth}
        title={e.path}
        onclick={() => files.open(projectId, e.path)}
      >
        <span class="kind" style:color={fileIcon(e.name).tint}><Icon name={fileIcon(e.name).icon} size={14} /></span>
        <span class="name">{e.name}</span>
        {#if pf.open?.path === e.path && pf.open.dirty}<span class="dot" aria-label="unsaved"></span>{/if}
      </button>
    {/if}
  {/each}
  {#if listing.hidden > 0}
    <div class="more" style:--depth={depth}>{listing.hidden} more not shown</div>
  {/if}
  {#if listing.entries.length === 0 && depth > 0}
    <div class="more" style:--depth={depth}>Empty</div>
  {/if}
{:else if pf.loading[dir]}
  <div class="more" style:--depth={depth}>Loading…</div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: 26px;
    padding: 0 var(--space-4) 0 calc(var(--space-3) + var(--depth) * 14px);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-soft);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .row.file {
    padding-left: calc(var(--space-3) + var(--depth) * 14px + 18px);
  }
  .row:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .row.active {
    background: var(--accent-active);
    color: var(--text);
  }
  .row.ignored {
    color: var(--text-faint);
  }
  .caret {
    display: grid;
    flex-shrink: 0;
    width: 16px;
    place-items: center;
    color: var(--icon);
    transform: rotate(-90deg);
    transition: transform var(--t-fast) var(--ease);
  }
  .caret.open {
    transform: none;
  }
  .kind {
    display: grid;
    flex-shrink: 0;
    place-items: center;
    width: 16px;
  }
  .kind.folder {
    color: var(--accent-fg);
  }
  .row.ignored .kind {
    opacity: 0.55;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    margin-left: auto;
    border-radius: 50%;
    background: var(--accent-fg);
  }
  .more {
    padding: 3px var(--space-4) 3px calc(var(--space-3) + var(--depth) * 14px + 18px);
    color: var(--text-faint);
    font-size: 12px;
  }
</style>

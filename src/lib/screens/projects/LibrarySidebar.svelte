<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { shortPath } from "$lib/format";
  import PathLine from "$lib/components/PathLine.svelte";
  import type { Library } from "$lib/types";

  const counts = $derived(app.lib?.counts);
  const items = $derived<{ library: Library; label: string; count: number }[]>([
    { library: "all", label: "All projects", count: counts?.all ?? 0 },
    { library: "favorites", label: "Favorites", count: counts?.favorites ?? 0 },
    { library: "recent", label: "Recently opened", count: counts?.recent ?? 0 },
    { library: "missing", label: "Missing", count: counts?.missing ?? 0 },
    ...(counts?.new ? [{ library: "new" as const, label: "New from scan", count: counts.new }] : []),
  ]);
</script>

<aside class="sidebar">
  <section>
    <div class="eyebrow head">Library</div>
    <nav>
      {#each items as item (item.library)}
        <button
          class="lib"
          class:active={app.query.library === item.library}
          class:alert={item.library === "missing" && item.count > 0}
          onclick={() => app.setQuery({ library: item.library })}
        >
          <span>{item.label}</span>
          <span class="count mono">{item.count}</span>
        </button>
      {/each}
    </nav>
  </section>

  <section>
    <div class="head row">
      <span class="eyebrow">Scan roots</span>
      {#if app.lib?.scanRoots.length}
        <button
          class="icon-btn"
          title="Rescan all roots"
          aria-label="Rescan all roots"
          disabled={app.scanning}
          onclick={() => app.rescan()}
        >
          <Icon name="refresh" size={14} />
        </button>
      {/if}
    </div>
    <ul class="roots">
      {#each app.lib?.scanRoots ?? [] as root (root.path)}
        <li class="root">
          <span class="root-path mono" title={root.path}>{shortPath(root.path, 2)}</span>
          <span class="depth mono" title="Scan depth">d{root.depth}</span>
          <button
            class="icon-btn remove"
            title="Stop scanning this folder (projects already found stay)"
            aria-label="Remove scan root {root.path}"
            onclick={() => app.removeScanRoot(root.path)}
          >
            <Icon name="x" size={12} />
          </button>
        </li>
      {/each}
    </ul>
    <button class="add-root" onclick={() => app.addScanRoot()} disabled={app.scanning}>
      <Icon name="plus" size={13} /> Add scan root
    </button>
  </section>

  <footer>
    <div class="eyebrow">Saved to</div>
    {#if app.lib?.savedTo}
      <PathLine path={app.lib.savedTo} tail={2} size={10} wrap />
    {/if}
  </footer>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    width: 228px;
    flex-shrink: 0;
    padding: var(--space-10) var(--space-5);
    background: var(--bg-chrome);
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .head {
    padding: 0 var(--space-4) var(--space-3);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .lib {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 32px;
    padding: 0 var(--space-4);
    border: 0;
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    text-align: left;
  }
  .lib:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .lib.active {
    background: var(--accent-tint);
    color: var(--accent-fg);
    font-weight: 600;
  }
  .count {
    font-size: 10.5px;
  }
  .lib.alert .count {
    color: var(--error-text);
  }
  .icon-btn {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .roots {
    list-style: none;
    margin: 0 0 var(--space-3);
    padding: 0;
  }
  .root {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 30px;
    padding: 0 var(--space-2) 0 var(--space-4);
    border-radius: var(--radius-button);
  }
  .root:hover {
    background: var(--bg-hover);
  }
  .root-path {
    flex: 1;
    min-width: 0;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .depth {
    font-size: 10px;
    color: var(--text-faint);
  }
  .remove {
    opacity: 0;
  }
  .root:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .add-root {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    height: 32px;
    padding: 0 var(--space-4);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .add-root:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--text-faint);
  }
  footer {
    margin-top: auto;
    padding: 0 var(--space-4);
  }
  footer .eyebrow {
    margin-bottom: var(--space-1);
  }
</style>

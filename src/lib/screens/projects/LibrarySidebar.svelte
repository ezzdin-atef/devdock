<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { shortPath } from "$lib/format";
  import PathLine from "$lib/components/PathLine.svelte";
  import { UNGROUPED, type Library } from "$lib/types";
  import { atCursor } from "$lib/components/Menu.svelte";
  import { NEW_GROUP, drag } from "$lib/drag.svelte";
  import { colorOf } from "$lib/sections";
  import GroupMenu from "./GroupMenu.svelte";
  import type { MenuAnchor } from "$lib/components/Menu.svelte";

  const groups = $derived(app.lib?.groups ?? []);
  let groupMenu = $state<{ id: string; at: MenuAnchor } | null>(null);
  const menuGroup = $derived(groupMenu ? groups.find((g) => g.id === groupMenu?.id) : undefined);

  /** Clicking the active group again shows everything. */
  function pickGroup(id: string) {
    app.select(null);
    app.setQuery({ group: app.query.group === id ? null : id, library: "all" });
  }

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
          class:active={app.query.library === item.library && app.query.group === null}
          class:alert={item.library === "missing" && item.count > 0}
          onclick={() => {
            app.select(null);
            app.setQuery({ library: item.library, group: null });
          }}
        >
          <span>{item.label}</span>
          <span class="count mono">{item.count}</span>
        </button>
      {/each}
    </nav>
  </section>

  <section>
    <div class="head row">
      <span class="eyebrow">Groups</span>
      <button class="icon-btn" title="New group" aria-label="New group" onclick={() => app.createGroup()}>
        <Icon name="plus" size={14} />
      </button>
    </div>
    <nav>
      {#each groups as g (g.id)}
        <button
          class="lib"
          class:active={app.query.group === g.id}
          class:drop={drag.active && drag.over === g.id}
          class:lifted={drag.group === g.id}
          data-drop-group={g.id}
          data-group-slot={g.id}
          data-axis="y"
          data-slot={drag.slot?.id === g.id ? (drag.slot.after ? "after" : "before") : undefined}
          onpointerdown={(e) => drag.begin(e, { kind: "group", id: g.id }, g.name, g.color)}
          title="Show only {g.name}. Right-click for more."
          onclick={() => pickGroup(g.id)}
          ondblclick={() => (app.renamingGroup = g.id)}
          oncontextmenu={(e) => {
            e.preventDefault();
            groupMenu = { id: g.id, at: atCursor(e) };
          }}
        >
          <span class="gname"><span class="gdot" style:background={colorOf(g.color)}></span>{g.name}</span>
          <span class="count mono">{app.lib?.counts.groups[g.id] ?? 0}</span>
        </button>
      {/each}
      {#if drag.active && drag.payload?.kind === "projects"}
        <div class="new-zone" class:drop={drag.over === NEW_GROUP} data-drop-group={NEW_GROUP}>
          <Icon name="plus" size={13} /> Drop here for a new group
        </div>
      {/if}
      {#if groups.length}
        <button
          class="lib"
          class:active={app.query.group === UNGROUPED}
          class:drop={drag.active && drag.over === UNGROUPED}
          data-drop-group={UNGROUPED}
          onclick={() => pickGroup(UNGROUPED)}
        >
          <span class="gname muted-name">Ungrouped</span>
          <span class="count mono">{app.lib?.counts.ungrouped ?? 0}</span>
        </button>
      {:else}
        <p class="ghint">Make groups to sort projects into folders, like "Client work" or "Experiments".</p>
      {/if}
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
      <PathLine path={app.lib.savedTo} tail={2} size={11} wrap />
    {/if}
  </footer>
</aside>

{#if groupMenu && menuGroup}
  <GroupMenu group={menuGroup} at={groupMenu.at} onclose={() => (groupMenu = null)} />
{/if}

<style>
  .gname {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gdot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .muted-name {
    color: var(--text-muted);
  }
  .lib.lifted {
    opacity: 0.45;
  }
  .new-zone {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-2) 0;
    padding: var(--space-4) var(--space-4);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-button-sm);
    font-size: 12.5px;
    color: var(--text-muted);
    animation: dd-fade 140ms var(--ease);
  }
  .new-zone.drop {
    border-color: var(--accent-fg);
    background: var(--accent-panel);
    color: var(--accent-fg);
  }
  .lib.drop {
    outline: 1px dashed var(--accent-fg);
    background: var(--accent-panel);
  }
  .ghint {
    margin: 0;
    padding: var(--space-2) var(--space-4);
    font-size: 12px;
    color: var(--text-faint);
  }
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
    font-size: 11.5px;
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
    color: var(--icon);
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
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .depth {
    font-size: 11px;
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

<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { plural, shortPath } from "$lib/format";
  import { KIND_NAME, type Kind, type SortBy } from "$lib/types";
  import { runs } from "$lib/runs.svelte";
  import GitBadge from "./GitBadge.svelte";
  import RenameInput from "./RenameInput.svelte";
  import RowMenu from "./RowMenu.svelte";

  // A calm list: one strong line (name), one quiet line (branch and changes).
  // Everything else (path, type filters, sort) moves to tooltips and two small
  // menus, so the column reads as a list of names first.
  let menuFor = $state<string | null>(null);

  const SORTS: { sort: SortBy; label: string }[] = [
    { sort: "recent", label: "Recent" },
    { sort: "name", label: "Name" },
    { sort: "kind", label: "Type" },
  ];

  const kinds = $derived(
    (Object.entries(app.lib?.counts.kinds ?? {}) as [Kind, number][]).filter(([, n]) => n > 0),
  );
  const projects = $derived(app.lib?.projects ?? []);
  const total = $derived(app.lib?.counts.all ?? 0);
  const filtered = $derived(
    app.query.search !== "" || app.query.library !== "all" || app.query.kind !== null,
  );

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Escape") app.setQuery({ search: "" });
  }

  function scanLine(): string {
    const s = app.scanSummary;
    if (!s) return "";
    const parts = [`${s.added} new`];
    if (s.duplicates) parts.push(`${s.duplicates} already docked`);
    if (s.ignored) parts.push(`${plural(s.ignored, "ignored folder")} skipped`);
    if (s.unreadable) parts.push(`${plural(s.unreadable, "folder")} unreadable`);
    return parts.join(", ") + ".";
  }
</script>

<section class="list">
  <div class="top">
    <label class="search">
      <Icon name="search" size={14} />
      <input
        type="search"
        placeholder="Search projects"
        value={app.query.search}
        oninput={(e) => app.setQuery({ search: e.currentTarget.value })}
        onkeydown={onSearchKey}
        spellcheck="false"
      />
    </label>
    <button class="add" onclick={() => app.addProject()} title="Add a project folder" aria-label="Add a project folder">
      <Icon name="plus" size={16} />
    </button>
  </div>

  {#if total > 0}
    <div class="bar">
      <span class="count">
        {filtered ? `${projects.length} of ${total}` : plural(total, "project")}
      </span>
      <span class="spacer"></span>
      {#if kinds.length > 1 || app.query.kind}
        <label class="pick" class:on={app.query.kind !== null} title="Show one type">
          <select
            value={app.query.kind ?? ""}
            onchange={(e) => app.setQuery({ kind: (e.currentTarget.value || null) as Kind | null })}
            aria-label="Filter by type"
          >
            <option value="">All types</option>
            {#each kinds as [kind, n] (kind)}
              <option value={kind}>{KIND_NAME[kind]} ({n})</option>
            {/each}
          </select>
          <Icon name="chevron" size={12} />
        </label>
      {/if}
      <label class="pick" title="Sort order">
        <select
          value={app.query.sort}
          onchange={(e) => app.setQuery({ sort: e.currentTarget.value as SortBy })}
          aria-label="Sort by"
        >
          {#each SORTS as s (s.sort)}
            <option value={s.sort}>{s.label}</option>
          {/each}
        </select>
        <Icon name="chevron" size={12} />
      </label>
    </div>
  {/if}

  {#if app.scanProgress}
    <div class="scan-banner" role="status">
      <span class="muted">
        Scanning <span class="mono">{shortPath(app.scanProgress.root, 2)}</span>… {app.scanProgress.found} found
      </span>
    </div>
  {:else if app.scanSummary}
    <div class="scan-banner" role="status">
      <div class="scan-text">
        <span>{scanLine()}</span>
        {#each app.scanSummary.missingRoots as root (root)}
          <span class="warn">Scan root not found: <span class="mono">{root}</span></span>
        {/each}
      </div>
      {#if app.scanSummary.added > 0 && app.query.library !== "new"}
        <Button variant="ghost" onclick={() => app.reviewNew()}>Review new</Button>
      {/if}
      <button class="x" aria-label="Dismiss scan result" title="Dismiss" onclick={() => app.dismissScanSummary()}>
        <Icon name="x" size={12} />
      </button>
    </div>
  {/if}

  {#if total === 0}
    <div class="empty-actions">
      <button class="add-target" onclick={() => app.addProject()}>
        <Icon name="folder" /> Choose a folder to dock…
      </button>
      <button class="add-target" onclick={() => app.addScanRoot()}>
        <Icon name="search" /> Add scan root
      </button>
    </div>
  {:else if projects.length === 0}
    <div class="no-match">
      <span>No projects match.</span>
      {#if filtered}
        <Button variant="ghost" onclick={() => app.clearFilters()}>Clear filters</Button>
      {/if}
    </div>
  {:else}
    <ul class="rows">
      {#each projects as project (project.id)}
        {@const running = runs.activeIn(project.id)}
        {@const isNew = project.isNew}
        <li class="row-wrap">
          <div
            class="row"
            class:selected={project.id === app.selectedId}
            class:missing={project.missing}
            title={project.path}
          >
            <button
              class="row-hit"
              aria-label="Select {project.name}"
              onclick={() => app.select(project.id)}
              ondblclick={() => app.startRename(project.id)}
            ></button>
            <span class="row-main">
              <span class="row-top">
                {#if app.renamingId === project.id}
                  <RenameInput {project} />
                {:else}
                  <span class="name">{project.name}</span>
                {/if}
                {#if project.favorite}
                  <span class="fav" title="Favorite"><Icon name="star" size={12} filled /></span>
                {/if}
                {#if isNew}
                  <span class="new" title="New from the last scan" aria-label="New"></span>
                {/if}
                <span class="spacer"></span>
                {#if running > 0}
                  <span class="live" title="{running} running"><span class="live-dot"></span>{running > 1 ? `${running} running` : "running"}</span>
                {:else if !project.missing}
                  <span class="kind">{KIND_NAME[project.kind]}</span>
                {/if}
              </span>
              {#if project.missing}
                <span class="sub missing-text">Folder not found</span>
              {:else if project.git?.state === "noRepo"}
                <span class="sub mono">{shortPath(project.path, 2)}</span>
              {:else}
                <span class="sub"><GitBadge git={project.git} /></span>
              {/if}
            </span>
            <button
              class="kebab"
              class:open={menuFor === project.id}
              aria-label="Actions for {project.name}"
              aria-haspopup="menu"
              title="Rename, copy path, remove"
              onclick={() => (menuFor = menuFor === project.id ? null : project.id)}
            >
              <Icon name="kebab" size={14} />
            </button>
          </div>
          {#if menuFor === project.id}
            <RowMenu {project} onclose={() => (menuFor = null)} />
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    width: 344px;
    flex-shrink: 0;
    padding: var(--space-10) var(--space-5);
    background: var(--bg-sidebar);
    border-right: 1px solid var(--line);
    overflow-y: auto;
    overflow-x: hidden;
  }
  .top {
    display: flex;
    gap: var(--space-4);
  }
  .search {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    height: 36px;
    padding: 0 var(--space-5);
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    color: var(--icon);
  }
  .search:focus-within {
    border-color: var(--accent-fg);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  .search input::placeholder {
    color: var(--text-muted);
  }
  .add {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-button);
    background: var(--bg-raised);
    color: var(--text);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  .add:hover {
    border-color: var(--line-strong);
    background: var(--bg-hover);
  }

  /* Count on the left, two quiet menus on the right: replaces the sort segments
     and the wrapping row of type chips. */
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2) 0 var(--space-4);
    margin-top: calc(-1 * var(--space-1));
  }
  .count {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .spacer {
    flex: 1;
  }
  .pick {
    position: relative;
    display: inline-flex;
    align-items: center;
    height: 28px;
    border-radius: var(--radius-button-sm);
    color: var(--text-soft);
    transition: background var(--t-fast) var(--ease);
  }
  .pick:hover {
    background: var(--bg-hover);
  }
  .pick.on {
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .pick select {
    appearance: none;
    height: 100%;
    padding: 0 22px 0 var(--space-4);
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
  }
  .pick select:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 0;
    border-radius: var(--radius-button-sm);
  }
  .pick select option {
    background: var(--bg-overlay);
    color: var(--text);
  }
  .pick :global(svg) {
    position: absolute;
    right: 6px;
    pointer-events: none;
    color: var(--icon);
  }

  .scan-banner {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-4) var(--space-4) var(--space-6);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
    font-size: 13px;
  }
  .scan-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .muted {
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .warn {
    color: var(--error-text);
    overflow-wrap: anywhere;
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .x:hover {
    background: var(--bg-hover);
  }
  .empty-actions {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .add-target {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    padding: var(--space-7) var(--space-6);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    text-align: left;
  }
  .add-target:hover {
    color: var(--text);
    border-color: var(--text-faint);
  }
  .no-match {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-4) var(--space-6);
    color: var(--text-muted);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .row-wrap {
    position: relative;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-3) var(--space-4) var(--space-6);
    border: 1px solid transparent;
    border-radius: var(--radius-row);
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.selected {
    background: var(--accent-panel);
    border-color: var(--accent-panel-border);
  }
  /* Full-row click target under the content; inner controls sit above it. */
  .row-hit {
    position: absolute;
    inset: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
    cursor: pointer;
  }
  .row-main {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
    pointer-events: none;
  }
  .row-main :global(input) {
    pointer-events: auto;
  }
  .row-top {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }
  .name {
    font-size: 14.5px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.missing .name {
    color: var(--text-muted);
    text-decoration: line-through;
    text-decoration-color: var(--text-faint);
  }
  .fav {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--accent-fg);
  }
  /* "New from scan": a small blue dot, not a word on every row. Blue keeps it
     apart from the yellow changed-files dot on the line below. */
  .new {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--info);
  }
  .kind {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
    font-size: 12px;
    font-weight: 500;
    color: var(--success);
  }
  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-pill);
  }
  .sub {
    display: flex;
    min-width: 0;
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .missing-text {
    color: var(--error-text);
  }
  /* Row actions appear in place of the type label, so a hidden button doesn't
     reserve an empty strip on every row. */
  .kebab {
    position: absolute;
    top: 50%;
    right: var(--space-3);
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .kebab,
  .row.selected .kebab,
  .kebab.open,
  .kebab:focus-visible {
    opacity: 1;
  }
  .row:hover .kind,
  .row:hover .live,
  .row.selected .kind,
  .row.selected .live,
  .row:has(.kebab.open) .kind {
    visibility: hidden;
  }
  .kebab:hover,
  .kebab.open {
    background: var(--bg-raised);
    color: var(--text);
  }
</style>

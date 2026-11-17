<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { plural, shortPath } from "$lib/format";
  import { KIND_LABEL, type Kind, type SortBy } from "$lib/types";
  import { runs } from "$lib/runs.svelte";
  import GitBadge from "./GitBadge.svelte";
  import RenameInput from "./RenameInput.svelte";
  import RowMenu from "./RowMenu.svelte";

  let menuFor = $state<string | null>(null);

  const SORTS: { sort: SortBy; label: string }[] = [
    { sort: "recent", label: "Recent" },
    { sort: "name", label: "Name" },
    { sort: "kind", label: "Type" },
  ];

  const chips = $derived(
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
        placeholder="Search name or path"
        value={app.query.search}
        oninput={(e) => app.setQuery({ search: e.currentTarget.value })}
        onkeydown={onSearchKey}
        spellcheck="false"
      />
    </label>
    <Button variant="secondary" onclick={() => app.addProject()} title="Add a project folder">
      <Icon name="plus" size={14} /> Add
    </Button>
  </div>

  <div class="controls">
    <div class="segmented" role="radiogroup" aria-label="Sort">
      {#each SORTS as s (s.sort)}
        <button
          role="radio"
          aria-checked={app.query.sort === s.sort}
          class:on={app.query.sort === s.sort}
          onclick={() => app.setQuery({ sort: s.sort })}>{s.label}</button
        >
      {/each}
    </div>
  </div>

  {#if chips.length > 1 || app.query.kind}
    <div class="chips">
      {#each chips as [kind, n] (kind)}
        <button
          class="chip mono"
          class:on={app.query.kind === kind}
          aria-pressed={app.query.kind === kind}
          onclick={() => app.setQuery({ kind: app.query.kind === kind ? null : kind })}
        >
          {KIND_LABEL[kind]} <span class="n">{n}</span>
        </button>
      {/each}
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
        <li class="row-wrap">
          <div
            class="row"
            class:selected={project.id === app.selectedId}
            class:missing={project.missing}
          >
            <button
              class="row-hit"
              aria-label="Select {project.name}"
              onclick={() => app.select(project.id)}
              ondblclick={() => app.startRename(project.id)}
            ></button>
            <span
              class="dot"
              class:dot-missing={project.missing}
              class:dot-running={runs.activeIn(project.id) > 0}
              title={project.missing
                ? "Folder not found"
                : runs.activeIn(project.id) > 0
                  ? `${runs.activeIn(project.id)} running`
                  : "Idle"}
            ></span>
            <span class="row-main">
              <span class="row-top">
                {#if app.renamingId === project.id}
                  <RenameInput {project} />
                {:else}
                  <span class="name">{project.name}</span>
                {/if}
                {#if project.favorite}
                  <span class="fav" title="Favorite"><Icon name="star" size={11} filled /></span>
                {/if}
                {#if project.isNew || project.id === app.justAddedId}
                  <span class="tag-new mono">NEW</span>
                {/if}
                <span class="kind mono">{KIND_LABEL[project.kind]}</span>
              </span>
              {#if project.missing}
                <span class="sub mono missing-text">Folder not found</span>
              {:else}
                <span class="sub-row">
                  <GitBadge git={project.git} />
                  <span class="sub mono" title={project.path}>{shortPath(project.path)}</span>
                </span>
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
  }
  .top {
    display: flex;
    gap: var(--space-4);
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    height: 36px;
    padding: 0 var(--space-5);
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    color: var(--text-muted);
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
  .segmented {
    display: inline-flex;
    padding: 3px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
  }
  .segmented button {
    height: 26px;
    padding: 0 var(--space-6);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 12px;
    cursor: pointer;
  }
  .segmented button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .chip {
    height: 26px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-pill);
    background: transparent;
    color: var(--text-muted);
    font-size: 10px;
    letter-spacing: 0.05em;
    cursor: pointer;
  }
  .chip .n {
    margin-left: var(--space-2);
    opacity: 0.8;
  }
  .chip.on {
    border-color: var(--accent-fg);
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .scan-banner {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4) var(--space-4) var(--space-4) var(--space-6);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
    font-size: 12.5px;
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
    color: var(--text-muted);
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
    gap: var(--space-1);
  }
  .row-wrap {
    position: relative;
  }
  .row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: var(--space-5);
    padding: var(--space-5) var(--space-4) var(--space-5) var(--space-6);
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
  .dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    margin-top: 6px;
    border-radius: 50%;
    border: 1.5px solid var(--idle);
    pointer-events: none;
  }
  .dot-running {
    border: 0;
    background: var(--success);
  }
  .row.selected .dot-running {
    box-shadow: 0 0 0 3px var(--success-pill);
  }
  .dot-missing {
    border: 1.5px dashed var(--error);
  }
  .row-main {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
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
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.missing .name {
    color: var(--text-muted);
  }
  .fav {
    color: var(--accent-fg);
    display: inline-flex;
  }
  .tag-new {
    font-size: 10px;
    color: var(--accent-fg);
  }
  .kind {
    margin-left: auto;
    padding-left: var(--space-4);
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .sub {
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub-row {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    min-width: 0;
  }
  .sub-row .sub {
    flex: 1;
    min-width: 0;
  }
  .missing-text {
    color: var(--error-text);
  }
  .kebab {
    position: relative;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .kebab,
  .row.selected .kebab,
  .kebab.open,
  .kebab:focus-visible {
    opacity: 1;
  }
  .kebab:hover,
  .kebab.open {
    background: var(--bg-raised);
    color: var(--text);
  }
</style>

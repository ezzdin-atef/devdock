<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Menu, { belowButton, type MenuAnchor } from "$lib/components/Menu.svelte";
  import Select from "$lib/components/Select.svelte";
  import { drag } from "$lib/drag.svelte";
  import { plural, shortPath } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { colorOf, buildSections, loadCollapsed, saveCollapsed } from "$lib/sections";
  import { KIND_NAME, UNGROUPED, type Kind, type ListGrouping, type SortBy } from "$lib/types";
  import { DETAIL_LABELS, view, type Layout, type Size } from "$lib/viewPrefs.svelte";
  import { flip } from "svelte/animate";
  import ProjectItem from "./ProjectItem.svelte";
  import SectionHead from "./SectionHead.svelte";

  // The Projects page: every project as cards (by folder or group), project cards,
  // or compact rows. Layout, size, grouping, sort and details are the user's choice.
  const projects = $derived(app.lib?.projects ?? []);
  const groups = $derived(app.lib?.groups ?? []);
  const total = $derived(app.lib?.counts.all ?? 0);
  const loading = $derived(app.lib === null);
  const filtered = $derived(
    app.query.search !== "" || app.query.library !== "all" || app.query.kind !== null || app.query.group !== null,
  );
  /** A group picked in the sidebar: show its projects as plain cards, not sections. */
  const pickedGroup = $derived(app.query.group);
  const pickedInfo = $derived(
    pickedGroup === UNGROUPED
      ? { name: "Ungrouped", color: undefined }
      : groups.find((g) => g.id === pickedGroup),
  );
  const layout = $derived(pickedGroup && view.layout === "folders" ? "cards" : view.layout);
  const grouping = $derived(pickedGroup ? "none" : view.grouping);
  const sections = $derived(buildSections(projects, groups, grouping, !filtered));
  const favorites = $derived(projects.filter((p) => p.favorite));
  const kinds = $derived(
    (Object.entries(app.lib?.counts.kinds ?? {}) as [Kind, number][]).filter(([, n]) => n > 0),
  );

  const running = $derived(projects.filter((p) => runs.activeIn(p.id) > 0).length);
  const needInstall = $derived(projects.filter((p) => p.deps && p.deps.state !== "installed").length);
  const changed = $derived(projects.filter((p) => p.git?.state === "repo" && p.git.data.changedFiles > 0).length);
  const gitPending = $derived(projects.filter((p) => !p.missing && p.git == null).length);

  let collapsed = $state<Record<string, true>>(loadCollapsed());
  function toggle(key: string) {
    if (collapsed[key]) delete collapsed[key];
    else collapsed[key] = true;
    saveCollapsed($state.snapshot(collapsed));
  }
  /** Folder cards show this many projects, then "Show all". */
  const SHOW = $derived(view.size === "s" ? 8 : view.size === "m" ? 6 : 5);
  let expanded = $state<Record<string, true>>({});

  let viewMenu = $state<MenuAnchor | null>(null);

  const GROUPINGS: { value: ListGrouping; label: string }[] = [
    { value: "none", label: "All projects" },
    { value: "groups", label: "By group" },
    { value: "folders", label: "By folder" },
    { value: "kind", label: "By type" },
  ];
  const SORTS: { value: SortBy; label: string }[] = [
    { value: "recent", label: "Recently opened" },
    { value: "name", label: "Name" },
    { value: "kind", label: "Type" },
    { value: "manual", label: "Manual order" },
  ];
  const LAYOUTS: { value: Layout; label: string; title: string }[] = [
    { value: "folders", label: "Folders", title: "One card per group or folder" },
    { value: "cards", label: "Cards", title: "One card per project" },
    { value: "rows", label: "Rows", title: "Compact rows with columns" },
  ];
  const SIZES: { value: Size; label: string }[] = [
    { value: "s", label: "S" },
    { value: "m", label: "M" },
    { value: "l", label: "L" },
  ];

  /** Row layout: columns follow the details that are on. */
  const rowColumns = $derived(
    [
      "minmax(160px, 2fr)",
      view.details.git ? "minmax(120px, 1.4fr)" : "",
      view.details.packages ? "minmax(110px, auto)" : "",
      view.details.kind ? "minmax(80px, 0.8fr)" : "",
      view.details.opened ? "minmax(90px, 0.8fr)" : "",
      view.details.path ? "minmax(140px, 1.6fr)" : "",
      "32px",
    ]
      .filter(Boolean)
      .join(" "),
  );

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

<div class="home size-{view.size}">
  <header>
    <div class="title">
      {#if pickedInfo}
        <h1 class="picked">
          <button class="crumb" onclick={() => app.setQuery({ group: null })} title="Show all projects">Projects</button>
          <span class="slash">/</span>
          {#if pickedInfo.color}<span class="gdot" style:background={colorOf(pickedInfo.color)}></span>{/if}
          {pickedInfo.name}
        </h1>
      {:else}
        <h1>Projects</h1>
      {/if}
      <div class="stats">
        <span class="stat">{pickedInfo ? plural(projects.length, "project") : plural(total, "project")}</span>
        {#if running}<span class="stat ok"><span class="live-dot"></span>{running} running</span>{/if}
        {#if needInstall}<span class="stat warn">{needInstall} need packages</span>{/if}
        {#if changed}<span class="stat">{changed} with changes</span>{/if}
        {#if gitPending}
          <span class="stat faint" role="status"
            ><span class="spinner" aria-hidden="true"></span>git {projects.length - gitPending}/{projects.length}</span
          >
        {/if}
      </div>
    </div>
    <Button variant="secondary" onclick={() => app.addProject()}><Icon name="plus" size={14} /> Add project</Button>
  </header>

  <div class="toolbar">
    <label class="search">
      <Icon name="search" size={14} />
      <input
        type="search"
        placeholder="Search projects"
        value={app.query.search}
        oninput={(e) => app.setQuery({ search: e.currentTarget.value })}
        onkeydown={(e) => {
          if (e.key === "Escape") app.setQuery({ search: "" });
        }}
        spellcheck="false"
      />
    </label>
    <Select
      value={grouping}
      disabled={!!pickedGroup}
      variant="quiet"
      ariaLabel="Group projects"
      title="Split the page into sections"
      options={GROUPINGS}
      onchange={(v) => view.set({ grouping: v })}
    />
    {#if kinds.length > 1 || app.query.kind}
      <Select
        value={app.query.kind ?? ""}
        variant="quiet"
        ariaLabel="Filter by type"
        options={[
          { value: "", label: "All types" },
          ...kinds.map(([kind, n]) => ({ value: kind as string, label: KIND_NAME[kind], hint: String(n) })),
        ]}
        onchange={(v) => app.setQuery({ kind: (v || null) as Kind | null })}
      />
    {/if}
    <Select
      value={app.query.sort}
      variant="quiet"
      ariaLabel="Sort by"
      options={SORTS}
      onchange={(v) => {
        view.set({ sort: v });
        app.setQuery({ sort: v });
      }}
    />
    <span class="spacer"></span>
    <div class="seg" role="radiogroup" aria-label="Layout">
      {#each LAYOUTS as l (l.value)}
        <button
          role="radio"
          aria-checked={view.layout === l.value}
          class:on={view.layout === l.value}
          title={l.title}
          onclick={() => view.set({ layout: l.value })}>{l.label}</button
        >
      {/each}
    </div>
    <div class="seg" role="radiogroup" aria-label="Size">
      {#each SIZES as s (s.value)}
        <button
          role="radio"
          aria-checked={view.size === s.value}
          aria-label="Size {s.label}"
          class:on={view.size === s.value}
          onclick={() => view.set({ size: s.value })}>{s.label}</button
        >
      {/each}
    </div>
    <button class="view-btn" aria-haspopup="menu" onclick={(e) => (viewMenu = belowButton(e))}>
      <Icon name="sliders" size={14} /> View
    </button>
  </div>

  {#if app.scanProgress}
    <div class="banner" role="status">
      Scanning <span class="mono">{shortPath(app.scanProgress.root, 2)}</span>… {app.scanProgress.found} found
    </div>
  {:else if app.scanSummary}
    <div class="banner" role="status">
      <span class="grow">
        {scanLine()}
        {#each app.scanSummary.missingRoots as root (root)}
          <span class="warn">Scan root not found: <span class="mono">{root}</span></span>
        {/each}
      </span>
      {#if app.scanSummary.added > 0 && app.query.library !== "new"}
        <Button variant="ghost" onclick={() => app.reviewNew()}>Review new</Button>
      {/if}
      <button class="x" aria-label="Dismiss scan result" onclick={() => app.dismissScanSummary()}>
        <Icon name="x" size={12} />
      </button>
    </div>
  {/if}

  {#if loading}
    <div class="grid cards-grid" aria-busy="true" aria-label="Loading projects">
      {#each Array(8) as _, i (i)}
        <div class="skel-card" style:animation-delay="{i * 50}ms">
          <span class="skel" style:width="{55 + ((i * 13) % 30)}%"></span>
          <span class="skel thin" style:width="40%"></span>
        </div>
      {/each}
    </div>
  {:else if projects.length === 0}
    <div class="none">
      <p>No projects match.</p>
      {#if filtered}<Button variant="secondary" onclick={() => app.clearFilters()}>Clear filters</Button>{/if}
    </div>
  {:else}
    {#if view.favoritesRow && favorites.length && !filtered}
      <section class="favs" aria-label="Favorites">
        <div class="favs-head"><Icon name="star" size={13} filled /> Favorites</div>
        <div class="favs-row">
          {#each favorites as p (p.id)}
            <div class="fav-card"><ProjectItem project={p} variant="card" details={view.details} slot={false} /></div>
          {/each}
        </div>
      </section>
    {/if}

    {#if layout === "folders"}
      <div class="grid folders-grid">
        {#each sections as sec (sec.key)}
          {@const shown = expanded[sec.key] ? sec.projects : sec.projects.slice(0, SHOW)}
          {@const gid = sec.groupId && sec.groupId !== "-" ? sec.groupId : undefined}
          <section
            class="folder"
            class:drop={drag.active && sec.groupId !== undefined && drag.over === sec.groupId}
            class:lifted={!!gid && drag.group === gid}
            data-drop-group={sec.groupId}
            data-group-slot={gid}
            data-axis="x"
            data-slot={gid && drag.slot?.id === gid ? (drag.slot.after ? "after" : "before") : undefined}
            style:--bar={colorOf(sec.color)}
          >
            <SectionHead section={sec} slot={false} />
            <div class="folder-list">
              {#each shown as p (p.id)}
                <div animate:flip={{ duration: 200 }}>
                  <ProjectItem project={p} variant="line" details={view.details} groupId={grouping === "groups" ? sec.groupId : undefined} />
                </div>
              {:else}
                <p class="empty">Empty. Drag projects here.</p>
              {/each}
            </div>
            {#if sec.projects.length > SHOW}
              <button
                class="more"
                onclick={() => {
                  if (expanded[sec.key]) delete expanded[sec.key];
                  else expanded[sec.key] = true;
                }}>{expanded[sec.key] ? "Show fewer" : `Show all ${sec.projects.length}`}</button
              >
            {/if}
          </section>
        {/each}
      </div>
    {:else}
      {#each sections as sec (sec.key)}
        <section class="sec">
          {#if grouping !== "none"}
            <SectionHead section={sec} collapsed={!!collapsed[sec.key]} ontoggle={() => toggle(sec.key)} />
          {/if}
          {#if grouping === "none" || !collapsed[sec.key]}
            {#if layout === "cards"}
              <div class="grid cards-grid" data-drop-group={sec.groupId}>
                {#each sec.projects as p (p.id)}
                  <div class="slot" animate:flip={{ duration: 220 }}>
                    <ProjectItem project={p} variant="card" details={view.details} groupId={grouping === "groups" ? sec.groupId : undefined} />
                  </div>
                {:else}
                  <p class="empty">Empty. Drag projects here.</p>
                {/each}
              </div>
            {:else}
              <div class="rows" style:grid-template-columns={rowColumns} data-drop-group={sec.groupId}>
                <div class="rows-head">
                  <span>Name</span>
                  {#if view.details.git}<span>Git</span>{/if}
                  {#if view.details.packages}<span>Packages</span>{/if}
                  {#if view.details.kind}<span>Type</span>{/if}
                  {#if view.details.opened}<span>Opened</span>{/if}
                  {#if view.details.path}<span>Folder</span>{/if}
                  <span></span>
                </div>
                {#each sec.projects as p (p.id)}
                  <ProjectItem project={p} variant="row" details={view.details} groupId={grouping === "groups" ? sec.groupId : undefined} />
                {:else}
                  <p class="empty wide">Empty. Drag projects here.</p>
                {/each}
              </div>
            {/if}
          {/if}
        </section>
      {/each}
    {/if}
  {/if}
</div>

{#if viewMenu}
  <Menu at={viewMenu} label="View options" width={250} onclose={() => (viewMenu = null)}>
    <div class="menu-label">Show on each project</div>
    {#each DETAIL_LABELS as d (d.key)}
      <button role="menuitemcheckbox" aria-checked={view.details[d.key]} onclick={() => view.toggleDetail(d.key)}>
        <span class="box" class:on={view.details[d.key]}>{#if view.details[d.key]}<Icon name="check" size={11} />{/if}</span>
        {d.label}
      </button>
    {/each}
    <hr />
    <button
      role="menuitemcheckbox"
      aria-checked={view.favoritesRow}
      onclick={() => view.set({ favoritesRow: !view.favoritesRow })}
    >
      <span class="box" class:on={view.favoritesRow}>{#if view.favoritesRow}<Icon name="check" size={11} />{/if}</span>
      Favorites row at the top
    </button>
    <hr />
    <button role="menuitem" onclick={() => view.reset()}>Reset view</button>
  </Menu>
{/if}

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: var(--space-7);
    animation: dd-enter 200ms var(--ease);
  }
  .size-s {
    --fs: 12.5px;
    --line-h: 26px;
    --row-h: 32px;
    --card-pad: 9px 11px;
    --name-fs: 13.5px;
    --card-min: 200px;
    --folder-min: 220px;
  }
  .size-m {
    --fs: 13px;
    --line-h: 30px;
    --row-h: 40px;
    --card-pad: 12px 14px;
    --name-fs: 14.5px;
    --card-min: 250px;
    --folder-min: 260px;
  }
  .size-l {
    --fs: 14px;
    --line-h: 34px;
    --row-h: 50px;
    --card-pad: 16px 18px;
    --name-fs: 16px;
    --card-min: 310px;
    --folder-min: 320px;
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-6);
  }
  .title {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: var(--space-4) var(--space-8);
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 700;
  }
  h1.picked {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .crumb {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
  }
  .crumb:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .slash {
    color: var(--text-faint);
    font-weight: 400;
  }
  .gdot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
  .stats {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .stat {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    font-size: 12.5px;
    color: var(--text-soft);
  }
  .stat.ok {
    color: var(--success);
    background: var(--success-pill);
    border-color: transparent;
  }
  .stat.warn {
    color: var(--accent-fg);
    border-color: var(--warning-border);
  }
  .stat.faint {
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .live-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
  }
  .spinner {
    width: 10px;
    height: 10px;
    border: 1.5px solid var(--line-strong);
    border-top-color: var(--accent-fg);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .toolbar {
    position: sticky;
    top: calc(var(--space-12) * -1);
    z-index: 5;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-4);
    padding: var(--space-4) 0;
    background: var(--bg-app);
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    /* Starts small and grows into spare room, so the toolbar stays on one line
       (a wrapping row wraps before it shrinks anything). */
    flex: 1 1 180px;
    min-width: 140px;
    max-width: 300px;
    height: 34px;
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
    font-size: 14px;
    user-select: text;
  }
  .spacer,
  .grow {
    flex: 1;
  }
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-button-sm);
  }
  .seg button {
    height: 26px;
    min-width: 28px;
    padding: 0 var(--space-4);
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }
  .seg button:hover:not(.on) {
    background: var(--btn-hover);
    color: var(--text);
  }
  .seg button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
    box-shadow: inset 0 0 0 1px var(--btn-border);
  }
  .view-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 32px;
    padding: 0 var(--space-5);
    border: 1px solid var(--btn-border);
    border-radius: var(--radius-button-sm);
    background: var(--btn-bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  .view-btn:hover {
    background: var(--btn-hover);
    border-color: var(--btn-hover-border);
  }
  .view-btn:active {
    background: var(--btn-press);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-4) var(--space-6);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
    background: var(--bg-panel);
    font-size: 13px;
    color: var(--text-soft);
  }
  .banner .warn {
    display: block;
    color: var(--error-text);
  }
  .x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }

  .grid {
    display: grid;
    gap: var(--space-5);
    align-items: start;
  }
  .cards-grid {
    grid-template-columns: repeat(auto-fill, minmax(var(--card-min), 1fr));
  }
  .folders-grid {
    grid-template-columns: repeat(auto-fill, minmax(var(--folder-min), 1fr));
  }
  .folder {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4) var(--space-3) var(--space-4);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    overflow: hidden;
  }
  .folder::before {
    content: "";
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: var(--bar);
  }
  .folder.lifted {
    opacity: 0.45;
  }
  .folder.drop {
    border-color: var(--accent-fg);
    background: var(--accent-panel);
  }
  .slot {
    min-width: 0;
  }
  .slot > :global(.item) {
    height: 100%;
  }
  .folder-list {
    display: flex;
    flex-direction: column;
  }
  .more {
    align-self: flex-start;
    margin: var(--space-2) var(--space-3) 0;
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .empty {
    margin: 0;
    padding: var(--space-4) var(--space-3);
    font-size: 12px;
    color: var(--text-faint);
  }
  .empty.wide {
    grid-column: 1 / -1;
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .rows {
    display: grid;
    column-gap: var(--space-6);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    overflow: hidden;
  }
  .rows-head {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: center;
    height: 30px;
    padding: 0 var(--space-3) 0 var(--space-5);
    border-bottom: 1px solid var(--line);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .favs {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .favs-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    font-size: 13.5px;
    font-weight: 600;
    color: var(--accent-fg);
  }
  .favs-row {
    display: flex;
    gap: var(--space-5);
    overflow-x: auto;
    padding-bottom: var(--space-2);
  }
  .fav-card {
    flex: 0 0 var(--card-min);
  }

  .none {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    color: var(--text-muted);
  }
  .none p {
    margin: 0;
  }
  .skel-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 74px;
    justify-content: center;
    padding: 0 var(--space-7);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    animation: dd-fade 300ms var(--ease) both;
  }
  .skel {
    display: block;
    height: 11px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--bg-hover) 0%, var(--line) 50%, var(--bg-hover) 100%);
    background-size: 200% 100%;
    animation: shimmer 1.4s ease-in-out infinite;
  }
  .skel.thin {
    height: 8px;
    opacity: 0.7;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .skel,
    .spinner {
      animation: none;
    }
  }

  /* View menu check boxes. */
  :global([role="menuitemcheckbox"]) {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    height: 32px;
    padding: 0 var(--space-5);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  :global([role="menuitemcheckbox"]:hover),
  :global([role="menuitemcheckbox"]:focus-visible) {
    background: var(--bg-hover);
    outline: none;
  }
  .box {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border: 1.5px solid var(--line-strong);
    border-radius: 4px;
    color: var(--accent-ink);
  }
  .box.on {
    background: var(--accent);
    border-color: var(--accent);
  }
</style>

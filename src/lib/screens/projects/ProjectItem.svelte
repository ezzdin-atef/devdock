<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { atCursor, belowButton, type MenuAnchor } from "$lib/components/Menu.svelte";
  import { drag } from "$lib/drag.svelte";
  import { relativeTime, shortPath } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { KIND_NAME, type Project } from "$lib/types";
  import type { Details } from "$lib/viewPrefs.svelte";
  import GitBadge from "./GitBadge.svelte";
  import RowMenu from "./RowMenu.svelte";

  // One project on the Projects page, in any layout: a card, a compact row, or a
  // line inside a folder card. Shows only the details the user turned on.
  let {
    project,
    variant,
    details,
    groupId,
    slot = true,
  }: {
    project: Project;
    variant: "card" | "row" | "line";
    details: Details;
    /** The group section it's shown in, when sections are groups ("-" = Ungrouped). */
    groupId?: string;
    /** Can other projects be dropped next to this one? Off in the favorites row. */
    slot?: boolean;
  } = $props();
  const axis = $derived(variant === "card" ? "x" : "y");
  const mark = $derived(
    slot && drag.place?.id === project.id ? (drag.place.after ? "after" : "before") : undefined,
  );

  let menu = $state<MenuAnchor | null>(null);
  const running = $derived(runs.activeIn(project.id));
  const deps = $derived(
    project.deps && project.deps.state !== "installed"
      ? project.deps.state === "missing"
        ? "not installed"
        : "update packages"
      : null,
  );
  const changes = $derived(project.git?.state === "repo" ? project.git.data.changedFiles : 0);

  function onPointerDown(e: PointerEvent) {
    if ((e.target as HTMLElement).closest(".kebab")) return;
    drag.begin(e, { kind: "projects", ids: [project.id] }, project.name);
  }

  /** One short status for a line in a folder card: the most useful thing first. */
  const line = $derived.by(() => {
    if (details.status && running) return { text: running > 1 ? `${running} running` : "running", tone: "ok" };
    if (project.missing) return { text: "missing", tone: "bad" };
    if (details.packages && deps) return { text: deps, tone: "warn" };
    if (details.git && changes) return { text: `${changes} changed`, tone: "muted" };
    if (details.opened && project.lastOpened) return { text: relativeTime(project.lastOpened, runs.now), tone: "muted" };
    if (details.kind) return { text: KIND_NAME[project.kind], tone: "muted" };
    return null;
  });
</script>

<div
  class="item {variant}"
  class:missing={project.missing}
  class:dragged={drag.has(project.id)}
  data-project-slot={slot ? project.id : undefined}
  data-in-group={groupId}
  data-axis={axis}
  data-slot={mark}
  class:menu-open={menu !== null}
  title={project.path}
  role="presentation"
  onpointerdown={onPointerDown}
  oncontextmenu={(e) => {
    e.preventDefault();
    menu = atCursor(e);
  }}
>
  <button class="hit" aria-label="Open {project.name}" onclick={() => app.select(project.id)}></button>

  {#if variant === "line"}
    <span class="name">{project.name}</span>
    {#if project.favorite}<span class="fav"><Icon name="star" size={11} filled /></span>{/if}
    <span class="spacer"></span>
    {#if line}<span class="st {line.tone}">{line.text}</span>{/if}
  {:else if variant === "row"}
    <span class="name-cell">
      {#if details.status && running}<span class="live-dot" title="{running} running"></span>{/if}
      <span class="name">{project.name}</span>
      {#if project.favorite}<span class="fav"><Icon name="star" size={11} filled /></span>{/if}
      {#if project.isNew}<span class="new" title="New from the last scan"></span>{/if}
    </span>
    {#if details.git}
      <span class="cell git">
        {#if project.missing}<span class="bad">folder not found</span>{:else if project.git == null}<span
            class="skel"
          ></span>{:else}<GitBadge git={project.git} />{/if}
      </span>
    {/if}
    {#if details.packages}<span class="cell">{#if deps}<span class="pill warn">{deps}</span>{/if}</span>{/if}
    {#if details.kind}<span class="cell muted">{KIND_NAME[project.kind]}</span>{/if}
    {#if details.opened}
      <span class="cell muted">{project.lastOpened ? relativeTime(project.lastOpened, runs.now) : "never"}</span>
    {/if}
    {#if details.path}<span class="cell path mono">{shortPath(project.path, 3)}</span>{/if}
    <button class="kebab" aria-label="Actions for {project.name}" onclick={(e) => (menu = belowButton(e))}>
      <Icon name="kebab" size={14} />
    </button>
  {:else}
    <div class="top">
      <span class="name">{project.name}</span>
      {#if project.favorite}<span class="fav"><Icon name="star" size={12} filled /></span>{/if}
      {#if project.isNew}<span class="new" title="New from the last scan"></span>{/if}
      <span class="spacer"></span>
      <button class="kebab" aria-label="Actions for {project.name}" onclick={(e) => (menu = belowButton(e))}>
        <Icon name="kebab" size={14} />
      </button>
    </div>
    {#if details.git}
      <div class="git">
        {#if project.missing}<span class="bad">Folder not found</span>{:else if project.git == null}<span
            class="skel"
          ></span>{:else if project.git.state === "noRepo"}<span class="muted">no git</span>{:else}<GitBadge
            git={project.git}
          />{/if}
      </div>
    {/if}
    {#if details.path}<div class="path mono">{shortPath(project.path, 3)}</div>{/if}
    <div class="chips">
      {#if details.status && running}
        <span class="pill ok"><span class="live-dot"></span>{running > 1 ? `${running} running` : "running"}</span>
      {/if}
      {#if details.packages && deps}<span class="pill warn">{deps}</span>{/if}
      {#if details.kind}<span class="pill">{KIND_NAME[project.kind]}</span>{/if}
      {#if details.opened && project.lastOpened}
        <span class="when">{relativeTime(project.lastOpened, runs.now)}</span>
      {/if}
    </div>
  {/if}
</div>

{#if menu}
  <RowMenu {project} at={menu} onclose={() => (menu = null)} />
{/if}

<style>
  .item {
    position: relative;
    min-width: 0;
    border-radius: var(--radius-button-sm);
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .hit {
    position: absolute;
    inset: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
    cursor: pointer;
  }
  .hit:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 1px;
  }
  /* Content sits above the hit area but lets clicks through, except buttons. */
  .item > :not(.hit),
  .top > :not(.kebab) {
    position: relative;
    pointer-events: none;
  }
  .kebab {
    position: relative;
    pointer-events: auto !important;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
    opacity: 0;
  }
  .item:hover .kebab,
  .item.menu-open .kebab,
  .kebab:focus-visible {
    opacity: 1;
  }
  .kebab:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .item.dragged {
    opacity: 0.35;
    filter: saturate(0.6);
  }
  /* Where a dragged project will land: a short line in the gap, not a box. */
  .item[data-slot]::after {
    content: "";
    position: absolute;
    z-index: 3;
    border-radius: 2px;
    background: var(--accent-fg);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent-fg) 22%, transparent);
    pointer-events: none;
    animation: dd-fade 90ms var(--ease);
  }
  .item[data-axis="x"][data-slot="before"]::after {
    left: -7px;
    top: 6px;
    bottom: 6px;
    width: 3px;
  }
  .item[data-axis="x"][data-slot="after"]::after {
    right: -7px;
    top: 6px;
    bottom: 6px;
    width: 3px;
  }
  .item[data-axis="y"][data-slot="before"]::after {
    top: -2px;
    left: 6px;
    right: 6px;
    height: 3px;
  }
  .item[data-axis="y"][data-slot="after"]::after {
    bottom: -2px;
    left: 6px;
    right: 6px;
    height: 3px;
  }
  .item.missing .name {
    color: var(--text-muted);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .fav {
    display: grid;
    color: var(--accent-fg);
  }
  .new {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent-fg);
  }
  .spacer {
    flex: 1;
  }
  .muted {
    color: var(--text-muted);
  }
  .bad {
    color: var(--error-text);
  }
  .live-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-pill);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 1px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-pill);
    font-size: 11.5px;
    color: var(--text-soft);
    white-space: nowrap;
  }
  .pill.ok {
    color: var(--success);
    background: var(--success-pill);
    border-color: transparent;
  }
  .pill.ok .live-dot {
    width: 6px;
    height: 6px;
    box-shadow: none;
  }
  .pill.warn {
    color: var(--accent-fg);
    border-color: var(--warning-border);
  }
  .skel {
    display: inline-block;
    width: 90px;
    height: 8px;
    border-radius: 5px;
    background: linear-gradient(90deg, var(--bg-hover), var(--line), var(--bg-hover));
    background-size: 200% 100%;
    animation: shimmer 1.4s ease-in-out infinite;
  }
  @keyframes shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -100% 0;
    }
  }

  /* Line: inside a folder card. */
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--line-h, 30px);
    padding: 0 var(--space-3);
    font-size: var(--fs, 13px);
  }
  .line .name {
    font-weight: 500;
  }
  .line:hover,
  .line.menu-open {
    background: var(--bg-hover);
  }
  .st {
    flex-shrink: 0;
    font-size: 11.5px;
    white-space: nowrap;
  }
  .st.ok {
    color: var(--success);
  }
  .st.warn {
    color: var(--accent-fg);
  }
  .st.bad {
    color: var(--error-text);
  }
  .st.muted {
    color: var(--text-faint);
  }

  /* Card: one per project. */
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--card-pad, 12px 14px);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    font-size: var(--fs, 13.5px);
  }
  .card:hover,
  .card.menu-open {
    border-color: var(--line-strong);
    box-shadow: 0 2px 10px rgb(0 0 0 / 0.06);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }
  .card .name {
    font-size: var(--name-fs, 14.5px);
  }
  .git {
    min-height: 16px;
    font-size: 12px;
  }
  .path {
    font-size: 11.5px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .chips:empty {
    display: none;
  }
  .when {
    font-size: 11.5px;
    color: var(--text-faint);
  }

  /* Row: compact, columns line up across the page. */
  .row {
    display: grid;
    grid-template-columns: subgrid;
    grid-column: 1 / -1;
    align-items: center;
    min-height: var(--row-h, 40px);
    padding: 0 var(--space-3) 0 var(--space-5);
    font-size: var(--fs, 13.5px);
    border-bottom: 1px solid var(--line);
    border-radius: 0;
  }
  .row:hover,
  .row.menu-open {
    background: var(--bg-hover);
  }
  .name-cell {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }
  .cell {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .cell.path {
    font-size: 11.5px;
    color: var(--text-faint);
  }
  .row .kebab {
    justify-self: end;
  }
</style>

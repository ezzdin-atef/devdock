<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { atCursor, belowButton, type MenuAnchor } from "$lib/components/Menu.svelte";
  import { drag } from "$lib/drag.svelte";
  import { runs } from "$lib/runs.svelte";
  import { colorOf, type Section } from "$lib/sections";
  import GroupMenu from "./GroupMenu.svelte";

  // A group / folder / type heading on the Projects page: collapse, name, counts,
  // and for the user's groups a menu, rename in place, and a drop target.
  let {
    section,
    collapsed = false,
    ontoggle,
    slot = true,
  }: {
    section: Section;
    collapsed?: boolean;
    ontoggle?: () => void;
    /** Is this heading itself a place to drop a dragged group? Off inside folder cards (the card is). */
    slot?: boolean;
  } = $props();

  const group = $derived(app.lib?.groups.find((g) => g.id === section.groupId));
  const live = $derived(section.projects.reduce((n, p) => n + runs.activeIn(p.id), 0));
  let menu = $state<MenuAnchor | null>(null);

  let name = $state("");
  const renaming = $derived(!!group && app.renamingGroup === group.id);
  $effect(() => {
    if (renaming) name = untrack(() => group?.name ?? "");
  });
  function commit() {
    if (!group || app.renamingGroup !== group.id) return;
    const next = name.trim();
    app.renamingGroup = null;
    if (next && next !== group.name) app.updateGroup(group.id, { name: next });
  }
  function focusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<div
  class="head"
  class:drop={drag.active && section.groupId !== undefined && drag.over === section.groupId}
  class:lifted={!!group && drag.group === group.id}
  data-drop-group={section.groupId}
  data-group-slot={slot ? group?.id : undefined}
  data-axis="y"
  data-slot={slot && group && drag.slot?.id === group.id ? (drag.slot.after ? "after" : "before") : undefined}
  title={section.title}
  role="presentation"
  oncontextmenu={(e) => {
    if (group) {
      e.preventDefault();
      menu = atCursor(e);
    }
  }}
>
  {#if renaming}
    <span class="dot" style:background={colorOf(group?.color)}></span>
    <input
      class="rename"
      bind:value={name}
      use:focusSelect
      aria-label="Group name"
      spellcheck="false"
      onblur={commit}
      onkeydown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape") app.renamingGroup = null;
      }}
    />
  {:else}
    <button
      class="toggle"
      aria-expanded={ontoggle ? !collapsed : undefined}
      onclick={() => ontoggle?.()}
      ondblclick={() => group && (app.renamingGroup = group.id)}
      onpointerdown={(e) => group && drag.begin(e, { kind: "group", id: group.id }, group.name, group.color)}
    >
      {#if ontoggle}<span class="chev" class:closed={collapsed}><Icon name="chevron" size={12} /></span>{/if}
      {#if section.color}
        <span class="dot" style:background={colorOf(section.color)}></span>
      {:else if section.key.startsWith("folder:")}
        <span class="ficon"><Icon name="folder" size={14} /></span>
      {/if}
      <span class="name">{section.label}</span>
      <span class="count mono">{section.projects.length}</span>
      {#if live}<span class="live mono" title="{live} running"><span class="live-dot"></span>{live}</span>{/if}
    </button>
  {/if}
  {#if group}
    <button
      class="kebab"
      aria-label="Actions for group {group.name}"
      title="Rename, colour, delete"
      onclick={(e) => (menu = belowButton(e))}
    >
      <Icon name="kebab" size={13} />
    </button>
  {/if}
</div>

{#if menu && group}
  <GroupMenu {group} at={menu} onclose={() => (menu = null)} />
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 32px;
    min-width: 0;
    border: 1px dashed transparent;
    border-radius: var(--radius-button-sm);
  }
  .head.lifted {
    opacity: 0.45;
  }
  .head.drop {
    border-color: var(--accent-fg);
    background: var(--accent-panel);
  }
  .toggle {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 30px;
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }
  .toggle:hover {
    background: var(--bg-hover);
  }
  .chev {
    display: grid;
    color: var(--icon);
    transition: transform var(--t-fast) var(--ease);
  }
  .chev.closed {
    transform: rotate(-90deg);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .ficon {
    display: grid;
    color: var(--icon);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-faint);
  }
  .live {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 400;
    color: var(--success);
  }
  .live-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--success);
  }
  .rename {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--accent-fg);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    font-weight: 600;
    outline: none;
    user-select: text;
  }
  .kebab {
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
  .head:hover .kebab,
  .kebab:focus-visible {
    opacity: 1;
  }
  .kebab:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
</style>

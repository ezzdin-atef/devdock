<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { NEW_GROUP, drag } from "$lib/drag.svelte";
  import { colorOf } from "$lib/sections";
  import { UNGROUPED } from "$lib/types";
  import Icon from "./Icon.svelte";

  // What follows the pointer while dragging: the project (or group) and, under it,
  // where it will go if dropped now.
  const target = $derived.by(() => {
    if (drag.payload?.kind === "group") {
      if (!drag.slot) return null;
      const name = app.lib?.groups.find((g) => g.id === drag.slot?.id)?.name ?? "";
      return `${drag.slot.after ? "after" : "before"} ${name}`;
    }
    if (drag.place) {
      const near = app.lib?.projects.find((p) => p.id === drag.place?.id)?.name ?? "";
      const side = `${drag.place.after ? "after" : "before"} ${near}`;
      const g = drag.place.group;
      const moving = drag.ids.map((id) => app.lib?.projects.find((p) => p.id === id)?.group ?? UNGROUPED);
      const target = g === undefined ? null : g;
      if (target && moving.some((m) => m !== target)) {
        const name = target === UNGROUPED ? "Ungrouped" : app.lib?.groups.find((x) => x.id === target)?.name;
        return `${side}, in ${name}`;
      }
      return side;
    }
    if (drag.over === NEW_GROUP) return "a new group";
    if (drag.over === UNGROUPED) return "Ungrouped";
    return app.lib?.groups.find((g) => g.id === drag.over)?.name ?? null;
  });
  const count = $derived(drag.ids.length);
</script>

{#if drag.active}
  <div class="ghost" style="transform: translate({drag.x + 14}px, {drag.y + 12}px)" aria-hidden="true">
    <div class="card" class:ok={!!target}>
      {#if drag.payload?.kind === "group"}
        <span class="dot" style:background={colorOf(drag.color)}></span>
      {:else}
        <Icon name="folder" size={13} />
      {/if}
      <span class="label">{drag.label}</span>
      {#if count > 1}<span class="count mono">{count}</span>{/if}
    </div>
    <div class="where" class:ok={!!target}>
      {#if target}
        <Icon name="check" size={11} />
        {drag.payload?.kind === "group" || drag.place ? "Move" : "Move to"} <strong>{target}</strong>
      {:else}
        {drag.payload?.kind === "group" ? "Drop between groups" : "Drop between projects or on a group"} · Esc to cancel
      {/if}
    </div>
  </div>
{/if}

<style>
  .ghost {
    position: fixed;
    left: 0;
    top: 0;
    z-index: 1300;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    pointer-events: none;
    will-change: transform;
  }
  .card {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    max-width: 280px;
    padding: var(--space-3) var(--space-5);
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-button-sm);
    box-shadow: var(--shadow-menu);
    color: var(--icon);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    transform: rotate(-1.5deg);
    transition:
      border-color 120ms var(--ease),
      transform 120ms var(--ease);
  }
  .card.ok {
    border-color: var(--accent-fg);
    transform: rotate(0deg) scale(1.02);
  }
  .label {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .count {
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--accent);
    color: var(--accent-ink);
    font-size: 11px;
  }
  .where {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border-radius: var(--radius-pill);
    background: var(--bg-overlay);
    box-shadow: var(--shadow-menu);
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .where.ok {
    color: var(--accent-fg);
  }
</style>

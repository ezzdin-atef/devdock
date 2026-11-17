<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Menu, { type MenuAnchor } from "$lib/components/Menu.svelte";
  import ColorPicker from "$lib/components/ColorPicker.svelte";
  import { GROUP_HEX, colorOf } from "$lib/sections";
  import { GROUP_COLORS, type Group } from "$lib/types";

  // Group actions: rename, colour, order, delete (projects stay, ungrouped).
  let { group, at, onclose }: { group: Group; at: MenuAnchor; onclose: () => void } = $props();
  const groups = $derived(app.lib?.groups ?? []);
  const index = $derived(groups.findIndex((g) => g.id === group.id));
  const count = $derived(app.lib?.counts.groups[group.id] ?? 0);

  const custom = $derived(group.color.startsWith("#"));
  let picking = $state(false);

  function act(fn: () => void) {
    onclose();
    fn();
  }
</script>

<Menu {at} {onclose} label="Group {group.name}" width={236}>
  <button role="menuitem" onclick={() => act(() => (app.renamingGroup = group.id))}>Rename</button>
  <div class="menu-label">Colour</div>
  <div class="swatches" role="group" aria-label="Colour">
    {#each GROUP_COLORS as c (c)}
      <button
        class="swatch"
        class:on={group.color === c}
        style:background={GROUP_HEX[c]}
        aria-label={c}
        title={c}
        onclick={() => act(() => app.updateGroup(group.id, { color: c }))}
      ></button>
    {/each}
    <button
      class="swatch custom"
      class:on={custom}
      style:background={custom ? colorOf(group.color) : undefined}
      aria-label="Custom colour"
      aria-expanded={picking}
      title="Custom colour"
      onclick={() => (picking = !picking)}
    >{#if !custom}+{/if}</button>
  </div>
  {#if picking}
    <ColorPicker
      value={colorOf(group.color).startsWith("#") ? colorOf(group.color) : "#3b82f6"}
      onpick={(hex) => app.updateGroup(group.id, { color: hex })}
    />
  {/if}
  <hr />
  <button role="menuitem" disabled={index <= 0} onclick={() => act(() => app.moveGroup(group.id, -1))}>
    <span class="up"><Icon name="chevron" size={12} /></span> Move up
  </button>
  <button
    role="menuitem"
    disabled={index < 0 || index >= groups.length - 1}
    onclick={() => act(() => app.moveGroup(group.id, 1))}
  >
    <span class="down"><Icon name="chevron" size={12} /></span> Move down
  </button>
  <hr />
  <button role="menuitem" class="danger" onclick={() => act(() => app.deleteGroup(group.id))}>Delete group</button>
  <p class="note">
    {count ? `Its ${count === 1 ? "project stays" : `${count} projects stay`}, ungrouped.` : "It has no projects."}
  </p>
</Menu>

<style>
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: var(--space-2) var(--space-5) var(--space-3);
  }
  .swatch {
    width: 20px;
    height: 20px;
    border: 2px solid transparent;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  .swatch.on {
    border-color: var(--text);
  }
  .swatch.custom {
    display: grid;
    place-items: center;
    background: conic-gradient(#f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    text-shadow: 0 0 2px rgb(0 0 0 / 0.6);
  }
  .swatch:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 1px;
  }
  /* The chevron icon points down; turn it over for "up". */
  .up,
  .down {
    display: grid;
  }
  .up :global(svg) {
    transform: rotate(180deg);
  }
</style>

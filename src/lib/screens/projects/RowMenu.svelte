<script lang="ts" module>
  export type { MenuAnchor } from "$lib/components/Menu.svelte";
</script>

<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Menu, { type MenuAnchor } from "$lib/components/Menu.svelte";
  import { colorOf } from "$lib/sections";
  import type { Project } from "$lib/types";

  // Project actions: open, rename, move to a group, copy path, favorite, remove.
  let { project, at, onclose }: { project: Project; at: MenuAnchor; onclose: () => void } = $props();
  const groups = $derived(app.lib?.groups ?? []);

  function act(fn: () => void) {
    onclose();
    fn();
  }
</script>

<Menu {at} {onclose} label="Actions for {project.name}" width={232}>
  <button role="menuitem" onclick={() => act(() => app.select(project.id))}>Open</button>
  <button role="menuitem" onclick={() => act(() => app.startRename(project.id))}>Rename</button>
  <button role="menuitem" onclick={() => act(() => app.copyPath(project.path))}>Copy path</button>
  <button role="menuitem" onclick={() => act(() => app.toggleFavorite(project.id, !project.favorite))}>
    {project.favorite ? "Remove from favorites" : "Add to favorites"}
  </button>
  {#if !project.missing}
    <button role="menuitem" onclick={() => act(() => app.openInExplorer(project.id))}>Show in Explorer</button>
  {:else}
    <button role="menuitem" onclick={() => act(() => app.locate(project.id))}>Locate…</button>
  {/if}
  <hr />
  <div class="menu-label">Move to group</div>
  {#each groups as g (g.id)}
    <button
      role="menuitem"
      disabled={project.group === g.id}
      onclick={() => act(() => app.moveToGroup([project.id], g.id))}
    >
      <span class="dot" style:background={colorOf(g.color)}></span>
      <span class="grow">{g.name}</span>
      {#if project.group === g.id}<Icon name="check" size={12} />{/if}
    </button>
  {/each}
  <button role="menuitem" onclick={() => act(() => app.createGroup("New group", [project.id]))}>
    <Icon name="plus" size={12} /> New group…
  </button>
  {#if project.group}
    <button role="menuitem" onclick={() => act(() => app.moveToGroup([project.id], null))}>Remove from group</button>
  {/if}
  <hr />
  <button role="menuitem" class="danger" onclick={() => act(() => app.remove(project.id))}>Remove from DevDock</button>
  <p class="note">The folder on disk is not touched.</p>
</Menu>

<style>
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .grow {
    flex: 1;
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import type { Project } from "$lib/types";

  // DESIGN.md "Context menu": destructive item last, below a divider, with a note.
  let { project, onclose }: { project: Project; onclose: () => void } = $props();
  let menu: HTMLDivElement;

  onMount(() => {
    menu.querySelector<HTMLButtonElement>("button")?.focus();
    const onPointer = (e: PointerEvent) => {
      if (!menu.contains(e.target as Node)) onclose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onclose();
    };
    // Defer so the click that opened the menu doesn't close it.
    setTimeout(() => window.addEventListener("pointerdown", onPointer));
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  });

  function act(fn: () => void) {
    onclose();
    fn();
  }
</script>

<div class="menu" role="menu" bind:this={menu}>
  <button role="menuitem" onclick={() => act(() => app.startRename(project.id))}>Rename</button>
  <button role="menuitem" onclick={() => act(() => app.copyPath(project.path))}>Copy path</button>
  <button
    role="menuitem"
    onclick={() => act(() => app.toggleFavorite(project.id, !project.favorite))}
  >
    {project.favorite ? "Remove from favorites" : "Add to favorites"}
  </button>
  {#if project.missing}
    <button role="menuitem" onclick={() => act(() => app.locate(project.id))}>Locate…</button>
  {/if}
  <hr />
  <button role="menuitem" class="danger" onclick={() => act(() => app.remove(project.id))}>
    Remove from DevDock
  </button>
  <p class="note">The folder on disk is not touched.</p>
</div>

<style>
  .menu {
    position: absolute;
    right: var(--space-4);
    top: calc(100% - var(--space-2));
    z-index: 20;
    width: 220px;
    padding: var(--space-3);
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: 10px;
    box-shadow: var(--shadow-menu);
    animation: dd-overlay 140ms var(--ease);
    transform-origin: top right;
  }
  button {
    display: block;
    width: 100%;
    height: 34px;
    padding: 0 var(--space-5);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  button:hover,
  button:focus-visible {
    background: var(--bg-hover);
    outline: none;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--line);
    margin: var(--space-3) 0;
  }
  .danger {
    color: var(--error-text);
  }
  .note {
    margin: var(--space-1) var(--space-5) var(--space-2);
    font-size: 11.5px;
    color: var(--text-muted);
  }
</style>

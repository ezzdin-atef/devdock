<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import type { Project } from "$lib/types";

  // Inline rename: Enter or blur saves, Esc cancels (T2.9).
  let { project, large = false }: { project: Project; large?: boolean } = $props();
  let input: HTMLInputElement;
  let done = false;

  onMount(() => {
    input.focus();
    input.select();
  });

  function finish(name: string | null) {
    if (done) return; // Enter then blur would otherwise save twice
    done = true;
    app.finishRename(project.id, name === project.name ? null : name);
  }
</script>

<input
  bind:this={input}
  class="rename"
  class:large
  value={project.name}
  aria-label="Project name"
  spellcheck="false"
  onkeydown={(e) => {
    if (e.key === "Enter") finish(e.currentTarget.value);
    if (e.key === "Escape") finish(null);
  }}
  onblur={(e) => finish(e.currentTarget.value)}
  onclick={(e) => e.stopPropagation()}
/>

<style>
  .rename {
    min-width: 0;
    flex: 1;
    height: 26px;
    padding: 0 var(--space-4);
    background: var(--bg-panel);
    border: 1px solid var(--accent-fg);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    outline: none;
    user-select: text;
  }
  .large {
    height: 40px;
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
</style>

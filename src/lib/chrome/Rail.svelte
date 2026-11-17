<script lang="ts">
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import type { View } from "$lib/types";

  let { current = $bindable() }: { current: View } = $props();

  const MAIN: { view: View; label: string; icon: IconName }[] = [
    { view: "projects", label: "Projects", icon: "folder" },
    { view: "spaces", label: "Spaces", icon: "layers" },
    { view: "running", label: "Running", icon: "play" },
    { view: "ports", label: "Ports", icon: "plug" },
    { view: "agents", label: "Agents", icon: "terminal" },
  ];
</script>

{#snippet item(view: View, label: string, icon: IconName)}
  <button
    class="item"
    class:active={current === view}
    aria-current={current === view ? "page" : undefined}
    title={label}
    onclick={() => (current = view)}
  >
    <Icon name={icon} size={18} />
    <span class="label">{label}</span>
  </button>
{/snippet}

<nav class="rail" aria-label="Screens">
  {#each MAIN as entry (entry.view)}
    {@render item(entry.view, entry.label, entry.icon)}
  {/each}
  <span class="spacer"></span>
  {@render item("settings", "Settings", "sliders")}
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    width: var(--rail-w);
    padding: var(--space-4) 0;
    background: var(--bg-chrome);
    border-right: 1px solid var(--line);
  }
  .spacer {
    flex: 1;
  }
  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    width: 60px;
    height: 56px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .item:hover {
    background: var(--bg-hover);
  }
  .item.active {
    background: var(--accent-tint);
    color: var(--accent-fg);
    font-weight: 600;
  }
  .label {
    font-size: 10.5px;
  }
</style>

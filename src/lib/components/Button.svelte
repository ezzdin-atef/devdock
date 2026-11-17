<script lang="ts">
  import type { Snippet } from "svelte";

  // DESIGN.md "Buttons". At most one primary per view.
  type Variant = "primary" | "secondary" | "ghost";
  let {
    variant = "secondary",
    onclick,
    disabled = false,
    title,
    children,
  }: {
    variant?: Variant;
    onclick?: () => void;
    disabled?: boolean;
    title?: string;
    children: Snippet;
  } = $props();
</script>

<button class="btn {variant}" {onclick} {disabled} {title} type="button">
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    height: 36px;
    padding: 0 var(--space-8);
    border-radius: var(--radius-button);
    border: 1px solid transparent;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      filter var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease);
  }
  .btn:hover:not(:disabled) {
    filter: brightness(1.15);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .primary {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
  }
  .secondary {
    background: var(--bg-raised);
    border-color: var(--line);
    color: var(--text);
  }
  .ghost {
    background: transparent;
    color: var(--accent-fg);
    font-weight: 600;
    padding: 0 var(--space-4);
  }
</style>

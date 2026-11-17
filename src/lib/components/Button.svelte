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
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease),
      transform 60ms var(--ease);
  }
  .btn:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 2px;
  }
  .btn:active:not(:disabled) {
    transform: translateY(1px);
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
  .primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .primary:active:not(:disabled) {
    background: var(--accent-press);
  }

  .secondary {
    background: var(--btn-bg);
    border-color: var(--btn-border);
    color: var(--text);
    box-shadow: 0 1px 1px rgb(0 0 0 / 0.04);
  }
  .secondary:hover:not(:disabled) {
    background: var(--btn-hover);
    border-color: var(--btn-hover-border);
  }
  .secondary:active:not(:disabled) {
    background: var(--btn-press);
    box-shadow: none;
  }

  .ghost {
    background: transparent;
    color: var(--accent-fg);
    font-weight: 600;
    padding: 0 var(--space-4);
  }
  .ghost:hover:not(:disabled) {
    background: var(--accent-tint);
  }
  .ghost:active:not(:disabled) {
    background: var(--accent-active);
  }
</style>

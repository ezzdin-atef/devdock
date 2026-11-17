<script lang="ts">
  import type { ErrorDto } from "$lib/types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  // DESIGN.md "Banners": error = what happened + how to fix + Dismiss.
  // `warning` uses the conflict palette for notices that aren't failures.
  let {
    error,
    ondismiss,
    tone = "error",
  }: { error: ErrorDto; ondismiss: () => void; tone?: "error" | "warning" } = $props();
</script>

<div class="banner {tone}" role={tone === "error" ? "alert" : "status"}>
  <span class="icon"><Icon name="alert" /></span>
  <div class="body">
    <div class="message selectable">{error.message}</div>
    {#if error.fix}
      <div class="fix selectable">{error.fix}</div>
    {/if}
  </div>
  <Button variant="secondary" onclick={ondismiss}>Dismiss</Button>
</div>

<style>
  .banner {
    display: flex;
    align-items: flex-start;
    gap: var(--space-6);
    padding: var(--space-7) var(--space-8);
    border-radius: var(--radius-panel);
    border: 1px solid;
  }
  .error {
    background: var(--error-panel);
    border-color: var(--error-border);
  }
  .warning {
    background: var(--warning-panel);
    border-color: var(--warning-border);
  }
  .icon {
    padding-top: 2px;
  }
  .error .icon {
    color: var(--error);
  }
  .warning .icon {
    color: var(--accent-fg);
  }
  .body {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .message {
    font-weight: 600;
    font-size: 14.5px;
  }
  .error .message {
    color: var(--error-text);
  }
  .warning .message {
    color: var(--text);
  }
  .fix {
    color: var(--text-soft);
    margin-top: var(--space-2);
  }
</style>

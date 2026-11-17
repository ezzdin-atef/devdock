<script lang="ts">
  import { agents } from "$lib/agents.svelte";
  import Icon from "./Icon.svelte";
</script>

{#if agents.toast}
  <div class="toast {agents.toast.tone}" role="status">
    <Icon name={agents.toast.tone === "ok" ? "check" : "alert"} size={14} />
    <span>{agents.toast.text}</span>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: var(--space-12);
    bottom: calc(var(--status-strip-h) + var(--space-8));
    z-index: 40;
    display: flex;
    align-items: center;
    gap: var(--space-5);
    max-width: 420px;
    padding: var(--space-6) var(--space-8);
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-row);
    box-shadow: var(--shadow-menu);
    font-size: 13px;
    animation: dd-enter 220ms var(--ease);
  }
  .ok :global(svg) {
    color: var(--success);
  }
  .bad {
    border-color: var(--error-border);
  }
  .bad :global(svg) {
    color: var(--error);
  }
</style>

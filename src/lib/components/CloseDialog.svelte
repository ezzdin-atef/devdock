<script lang="ts">
  import { onMount } from "svelte";
  import { runs } from "$lib/runs.svelte";
  import Button from "./Button.svelte";

  // DESIGN.md "Dialog": eyebrow, title, one-line explanation, details, actions (T4.8).
  let cancelBtn: HTMLDivElement;

  onMount(() => {
    cancelBtn.querySelector("button")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") runs.closeRequested = null;
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="scrim">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="close-title">
    <div class="eyebrow">Processes running</div>
    <h2 id="close-title">Quit DevDock?</h2>
    <p>
      DevDock started {runs.active.length === 1 ? "this process" : "these processes"}. Quitting stops
      {runs.active.length === 1 ? "it" : "them"}, including any child processes.
    </p>
    <ul class="details">
      {#each runs.active as r (r.projectId + r.commandId)}
        <li>
          <span>{r.projectName}</span>
          <span class="mono">{r.commandLine}</span>
          {#if r.portHint}<span class="mono port">:{r.portHint}</span>{/if}
        </li>
      {/each}
    </ul>
    <div class="actions">
      <div bind:this={cancelBtn}>
        <Button variant="secondary" onclick={() => (runs.closeRequested = null)}>Cancel</Button>
      </div>
      <button class="confirm" onclick={() => runs.quit()}>Stop all and quit</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: var(--scrim);
  }
  .dialog {
    width: min(480px, calc(100vw - 48px));
    padding: 22px;
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-dialog);
    box-shadow: var(--shadow-dialog);
    animation: in 180ms var(--ease);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: scale(0.98);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dialog {
      animation: none;
    }
  }
  h2 {
    margin: var(--space-3) 0 var(--space-4);
    font-size: 19px;
    font-weight: 700;
  }
  p {
    margin: 0;
    color: var(--text-soft);
  }
  .details {
    list-style: none;
    margin: var(--space-8) 0;
    padding: var(--space-4) var(--space-6);
    background: var(--bg-app);
    border-radius: var(--radius-button);
  }
  .details li {
    display: flex;
    gap: var(--space-5);
    padding: var(--space-3) 0;
    font-size: 12.5px;
  }
  .details .mono {
    color: var(--text-muted);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .port {
    margin-left: auto;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-4);
  }
  .confirm {
    height: 36px;
    padding: 0 var(--space-8);
    border: 0;
    border-radius: var(--radius-button);
    background: var(--error);
    color: var(--error-ink);
    font-weight: 600;
    cursor: pointer;
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { relativeTime } from "$lib/format";
  import { ports } from "$lib/ports.svelte";
  import { runs } from "$lib/runs.svelte";
  import Button from "./Button.svelte";
  import ErrorBanner from "./ErrorBanner.svelte";

  // DESIGN.md "Dialog" + T5.6: everything about the process, then an explicit confirm.
  // DevDock-owned and system processes get no "End" button here.
  const row = $derived(ports.inspecting);
  const p = $derived(row?.process ?? null);
  let cancelWrap: HTMLDivElement | undefined = $state();

  onMount(() => {
    cancelWrap?.querySelector("button")?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") ports.closeDialog();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const started = $derived(
    p?.startTime
      ? `${new Date(p.startTime * 1000).toLocaleString("en-GB")} (${relativeTime(p.startTime * 1000, runs.now)})`
      : "unknown",
  );
</script>

{#if row}
  <div class="scrim">
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="end-title">
      <div class="eyebrow">
        {row.owner === "external" ? "External process" : row.owner === "system" ? "System process" : "Started by DevDock"}
      </div>
      <h2 id="end-title">
        {row.owner === "external" ? "End" : "Process"}
        {p?.name ?? `PID ${row.pid}`}{row.owner === "external" ? "?" : ""}
      </h2>
      <p>
        {#if row.owner === "external"}
          DevDock didn't start this process, so it won't stop it unless you confirm.
        {:else if row.owner === "system"}
          Windows or another account owns this process. DevDock never ends it.
        {:else}
          DevDock started this. Use Stop on the Ports or Running screen.
        {/if}
      </p>

      <dl class="details">
        <dt>Port</dt>
        <dd class="mono">{row.port} <span class="muted">on {row.addresses.join(", ")}</span></dd>
        <dt>PID</dt>
        <dd class="mono">{row.pid}</dd>
        <dt>Command</dt>
        <dd class="mono selectable">{p?.cmd ?? "unknown (Windows didn't allow reading it)"}</dd>
        <dt>Folder</dt>
        <dd class="mono selectable">{p?.cwd ?? "unknown"}</dd>
        <dt>Project</dt>
        <dd>{row.project?.name ?? "none matched"}</dd>
        <dt>Started</dt>
        <dd>{started}</dd>
        <dt>Children</dt>
        <dd class="mono">
          {#if row.children.length}
            {row.children.map((c) => `${c.name} (${c.pid})`).join(", ")}
          {:else}none{/if}
        </dd>
      </dl>

      {#if row.owner === "external" && row.children.length > 0}
        <label class="option">
          <input type="checkbox" bind:checked={ports.withChildren} />
          Also end its {row.children.length === 1 ? "child process" : `${row.children.length} child processes`}
        </label>
      {/if}

      {#if ports.dialogError}
        <div class="err">
          <ErrorBanner error={ports.dialogError} ondismiss={() => (ports.dialogError = null)} />
        </div>
      {/if}

      <div class="actions">
        <div bind:this={cancelWrap}>
          <Button variant="secondary" onclick={() => ports.closeDialog()}>
            {row.owner === "external" ? "Cancel" : "Close"}
          </Button>
        </div>
        {#if row.owner === "external"}
          <button class="confirm" disabled={ports.ending} onclick={() => ports.endConfirmed()}>
            {ports.ending ? "Ending…" : `End PID ${row.pid}`}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

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
    width: min(520px, calc(100vw - 48px));
    max-height: calc(100vh - 48px);
    overflow-y: auto;
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
    display: grid;
    grid-template-columns: 92px 1fr;
    gap: var(--space-3) var(--space-6);
    margin: var(--space-8) 0;
    padding: var(--space-6) var(--space-7);
    background: var(--bg-app);
    border-radius: var(--radius-button);
    font-size: 12.5px;
  }
  dt {
    color: var(--text-muted);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  dd.mono {
    font-size: 11.5px;
  }
  .muted {
    color: var(--text-muted);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin-bottom: var(--space-8);
    cursor: pointer;
  }
  .option input {
    accent-color: var(--error);
  }
  .err {
    margin-bottom: var(--space-8);
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
  .confirm:disabled {
    opacity: 0.6;
  }
</style>

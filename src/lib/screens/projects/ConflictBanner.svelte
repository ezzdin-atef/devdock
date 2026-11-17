<script lang="ts">
  import Button from "$lib/components/Button.svelte";
  import { ports } from "$lib/ports.svelte";
  import { runs } from "$lib/runs.svelte";
  import type { CommandId, Conflict, ProjectId } from "$lib/types";

  // DESIGN.md "Banners > Conflict": port tile, what's happening, then the safe option
  // first, inspect second, destructive last (T5.5).
  let {
    projectId,
    commandId,
    conflict,
  }: { projectId: ProjectId; commandId: CommandId; conflict: Conflict } = $props();

  const h = $derived(conflict.holder);
  const who = $derived(h.process?.name ?? `PID ${h.pid}`);
  const retry = () => runs.start(projectId, commandId);
</script>

<div class="banner" role="alert">
  <div class="tile mono" aria-hidden="true">{conflict.port}</div>
  <div class="body">
    <div class="title">Port {conflict.port} is already in use</div>
    <p>
      {#if h.owner === "devDock"}
        DevDock is already running something on it{h.project ? ` (${h.project.name})` : ""}.
      {:else if h.owner === "system"}
        {who} (PID {h.pid}) holds it. It's a protected process, so DevDock won't end it.
      {:else}
        {who} (PID {h.pid}){h.project ? ` from ${h.project.name}` : ""} is listening on it. DevDock didn't
        start this process, so it won't stop it unless you confirm.
      {/if}
      Nothing was started.
    </p>
    <div class="actions">
      {#if conflict.suggestedPort}
        <Button
          variant="primary"
          onclick={() => runs.start(projectId, commandId, { port: conflict.suggestedPort ?? undefined })}
        >
          Start on port {conflict.suggestedPort}
        </Button>
      {/if}
      <Button variant="secondary" onclick={() => ports.inspect(h, retry)}>Show process</Button>
      {#if h.owner === "devDock" && h.run}
        {@const run = h.run}
        <button
          class="destructive"
          onclick={async () => {
            await runs.stop(run.projectId, run.commandId);
            await runs.waitEnded(run.projectId, run.commandId);
            retry();
          }}>Stop it and start</button
        >
      {:else if h.owner === "external"}
        <button class="destructive" onclick={() => ports.inspect(h, retry)}>End PID {h.pid}…</button>
      {/if}
      <Button variant="ghost" onclick={() => runs.start(projectId, commandId, { force: true })}>
        Start anyway
      </Button>
      <Button variant="ghost" onclick={() => runs.dismissConflict(projectId, commandId)}>Dismiss</Button>
    </div>
    {#if conflict.suggestedPort}
      <p class="hint">
        "Start on port {conflict.suggestedPort}" sets <span class="mono">PORT={conflict.suggestedPort}</span>;
        it works if the server reads <span class="mono">PORT</span> (Next.js, Express and most Node servers do).
      </p>
    {/if}
  </div>
</div>

<style>
  .banner {
    display: flex;
    gap: var(--space-8);
    padding: var(--space-8);
    background: var(--warning-panel);
    border: 1px solid var(--warning-border);
    border-radius: var(--radius-panel);
  }
  .tile {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 64px;
    height: 64px;
    border: 1px solid var(--warning-border);
    border-radius: var(--radius-button);
    color: var(--accent-fg);
    font-size: 16px;
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 16px;
    font-weight: 600;
  }
  p {
    margin: var(--space-3) 0 0;
    color: var(--text-soft);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-4);
    margin-top: var(--space-6);
  }
  .destructive {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
  .destructive:hover {
    background: var(--error-pill);
  }
  .hint {
    font-size: 12.5px;
    color: var(--text-muted);
  }
</style>

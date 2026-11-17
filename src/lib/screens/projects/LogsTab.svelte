<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Console from "$lib/components/Console.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Select from "$lib/components/Select.svelte";
  import StatePill from "$lib/components/StatePill.svelte";
  import { duration } from "$lib/format";
  import { runKey, runs } from "$lib/runs.svelte";
  import { isActive, type Project, type RunInfo } from "$lib/types";

  // The log on its own: the whole height of the window for one command's output,
  // with the controls to run, stop, restart and clear it.
  let { project }: { project: Project } = $props();

  const projectId = $derived(project.id);
  $effect(() => {
    runs.loadCommands(projectId);
  });

  const list = $derived(runs.commandsByProject[project.id]);
  const commands = $derived(list?.commands ?? []);
  const runOf = (id: string): RunInfo | null => runs.runOf(project.id, id) ?? commands.find((c) => c.id === id)?.run ?? null;

  const selectedId = $derived(runs.selectedCommand[project.id] ?? commands[0]?.id ?? "");
  const selected = $derived(commands.find((c) => c.id === selectedId) ?? null);
  const run = $derived(selected ? runOf(selected.id) : null);
  const active = $derived(isActive(run?.state));
  const busy = $derived(selected ? runKey(project.id, selected.id) in runs.busy : false);

  function stateHint(id: string): string | undefined {
    const r = runOf(id);
    if (!r) return undefined;
    if (isActive(r.state)) return "running";
    return r.state === "failed" ? "failed" : "ended";
  }
</script>

<div class="logs">
  {#if project.missing}
    <p class="muted">The folder is missing, so nothing can run here.</p>
  {:else if !list}
    <p class="muted">Reading commands…</p>
  {:else if commands.length === 0}
    <p class="muted">
      No commands yet. Add one in the <button class="link" onclick={() => (app.tab = "run")}>Run</button> tab and its
      output shows up here.
    </p>
  {:else if selected}
    <div class="bar">
      <Select
        value={selected.id}
        mono
        width="200px"
        ariaLabel="Command"
        options={commands.map((c) => ({ value: c.id, label: c.name, hint: stateHint(c.id) }))}
        onchange={(id) => (runs.selectedCommand[project.id] = id)}
      />
      <StatePill {run} />
      <span class="cmd mono" title={run?.commandLine ?? selected.commandLine}>{run?.commandLine ?? selected.commandLine}</span>
      {#if run?.startedAt && active}
        <span class="up mono">up {duration(runs.now - run.startedAt)}</span>
      {/if}
      <span class="spacer"></span>
      {#if active}
        <Button
          variant="secondary"
          onclick={() => runs.restart(project.id, selected.id)}
          disabled={busy || run?.state === "stopping"}
        >
          <Icon name="refresh" size={14} /> Restart
        </Button>
        <button
          class="stop"
          onclick={() => runs.stop(project.id, selected.id)}
          disabled={busy || run?.state === "stopping"}>■ Stop</button
        >
      {:else}
        {#if run}
          <Button
            variant="ghost"
            title="Forget this run: its status, error and log"
            onclick={() => runs.forget(project.id, selected.id)}
            disabled={busy}
          >
            Clear run
          </Button>
        {/if}
        <Button
          variant={run?.state === "failed" ? "primary" : "secondary"}
          onclick={() => runs.start(project.id, selected.id)}
          disabled={busy}
        >
          <Icon name={run ? "refresh" : "play"} size={14} />
          {run ? "Run again" : "Run"}
        </Button>
      {/if}
    </div>

    {#if run?.state === "failed" && run.lastError}
      <div class="last-error mono selectable">{run.lastError}</div>
    {/if}

    <Console projectId={project.id} commandId={selected.id} live={active} />
  {/if}
</div>

<style>
  .logs {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  .muted {
    margin: 0;
    color: var(--text-muted);
  }
  .link {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    min-width: 0;
  }
  .bar > :global(*) {
    flex-shrink: 0;
  }
  .bar > .cmd {
    flex-shrink: 1;
  }
  .cmd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .up {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--success);
  }
  .spacer {
    flex: 1;
  }
  .stop {
    flex-shrink: 0;
    white-space: nowrap;
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
  .stop:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .last-error {
    padding: var(--space-5) var(--space-6);
    background: var(--error-row);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-row);
    color: var(--error-text);
    font-size: 12.5px;
    overflow-wrap: anywhere;
  }
  /* The console takes every pixel that's left. */
  .logs :global(.console-wrap) {
    flex: 1;
    min-height: 0;
  }
  .logs :global(.console) {
    flex: 1;
    height: auto;
    min-height: 200px;
  }
</style>

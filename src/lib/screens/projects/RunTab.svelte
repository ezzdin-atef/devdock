<script lang="ts">
  import Button from "$lib/components/Button.svelte";
  import Console from "$lib/components/Console.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import StatePill from "$lib/components/StatePill.svelte";
  import ConflictBanner from "./ConflictBanner.svelte";
  import { duration, relativeTime } from "$lib/format";
  import { runKey, runs } from "$lib/runs.svelte";
  import { isActive, type Project, type RunCommand, type RunInfo } from "$lib/types";

  let { project }: { project: Project } = $props();

  let adding = $state(false);
  let form = $state({ name: "", line: "", subdir: "" });

  // Keyed on the id, not the object: the project object is replaced on every list
  // refresh, and that alone shouldn't refetch the commands.
  const projectId = $derived(project.id);
  $effect(() => {
    runs.loadCommands(projectId);
  });

  const list = $derived(runs.commandsByProject[project.id]);
  const selectedId = $derived(runs.selectedCommand[project.id]);
  const selected = $derived(list?.commands.find((c) => c.id === selectedId) ?? null);
  // The global list updates first on events; prefer it over the per-project copy.
  const run = $derived<RunInfo | null>(
    selected ? (runs.runOf(project.id, selected.id) ?? selected.run) : null,
  );
  const busy = $derived(selected ? runKey(project.id, selected.id) in runs.busy : false);

  function lastResult(c: RunCommand): { text: string; tone: string } {
    const r = runs.runOf(project.id, c.id) ?? c.run;
    if (!r) return { text: "not run yet", tone: "muted" };
    const when = r.endedAt ? ` · ${relativeTime(r.endedAt, runs.now)}` : "";
    switch (r.state) {
      case "running":
      case "starting":
        return {
          text: r.startedAt ? `up ${duration(runs.now - r.startedAt)}` : "starting…",
          tone: "ok",
        };
      case "stopping":
        return { text: "stopping…", tone: "warn" };
      case "exited":
        return { text: `exited ${r.code ?? 0}${when}`, tone: "muted" };
      case "stopped":
        return { text: `stopped${when}`, tone: "muted" };
      case "failed":
        return { text: r.code == null ? `failed to start${when}` : `failed ${r.code}${when}`, tone: "bad" };
      default:
        return { text: "", tone: "muted" };
    }
  }

  async function save() {
    if (await runs.addCommand(project.id, form.name, form.line, form.subdir)) {
      adding = false;
      form = { name: "", line: "", subdir: "" };
    }
  }
</script>

<div class="run">
  {#if project.missing}
    <p class="muted">The folder is missing, so nothing can run here.</p>
  {:else if !list}
    <p class="muted">Reading commands…</p>
  {:else}
    {#if list.warning}
      <p class="warning">{list.warning}</p>
    {/if}

    <div class="cards">
      {#each list.commands as c (c.id)}
        {@const r = runs.runOf(project.id, c.id) ?? c.run}
        {@const active = isActive(r?.state)}
        {@const result = lastResult(c)}
        <div class="card" class:selected={c.id === selectedId} class:running={active}>
          <button
            class="card-hit"
            aria-label="Show output of {c.name}"
            onclick={() => (runs.selectedCommand[project.id] = c.id)}
          ></button>
          <div class="card-main">
            <div class="card-name mono">
              {c.name}
              {#if c.source.kind !== "script"}<span class="src">{c.source.kind}</span>{/if}
            </div>
            <div class="card-line mono" title={c.commandLine}>
              {c.commandLine}{#if c.knownPort}<span class="known" title="Port learned from an earlier run; checked before starting"> :{c.knownPort}</span>{/if}
            </div>
            <div class="card-result mono {result.tone}">{result.text}</div>
          </div>
          {#if active}
            <span class="running-label mono">RUNNING</span>
          {:else}
            <button
              class="play"
              title="Run {c.commandLine}"
              aria-label="Run {c.name}"
              disabled={runKey(project.id, c.id) in runs.busy}
              onclick={() => runs.start(project.id, c.id)}
            >
              <Icon name="play" size={14} />
            </button>
          {/if}
          {#if c.source.kind === "custom" && !active}
            <button
              class="remove"
              title="Remove this command"
              aria-label="Remove {c.name}"
              onclick={() => runs.removeCommand(project.id, c.id)}
            >
              <Icon name="x" size={11} />
            </button>
          {/if}
        </div>
      {/each}

      {#if adding}
        <form
          class="card form"
          onsubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <input class="mono" placeholder="Name, e.g. api" bind:value={form.name} />
          <input class="mono" placeholder="Command, e.g. cargo run" bind:value={form.line} />
          <input class="mono" placeholder="Subfolder (optional)" bind:value={form.subdir} />
          <div class="form-actions">
            <Button variant="secondary" onclick={() => (adding = false)}>Cancel</Button>
            <button class="save" type="submit">Save</button>
          </div>
        </form>
      {:else}
        <button class="card add" onclick={() => (adding = true)}>
          <Icon name="plus" size={14} /> Add command
        </button>
      {/if}
    </div>

    {#if list.commands.length === 0 && !adding}
      <p class="muted">
        No scripts in package.json and nothing detected. Add a command such as
        <span class="mono">cargo run</span> or <span class="mono">python manage.py runserver</span>.
      </p>
    {/if}

    {#if selected && runs.conflicts[runKey(project.id, selected.id)]}
      <ConflictBanner
        projectId={project.id}
        commandId={selected.id}
        conflict={runs.conflicts[runKey(project.id, selected.id)]}
      />
    {/if}

    {#if selected}
      <section class="process">
        <div class="proc-head">
          <div class="proc-title">
            <StatePill {run} />
            <span class="mono proc-cmd" title={selected.cwd}>{selected.commandLine}</span>
          </div>
          <div class="proc-actions">
            {#if run && isActive(run.state)}
              <Button
                variant="secondary"
                onclick={() => runs.restart(project.id, selected.id)}
                disabled={busy || run.state === "stopping"}
              >
                <Icon name="refresh" size={14} /> Restart
              </Button>
              <button
                class="stop"
                onclick={() => runs.stop(project.id, selected.id)}
                disabled={busy || run.state === "stopping"}
              >
                ■ Stop
              </button>
            {:else}
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
        </div>

        {#if run}
          <dl class="stats mono">
            <div><dt>PID</dt><dd>{run.pid ?? "—"}</dd></div>
            <div>
              <dt>Tree</dt>
              <dd>{run.processCount != null ? `${run.processCount} proc` : "—"}</dd>
            </div>
            <div>
              <dt>Uptime</dt>
              <dd>
                {#if run.startedAt}
                  {duration((isActive(run.state) ? runs.now : (run.endedAt ?? runs.now)) - run.startedAt)}
                {:else}—{/if}
              </dd>
            </div>
            <div>
              <dt>Port</dt>
              <dd title="Read from the output; M5 checks the real port table">
                {#if run.portHint}{run.portHint} <span class="hint">from logs</span>{:else}—{/if}
              </dd>
            </div>
          </dl>
          {#if run.state === "failed" && run.lastError}
            <div class="last-error mono selectable">{run.lastError}</div>
          {/if}
        {/if}

        <Console projectId={project.id} commandId={selected.id} live={isActive(run?.state)} />
      </section>
    {/if}
  {/if}
</div>

<style>
  .run {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .muted {
    color: var(--text-muted);
    margin: 0;
  }
  .warning {
    margin: 0;
    padding: var(--space-5) var(--space-6);
    border: 1px solid var(--warning-border);
    background: var(--warning-panel);
    border-radius: var(--radius-row);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: var(--space-5);
  }
  .card {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-6) var(--space-6) var(--space-7);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
    min-width: 0;
  }
  .card.selected {
    border-color: var(--accent-panel-border);
    background: var(--accent-panel);
  }
  .card.running {
    background: var(--success-panel);
    border-color: var(--success-border);
  }
  .card-hit {
    position: absolute;
    inset: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
    cursor: pointer;
  }
  .card-main {
    flex: 1;
    min-width: 0;
    pointer-events: none;
  }
  .card-name {
    font-size: 12.5px;
    font-weight: 600;
  }
  .src {
    margin-left: var(--space-3);
    font-size: 9.5px;
    font-weight: 400;
    color: var(--text-faint);
    text-transform: uppercase;
  }
  .card-line {
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: var(--space-1);
  }
  .known {
    margin-left: var(--space-2);
    color: var(--text-faint);
  }
  .card-result {
    font-size: 10.5px;
    margin-top: var(--space-2);
  }
  .ok {
    color: var(--success);
  }
  .bad {
    color: var(--error-text);
  }
  .warn {
    color: var(--accent-fg);
  }
  .running-label {
    position: relative;
    font-size: 10px;
    color: var(--success);
  }
  .play {
    position: relative;
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    background: var(--bg-raised);
    color: var(--text);
    cursor: pointer;
  }
  .play:hover:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }
  .play:disabled {
    opacity: 0.5;
  }
  .remove {
    position: absolute;
    top: 4px;
    right: 4px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    opacity: 0;
  }
  .card:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .remove:hover {
    color: var(--error-text);
  }
  .add {
    justify-content: center;
    gap: var(--space-3);
    border-style: dashed;
    border-color: var(--line-strong);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    min-height: 74px;
  }
  .add:hover {
    color: var(--text);
  }
  .form {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-4);
    grid-column: span 2;
  }
  .form input {
    height: 32px;
    padding: 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font-size: 12px;
    user-select: text;
  }
  .form input:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-4);
  }
  .save {
    height: 36px;
    padding: 0 var(--space-8);
    border: 0;
    border-radius: var(--radius-button);
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
    cursor: pointer;
  }
  .process {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }
  .proc-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-6);
  }
  .proc-title {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    min-width: 0;
  }
  .proc-cmd {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .proc-actions {
    display: flex;
    gap: var(--space-4);
    flex-shrink: 0;
  }
  .stop {
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
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    margin: 0;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .stats div {
    padding: var(--space-5) var(--space-7);
    border-left: 1px solid var(--line);
  }
  .stats div:first-child {
    border-left: 0;
  }
  .stats dt {
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .stats dd {
    margin: var(--space-1) 0 0;
    font-size: 13px;
  }
  .hint {
    font-size: 10px;
    color: var(--text-faint);
  }
  .last-error {
    padding: var(--space-5) var(--space-6);
    background: var(--error-row);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-row);
    color: var(--error-text);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
</style>

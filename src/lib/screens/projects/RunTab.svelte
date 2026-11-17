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
  let filter = $state("");
  /** Collapsed: only the selected command shows, so the log gets the whole height. */
  let collapsed = $state(false);
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
  const shown = $derived.by(() => {
    const all = list?.commands ?? [];
    const q = filter.trim().toLowerCase();
    if (collapsed) return all.filter((c) => c.id === selectedId);
    if (!q) return all;
    return all.filter((c) => c.name.toLowerCase().includes(q) || c.commandLine.toLowerCase().includes(q));
  });
  const runningCount = $derived(
    (list?.commands ?? []).filter((c) => isActive((runs.runOf(project.id, c.id) ?? c.run)?.state)).length,
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

  let rowsEl = $state<HTMLUListElement>();
  $effect(() => {
    void selectedId;
    void shown.length;
    rowsEl?.querySelector('[aria-current="true"]')?.closest("li")?.scrollIntoView({ block: "nearest" });
  });

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

    <section class="commands">
      <div class="cmd-head">
        <button
          class="collapse"
          aria-expanded={!collapsed}
          title={collapsed ? "Show all commands" : "Show only the selected command"}
          onclick={() => (collapsed = !collapsed)}
        >
          <span class="chev" class:closed={collapsed}><Icon name="chevron" size={13} /></span>
          <span class="cmd-title">Commands</span>
          <span class="cmd-count mono">{list.commands.length}</span>
          {#if runningCount}<span class="cmd-running mono">{runningCount} running</span>{/if}
        </button>
        <span class="spacer"></span>
        {#if list.commands.length > 5 && !collapsed}
          <label class="filter">
            <Icon name="search" size={13} />
            <input
              placeholder="Filter commands"
              bind:value={filter}
              spellcheck="false"
              onkeydown={(e) => {
                if (e.key === "Escape") filter = "";
              }}
            />
          </label>
        {/if}
        {#if !collapsed && !adding}
          <button class="add-btn" onclick={() => (adding = true)} title="Add a custom command">
            <Icon name="plus" size={13} /> Add
          </button>
        {/if}
      </div>

      <ul class="rows" class:collapsed bind:this={rowsEl}>
        {#each shown as c (c.id)}
          {@const r = runs.runOf(project.id, c.id) ?? c.run}
          {@const active = isActive(r?.state)}
          {@const result = lastResult(c)}
          <li class="row" class:selected={c.id === selectedId} class:running={active}>
            <button
              class="row-hit"
              aria-label="Show output of {c.name}"
              aria-current={c.id === selectedId ? "true" : undefined}
              onclick={() => (runs.selectedCommand[project.id] = c.id)}
            ></button>
            <span class="state-dot" class:on={active} class:bad={result.tone === "bad"}></span>
            <span class="row-name mono">
              {c.name}
              {#if c.source.kind !== "script"}<span class="src">{c.source.kind}</span>{/if}
            </span>
            <span class="row-line mono" title={c.commandLine}>
              {c.commandLine}{#if c.knownPort}<span class="known" title="Port learned from an earlier run; checked before starting"> :{c.knownPort}</span>{/if}
            </span>
            <span class="row-result mono {result.tone}">{result.text}</span>
            <span class="row-actions">
              {#if c.source.kind === "custom" && !active}
                <button
                  class="remove"
                  title="Remove this command"
                  aria-label="Remove {c.name}"
                  onclick={() => runs.removeCommand(project.id, c.id)}
                >
                  <Icon name="x" size={12} />
                </button>
              {/if}
              {#if active}
                <button
                  class="play stop-mini"
                  title="Stop {c.commandLine}"
                  aria-label="Stop {c.name}"
                  disabled={runKey(project.id, c.id) in runs.busy || r?.state === "stopping"}
                  onclick={() => runs.stop(project.id, c.id)}
                >
                  <Icon name="stop" size={12} />
                </button>
              {:else}
                <button
                  class="play"
                  title="Run {c.commandLine}"
                  aria-label="Run {c.name}"
                  disabled={runKey(project.id, c.id) in runs.busy}
                  onclick={() => runs.start(project.id, c.id)}
                >
                  <Icon name="play" size={12} />
                </button>
              {/if}
            </span>
          </li>
        {:else}
          {#if filter}
            <li class="none">
              No command matches "{filter}".
              <button class="link" onclick={() => (filter = "")}>Clear filter</button>
            </li>
          {/if}
        {/each}
      </ul>

      {#if adding}
        <form
          class="form"
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
      {/if}
    </section>

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
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-7);
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
  /* Commands: a dense, height-capped list so the log below stays in view,
     however many scripts a project has. */
  .commands {
    flex: 0 1 auto;
    max-height: 42%;
    min-height: 104px;
    display: flex;
    flex-direction: column;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    overflow: hidden;
  }
  .cmd-head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-4) var(--space-3) var(--space-3);
    border-bottom: 1px solid var(--line);
  }
  .collapse {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 30px;
    padding: 0 var(--space-4);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text);
    cursor: pointer;
  }
  .collapse:hover {
    background: var(--bg-hover);
  }
  .chev {
    display: grid;
    color: var(--icon);
    transition: transform var(--t-fast) var(--ease);
  }
  .chev.closed {
    transform: rotate(-90deg);
  }
  .cmd-title {
    font-weight: 600;
  }
  .cmd-count {
    font-size: 12px;
    color: var(--text-muted);
  }
  .cmd-running {
    font-size: 12px;
    padding: 1px 7px;
    border-radius: var(--radius-pill);
    background: var(--success-pill);
    color: var(--success);
  }
  .spacer {
    flex: 1;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 190px;
    height: 30px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--icon);
  }
  .filter:focus-within {
    border-color: var(--accent-fg);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 14px;
    user-select: text;
  }
  .filter input::placeholder {
    color: var(--icon);
  }
  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 30px;
    padding: 0 var(--space-5);
    border: 1px solid var(--line);
    border-radius: var(--radius-button-sm);
    background: var(--bg-raised);
    color: var(--text);
    font-size: 13.5px;
    cursor: pointer;
  }
  .add-btn:hover {
    border-color: var(--line-strong);
  }
  /* One grid for all rows (rows use subgrid), so names and commands line up in
     columns however long each name is. */
  .rows {
    flex: 1;
    min-height: 0;
    list-style: none;
    margin: 0;
    padding: var(--space-2);
    overflow-y: auto;
    overscroll-behavior: contain;
    display: grid;
    grid-template-columns: 20px minmax(80px, max-content) minmax(0, 1fr) auto auto;
    align-content: start;
    column-gap: var(--space-6);
    row-gap: 1px;
  }
  .row {
    position: relative;
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: subgrid;
    align-items: center;
    min-height: 44px;
    /* Subgrid rows take no side padding: the 20 px first column is the inset. */
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-button);
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.selected {
    background: var(--accent-panel);
    border-color: var(--accent-panel-border);
  }
  .row.running:not(.selected) {
    background: var(--success-panel);
  }
  .row-hit {
    position: absolute;
    inset: 0;
    border: 0;
    border-radius: inherit;
    background: transparent;
    cursor: pointer;
  }
  .state-dot {
    justify-self: end;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid var(--idle);
    pointer-events: none;
  }
  .state-dot.on {
    border: 0;
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-pill);
  }
  .state-dot.bad {
    border-color: var(--error);
    background: var(--error);
  }
  .row-name {
    font-size: 13.5px;
    font-weight: 600;
    white-space: nowrap;
    pointer-events: none;
  }
  .src {
    margin-left: var(--space-3);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .row-line {
    font-size: 12.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
  }
  .known {
    margin-left: var(--space-2);
    color: var(--text-faint);
  }
  .row-result {
    font-size: 12px;
    white-space: nowrap;
    pointer-events: none;
  }
  .row-result.muted {
    color: var(--text-muted);
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
  .row-actions {
    position: relative;
    padding-right: var(--space-3);
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .play {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    background: var(--bg-raised);
    color: var(--text);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .play:hover:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-ink);
  }
  .stop-mini {
    color: var(--error-text);
    border-color: var(--error-border);
    background: var(--error-pill);
  }
  .stop-mini:hover:not(:disabled) {
    background: var(--error);
    border-color: var(--error);
    color: var(--error-ink);
  }
  .play:disabled {
    opacity: 0.5;
  }
  .remove {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .remove,
  .remove:focus-visible {
    opacity: 1;
  }
  .remove:hover {
    color: var(--error-text);
    background: var(--error-pill);
  }
  .none {
    grid-column: 1 / -1;
    padding: var(--space-6);
    color: var(--text-muted);
  }
  .link {
    border: 0;
    padding: 0;
    margin-left: var(--space-3);
    background: transparent;
    color: var(--accent-fg);
    font-weight: 600;
    cursor: pointer;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-6);
    border-top: 1px solid var(--line);
  }
  .form input {
    height: 34px;
    padding: 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font-size: 13.5px;
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
    flex: 1 1 0;
    min-height: 260px;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
  }
  /* The log takes whatever height is left. */
  .process :global(.console-wrap) {
    flex: 1;
    min-height: 0;
  }
  .process :global(.console) {
    flex: 1;
    height: auto;
    min-height: 140px;
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
    font-size: 12.5px;
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
    font-size: 11px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }
  .stats dd {
    margin: var(--space-1) 0 0;
    font-size: 14px;
  }
  .hint {
    font-size: 11px;
    color: var(--text-faint);
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
</style>

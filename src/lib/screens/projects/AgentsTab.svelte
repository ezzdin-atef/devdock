<script lang="ts">
  import Select from "$lib/components/Select.svelte";
  import { agents } from "$lib/agents.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { relativeTime } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import type { AgentId, Project } from "$lib/types";

  // T6.3: agent cards, terminal + folder selects, last launch; T6.4: shortcuts.
  let { project }: { project: Project } = $props();

  const projectId = $derived(project.id);
  $effect(() => {
    agents.loadFolders(projectId);
    if (!agents.overview) agents.load();
  });
  $effect(() => {
    if (agents.overview) agents.ensureChoice(projectId);
  });

  const ov = $derived(agents.overview);
  const choice = $derived(agents.choice[projectId]);
  const folders = $derived(agents.folders[projectId] ?? []);
  const terminalOk = $derived(ov?.terminals.find((t) => t.kind === choice?.terminal)?.available ?? true);
  const last = $derived(project.launches?.[0]);
  const mine = $derived(ov?.shortcuts.filter((s) => s.projectId === projectId) ?? []);
  const freeSlots = $derived(
    [1, 2, 3, 4, 5, 6, 7, 8, 9].filter((n) => !ov?.shortcuts.some((s) => s.slot === n)),
  );

  let bindSlot = $state(0);
  let bindAgent = $state<AgentId | "terminal">("claude");
  $effect(() => {
    if (!freeSlots.includes(bindSlot)) bindSlot = freeSlots[0] ?? 0;
  });

  function nameOf(agent: AgentId | null) {
    return agent ? (ov?.agents.find((a) => a.id === agent)?.name ?? agent) : "Terminal";
  }
  function terminalName(kind: string) {
    return ov?.terminals.find((t) => t.kind === kind)?.name ?? kind;
  }
</script>

<div class="agents">
  {#if project.missing}
    <p class="muted">The folder is missing, so nothing can be opened here.</p>
  {:else if !ov || !choice}
    <p class="muted">Looking for agents…</p>
  {:else}
    <div class="selects">
      <label>
        <span class="eyebrow">Terminal</span>
        <Select
          bind:value={choice.terminal}
          ariaLabel="Terminal"
          options={ov.terminals.map((t) => ({ value: t.kind, label: t.name, hint: t.available ? undefined : "not found" }))}
        />
      </label>
      <label>
        <span class="eyebrow">Working folder</span>
        <Select
          bind:value={choice.subdir}
          mono
          ariaLabel="Working folder"
          options={[{ value: "", label: "project root" }, ...folders.map((f) => ({ value: f, label: f }))]}
        />
      </label>
    </div>
    {#if !terminalOk}
      <p class="warn">{terminalName(choice.terminal)} wasn't found. Pick another terminal, or install it and press Re-check.</p>
    {/if}

    <div class="cards">
      {#each ov.agents as a (a.id)}
        {@const found = !!a.path}
        <div class="card" class:missing={!found}>
          <div class="card-top">
            <span class="glyph mono" aria-hidden="true">&gt;_</span>
            <span class="state mono" class:ok={found}>{found ? "DETECTED" : "NOT INSTALLED"}</span>
          </div>
          <div class="name">
            {a.name}
            {#if ov.settings.agent === a.id}<span class="default mono">DEFAULT</span>{/if}
          </div>
          {#if found}
            <div class="path mono" title={a.path}>{a.path}</div>
            <button
              class="launch"
              disabled={!terminalOk || agents.launching !== null}
              onclick={() => agents.launch(projectId, a.id, choice.terminal, choice.subdir)}
            >
              <Icon name="terminal" size={14} /> Launch in {terminalName(choice.terminal)}
            </button>
          {:else}
            <div class="guide">
              Not on PATH. One way to install:
              <code class="mono selectable">{a.installHint}</code>
            </div>
            <Button variant="secondary" onclick={() => agents.recheck()} disabled={agents.checking}>
              <Icon name="refresh" size={13} /> {agents.checking ? "Checking…" : "Re-check"}
            </Button>
          {/if}
        </div>
      {/each}
      <div class="card plain">
        <div class="card-top">
          <span class="glyph mono" aria-hidden="true">$</span>
        </div>
        <div class="name">Terminal only</div>
        <div class="path mono">no agent, just a shell here</div>
        <button
          class="launch secondary"
          disabled={!terminalOk || agents.launching !== null}
          onclick={() => agents.launch(projectId, null, choice.terminal, choice.subdir)}
        >
          Open {terminalName(choice.terminal)}
        </button>
      </div>
    </div>

    <p class="history">
      {#if last}
        Last: <strong>{nameOf(last.agent)}</strong> in {terminalName(last.terminal)} ·
        <span class="mono">{last.subdir ?? "project root"}</span> · {relativeTime(last.at, runs.now)}
      {:else}
        Nothing launched from DevDock for this project yet.
      {/if}
    </p>

    <section class="shortcuts">
      <div class="eyebrow">Shortcuts</div>
      {#each mine as s (s.slot)}
        <div class="sc">
          <span class="keys mono">Ctrl Alt {s.slot}</span>
          <span>{nameOf(s.agent)} in {terminalName(s.terminal)}</span>
          <span class="mono muted">{s.subdir ?? "project root"}</span>
          <button class="x" aria-label="Remove Ctrl Alt {s.slot}" title="Remove" onclick={() => agents.unbind(s.slot)}>
            <Icon name="x" size={12} />
          </button>
        </div>
      {/each}
      {#if freeSlots.length}
        <div class="bind">
          <span>Bind</span>
          <Select
            bind:value={bindSlot}
            mono
            ariaLabel="Slot"
            options={freeSlots.map((n) => ({ value: n, label: `Ctrl Alt ${n}` }))}
          />
          <span>to</span>
          <Select
            bind:value={bindAgent}
            ariaLabel="What to launch"
            options={[...ov.agents.map((a) => ({ value: a.id as string, label: a.name })), { value: "terminal", label: "Terminal only" }]}
          />
          <span class="muted">with the terminal and folder above</span>
          <Button
            variant="secondary"
            onclick={() =>
              agents.bind(bindSlot, projectId, bindAgent === "terminal" ? null : bindAgent, choice.terminal, choice.subdir)}
          >
            Bind
          </Button>
        </div>
      {:else}
        <p class="muted">All nine slots are in use; remove one on the Agents screen.</p>
      {/if}
      <p class="muted small">Shortcuts work while the DevDock window is focused.</p>
    </section>
  {/if}
</div>

<style>
  .agents {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .muted {
    color: var(--text-muted);
    margin: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .warn {
    margin: 0;
    padding: var(--space-5) var(--space-6);
    border: 1px solid var(--warning-border);
    background: var(--warning-panel);
    border-radius: var(--radius-row);
  }
  .selects {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-6);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--space-6);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    min-width: 0;
  }
  .card.missing {
    background: transparent;
    border-style: dashed;
    border-color: var(--line-strong);
  }
  .card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button);
    color: var(--accent-fg);
    font-size: 14px;
  }
  .state {
    font-size: 11px;
    color: var(--text-muted);
  }
  .state.ok {
    color: var(--success);
  }
  .name {
    font-size: 16px;
    font-weight: 600;
  }
  .default {
    margin-left: var(--space-3);
    font-size: 10.5px;
    font-weight: 400;
    color: var(--accent-fg);
  }
  .path {
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .guide {
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .guide code {
    display: block;
    margin-top: var(--space-2);
    padding: var(--space-3) var(--space-4);
    background: var(--bg-app);
    border-radius: var(--radius-badge);
    color: var(--text-soft);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .launch {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    height: 38px;
    margin-top: auto;
    border: 0;
    border-radius: var(--radius-button);
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
    cursor: pointer;
  }
  .launch.secondary {
    background: var(--bg-raised);
    border: 1px solid var(--line);
    color: var(--text);
    font-weight: 500;
  }
  .launch:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .history {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }
  .history strong {
    color: var(--text);
    font-weight: 600;
  }
  .shortcuts {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .sc,
  .bind {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-5);
    font-size: 14px;
  }
  .keys {
    padding: 2px 7px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-badge);
    font-size: 11.5px;
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-left: auto;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .x:hover {
    color: var(--error-text);
  }
</style>

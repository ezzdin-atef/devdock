<script lang="ts">
  import { onMount } from "svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";

  // Rail "Agents": what's installed, how to install the rest, and every shortcut.
  onMount(() => {
    agents.load();
  });

  const ov = $derived(agents.overview);
  const terminalName = (k: string) => ov?.terminals.find((t) => t.kind === k)?.name ?? k;
  const agentName = (id: string | null) =>
    id ? (ov?.agents.find((a) => a.id === id)?.name ?? id) : "Terminal only";
</script>

<section class="screen">
  <header>
    <div>
      <div class="eyebrow">Coding agents</div>
      <h1>Agents</h1>
    </div>
    <Button variant="secondary" onclick={() => agents.recheck()} disabled={agents.checking}>
      <Icon name="refresh" size={14} /> {agents.checking ? "Checking…" : "Re-check"}
    </Button>
  </header>

  {#if ov}
    <div class="cards">
      {#each ov.agents as a (a.id)}
        <div class="card" class:missing={!a.path}>
          <div class="top">
            <span class="glyph mono" aria-hidden="true">&gt;_</span>
            <span class="state mono" class:ok={!!a.path}>{a.path ? "DETECTED" : "NOT INSTALLED"}</span>
          </div>
          <div class="name">{a.name}</div>
          <div class="mono small muted">runs <span class="cmd">{a.command}</span></div>
          {#if a.path}
            <div class="path mono" title={a.path}>{a.path}</div>
          {:else}
            <div class="guide">
              Not found on PATH. One way to install:
              <code class="mono selectable">{a.installHint}</code>
              Then press Re-check.
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <p class="muted">
      Launch an agent from a project's <strong>Agents</strong> tab: DevDock opens your terminal in the project
      folder and starts it there. DevDock doesn't talk to the agent itself.
    </p>

    <section class="panel">
      <div class="panel-head">
        <span class="title">Shortcuts</span>
        <span class="muted small">Ctrl Alt 1–9, while DevDock is focused</span>
      </div>
      {#if ov.shortcuts.length}
        <table>
          <tbody>
            {#each ov.shortcuts as s (s.slot)}
              <tr>
                <td><span class="keys mono">Ctrl Alt {s.slot}</span></td>
                <td>{agentName(s.agent)}</td>
                <td class="muted">{terminalName(s.terminal)}</td>
                <td>
                  <button class="link" onclick={() => { app.view = "projects"; app.tab = "agents"; app.clearFilters(); app.select(s.projectId); }}>
                    {s.projectName}
                  </button>
                  <span class="mono muted small">{s.subdir ?? ""}</span>
                </td>
                <td class="right">
                  <button class="x" aria-label="Remove Ctrl Alt {s.slot}" title="Remove" onclick={() => agents.unbind(s.slot)}>
                    <Icon name="x" size={12} />
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <p class="muted pad">No shortcuts yet. Bind one from a project's Agents tab.</p>
      {/if}
    </section>
  {:else}
    <p class="muted">Looking for agents…</p>
  {/if}
</section>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    padding: var(--space-12) var(--space-14);
    height: 100%;
    overflow-y: auto;
  }
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
  }
  h1 {
    margin: var(--space-1) 0 0;
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .muted {
    color: var(--text-muted);
    margin: 0;
  }
  .small {
    font-size: 12.5px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
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
  .top {
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
  .cmd {
    color: var(--text);
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
    margin: var(--space-2) 0;
    padding: var(--space-3) var(--space-4);
    background: var(--bg-app);
    border-radius: var(--radius-badge);
    color: var(--text-soft);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .panel-head {
    display: flex;
    align-items: baseline;
    gap: var(--space-6);
    padding: var(--space-7) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
  .pad {
    padding: var(--space-8);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  td {
    padding: var(--space-5) var(--space-8);
    border-top: 1px solid var(--line);
  }
  tr:first-child td {
    border-top: 0;
  }
  .keys {
    padding: 2px 7px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-badge);
    font-size: 11.5px;
  }
  .link {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--accent-fg);
    font-weight: 600;
    cursor: pointer;
  }
  .right {
    text-align: right;
  }
  .x {
    display: inline-grid;
    place-items: center;
    width: 24px;
    height: 24px;
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

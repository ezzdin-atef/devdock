<script lang="ts">
  import { onMount } from "svelte";
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { ordered, spaces, tone } from "$lib/spaces.svelte";
  import type { Service, WorkspaceView } from "$lib/types";
  import CombinedLogs from "./spaces/CombinedLogs.svelte";
  import RulesPanel from "./spaces/RulesPanel.svelte";
  import ServiceCard from "./spaces/ServiceCard.svelte";
  import ServiceForm from "./spaces/ServiceForm.svelte";

  // T7.8: list with health bars; detail with service cards joined by arrows,
  // Start all / Stop all, rules, after-start preset and combined logs.
  onMount(() => {
    spaces.init();
    if (!agents.overview) agents.load();
  });

  let newName = $state("");
  let form = $state<{ editing: Service | null } | null>(null);
  let renaming = $state(false);

  const ws = $derived(spaces.selected);
  const phase = $derived(ws?.live?.phase ?? "idle");
  const busy = $derived(phase === "starting" || phase === "running" || phase === "stopping");
  const live = $derived(busy || (phase === "failed" && (ws?.live?.services.some((s) => ["starting", "healthy", "up", "unhealthy"].includes(s.state)) ?? false)));

  function stateOf(w: WorkspaceView, s: Service) {
    const l = w.live?.services.find((x) => x.id === s.id);
    if (l && w.live?.phase !== "stopped") return l.state;
    const r = w.extra[s.id]?.run;
    return r && ["starting", "running", "stopping"].includes(r.state) ? "up" : "idle";
  }

  async function create() {
    if (newName.trim() && (await spaces.create(newName))) newName = "";
  }

  const PHASE_LABEL: Record<string, string> = {
    idle: "IDLE",
    starting: "STARTING",
    running: "RUNNING",
    stopping: "STOPPING",
    stopped: "STOPPED",
    failed: "FAILED",
  };
</script>

<div class="screen">
  <aside class="list">
    <div class="eyebrow head">Workspaces</div>
    <ul>
      {#each spaces.list as w (w.id)}
        <li>
          <button class="row" class:selected={w.id === spaces.selectedId} onclick={() => (spaces.selectedId = w.id)}>
            <span class="row-top">
              <span class="name">{w.name}</span>
              <span class="phase mono">{PHASE_LABEL[w.live?.phase ?? "idle"]}</span>
            </span>
            <span class="bars" aria-label="{w.services.length} services">
              {#each ordered(w) as s (s.id)}
                <span class="bar {tone(stateOf(w, s))}" title="{s.name}: {stateOf(w, s)}"></span>
              {:else}
                <span class="none mono">no services</span>
              {/each}
            </span>
          </button>
        </li>
      {/each}
    </ul>
    <form
      class="new"
      onsubmit={(e) => {
        e.preventDefault();
        create();
      }}
    >
      <input bind:value={newName} placeholder="New workspace, e.g. Wallet" aria-label="New workspace name" />
      <button type="submit" class="add" disabled={!newName.trim()} aria-label="Create workspace" title="Create workspace"><Icon name="plus" size={14} /></button>
    </form>
  </aside>

  <section class="detail">
    {#if app.error}
      <ErrorBanner error={app.error} ondismiss={() => app.dismissError()} />
    {/if}

    {#if !ws}
      <div class="empty">
        <div class="eyebrow">No workspace selected</div>
        <p>
          A workspace starts several services in order, for example a database, an API and a web app, waits
          for each to be healthy, and shows their logs together. Create one on the left.
        </p>
      </div>
    {:else}
      <header>
        <div>
          <div class="eyebrow">Workspace · {ws.services.length} {ws.services.length === 1 ? "service" : "services"}</div>
          {#if renaming}
            <input
              class="rename"
              value={ws.name}
              onkeydown={(e) => {
                if (e.key === "Enter") {
                  spaces.rename(ws.id, e.currentTarget.value);
                  renaming = false;
                }
                if (e.key === "Escape") renaming = false;
              }}
              onblur={() => (renaming = false)}
            />
          {:else}
            <h1 ondblclick={() => !live && (renaming = true)} title="Double-click to rename">{ws.name}</h1>
          {/if}
          {#if ws.live?.phase === "failed"}
            <p class="reason">{ws.live.reason}</p>
          {/if}
        </div>
        <div class="actions">
          <span class="phase-pill mono {phase}">{PHASE_LABEL[phase]}</span>
          {#if live}
            <button class="stop" onclick={() => spaces.stop(ws.id)} disabled={phase === "stopping"}>■ Stop all</button>
          {:else}
            <Button variant="primary" onclick={() => spaces.start(ws.id)} disabled={!ws.services.length}>
              <Icon name="play" size={13} /> Start all
            </Button>
            <button class="icon" title="Delete workspace" aria-label="Delete workspace" onclick={() => spaces.remove(ws.id)}>
              <Icon name="x" size={14} />
            </button>
          {/if}
        </div>
      </header>

      <div class="flow">
        {#each ordered(ws) as s, i (s.id)}
          {#if i > 0}<span class="arrow" aria-hidden="true">→</span>{/if}
          <ServiceCard {ws} svc={s} index={i} editable={!live} onedit={() => (form = { editing: s })} />
        {/each}
        {#if !live}
          {#if ws.services.length}<span class="arrow faint" aria-hidden="true">→</span>{/if}
          <button class="add-card" onclick={() => (form = { editing: null })}>
            <Icon name="plus" size={14} /> Add service
          </button>
        {/if}
      </div>

      {#if form}
        {#key form.editing?.id ?? "new"}
          <ServiceForm {ws} editing={form.editing} onclose={() => (form = null)} />
        {/key}
      {/if}

      {#key ws.id}
        <RulesPanel {ws} />
        <CombinedLogs {ws} />
      {/key}
    {/if}
  </section>
</div>

<style>
  .screen {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    width: 300px;
    flex-shrink: 0;
    padding: var(--space-10) var(--space-5);
    background: var(--bg-sidebar);
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .head {
    padding: 0 var(--space-4);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    width: 100%;
    padding: var(--space-5) var(--space-6);
    border: 1px solid transparent;
    border-radius: var(--radius-row);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-hover);
  }
  .row.selected {
    background: var(--accent-panel);
    border-color: var(--accent-panel-border);
  }
  .row-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .name {
    font-size: 14px;
    font-weight: 600;
  }
  .phase {
    font-size: 9.5px;
    color: var(--text-muted);
  }
  /* DESIGN.md: one 18 x 5 bar per service. */
  .bars {
    display: flex;
    gap: 3px;
  }
  .bar {
    width: 18px;
    height: 5px;
    border-radius: 2px;
    background: var(--line-strong);
  }
  .bar.ok {
    background: var(--success);
  }
  .bar.busy {
    background: var(--accent-fg);
  }
  .bar.bad {
    background: var(--error);
  }
  .none {
    font-size: 10px;
    color: var(--text-faint);
  }
  .new {
    display: flex;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }
  .new input {
    flex: 1;
    min-width: 0;
    height: 34px;
    padding: 0 var(--space-5);
    background: var(--bg-panel);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-button);
    color: var(--text);
    font: inherit;
    user-select: text;
  }
  .new input:focus {
    outline: none;
    border-color: var(--accent-fg);
    border-style: solid;
  }
  .add {
    display: grid;
    place-items: center;
    width: 34px;
    border: 1px solid var(--line);
    border-radius: var(--radius-button);
    background: var(--bg-raised);
    color: var(--text);
    cursor: pointer;
  }
  .add:disabled {
    opacity: 0.4;
  }
  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
    padding: var(--space-12) var(--space-14);
    overflow-y: auto;
  }
  .detail > :global(*) {
    flex-shrink: 0;
  }
  .empty p {
    color: var(--text-muted);
    max-width: 560px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--space-8);
  }
  h1 {
    margin: var(--space-1) 0 0;
    font-size: 27px;
    font-weight: 700;
    letter-spacing: -0.02em;
    cursor: text;
  }
  .rename {
    margin-top: var(--space-1);
    height: 40px;
    padding: 0 var(--space-4);
    background: var(--bg-panel);
    border: 1px solid var(--accent-fg);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font: inherit;
    font-size: 22px;
    font-weight: 700;
  }
  .reason {
    margin: var(--space-3) 0 0;
    color: var(--error-text);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-5);
  }
  .phase-pill {
    padding: 4px 9px;
    border-radius: var(--radius-pill);
    font-size: 10px;
    letter-spacing: 0.06em;
    background: var(--bg-raised);
    color: var(--text-muted);
  }
  .phase-pill.running {
    background: var(--success-pill);
    color: var(--success);
  }
  .phase-pill.starting,
  .phase-pill.stopping {
    background: var(--warning-panel);
    color: var(--accent-fg);
  }
  .phase-pill.failed {
    background: var(--error-pill);
    color: var(--error-text);
  }
  .stop {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    cursor: pointer;
  }
  .stop:disabled {
    opacity: 0.5;
  }
  .icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .icon:hover {
    color: var(--error-text);
    background: var(--bg-hover);
  }
  .flow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-5);
  }
  .arrow {
    color: var(--text-faint);
    font-size: 18px;
  }
  .arrow.faint {
    opacity: 0.5;
  }
  .add-card {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    width: 200px;
    height: 120px;
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius-panel);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .add-card:hover {
    color: var(--text);
  }
</style>

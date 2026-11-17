<script lang="ts">
  import { untrack } from "svelte";
  import { agents } from "$lib/agents.svelte";
  import Button from "$lib/components/Button.svelte";
  import { spaces } from "$lib/spaces.svelte";
  import type { AgentId, TerminalKind, WorkspaceView } from "$lib/types";

  // Rules (restart policy) and the "after start" preset (T7.8).
  let { ws }: { ws: WorkspaceView } = $props();

  // Seeded once: the parent re-creates this panel ({#key ws.id}) for another workspace.
  const seed = untrack(() => ws);
  let enabled = $state(seed.restart.enabled);
  let max = $state(String(seed.restart.max));
  let windowSecs = $state(String(seed.restart.windowSecs));
  let projectId = $state(seed.afterStart.projectId ?? "");
  let openEditor = $state(seed.afterStart.openEditor);
  let agent = $state<AgentId | "">(seed.afterStart.agent ?? "");
  let terminal = $state<TerminalKind>(seed.afterStart.terminal);

  const projectChoices = $derived(
    [...new Map(ws.services.map((s) => [s.source.projectId, ws.extra[s.id]?.projectName ?? "?"])).entries()],
  );
  const dirty = $derived(
    enabled !== ws.restart.enabled ||
      Number(max) !== ws.restart.max ||
      Number(windowSecs) !== ws.restart.windowSecs ||
      (projectId || null) !== ws.afterStart.projectId ||
      openEditor !== ws.afterStart.openEditor ||
      (agent || null) !== ws.afterStart.agent ||
      terminal !== ws.afterStart.terminal,
  );

  function save() {
    spaces.setRules(
      ws.id,
      { enabled, max: Number(max) || 0, windowSecs: Number(windowSecs) || 0 },
      { projectId: projectId || null, openEditor, agent: agent || null, terminal },
    );
  }
</script>

<section class="panel">
  <div class="head"><span class="title">Rules</span></div>
  <div class="body">
    <label class="row">
      <input type="checkbox" bind:checked={enabled} />
      Restart a service that crashes, up to
      <input class="num mono" bind:value={max} inputmode="numeric" disabled={!enabled} />
      times within
      <input class="num mono" bind:value={windowSecs} inputmode="numeric" disabled={!enabled} />
      seconds
    </label>
    <p class="help">
      Each service waits for the previous one's health check (60 s at most). Checks run every 10 s; 3 misses
      mark a service unhealthy. Stop all stops services in reverse order.
    </p>

    <div class="sub eyebrow">After start</div>
    <div class="row wrap">
      <span>In</span>
      <select bind:value={projectId}>
        <option value="">— no preset —</option>
        {#each projectChoices as [id, name] (id)}<option value={id}>{name}</option>{/each}
      </select>
      <label class="inline"><input type="checkbox" bind:checked={openEditor} disabled={!projectId} /> open the editor</label>
      <span>and launch</span>
      <select bind:value={agent} disabled={!projectId}>
        <option value="">no agent</option>
        {#each agents.overview?.agents ?? [] as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
      </select>
      <span>in</span>
      <select bind:value={terminal} disabled={!projectId || !agent}>
        {#each agents.overview?.terminals ?? [] as t (t.kind)}<option value={t.kind}>{t.name}</option>{/each}
      </select>
    </div>
    <div class="actions">
      <Button variant="secondary" onclick={save} disabled={!dirty}>Save rules</Button>
    </div>
  </div>
</section>

<style>
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .head {
    padding: var(--space-6) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-7) var(--space-8);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .wrap {
    flex-wrap: wrap;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .num {
    width: 56px;
    height: 30px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    text-align: center;
    user-select: text;
  }
  select {
    height: 32px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font: inherit;
  }
  input[type="checkbox"] {
    accent-color: var(--accent-fg);
  }
  .help {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
  .sub {
    margin-top: var(--space-4);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>

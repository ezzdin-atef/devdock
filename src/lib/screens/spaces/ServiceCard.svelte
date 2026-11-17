<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { duration, relativeTime } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { tone } from "$lib/spaces.svelte";
  import { isActive, type Service, type SvcRuntime, type WorkspaceView } from "$lib/types";

  // DESIGN.md "Workspace service card": order tile + health label, name, source and
  // command in mono, 2x2 stats (port, uptime, check, restarts).
  let {
    ws,
    svc,
    index,
    editable,
    onedit,
  }: { ws: WorkspaceView; svc: Service; index: number; editable: boolean; onedit: () => void } = $props();

  const live = $derived<SvcRuntime | undefined>(ws.live?.services.find((s) => s.id === svc.id));
  const extra = $derived(ws.extra[svc.id]);
  const run = $derived(extra?.run ?? null);
  const running = $derived(isActive(run?.state));
  const label = $derived.by(() => {
    if (live && ws.live && ws.live.phase !== "stopped") {
      return live.state === "gaveUp" ? "GAVE UP" : live.state === "pending" ? "WAITING" : live.state.toUpperCase();
    }
    return running ? "RUNNING" : "IDLE";
  });
  const t = $derived(tone(live && ws.live?.phase !== "stopped" ? live.state : running ? "up" : "idle"));
  const check = $derived.by(() => {
    // No check configured: the orchestrator treats it as up after a short grace period.
    if (!svc.health) return "none";
    const kind = svc.health.kind === "http" ? "http" : `tcp :${svc.health.port}`;
    if (!live?.lastCheck) return kind;
    return `${kind} · ${relativeTime(live.lastCheck, runs.now)}`;
  });
</script>

<div class="card {t}">
  <div class="top">
    <span class="order mono">{index + 1}</span>
    <span class="label mono {t}">{label}</span>
    {#if editable}
      <button class="edit" aria-label="Edit {svc.name}" title="Edit" onclick={onedit}><Icon name="kebab" size={13} /></button>
    {/if}
  </div>
  <div class="name">{svc.name}</div>
  <div class="src mono" title={extra?.detail}>
    {extra?.projectName ?? "?"} · {svc.source.kind === "compose" ? `compose ${svc.source.service}` : svc.source.commandId.replace(/^(script|detected|custom):/, "")}
  </div>
  <div class="cmd mono" title={extra?.detail}>{extra?.detail ?? ""}</div>
  <dl class="stats mono">
    <div><dt>port</dt><dd>{svc.port ?? run?.portHint ?? "—"}</dd></div>
    <div>
      <dt>uptime</dt>
      <dd>{running && run?.startedAt ? duration(runs.now - run.startedAt) : "—"}</dd>
    </div>
    <div><dt>check</dt><dd title={live?.message ?? ""}>{check}</dd></div>
    <div><dt>restarts</dt><dd>{live?.restarts.length ?? 0}</dd></div>
  </dl>
  {#if live?.message}
    <div class="msg">{live.message}</div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    width: 240px;
    padding: var(--space-7);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .card.ok {
    background: var(--success-panel);
    border-color: var(--success-border);
  }
  .card.bad {
    background: var(--error-row);
    border-color: var(--error-border);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .order {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 1px solid var(--warning-border);
    border-radius: var(--radius-button-sm);
    color: var(--accent-fg);
    font-size: 12.5px;
  }
  .label {
    font-size: 11px;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .label.ok {
    color: var(--success);
  }
  .label.busy {
    color: var(--accent-fg);
  }
  .label.bad {
    color: var(--error-text);
  }
  .edit {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .edit:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .name {
    font-size: 16px;
    font-weight: 600;
  }
  .src,
  .cmd {
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cmd {
    color: var(--text-faint);
  }
  .stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-3) var(--space-5);
    margin: var(--space-3) 0 0;
    font-size: 12.5px;
  }
  dt {
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .stats div {
    min-width: 0;
  }
  dd {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg {
    font-size: 12.5px;
    color: var(--text-soft);
    overflow-wrap: anywhere;
  }
</style>

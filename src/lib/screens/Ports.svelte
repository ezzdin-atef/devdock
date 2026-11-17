<script lang="ts">
  import { onMount } from "svelte";
  import Button from "$lib/components/Button.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { shortPath } from "$lib/format";
  import { ports, type PortFilter } from "$lib/ports.svelte";
  import { runKey, runs } from "$lib/runs.svelte";

  // T5.4 + T5.7: polls while visible; owner badges; actions depend on the owner.
  onMount(() => {
    ports.startPolling();
    return () => {
      ports.stopPolling();
      ports.highlight = null;
    };
  });

  // Scroll a port picked in the palette into view once its row exists.
  $effect(() => {
    const p = ports.highlight;
    if (p == null || !ports.rows.some((r) => r.port === p)) return;
    queueMicrotask(() => document.querySelector(`tr[data-port="${p}"]`)?.scrollIntoView({ block: "center" }));
  });

  const FILTERS: { value: PortFilter; label: string; key: keyof typeof ports.counts }[] = [
    { value: "all", label: "All", key: "all" },
    { value: "devDock", label: "DevDock", key: "devDock" },
    { value: "external", label: "External", key: "external" },
    { value: "system", label: "System", key: "system" },
  ];

  const ago = $derived(
    ports.refreshedAt ? Math.max(0, Math.round((runs.now - ports.refreshedAt) / 1000)) : null,
  );
</script>

<section class="ports">
  <header>
    <div>
      <div class="eyebrow">Listening TCP ports</div>
      <h1>Ports <span class="count mono">{ports.rows.length}</span></h1>
    </div>
    <div class="head-actions">
      <span class="stamp mono">
        {ports.loading && ago === null ? "reading…" : ago === null ? "" : `refreshed ${ago}s ago`}
      </span>
      <Button variant="secondary" onclick={() => ports.refresh()}>
        <Icon name="refresh" size={14} /> Refresh
      </Button>
    </div>
  </header>

  <div class="segmented" role="radiogroup" aria-label="Owner">
    {#each FILTERS as f (f.value)}
      <button
        role="radio"
        aria-checked={ports.filter === f.value}
        class:on={ports.filter === f.value}
        onclick={() => (ports.filter = f.value)}
      >
        {f.label} <span class="n mono">{ports.counts[f.key]}</span>
      </button>
    {/each}
  </div>

  {#if ports.notice}
    <div class="notice" role="status">
      <Icon name="check" size={14} />
      <span>{ports.notice}</span>
      <button class="x" aria-label="Dismiss" title="Dismiss" onclick={() => (ports.notice = null)}><Icon name="x" size={12} /></button>
    </div>
  {/if}
  {#if ports.error}
    <ErrorBanner error={ports.error} ondismiss={() => (ports.error = null)} />
  {/if}

  <div class="table-wrap">
    <table>
      <thead>
        <tr class="eyebrow">
          <th>Port</th>
          <th>Process</th>
          <th>Project</th>
          <th>Owner</th>
          <th>Address</th>
          <th class="right">Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each ports.shown as r (`${r.port}:${r.pid}`)}
          <tr class:highlight={ports.highlight === r.port} data-port={r.port}>
            <td class="port mono">{r.port}</td>
            <td class="proc">
              <div>
                <span class="name">{r.process?.name ?? "unknown"}</span>
                <span class="pid mono">PID {r.pid}</span>
              </div>
              {#if r.process?.cmd}
                <div class="cmd mono" title={r.process.cmd}>{r.process.cmd}</div>
              {/if}
            </td>
            <td>
              {#if r.project}
                <span>{r.project.name}</span>
              {:else if r.process?.cwd}
                <span class="muted mono small" title={r.process.cwd}>{shortPath(r.process.cwd, 2)}</span>
              {:else}
                <span class="muted">—</span>
              {/if}
            </td>
            <td>
              <span class="badge mono {r.owner}">
                {r.owner === "devDock" ? "DEVDOCK" : r.owner === "system" ? "SYSTEM" : "EXTERNAL"}
              </span>
            </td>
            <td class="mono small muted">{r.addresses.join(" ")}</td>
            <td class="right actions">
              {#if r.owner === "devDock" && r.run}
                {@const run = r.run}
                <Button variant="ghost" onclick={() => runs.showLogs(run.projectId, run.commandId)}>Logs</Button>
                <button
                  class="stop"
                  disabled={runKey(run.projectId, run.commandId) in runs.busy}
                  onclick={async () => {
                    await runs.stop(run.projectId, run.commandId);
                    ports.refresh();
                  }}>■ Stop</button
                >
              {:else if r.owner === "external"}
                <Button variant="secondary" onclick={() => ports.inspect(r)}>Inspect</Button>
              {:else}
                <span class="protected mono" title="DevDock never ends system processes">Protected</span>
                <button class="info" aria-label="Details for PID {r.pid}" title="Details" onclick={() => ports.inspect(r)}>
                  <Icon name="search" size={13} />
                </button>
              {/if}
            </td>
          </tr>
        {:else}
          <tr>
            <td colspan="6" class="muted empty">
              {ports.rows.length ? "Nothing matches this filter." : ports.loading ? "Reading the port table…" : "No listening TCP ports."}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <p class="note">
    DEVDOCK means the process is inside a job DevDock created; names and port numbers are never used to
    guess ownership. Projects are matched by the process's working folder.
  </p>
</section>

<style>
  .ports {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
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
  .count {
    font-size: 15px;
    font-weight: 400;
    color: var(--text-muted);
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: var(--space-6);
  }
  .stamp {
    font-size: 12px;
    color: var(--text-muted);
  }
  .segmented {
    display: inline-flex;
    align-self: flex-start;
    padding: 3px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
  }
  .segmented button {
    height: 28px;
    padding: 0 var(--space-7);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
  }
  .segmented button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
  }
  .n {
    font-size: 11px;
    margin-left: var(--space-2);
    opacity: 0.8;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .notice :global(svg) {
    color: var(--success);
    flex-shrink: 0;
  }
  .notice span {
    flex: 1;
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .table-wrap {
    /* Let the page scroll; don't squeeze the table into its own scroll box. */
    flex-shrink: 0;
    overflow-x: auto;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th {
    text-align: left;
    font-weight: 400;
    padding: var(--space-6) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: var(--space-6) var(--space-8);
    border-top: 1px solid var(--line);
    vertical-align: middle;
  }
  tbody tr:first-child td {
    border-top: 0;
  }
  tr.highlight td {
    background: var(--accent-tint);
  }
  .port {
    font-size: 16px;
    font-weight: 600;
  }
  .name {
    font-weight: 600;
  }
  .pid {
    margin-left: var(--space-4);
    font-size: 12px;
    color: var(--text-muted);
  }
  .cmd {
    max-width: 380px;
    margin-top: var(--space-1);
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    color: var(--text-muted);
  }
  .small {
    font-size: 12px;
  }
  .badge {
    display: inline-block;
    padding: 3px 7px;
    border-radius: var(--radius-badge);
    font-size: 11px;
    letter-spacing: 0.06em;
  }
  .badge.devDock {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .badge.external,
  .badge.system {
    border: 1px solid var(--line-badge);
    color: var(--text-soft);
  }
  .right {
    text-align: right;
  }
  .actions {
    white-space: nowrap;
  }
  .stop {
    height: 32px;
    padding: 0 var(--space-6);
    margin-left: var(--space-2);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button-sm);
    background: var(--error-pill);
    color: var(--error-text);
    cursor: pointer;
  }
  .stop:disabled {
    opacity: 0.5;
  }
  .protected {
    font-size: 11.5px;
    color: var(--text-muted);
  }
  .info {
    display: inline-grid;
    place-items: center;
    width: 28px;
    height: 28px;
    margin-left: var(--space-3);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    vertical-align: middle;
  }
  .info:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .empty {
    text-align: center;
    padding: var(--space-12);
  }
  .note {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
</style>

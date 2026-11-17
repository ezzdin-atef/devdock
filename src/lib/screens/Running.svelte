<script lang="ts">
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import StatePill from "$lib/components/StatePill.svelte";
  import { duration } from "$lib/format";
  import { runKey, runs } from "$lib/runs.svelte";
  import { isActive } from "$lib/types";

  // DESIGN.md "Tables": every process DevDock owns this session (T4.7).
</script>

<section class="running">
  <header>
    <div>
      <div class="eyebrow">Owned by DevDock</div>
      <h1>Running <span class="count mono">{runs.active.length}</span></h1>
    </div>
    {#if runs.active.length > 0}
      <button class="stop-all" onclick={() => runs.stopAll()}>■ Stop everything</button>
    {/if}
  </header>

  {#if runs.list.length === 0}
    <div class="empty">
      <p>Nothing has run yet. Start a command from a project's <strong>Run</strong> tab.</p>
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr class="eyebrow">
            <th>State</th>
            <th>Project · command</th>
            <th>PID</th>
            <th>Tree</th>
            <th>Port</th>
            <th>Uptime</th>
            <th class="right">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each runs.list as r (runKey(r.projectId, r.commandId))}
            {@const active = isActive(r.state)}
            {@const busy = runKey(r.projectId, r.commandId) in runs.busy}
            <tr class:failed={r.state === "failed"} class:ended={!active}>
              <td><StatePill run={r} /></td>
              <td>
                <div class="proj">{r.projectName}</div>
                <div class="cmd mono" title={r.cwd}>{r.commandLine}</div>
              </td>
              <td class="mono">{r.pid ?? "—"}</td>
              <td class="mono">{r.processCount ?? "—"}</td>
              <td class="mono">{r.portHint ?? "—"}</td>
              <td class="mono">
                {#if r.startedAt}
                  {duration((active ? runs.now : (r.endedAt ?? runs.now)) - r.startedAt)}
                {:else}—{/if}
              </td>
              <td class="right actions">
                <Button variant="ghost" onclick={() => runs.showLogs(r.projectId, r.commandId)}>Logs</Button>
                {#if active}
                  <button
                    class="icon-btn"
                    title="Restart"
                    aria-label="Restart {r.commandName}"
                    disabled={busy}
                    onclick={() => runs.restart(r.projectId, r.commandId)}
                  >
                    <Icon name="refresh" size={14} />
                  </button>
                  <button
                    class="icon-btn danger"
                    title="Stop"
                    aria-label="Stop {r.commandName}"
                    disabled={busy || r.state === "stopping"}
                    onclick={() => runs.stop(r.projectId, r.commandId)}
                  >
                    ■
                  </button>
                {:else}
                  <button
                    class="icon-btn"
                    title="Run again"
                    aria-label="Run {r.commandName} again"
                    disabled={busy}
                    onclick={() => runs.restart(r.projectId, r.commandId)}
                  >
                    <Icon name="play" size={13} />
                  </button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="note">
      Each command runs in its own Windows job: Stop ends the whole process tree, and closing
      DevDock ends everything listed here.
    </p>
  {/if}
</section>

<style>
  .running {
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
    font-size: 27px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .count {
    font-size: 14px;
    font-weight: 400;
    color: var(--success);
  }
  .stop-all {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: var(--error-pill);
    color: var(--error-text);
    font-weight: 500;
    cursor: pointer;
  }
  .empty {
    padding: var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    color: var(--text-muted);
  }
  .empty p {
    margin: 0;
  }
  .table-wrap {
    flex-shrink: 0;
    overflow-x: auto;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
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
  tr.failed {
    background: var(--error-row);
  }
  tr.ended .proj {
    color: var(--text-muted);
  }
  .proj {
    font-weight: 600;
  }
  .cmd {
    font-size: 11px;
    color: var(--text-muted);
    max-width: 360px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .right {
    text-align: right;
  }
  .actions {
    white-space: nowrap;
  }
  .icon-btn {
    display: inline-grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin-left: var(--space-2);
    border: 1px solid var(--line);
    border-radius: var(--radius-button-sm);
    background: var(--bg-raised);
    color: var(--text);
    cursor: pointer;
    vertical-align: middle;
  }
  .icon-btn.danger {
    color: var(--error-text);
    border-color: var(--error-border);
  }
  .icon-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>

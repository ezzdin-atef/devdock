<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/app.svelte";
  import * as api from "$lib/api";
  import Button from "$lib/components/Button.svelte";
  import { spaces } from "$lib/spaces.svelte";
  import type { Health, RunCommand, Service, WorkspaceView } from "$lib/types";

  // Add or edit one service (T7.1, T7.6 Compose, T7.7 worktree).
  let { ws, editing, onclose }: { ws: WorkspaceView; editing: Service | null; onclose: () => void } = $props();

  const projects = $derived(app.lib?.projects ?? []);
  // Seeded once: the parent re-creates the form ({#key}) for another service.
  const e = untrack(() => editing);
  const existing = untrack(() => ws.services);
  let name = $state(e?.name ?? "");
  let projectId = $state(e?.source.projectId ?? "");
  let kind = $state<"command" | "compose">(e?.source.kind ?? "command");
  let commandId = $state(e?.source.kind === "command" ? e.source.commandId : "");
  let worktree = $state(e?.source.kind === "command" ? (e.source.worktree ?? "") : "");
  let file = $state(e?.source.kind === "compose" ? e.source.file : "docker-compose.yml");
  let composeService = $state(e?.source.kind === "compose" ? e.source.service : "");
  let port = $state(e?.port ? String(e.port) : "");
  let healthKind = $state<"none" | "tcp" | "http">(e?.health?.kind ?? (e ? "none" : "tcp"));
  let url = $state(e?.health?.kind === "http" ? e.health.url : "");
  let expect = $state(e?.health?.kind === "http" && e.health.expect ? String(e.health.expect) : "");
  let order = $state(String(e?.order ?? (Math.max(0, ...existing.map((s) => s.order)) + 1)));
  let saving = $state(false);

  let commands = $state<RunCommand[]>([]);
  let worktrees = $state<string[]>([]);

  $effect(() => {
    const id = projectId;
    commands = [];
    worktrees = [];
    if (!id) return;
    api.runCommands(id).then((l) => (commands = l.commands)).catch(() => {});
    api.projectWorktrees(id).then((w) => (worktrees = w)).catch(() => {});
  });

  // Name the service after the command when the user hasn't typed one.
  $effect(() => {
    const c = commands.find((c) => c.id === commandId);
    if (!name && c) name = c.name;
  });

  const projectPath = $derived(projects.find((p) => p.id === projectId)?.path ?? "");
  // The main checkout is the default; only offer the other worktrees.
  const otherWorktrees = $derived(worktrees.filter((w) => w.toLowerCase() !== projectPath.toLowerCase()));
  const portNum = $derived(port.trim() ? Number(port) : null);
  const valid = $derived(
    !!name.trim() &&
      !!projectId &&
      (kind === "command" ? !!commandId : !!file.trim() && !!composeService.trim()) &&
      (portNum === null || (Number.isInteger(portNum) && portNum > 0 && portNum < 65536)) &&
      (healthKind !== "tcp" || portNum !== null) &&
      (healthKind !== "http" || url.trim().startsWith("http://")),
  );

  async function save() {
    if (!valid) return;
    saving = true;
    const health: Health | null =
      healthKind === "tcp" && portNum
        ? { kind: "tcp", port: portNum }
        : healthKind === "http"
          ? { kind: "http", url: url.trim(), expect: expect.trim() ? Number(expect) : null }
          : null;
    const svc: Service = {
      id: e?.id ?? `${Date.now().toString(36)}`,
      name: name.trim(),
      order: Number(order) || 1,
      source:
        kind === "command"
          ? { kind: "command", projectId, commandId, worktree: worktree || null }
          : { kind: "compose", projectId, file: file.trim(), service: composeService.trim() },
      port: portNum,
      health,
      dependsOn: e?.dependsOn ?? [],
    };
    const ok = await spaces.upsert(ws.id, svc);
    saving = false;
    if (ok) onclose();
  }
</script>

<form
  class="form"
  onsubmit={(ev) => {
    ev.preventDefault();
    save();
  }}
>
  <div class="eyebrow">{e ? "Edit service" : "Add service"}</div>
  <div class="grid">
    <label>Name<input bind:value={name} placeholder="api" /></label>
    <label>Start order<input bind:value={order} class="mono" inputmode="numeric" /></label>
    <label class="wide">
      Project
      <select bind:value={projectId}>
        <option value="" disabled>Choose a project</option>
        {#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      </select>
    </label>
    <div class="wide seg" role="radiogroup" aria-label="Source">
      <button type="button" class:on={kind === "command"} onclick={() => (kind = "command")}>Run command</button>
      <button type="button" class:on={kind === "compose"} onclick={() => (kind = "compose")}>Docker Compose</button>
    </div>
    {#if kind === "command"}
      <label class="wide">
        Command
        <select bind:value={commandId} class="mono">
          <option value="" disabled>{projectId ? "Choose a command" : "Choose a project first"}</option>
          {#each commands as c (c.id)}<option value={c.id}>{c.name} — {c.commandLine}</option>{/each}
        </select>
      </label>
      {#if otherWorktrees.length}
        <label class="wide">
          Run in
          <select bind:value={worktree} class="mono">
            <option value="">main checkout</option>
            {#each otherWorktrees as w (w)}<option value={w}>{w}</option>{/each}
          </select>
        </label>
      {/if}
    {:else}
      <label>Compose file<input bind:value={file} class="mono" /></label>
      <label>Service<input bind:value={composeService} class="mono" placeholder="db" /></label>
    {/if}
    <label>Port<input bind:value={port} class="mono" inputmode="numeric" placeholder="3000" /></label>
    <label>
      Health check
      <select bind:value={healthKind}>
        <option value="none">none (up after 1.5 s)</option>
        <option value="tcp">TCP port</option>
        <option value="http">HTTP GET</option>
      </select>
    </label>
    {#if healthKind === "http"}
      <label class="wide">URL<input bind:value={url} class="mono" placeholder="http://localhost:3000/health" /></label>
      <label>Expect status<input bind:value={expect} class="mono" placeholder="2xx/3xx" /></label>
    {/if}
  </div>
  {#if healthKind === "tcp" && portNum === null}
    <p class="hint">A TCP check needs the port.</p>
  {/if}
  <div class="actions">
    <Button variant="secondary" onclick={onclose}>Cancel</Button>
    {#if e}
      <button
        type="button"
        class="danger"
        onclick={async () => {
          if (await spaces.removeService(ws.id, e.id)) onclose();
        }}>Remove</button
      >
    {/if}
    <button class="save" type="submit" disabled={!valid || saving}>{saving ? "Saving…" : "Save"}</button>
  </div>
</form>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    padding: var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-panel);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-5) var(--space-6);
  }
  .wide {
    grid-column: span 2;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: 12px;
    color: var(--text-muted);
  }
  input,
  select {
    height: 34px;
    padding: 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    user-select: text;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }
  input:focus,
  select:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .seg {
    display: inline-flex;
    justify-self: start;
    padding: 3px;
    background: var(--bg-app);
    border: 1px solid var(--line);
    border-radius: var(--radius-row);
  }
  .seg button {
    height: 26px;
    padding: 0 var(--space-6);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }
  .seg button.on {
    background: var(--bg-hover);
    color: var(--text);
    font-weight: 600;
  }
  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--error-text);
  }
  .actions {
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
  .save:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .danger {
    height: 36px;
    padding: 0 var(--space-8);
    border: 1px solid var(--error-border);
    border-radius: var(--radius-button);
    background: transparent;
    color: var(--error-text);
    cursor: pointer;
  }
</style>

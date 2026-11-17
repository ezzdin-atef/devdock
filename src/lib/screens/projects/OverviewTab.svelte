<script lang="ts">
  import { agents } from "$lib/agents.svelte";
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { relativeTime, shortDate, shortPath } from "$lib/format";
  import { runs } from "$lib/runs.svelte";
  import { spaces } from "$lib/spaces.svelte";
  import { KIND_LABEL, isActive, type Activity, type Project } from "$lib/types";

  // T8.1: actions, summary tiles that lead to their tab or screen, detected facts,
  // and recent activity.
  let { project }: { project: Project } = $props();

  const ov = $derived(agents.overview);
  const editorName = $derived(ov?.editors.find((e) => e.command === ov.settings.editor)?.name ?? "editor");
  const defaultAgent = $derived(ov?.agents.find((a) => a.id === ov.settings.agent));

  const projectRuns = $derived(runs.list.filter((r) => r.projectId === project.id));
  const active = $derived(projectRuns.filter((r) => isActive(r.state)));
  const lastRun = $derived(projectRuns[0]);
  const livePorts = $derived(active.map((r) => r.portHint).filter((p): p is number => p != null));
  const learned = $derived(Object.entries(project.ports ?? {}));
  const lastLaunch = $derived(project.launches?.[0]);
  const memberOf = $derived(
    spaces.list.filter((w) => w.services.some((s) => s.source.projectId === project.id)).map((w) => w.name),
  );
  const details = $derived(app.gitDetails?.state === "repo" ? app.gitDetails.data : null);
  const git = $derived(project.git);

  const nameOf = (agent: string | null) => (agent ? (ov?.agents.find((a) => a.id === agent)?.name ?? agent) : "Terminal");
  const terminalName = (k: string) => ov?.terminals.find((t) => t.kind === k)?.name ?? k;

  function describe(a: Activity): { icon: "code" | "folder" | "play" | "terminal" | "alert" | "check"; text: string; tone: string } {
    switch (a.kind) {
      case "openedInEditor":
        return { icon: "code", text: `Opened in ${ov?.editors.find((e) => e.command === a.editor)?.name ?? a.editor}`, tone: "" };
      case "openedInExplorer":
        return { icon: "folder", text: "Opened in Explorer", tone: "" };
      case "started":
        return { icon: "play", text: `Started ${a.command}`, tone: "" };
      case "ended":
        return a.outcome === "failed"
          ? { icon: "alert", text: `${a.command} failed${a.code != null ? ` (exit ${a.code})` : ""}`, tone: "bad" }
          : { icon: "check", text: `${a.command} ${a.outcome === "stopped" ? "stopped" : `exited ${a.code ?? 0}`}`, tone: "muted" };
      case "agentLaunched":
        return { icon: "terminal", text: `${nameOf(a.agent)} in ${terminalName(a.terminal)}`, tone: "" };
    }
  }
</script>

<div class="overview">
  <div class="actions">
    <Button variant="primary" onclick={() => app.openInEditor(project.id)}>
      <Icon name="code" size={14} /> Open in {editorName}
    </Button>
    <Button variant="secondary" onclick={() => app.openInExplorer(project.id)}>
      <Icon name="folder" size={14} /> Explorer
    </Button>
    {#if defaultAgent?.path && ov}
      <Button
        variant="secondary"
        onclick={() => agents.launch(project.id, defaultAgent.id, ov.settings.terminal, "")}
        title="Opens {defaultAgent.name} in {terminalName(ov.settings.terminal)} here"
      >
        <Icon name="terminal" size={14} /> {defaultAgent.name}
      </Button>
    {/if}
  </div>

  <div class="tiles">
    <button class="tile" onclick={() => (app.tab = "git")}>
      <span class="eyebrow">Git</span>
      {#if git?.state === "repo"}
        <span class="value mono">{git.data.branch ?? "detached"}</span>
        <span class="detail">
          {git.data.changedFiles === 0 ? "clean" : `${git.data.changedFiles} changed`}{git.data.ahead ? ` · ↑${git.data.ahead}` : ""}{git.data.behind ? ` · ↓${git.data.behind}` : ""}{git.data.upstream ? "" : " · no upstream"}
        </span>
      {:else if git?.state === "noRepo"}
        <span class="value mono muted">no git</span>
        <span class="detail">not a repository</span>
      {:else if git?.state === "error"}
        <span class="value mono bad">error</span>
        <span class="detail">{git.error.message}</span>
      {:else}
        <span class="value mono muted">…</span>
        <span class="detail">checking</span>
      {/if}
    </button>

    <button class="tile" class:running={active.length > 0} onclick={() => (app.tab = "run")}>
      <span class="eyebrow">Dev server</span>
      {#if active.length}
        <span class="value mono ok">● {active.length} running</span>
        <span class="detail mono">{active.map((r) => r.commandLine).join(", ")}</span>
      {:else if lastRun}
        <span class="value mono" class:bad={lastRun.state === "failed"}>
          {lastRun.state === "failed" ? `FAILED ${lastRun.code ?? ""}` : lastRun.state.toUpperCase()}
        </span>
        <span class="detail mono">{lastRun.commandLine}</span>
      {:else}
        <span class="value mono muted">idle</span>
        <span class="detail">nothing run this session</span>
      {/if}
    </button>

    <button class="tile" onclick={() => (app.view = "ports")}>
      <span class="eyebrow">Ports</span>
      {#if livePorts.length}
        <span class="value mono ok">{livePorts.join(" · ")}</span>
        <span class="detail">in use by its runs</span>
      {:else if learned.length}
        <span class="value mono">{learned.map(([, p]) => p).join(" · ")}</span>
        <span class="detail">learned from earlier runs</span>
      {:else}
        <span class="value mono muted">—</span>
        <span class="detail">none known yet</span>
      {/if}
    </button>

    <button class="tile" onclick={() => (app.tab = "agents")}>
      <span class="eyebrow">Agents</span>
      {#if lastLaunch}
        <span class="value">{nameOf(lastLaunch.agent)}</span>
        <span class="detail">{terminalName(lastLaunch.terminal)} · {relativeTime(lastLaunch.at, runs.now)}</span>
      {:else}
        <span class="value muted">none yet</span>
        <span class="detail">{ov ? `${ov.agents.filter((a) => a.path).length} of 3 installed` : ""}</span>
      {/if}
    </button>
  </div>

  <dl class="facts panel">
    <dt>Type</dt>
    <dd class="mono">{KIND_LABEL[project.kind]}</dd>
    <dt>Repository</dt>
    <dd class="mono">
      {#if details}
        {#if details.root === project.path && !details.isWorktree}
          <span class="human">this folder</span>
        {:else}
          <span class="repo" title={details.root}>{shortPath(details.root, 3)}</span>
        {/if}
        {details.inSubfolder ? " (shared repo)" : ""}{details.isWorktree ? " (worktree)" : ""}
      {:else if git?.state === "noRepo"}
        none
      {:else}
        <span class="muted">see Git tab</span>
      {/if}
    </dd>
    {#if details}
      <dt>Remotes</dt>
      <dd class="mono">{details.remotes.map((r) => r.name).join(", ") || "none"}</dd>
    {/if}
    <dt>Commands</dt>
    <dd>
      {#if runs.commandsByProject[project.id]}
        {@const cmds = runs.commandsByProject[project.id].commands}
        <span class="mono">{cmds.map((c) => c.name).join(", ") || "none"}</span>
      {:else}
        <button class="link" onclick={() => (app.tab = "run")}>see Run tab →</button>
      {/if}
    </dd>
    <dt>Workspaces</dt>
    <dd>{memberOf.length ? memberOf.join(", ") : "none"}</dd>
    <dt>Last opened</dt>
    <dd>{project.lastOpened ? relativeTime(project.lastOpened, runs.now) : "Never from DevDock"}</dd>
    <dt>Added</dt>
    <dd>{project.addedAt ? shortDate(project.addedAt) : "Unknown"}</dd>
  </dl>

  <section class="activity panel">
    <div class="head"><span class="title">Recent activity</span></div>
    {#each (project.activity ?? []).slice(0, 10) as a, i (i)}
      {@const d = describe(a)}
      <div class="act {d.tone}">
        <Icon name={d.icon} size={13} />
        <span class="text">{d.text}</span>
        <span class="when mono">{relativeTime(a.at, runs.now)}</span>
      </div>
    {:else}
      <p class="none">Nothing yet. Opening, running and launching from DevDock shows up here.</p>
    {/each}
  </section>
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: var(--space-9);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-5);
  }
  /* Always 2 x 2: four tiles never leave an orphan on the last row. */
  .tiles {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-6);
  }
  /* DESIGN.md "Stat tile": eyebrow, value, muted detail; clickable. */
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    min-width: 0;
    padding: var(--space-7) var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
    text-align: left;
    cursor: pointer;
    transition: border-color var(--t-fast) var(--ease);
  }
  .tile:hover {
    border-color: var(--line-strong);
  }
  .tile.running {
    background: var(--success-panel);
    border-color: var(--success-border);
  }
  .value {
    font-size: 14px;
    font-weight: 600;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail {
    font-size: 12.5px;
    color: var(--text-muted);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail.mono {
    font-size: 11px;
  }
  .ok {
    color: var(--success);
  }
  .bad {
    color: var(--error-text);
  }
  .muted {
    color: var(--text-muted);
  }
  .human {
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .facts {
    display: grid;
    grid-template-columns: 140px 1fr;
    margin: 0;
    padding: var(--space-2) var(--space-8);
  }
  .facts dt,
  .facts dd {
    margin: 0;
    padding: var(--space-5) 0;
    border-top: 1px solid var(--line);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .facts dt:first-of-type,
  .facts dd:first-of-type {
    border-top: 0;
  }
  .facts dt {
    color: var(--text-muted);
  }
  .facts dd.mono {
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
  .head {
    padding: var(--space-6) var(--space-8);
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-weight: 600;
  }
  .act {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-4) var(--space-8);
    border-top: 1px solid var(--line);
    font-size: 13px;
  }
  .act:first-of-type {
    border-top: 0;
  }
  .act :global(svg) {
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .act.bad,
  .act.bad :global(svg) {
    color: var(--error-text);
  }
  .act.muted .text {
    color: var(--text-muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .when {
    font-size: 10.5px;
    color: var(--text-faint);
  }
  .none {
    margin: 0;
    padding: var(--space-7) var(--space-8);
    color: var(--text-muted);
  }
</style>

<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Select from "$lib/components/Select.svelte";
  import * as api from "$lib/api";
  import { clockTime, shortPath } from "$lib/format";
  import type { Branches, GitOp, OpResult, Project } from "$lib/types";

  let { project }: { project: Project } = $props();
  const snap = $derived(app.gitDetails);

  // Branches and the actions that change the repository (switch, fetch, pull, push).
  let branches = $state<Branches | null>(null);
  let busy = $state<GitOp["kind"] | null>(null);
  let result = $state<OpResult | null>(null);
  let showOutput = $state(false);
  let creating = $state(false);
  let newName = $state("");
  let showAll = $state(false);

  const projectId = $derived(project.id);
  $effect(() => {
    void projectId;
    void snap?.refreshedAt;
    loadBranches();
  });
  $effect(() => {
    void projectId;
    result = null;
    creating = false;
  });

  async function loadBranches() {
    if (project.missing) return;
    try {
      branches = await api.gitBranches(project.id);
    } catch {
      branches = null;
    }
  }

  async function act(op: GitOp) {
    busy = op.kind;
    result = null;
    let r: OpResult;
    try {
      r = await api.gitOp(project.id, op);
    } catch (err) {
      r = { ok: false, summary: api.toErrorDto(err).message, output: "" };
    }
    result = r;
    busy = null;
    showOutput = !r.ok && !!r.output;
    if (r.ok && op.kind === "create") {
      creating = false;
      newName = "";
    }
    await loadBranches();
    app.refreshGit();
    app.reload();
  }

  const branchOptions = $derived([
    ...(branches?.local ?? []).map((b) => ({
      value: b.name,
      label: b.name,
      hint: b.gone ? "remote deleted" : b.updated,
    })),
    ...(branches?.remoteOnly ?? []).map((r) => ({ value: r, label: r, hint: "remote" })),
  ]);
  const listed = $derived(showAll ? (branches?.local ?? []) : (branches?.local ?? []).slice(0, 6));
</script>

<div class="git">
  <div class="toolbar">
    <span class="stamp mono">
      {#if app.gitLoading}
        refreshing…
      {:else if snap}
        refreshed {clockTime(snap.refreshedAt)}
      {/if}
    </span>
    <Button variant="secondary" onclick={() => app.refreshGit()} disabled={app.gitLoading || project.missing}>
      <Icon name="refresh" size={14} /> Refresh
    </Button>
  </div>

  {#if project.missing}
    <p class="muted">The folder is missing, so there is no Git state to read.</p>
  {:else if !snap}
    <p class="muted">Reading the repository…</p>
  {:else if snap.state === "noRepo"}
    <div class="panel empty">
      <div class="eyebrow">No git</div>
      <p>
        This folder isn't inside a Git repository, so there is no branch or file state to show.
        Run <span class="mono">git init</span> in the folder if you want DevDock to track it.
      </p>
    </div>
  {:else if snap.state === "error"}
    <ErrorBanner error={snap.error} ondismiss={() => app.refreshGit()} />
  {:else}
    {@const info = snap.data}
    {@const s = info.status}
    <section class="branch-strip panel">
      <div class="branch-main">
        <div class="eyebrow">{s.branch ? "Branch" : "Detached HEAD"}</div>
        <div class="branch-row">
          {#if branches && s.branch}
            <Select
              value={s.branch}
              mono
              width="260px"
              ariaLabel="Switch branch"
              disabled={busy !== null}
              options={branchOptions}
              onchange={(b) => act({ kind: "switch", branch: b })}
            />
          {:else}
            <span class="branch mono selectable">{s.branch ?? s.oid?.slice(0, 7) ?? "no commits yet"}</span>
          {/if}
          {#if !creating}
            <Button variant="ghost" onclick={() => (creating = true)} disabled={busy !== null}>
              <Icon name="plus" size={13} /> New branch
            </Button>
          {/if}
        </div>
        <div class="upstream">
          {#if s.upstream}
            <span class="mono muted">tracks {s.upstream}</span>
          {:else}
            <span class="mono muted">no upstream yet; Push sets it</span>
          {/if}
        </div>
      </div>
      <div class="sync">
        <Button variant="secondary" onclick={() => act({ kind: "fetch" })} disabled={busy !== null}>
          <Icon name="refresh" size={13} />
          {busy === "fetch" ? "Fetching…" : "Fetch"}
        </Button>
        <Button
          variant={(s.behind ?? 0) > 0 ? "primary" : "secondary"}
          onclick={() => act({ kind: "pull" })}
          disabled={busy !== null || !s.upstream}
          title={s.upstream ? "Fast-forward to the remote branch" : "This branch has no upstream to pull from"}
        >
          {busy === "pull" ? "Pulling…" : "Pull"}
          <span class="mono count" class:on={(s.behind ?? 0) > 0}>↓{s.behind ?? 0}</span>
        </Button>
        <Button
          variant={(s.ahead ?? 0) > 0 && (s.behind ?? 0) === 0 ? "primary" : "secondary"}
          onclick={() => act({ kind: "push" })}
          disabled={busy !== null || !s.branch}
          title="Send your commits to the remote"
        >
          {busy === "push" ? "Pushing…" : "Push"}
          <span class="mono count" class:on={(s.ahead ?? 0) > 0}>↑{s.ahead ?? 0}</span>
        </Button>
      </div>
    </section>

    {#if creating}
      <form
        class="create panel"
        onsubmit={(e) => {
          e.preventDefault();
          if (newName.trim()) act({ kind: "create", name: newName });
        }}
      >
        <span class="muted small">New branch from <span class="mono">{s.branch ?? "this commit"}</span></span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="mono"
          bind:value={newName}
          placeholder="feature/login"
          spellcheck="false"
          autofocus
          aria-label="New branch name"
          onkeydown={(e) => {
            if (e.key === "Escape") creating = false;
          }}
        />
        <Button variant="secondary" onclick={() => (creating = false)}>Cancel</Button>
        <button class="go" type="submit" disabled={!newName.trim() || busy !== null}>
          {busy === "create" ? "Creating…" : "Create and switch"}
        </button>
      </form>
    {/if}

    {#if result}
      <div class="result" class:bad={!result.ok} role="status">
        <div class="result-head">
          <Icon name={result.ok ? "check" : "alert"} size={14} />
          <span class="result-title">{result.summary}</span>
          <span class="spacer"></span>
          {#if result.output}
            <button class="link" onclick={() => (showOutput = !showOutput)}>
              {showOutput ? "Hide" : "Show"} git output
            </button>
          {/if}
          <button class="x" aria-label="Dismiss" onclick={() => (result = null)}><Icon name="x" size={12} /></button>
        </div>
        {#if showOutput && result.output}
          <pre class="output mono selectable">{result.output}</pre>
        {/if}
      </div>
    {/if}

    {#if info.isWorktree || info.inSubfolder}
      <div class="notes">
        {#if info.isWorktree}
          <span class="note mono">WORKTREE</span>
          <span class="muted">This folder is a linked worktree of the repository.</span>
        {/if}
        {#if info.inSubfolder}
          <span class="note mono">SHARED REPO</span>
          <span class="muted">
            Inside <span class="mono">{shortPath(info.root)}</span>. Counts are for the whole repository,
            the same as <span class="mono">git status</span>.
          </span>
        {/if}
      </div>
    {/if}

    <section class="counters">
      <div class="tile panel">
        <div class="eyebrow">Staged</div>
        <div class="num mono">{s.staged}</div>
      </div>
      <div class="tile panel">
        <div class="eyebrow">Modified</div>
        <div class="num mono">{s.modified}</div>
      </div>
      <div class="tile panel">
        <div class="eyebrow">Untracked</div>
        <div class="num mono">{s.untracked}</div>
      </div>
      {#if s.conflicted > 0}
        <div class="tile panel conflict">
          <div class="eyebrow">Conflicts</div>
          <div class="num mono">{s.conflicted}</div>
        </div>
      {/if}
    </section>

    <dl class="facts panel">
      <dt>Repository</dt>
      <dd class="mono selectable" title={info.root}>{info.root}</dd>
      <dt>Last commit</dt>
      <dd>
        {#if info.lastCommit}
          <span class="mono accent">{info.lastCommit.hash}</span>
          <span class="selectable">{info.lastCommit.subject}</span>
          <div class="muted small">{info.lastCommit.author} · {info.lastCommit.relative}</div>
        {:else}
          <span class="muted">No commits yet</span>
        {/if}
      </dd>
      <dt>Remotes</dt>
      <dd>
        {#each info.remotes as r (r.name)}
          <div class="mono small selectable"><span class="muted">{r.name}</span> {r.url}</div>
        {:else}
          <span class="muted">None</span>
        {/each}
      </dd>
      <dt>Branches</dt>
      <dd>
        {#each listed as b (b.name)}
          <div class="br-row">
            <span class="mono small" class:accent={b.current}>{b.current ? "●" : "○"} {b.name}</span>
            <span class="muted small">
              {#if b.gone}remote deleted{:else if b.upstream}{b.ahead ? `↑${b.ahead} ` : ""}{b.behind
                  ? `↓${b.behind} `
                  : ""}{b.updated}{:else}local only · {b.updated}{/if}
            </span>
            <span class="spacer"></span>
            {#if !b.current}
              <button class="link" disabled={busy !== null} onclick={() => act({ kind: "switch", branch: b.name })}
                >Switch</button
              >
            {/if}
          </div>
        {:else}
          <span class="muted">{branches ? "None" : "Reading…"}</span>
        {/each}
        {#if (branches?.local.length ?? 0) > 6}
          <button class="link small" onclick={() => (showAll = !showAll)}
            >{showAll ? "Show fewer" : `Show all ${branches?.local.length}`}</button
          >
        {/if}
        {#if branches?.remoteOnly.length}
          <div class="muted small remote-title">On the remote only</div>
          {#each branches.remoteOnly.slice(0, showAll ? undefined : 6) as r (r)}
            <div class="br-row">
              <span class="mono small">○ {r}</span>
              <span class="spacer"></span>
              <button class="link" disabled={busy !== null} onclick={() => act({ kind: "switch", branch: r })}
                >Check out</button
              >
            </div>
          {/each}
        {/if}
      </dd>
      <dt>Worktrees</dt>
      <dd>
        {#each info.worktrees as w (w.path)}
          <div class="mono small" title={w.path}>
            <span class:accent={w.isCurrent}>{w.isCurrent ? "●" : "○"}</span>
            {shortPath(w.path)}
            <span class="muted">{w.bare ? "bare" : w.detached ? "detached" : (w.branch ?? "")}</span>
          </div>
        {/each}
      </dd>
    </dl>

    <p class="readonly">
      <Icon name="alert" size={13} /> Pull only fast-forwards. DevDock never merges, rebases, commits or resets;
      when that's needed it tells you, and you decide in a terminal.
    </p>
  {/if}
</div>

<style>
  .git {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-6);
  }
  .stamp {
    font-size: 12px;
    color: var(--text-muted);
  }
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  .empty {
    padding: var(--space-8);
  }
  .empty p {
    margin: var(--space-3) 0 0;
    color: var(--text-soft);
  }
  .muted {
    color: var(--text-muted);
  }
  .small {
    font-size: 12.5px;
  }
  .accent {
    color: var(--accent-fg);
  }
  .branch-strip {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-8);
    padding: var(--space-8);
  }
  .branch-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    margin: var(--space-3) 0;
  }
  .sync {
    display: flex;
    gap: var(--space-4);
  }
  .count {
    font-size: 12px;
    opacity: 0.6;
  }
  .count.on {
    opacity: 1;
  }
  .create {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-8);
  }
  .create input {
    flex: 1;
    min-width: 180px;
    height: 34px;
    padding: 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font-size: 13.5px;
    user-select: text;
  }
  .create input:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .go {
    height: 36px;
    padding: 0 var(--space-8);
    border: 0;
    border-radius: var(--radius-button);
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
    cursor: pointer;
  }
  .go:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .result {
    padding: var(--space-5) var(--space-7);
    border: 1px solid var(--line);
    border-left: 3px solid var(--success);
    border-radius: var(--radius-row);
    background: var(--bg-panel);
  }
  .result.bad {
    border-color: var(--error-border);
    border-left-color: var(--error);
    background: var(--error-row);
  }
  .result-head {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    color: var(--success);
  }
  .result.bad .result-head {
    color: var(--error-text);
  }
  .result-title {
    color: var(--text);
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .link {
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--accent-fg);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .link:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--icon);
    cursor: pointer;
  }
  .x:hover {
    background: var(--bg-hover);
  }
  .output {
    margin: var(--space-4) 0 0;
    padding: var(--space-5);
    max-height: 240px;
    overflow: auto;
    background: var(--bg-console);
    border-radius: var(--radius-row);
    font-size: 12px;
    white-space: pre-wrap;
    color: var(--text-soft);
  }
  .br-row {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: 2px 0;
  }
  .remote-title {
    margin: var(--space-5) 0 var(--space-2);
  }
  .branch {
    margin-top: var(--space-2);
    font-size: 18px;
    overflow-wrap: anywhere;
  }
  .upstream {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    font-size: 12.5px;
  }
  .notes {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--space-3) var(--space-5);
    align-items: baseline;
  }
  .note {
    font-size: 11px;
    padding: 2px 7px;
    border: 1px solid var(--line-badge);
    border-radius: var(--radius-badge);
    color: var(--text-soft);
  }
  .counters {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: var(--space-6);
  }
  .tile {
    padding: var(--space-7) var(--space-8);
  }
  .num {
    margin-top: var(--space-3);
    font-size: 30px;
    line-height: 1;
  }
  .conflict {
    background: var(--error-row);
    border-color: var(--error-border);
  }
  .conflict .num {
    color: var(--error-text);
  }
  .facts {
    display: grid;
    grid-template-columns: 120px 1fr;
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
    font-size: 12.5px;
  }
  .readonly {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: 0;
    font-size: 12.5px;
    color: var(--text-muted);
  }
</style>

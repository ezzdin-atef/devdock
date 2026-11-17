<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Button from "$lib/components/Button.svelte";
  import ErrorBanner from "$lib/components/ErrorBanner.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { clockTime, shortPath } from "$lib/format";
  import type { Project } from "$lib/types";

  let { project }: { project: Project } = $props();
  const snap = $derived(app.gitDetails);
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
        <div class="branch mono selectable">{s.branch ?? s.oid?.slice(0, 7) ?? "no commits yet"}</div>
      </div>
      <div class="upstream">
        {#if s.upstream}
          <span class="mono muted">→ {s.upstream}</span>
          <span class="ab mono">
            <span class:accent={(s.ahead ?? 0) > 0} title="Commits to push">↑{s.ahead ?? 0}</span>
            <span class:info={(s.behind ?? 0) > 0} title="Commits to pull">↓{s.behind ?? 0}</span>
          </span>
        {:else}
          <span class="mono muted">no upstream</span>
        {/if}
      </div>
    </section>

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
      <Icon name="alert" size={13} /> DevDock only reads this repository. It never commits, resets,
      pulls or switches branches.
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
  .info {
    color: var(--info);
  }
  .branch-strip {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-8);
    padding: var(--space-8);
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
  .ab {
    display: inline-flex;
    gap: var(--space-4);
    font-size: 14px;
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

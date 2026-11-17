<script lang="ts">
  import type { GitStatus, Snapshot } from "$lib/types";

  // Compact git state for a list row: branch, change count, ahead/behind.
  // Status is never colour alone: counts carry a symbol and a tooltip.
  let { git }: { git: Snapshot<GitStatus> | null } = $props();
</script>

{#if git?.state === "repo"}
  {@const s = git.data}
  <span class="badge mono" title={s.branch ? `Branch ${s.branch}` : "Detached HEAD"}>
    <span class="branch">{s.branch ?? "detached"}</span>
    {#if s.changedFiles > 0}
      <span class="changes" title="{s.changedFiles} changed {s.changedFiles === 1 ? 'file' : 'files'} (staged, modified or untracked)"
        ><span class="dot" aria-hidden="true"></span>{s.changedFiles}</span
      >
    {/if}
    {#if s.ahead}
      <span class="ahead" title="{s.ahead} commits to push">↑{s.ahead}</span>
    {/if}
    {#if s.behind}
      <span class="behind" title="{s.behind} commits to pull">↓{s.behind}</span>
    {/if}
  </span>
{:else if git?.state === "noRepo"}
  <span class="badge mono none">no git</span>
{:else if git?.state === "error"}
  <span class="badge mono err" title={git.error.message}>git error</span>
{/if}

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--space-4);
    min-width: 0;
    font-size: 12px;
    color: var(--text-muted);
  }
  .branch {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .changes {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    color: var(--accent-fg);
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
  }
  .ahead {
    color: var(--accent-fg);
  }
  .behind {
    color: var(--info);
  }
  .none {
    color: var(--text-faint);
  }
  .err {
    color: var(--error-text);
  }
</style>

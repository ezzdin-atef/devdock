<script lang="ts">
  import { agents } from "$lib/agents.svelte";
  import { toolPath, type ToolScan } from "$lib/types";

  // Instrument panel: counts left, tool detection right. Status always has a word.
  let { projectCount, tools }: { projectCount: number; tools: ToolScan | null } = $props();

  const editor = $derived(agents.overview?.settings.editor ?? "code");
  const items = $derived(
    tools
      ? [
          { id: "explorer", label: "explorer" },
          { id: editor, label: editor },
          { id: "git", label: "git" },
        ].map((t) => ({ ...t, path: toolPath(tools, t.id) }))
      : [],
  );
  const agentCount = $derived(agents.overview?.agents.filter((a) => a.path).length ?? null);
</script>

<footer class="strip mono">
  <span>{projectCount} {projectCount === 1 ? "project" : "projects"}</span>
  <span class="spacer"></span>
  {#if tools}
    {#each items as t (t.id)}
      <span class={t.path ? "ok" : "bad"} title={t.path ?? `${t.label} not found on PATH`}>
        {t.label} {t.path ? "found" : "missing"}
      </span>
    {/each}
    {#if agentCount !== null}
      <span class={agentCount > 0 ? "ok" : "idle"}>{agentCount}/3 agents</span>
    {/if}
  {:else}
    <span>checking tools</span>
  {/if}
</footer>

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: var(--space-8);
    height: var(--status-strip-h);
    padding: 0 var(--space-8);
    background: var(--bg-chrome);
    border-top: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-muted);
  }
  .spacer {
    flex: 1;
  }
  .ok::before,
  .bad::before {
    content: "●";
    margin-right: var(--space-3);
  }
  .ok::before {
    color: var(--success);
  }
  .idle::before {
    content: "○";
    margin-right: var(--space-3);
  }
  .bad {
    color: var(--error-text);
  }
  .bad::before {
    color: var(--error);
  }
</style>

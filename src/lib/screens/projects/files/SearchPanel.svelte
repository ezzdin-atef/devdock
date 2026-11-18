<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { files } from "$lib/files.svelte";
  import { fileIcon } from "./fileIcon";
  import type { LineMatch, ProjectId } from "$lib/types";

  // Search across the project's files (gitignored ones are skipped). Typing
  // searches after a short pause; Enter searches now.
  let { projectId }: { projectId: ProjectId } = $props();

  const pf = $derived(files.of(projectId));
  let input: HTMLInputElement;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let showFilters = $state(false);
  let collapsed = $state<Record<string, true>>({});

  onMount(() => {
    input.focus();
    input.select();
    if (pf.query.include || pf.query.exclude) showFilters = true;
    return () => {
      if (timer) clearTimeout(timer);
    };
  });

  /** Lets the tab focus the box (Ctrl+Shift+F while already open). */
  export function focus() {
    input?.focus();
    input?.select();
  }

  function soon() {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => files.search(projectId), 250);
  }

  function now() {
    if (timer) clearTimeout(timer);
    files.search(projectId);
  }

  function toggle(key: "regex" | "matchCase" | "wholeWord") {
    pf.query[key] = !pf.query[key];
    now();
  }

  /** The shown line split into plain and highlighted parts. */
  function parts(m: LineMatch): { text: string; hit: boolean }[] {
    const out: { text: string; hit: boolean }[] = [];
    let at = 0;
    for (const [a, b] of m.ranges) {
      if (a > at) out.push({ text: m.text.slice(at, a), hit: false });
      out.push({ text: m.text.slice(a, b), hit: true });
      at = b;
    }
    if (at < m.text.length) out.push({ text: m.text.slice(at), hit: false });
    return out;
  }

  function reveal(path: string, m: LineMatch) {
    const [a, b] = m.ranges[0] ?? [0, 0];
    files.open(projectId, path, { line: m.line, from: m.offset + a, to: m.offset + b });
  }

  const fileCount = $derived(pf.result?.files.length ?? 0);
</script>

<div class="search">
  <div class="box">
    <input
      bind:this={input}
      bind:value={pf.query.text}
      class="mono"
      placeholder="Search in files"
      aria-label="Search in files"
      spellcheck="false"
      oninput={soon}
      onkeydown={(e) => {
        if (e.key === "Enter") now();
        if (e.key === "Escape" && pf.query.text) {
          e.stopPropagation();
          pf.query.text = "";
          now();
        }
      }}
    />
    <div class="toggles">
      <button class:on={pf.query.matchCase} aria-pressed={pf.query.matchCase} title="Match case" onclick={() => toggle("matchCase")}>Aa</button>
      <button class:on={pf.query.wholeWord} aria-pressed={pf.query.wholeWord} title="Whole word" onclick={() => toggle("wholeWord")}><u>ab</u></button>
      <button class:on={pf.query.regex} aria-pressed={pf.query.regex} title="Regular expression" onclick={() => toggle("regex")}>.*</button>
    </div>
  </div>
  <button class="filters-link" aria-expanded={showFilters} onclick={() => (showFilters = !showFilters)}>
    {showFilters ? "Hide" : "Show"} file filters
  </button>
  {#if showFilters}
    <input
      class="filter mono"
      bind:value={pf.query.include}
      placeholder="Include: src/**, *.ts"
      aria-label="Files to include"
      spellcheck="false"
      oninput={soon}
    />
    <input
      class="filter mono"
      bind:value={pf.query.exclude}
      placeholder="Exclude: *.test.ts, dist"
      aria-label="Files to exclude"
      spellcheck="false"
      oninput={soon}
    />
  {/if}

  <div class="summary" aria-live="polite">
    {#if pf.searching}
      Searching…
    {:else if pf.error}
      <span class="bad">{pf.error.message}</span>
    {:else if pf.result && pf.query.text}
      {#if pf.result.total === 0}
        No results
      {:else}
        {pf.result.total}{pf.result.truncated ? "+" : ""} {pf.result.total === 1 ? "result" : "results"} in {fileCount} {fileCount === 1 ? "file" : "files"}
        {#if pf.result.truncated}<span class="faint">· showing the first {pf.result.total}</span>{/if}
      {/if}
    {/if}
  </div>

  <div class="results">
    {#each pf.result?.files ?? [] as f (f.path)}
      {@const slash = f.path.lastIndexOf("/")}
      <button class="file" onclick={() => (collapsed[f.path] ? delete collapsed[f.path] : (collapsed[f.path] = true))} title={f.path}>
        <span class="caret" class:open={!collapsed[f.path]}>▾</span>
        <span class="kind" style:color={fileIcon(f.path.slice(slash + 1)).tint}><Icon name={fileIcon(f.path.slice(slash + 1)).icon} size={14} /></span>
        <span class="fname">{f.path.slice(slash + 1)}</span>
        <span class="fdir">{slash > 0 ? f.path.slice(0, slash) : ""}</span>
        <span class="count mono">{f.matches.length}</span>
      </button>
      {#if !collapsed[f.path]}
        {#each f.matches as m (m.line)}
          <button class="hit mono" onclick={() => reveal(f.path, m)} title="Line {m.line}">
            <span class="ln">{m.line}</span>
            <span class="text">{#if m.offset > 0}…{/if}{#each parts(m) as p, i (i)}{#if p.hit}<mark>{p.text}</mark>{:else}{i === 0 ? p.text.trimStart() : p.text}{/if}{/each}</span>
          </button>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<style>
  .search {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-height: 0;
    flex: 1;
    padding: var(--space-4);
  }
  .box {
    position: relative;
  }
  input {
    width: 100%;
    height: 32px;
    padding: 0 96px 0 var(--space-4);
    border: 1px solid var(--btn-border);
    border-radius: var(--radius-button-sm);
    background: var(--btn-bg);
    color: var(--text);
    font-size: 12.5px;
  }
  input.filter {
    height: 28px;
    padding-right: var(--space-4);
    font-size: 12px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
  .toggles {
    position: absolute;
    top: 4px;
    right: 4px;
    display: flex;
    gap: 2px;
  }
  .toggles button {
    width: 28px;
    height: 24px;
    border: 1px solid transparent;
    border-radius: var(--radius-badge);
    background: transparent;
    color: var(--text-muted);
    font: 600 11.5px var(--font-mono);
    cursor: pointer;
  }
  .toggles button:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .toggles button.on {
    border-color: var(--accent-panel-border);
    background: var(--accent-tint);
    color: var(--accent-fg);
  }
  .filters-link {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .filters-link:hover {
    color: var(--text);
  }
  .summary {
    min-height: 18px;
    color: var(--text-muted);
    font-size: 12px;
  }
  .summary .bad {
    color: var(--error-text);
  }
  .faint {
    color: var(--text-faint);
  }
  .results {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: 0 calc(-1 * var(--space-4));
    padding: 0 var(--space-2);
  }
  .file,
  .hit {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-soft);
    text-align: left;
    cursor: pointer;
  }
  .file {
    height: 26px;
    padding: 0 var(--space-3);
    font: inherit;
    font-size: 13px;
  }
  .hit {
    min-height: 22px;
    padding: 2px var(--space-3) 2px 26px;
    font-size: 12px;
  }
  .file:hover,
  .hit:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .caret {
    width: 10px;
    color: var(--icon);
    font-size: 10px;
    transform: rotate(-90deg);
    transition: transform var(--t-fast) var(--ease);
  }
  .caret.open {
    transform: none;
  }
  .kind {
    display: grid;
    flex-shrink: 0;
  }
  .fname {
    flex-shrink: 0;
    font-weight: 600;
  }
  .fdir {
    min-width: 0;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    margin-left: auto;
    padding: 0 6px;
    border-radius: var(--radius-pill);
    background: var(--bg-raised);
    color: var(--text-muted);
    font-size: 11px;
  }
  .ln {
    flex-shrink: 0;
    min-width: 28px;
    color: var(--text-faint);
    text-align: right;
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  mark {
    border-radius: 2px;
    background: color-mix(in srgb, var(--accent) 32%, transparent);
    color: var(--text);
  }
</style>

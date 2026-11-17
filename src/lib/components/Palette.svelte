<script lang="ts">
  import { onMount, tick } from "svelte";
  import { palette } from "$lib/palette.svelte";
  import Icon from "./Icon.svelte";

  // DESIGN.md "Command palette": 600 wide, 80 px from the top, input row 54 high
  // with an Esc hint, grouped results, matched letters bold, mono hint on the right.
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();

  onMount(() => {
    input?.focus();
  });

  // Keep the active row visible while moving with the arrows.
  $effect(() => {
    const i = palette.active;
    tick().then(() => list?.querySelector(`[data-index="${i}"]`)?.scrollIntoView({ block: "nearest" }));
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      palette.move(1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      palette.move(-1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      palette.runActive();
    } else if (e.key === "Escape") {
      e.preventDefault();
      palette.close();
    }
  }

  /** Splits a label into runs of matched / unmatched characters. */
  function parts(label: string, positions: number[]): { text: string; hit: boolean }[] {
    const chars = [...label];
    const set = new Set(positions);
    const out: { text: string; hit: boolean }[] = [];
    chars.forEach((c, i) => {
      const hit = set.has(i);
      const last = out[out.length - 1];
      if (last && last.hit === hit) last.text += c;
      else out.push({ text: c, hit });
    });
    return out;
  }

  /** Index of each group's first hit in the flat list. */
  const offsets = $derived(
    palette.groups.map((_, gi) => palette.groups.slice(0, gi).reduce((n, g) => n + g.hits.length, 0)),
  );
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={() => palette.close()}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="palette" role="dialog" tabindex="-1" aria-modal="true" aria-label="Command palette" onclick={(e) => e.stopPropagation()}>
    <div class="input-row">
      <Icon name="search" size={16} />
      <input
        bind:this={input}
        value={palette.query}
        oninput={(e) => palette.setQuery(e.currentTarget.value)}
        onkeydown={onKey}
        placeholder="Jump to a project, command, agent, workspace or port…"
        aria-label="Search"
        aria-activedescendant={palette.flat.length ? `pal-${palette.active}` : undefined}
        aria-controls="palette-results"
        spellcheck="false"
      />
      <span class="key mono">Esc</span>
    </div>
    <div class="results" id="palette-results" role="listbox" bind:this={list}>
      {#if palette.loading}
        <p class="empty">Gathering projects, commands and ports…</p>
      {:else if !palette.flat.length}
        <p class="empty">Nothing matches “{palette.query}”.</p>
      {/if}
      {#each palette.groups as g, gi (g.group)}
        <div class="eyebrow group">{g.title}</div>
        {#each g.hits as hit, hi (hit.label + hit.hint)}
          {@const i = offsets[gi] + hi}
          <button
            id="pal-{i}"
            data-index={i}
            role="option"
            aria-selected={palette.active === i}
            class="hit"
            class:active={palette.active === i}
            onmousemove={() => (palette.active = i)}
            onclick={() => palette.run(hit.action)}
          >
            <span class="label">
              {#each parts(hit.label, hit.positions) as p, pi (pi)}{#if p.hit}<strong>{p.text}</strong>{:else}{p.text}{/if}{/each}
            </span>
            <span class="hint mono">{hit.hint}</span>
          </button>
        {/each}
      {/each}
    </div>
    <div class="foot mono">↑↓ move · Enter run · Esc close</div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--scrim);
  }
  .palette {
    position: absolute;
    top: 80px;
    left: 50%;
    transform: translateX(-50%);
    width: min(600px, calc(100vw - 48px));
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: var(--radius-dialog);
    box-shadow: var(--shadow-dialog);
    overflow: hidden;
    animation: in 180ms var(--ease);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateX(-50%) scale(0.98);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .palette {
      animation: none;
    }
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    height: 54px;
    padding: 0 var(--space-8);
    border-bottom: 1px solid var(--line);
    color: var(--text-muted);
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 16px;
    user-select: text;
  }
  input::placeholder {
    color: var(--text-muted);
  }
  .key {
    padding: 2px 6px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-badge);
    font-size: 11px;
  }
  .results {
    max-height: min(440px, 60vh);
    overflow-y: auto;
    padding: var(--space-3);
  }
  .group {
    padding: var(--space-5) var(--space-5) var(--space-3);
  }
  .hit {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    width: 100%;
    height: 38px;
    padding: 0 var(--space-5);
    border: 0;
    border-radius: var(--radius-button);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .hit.active {
    background: var(--accent-active);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-soft);
  }
  .label strong {
    color: var(--text);
    font-weight: 700;
  }
  .hint {
    max-width: 45%;
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    margin: 0;
    padding: var(--space-8);
    color: var(--text-muted);
  }
  .foot {
    padding: var(--space-4) var(--space-8);
    border-top: 1px solid var(--line);
    font-size: 11.5px;
    color: var(--text-faint);
  }
</style>

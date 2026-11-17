<script lang="ts">
  import { shortPath } from "$lib/format";
  import Icon from "./Icon.svelte";

  // A long Windows path on one line: drive + the folders that identify it, the full
  // path on hover, and a copy button that confirms in place.
  let {
    path,
    tail = 4,
    size = 12,
    wrap = false,
  }: { path: string; tail?: number; size?: number; wrap?: boolean } = $props();

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy() {
    try {
      await navigator.clipboard.writeText(path);
      copied = true;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1400);
    } catch {
      /* clipboard refused: the path is still selectable */
    }
  }
</script>

<span class="path-line" class:wrap>
  <span class="text mono selectable" title={path} style="font-size: {size}px">{shortPath(path, tail)}</span>
  <button
    class="copy"
    class:done={copied}
    onclick={copy}
    aria-label={copied ? "Path copied" : "Copy full path"}
    title={copied ? "Copied" : "Copy full path"}
  >
    <Icon name={copied ? "check" : "copy"} size={13} />
  </button>
</span>

<style>
  .path-line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  .text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
  }
  .wrap {
    align-items: flex-start;
  }
  .wrap .text {
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .copy {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    opacity: 0.7;
    transition:
      opacity var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease),
      color var(--t-fast) var(--ease);
  }
  .path-line:hover .copy,
  .copy:focus-visible {
    opacity: 1;
    color: var(--text-muted);
  }
  .copy:hover {
    background: var(--bg-hover);
    color: var(--text);
  }
  .copy.done {
    opacity: 1;
    color: var(--success);
  }
</style>

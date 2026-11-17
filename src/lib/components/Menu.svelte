<script lang="ts" module>
  /** Where to open: under a button (`end`: right edges line up) or at the cursor. */
  export interface MenuAnchor {
    x: number;
    y: number;
    /** Bottom edge to open above when there's no room below (button top, or cursor). */
    flipY: number;
    align: "start" | "end";
  }

  /** Anchor under a clicked button. */
  export function belowButton(e: MouseEvent): MenuAnchor {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return { x: r.right, y: r.bottom + 4, flipY: r.top - 4, align: "end" };
  }

  /** Anchor at the cursor (right-click). */
  export function atCursor(e: MouseEvent): MenuAnchor {
    return { x: e.clientX, y: e.clientY, flipY: e.clientY, align: "start" };
  }
</script>

<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  // DESIGN.md "Context menu". Rendered on <body> with fixed coordinates, so a
  // scrolling list can't clip it; it opens upward and shifts left when the window
  // has no room. Items are buttons with role="menuitem".
  let {
    at,
    label,
    width = 220,
    onclose,
    children,
  }: { at: MenuAnchor; label: string; width?: number; onclose: () => void; children: Snippet } = $props();
  let menu: HTMLDivElement;
  let pos = $state({ left: -9999, top: -9999, up: false });

  const EDGE = 8;

  function place() {
    const w = menu.offsetWidth;
    const h = menu.offsetHeight;
    let left = at.align === "end" ? at.x - w : at.x;
    left = Math.max(EDGE, Math.min(left, window.innerWidth - w - EDGE));
    let top = at.y;
    let up = false;
    if (top + h > window.innerHeight - EDGE && at.flipY - h >= EDGE) {
      top = at.flipY - h;
      up = true;
    }
    top = Math.max(EDGE, Math.min(top, window.innerHeight - h - EDGE));
    pos = { left, top, up };
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  const items = () => [...menu.querySelectorAll<HTMLButtonElement>('[role^="menuitem"]:not(:disabled)')];

  onMount(() => {
    place();
    items()[0]?.focus();
    const onPointer = (e: PointerEvent) => {
      if (!menu.contains(e.target as Node)) onclose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Tab") {
        e.preventDefault();
        onclose();
        return;
      }
      if (document.activeElement?.tagName === "INPUT") return;
      const list = items();
      const i = list.indexOf(document.activeElement as HTMLButtonElement);
      const next =
        e.key === "ArrowDown" ? (i + 1) % list.length
        : e.key === "ArrowUp" ? (i - 1 + list.length) % list.length
        : e.key === "Home" ? 0
        : e.key === "End" ? list.length - 1
        : -1;
      if (next >= 0) {
        e.preventDefault();
        list[next]?.focus();
      }
    };
    // The page moved under the menu: close rather than float in the wrong place.
    const onMove = (e: Event) => {
      if (!menu.contains(e.target as Node)) onclose();
    };
    const onBlur = () => onclose();
    // Defer so the click that opened the menu doesn't close it.
    const t = setTimeout(() => window.addEventListener("pointerdown", onPointer, true));
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", onBlur);
    window.addEventListener("blur", onBlur);
    window.addEventListener("scroll", onMove, true);
    return () => {
      clearTimeout(t);
      window.removeEventListener("pointerdown", onPointer, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", onBlur);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("scroll", onMove, true);
    };
  });
</script>

<div
  class="menu"
  class:up={pos.up}
  role="menu"
  tabindex="-1"
  aria-label={label}
  bind:this={menu}
  use:portal
  style="left: {pos.left}px; top: {pos.top}px; width: {width}px"
  oncontextmenu={(e) => e.preventDefault()}
>
  {@render children()}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 1000;
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: var(--space-3);
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: 10px;
    box-shadow: var(--shadow-menu);
    animation: dd-overlay 140ms var(--ease);
    transform-origin: top left;
    outline: none;
  }
  .menu.up {
    transform-origin: bottom left;
  }
  .menu :global([role="menuitem"]) {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    height: 32px;
    padding: 0 var(--space-5);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  .menu :global([role="menuitem"]:hover),
  .menu :global([role="menuitem"]:focus-visible) {
    background: var(--bg-hover);
    outline: none;
  }
  .menu :global([role="menuitem"]:disabled) {
    color: var(--text-faint);
    cursor: default;
    background: transparent;
  }
  .menu :global(hr) {
    border: 0;
    border-top: 1px solid var(--line);
    margin: var(--space-3) 0;
  }
  .menu :global(.danger) {
    color: var(--error-text);
  }
  .menu :global(.menu-label) {
    padding: var(--space-3) var(--space-5) var(--space-1);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  .menu :global(.note) {
    margin: var(--space-1) var(--space-5) var(--space-2);
    font-size: 12.5px;
    color: var(--text-muted);
  }
</style>

<script lang="ts" module>
  export interface SelectOption<T> {
    value: T;
    label: string;
    /** Muted text after the label, e.g. "not found". */
    hint?: string;
    disabled?: boolean;
  }
</script>

<script lang="ts" generics="T extends string | number">
  import { onDestroy, tick } from "svelte";
  import Icon from "./Icon.svelte";

  // A drop-down that looks like the rest of DevDock. The list floats on <body>
  // (fixed position), so scrolling panels can't clip it, and opens upward when
  // there's no room below. Keyboard: arrows, Home/End, Enter/Space, Escape,
  // and typing a letter jumps to the next option starting with it.
  let {
    value = $bindable(),
    options,
    placeholder = "Choose…",
    disabled = false,
    mono = false,
    variant = "field",
    ariaLabel,
    title,
    width,
    onchange,
  }: {
    value: T;
    options: SelectOption<T>[];
    placeholder?: string;
    disabled?: boolean;
    mono?: boolean;
    /** `field`: bordered like an input. `quiet`: text only, for toolbars. */
    variant?: "field" | "quiet";
    ariaLabel?: string;
    title?: string;
    /** CSS width of the trigger, e.g. "180px". */
    width?: string;
    onchange?: (value: T) => void;
  } = $props();

  const id = `sel-${Math.random().toString(36).slice(2, 9)}`;
  let trigger: HTMLButtonElement;
  let list = $state<HTMLDivElement>();
  let open = $state(false);
  let active = $state(-1);
  let pos = $state({ left: 0, top: 0, width: 0, up: false, maxHeight: 280 });

  const selected = $derived(options.find((o) => o.value === value));
  const EDGE = 8;
  /** Long lists (hundreds of branches) get a search box at the top. */
  const searchable = $derived(options.length > 12);
  let query = $state("");
  let search = $state<HTMLInputElement>();
  /** The options on screen: all of them, or the ones matching the search. */
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return options;
    return options.filter((o) => o.label.toLowerCase().includes(q) || o.hint?.toLowerCase().includes(q));
  });
  // Typing narrows the list: keep the highlight on the first match.
  $effect(() => {
    void query;
    if (open) active = shown.findIndex((o) => !o.disabled);
  });

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function place() {
    if (!list) return;
    const r = trigger.getBoundingClientRect();
    const width = Math.max(r.width, 160);
    const below = window.innerHeight - r.bottom - EDGE - 4;
    const above = r.top - EDGE - 4;
    const want = Math.min(list.scrollHeight, 280);
    const up = want > below && above > below;
    const maxHeight = Math.max(80, Math.min(280, up ? above : below));
    const h = Math.min(want, maxHeight);
    const left = Math.max(EDGE, Math.min(r.left, window.innerWidth - width - EDGE));
    pos = { left, top: up ? r.top - 4 - h : r.bottom + 4, width, up, maxHeight };
  }

  async function show() {
    if (disabled || open) return;
    query = "";
    open = true;
    const i = options.findIndex((o) => o.value === value);
    await tick();
    active = i >= 0 ? i : options.findIndex((o) => !o.disabled);
    place();
    (search ?? list)?.focus();
    scrollActive();
    window.addEventListener("pointerdown", onOutside, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", hide);
    window.addEventListener("blur", hide);
  }

  function hide() {
    if (!open) return;
    open = false;
    window.removeEventListener("pointerdown", onOutside, true);
    window.removeEventListener("scroll", onScroll, true);
    window.removeEventListener("resize", hide);
    window.removeEventListener("blur", hide);
  }
  onDestroy(hide);

  function onOutside(e: PointerEvent) {
    const t = e.target as Node;
    if (!list?.contains(t) && !trigger.contains(t)) hide();
  }
  function onScroll(e: Event) {
    if (!list?.contains(e.target as Node)) hide();
  }

  function choose(i: number) {
    const o = shown[i];
    if (!o || o.disabled) return;
    hide();
    trigger.focus();
    if (o.value !== value) {
      value = o.value;
      onchange?.(o.value);
    }
  }

  function move(step: number, from = active) {
    const n = shown.length;
    if (!n) return;
    let i = from;
    for (let k = 0; k < n; k++) {
      i = (((i + step) % n) + n) % n;
      if (!shown[i].disabled) break;
    }
    active = i;
    scrollActive();
  }

  function scrollActive() {
    list?.querySelector(`#${id}-${active}`)?.scrollIntoView({ block: "nearest" });
  }

  function typeAhead(key: string) {
    const k = key.toLowerCase();
    for (let n = 1; n <= shown.length; n++) {
      const i = (active + n) % shown.length;
      if (!shown[i].disabled && shown[i].label.toLowerCase().startsWith(k)) {
        active = i;
        scrollActive();
        return true;
      }
    }
    return false;
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
      e.preventDefault();
      show();
    } else if (e.key.length === 1 && /\S/.test(e.key)) {
      if (searchable) {
        // Long list: open it with the letter already in the search box.
        e.preventDefault();
        show().then(() => (query = e.key));
        return;
      }
      // Closed: a letter picks the next match right away, like a native select.
      const before = active;
      active = options.findIndex((o) => o.value === value);
      if (typeAhead(e.key) && active !== before) choose(active);
    }
  }

  function onListKey(e: KeyboardEvent) {
    const keys: Record<string, () => void> = {
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Home: () => move(1, -1),
      End: () => move(-1, shown.length),
      Enter: () => choose(active),
      ...(searchable ? {} : { " ": () => choose(active) }),
      Escape: () => {
        hide();
        trigger.focus();
      },
      Tab: () => {
        hide();
        trigger.focus();
      },
    };
    // In the search box, Home/End move the text cursor.
    const run = searchable && (e.key === "Home" || e.key === "End") ? undefined : keys[e.key];
    if (run) {
      e.preventDefault();
      run();
    } else if (!searchable && e.key.length === 1 && /\S/.test(e.key)) {
      typeAhead(e.key);
    }
  }
</script>

<button
  bind:this={trigger}
  type="button"
  class="trigger {variant}"
  class:mono
  class:open
  role="combobox"
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={open ? `${id}-list` : undefined}
  aria-label={ariaLabel}
  {title}
  {disabled}
  style:width
  onclick={() => (open ? hide() : show())}
  onkeydown={onTriggerKey}
>
  <span class="current" class:placeholder={!selected}>{selected?.label ?? placeholder}</span>
  {#if selected?.hint}<span class="hint">{selected.hint}</span>{/if}
  <span class="chev"><Icon name="chevron" size={12} /></span>
</button>

{#if open}
  <div
    bind:this={list}
    use:portal
    class="list"
    class:up={pos.up}
    class:mono
    style="left: {pos.left}px; top: {pos.top}px; min-width: {pos.width}px; max-height: {pos.maxHeight}px"
  >
    {#if searchable}
      <div class="search">
        <Icon name="search" size={12} />
        <input
          bind:this={search}
          bind:value={query}
          placeholder="Search {options.length}…"
          spellcheck="false"
          aria-label="Search {ariaLabel ?? 'options'}"
          aria-controls="{id}-list"
          aria-activedescendant={active >= 0 ? `${id}-${active}` : undefined}
          onkeydown={onListKey}
        />
        {#if query}<span class="count">{shown.length}</span>{/if}
      </div>
    {/if}
  <div
    id="{id}-list"
    class="items"
    role="listbox"
    tabindex="-1"
    aria-label={ariaLabel}
    aria-activedescendant={active >= 0 ? `${id}-${active}` : undefined}
    onkeydown={onListKey}
  >
    {#each shown as o, i (o.value)}
      <div
        id="{id}-{i}"
        class="option"
        class:active={i === active}
        class:selected={o.value === value}
        class:disabled={o.disabled}
        role="option"
        aria-selected={o.value === value}
        aria-disabled={o.disabled}
        tabindex="-1"
        onpointerenter={() => !o.disabled && (active = i)}
        onclick={() => choose(i)}
        onkeydown={() => {}}
      >
        <span class="check">{#if o.value === value}<Icon name="check" size={12} />{/if}</span>
        <span class="label">{o.label}</span>
        {#if o.hint}<span class="hint">{o.hint}</span>{/if}
      </div>
    {:else}
      <div class="empty">{query ? `Nothing matches "${query}"` : "Nothing to choose"}</div>
    {/each}
  </div>
  </div>
{/if}

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    max-width: 100%;
    font: inherit;
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }
  .trigger.field {
    height: 32px;
    padding: 0 var(--space-4) 0 var(--space-5);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    font-size: 13.5px;
    transition: border-color var(--t-fast) var(--ease);
  }
  .trigger.field:hover:not(:disabled) {
    border-color: var(--text-faint);
  }
  .trigger.field:focus-visible,
  .trigger.field.open {
    outline: none;
    border-color: var(--accent-fg);
  }
  .trigger.quiet {
    height: 26px;
    padding: 0 var(--space-3);
    border: 0;
    border-radius: var(--radius-button-sm);
    background: transparent;
    color: var(--text-muted);
    font-size: 13px;
  }
  .trigger.quiet:hover:not(:disabled),
  .trigger.quiet.open {
    background: var(--bg-hover);
    color: var(--text);
  }
  .trigger.quiet:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 1px;
  }
  .trigger:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .current {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .current.placeholder {
    color: var(--text-faint);
  }
  .hint {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-faint);
  }
  .chev {
    display: grid;
    flex-shrink: 0;
    color: var(--icon);
    transition: transform var(--t-fast) var(--ease);
  }
  .open .chev {
    transform: rotate(180deg);
  }
  .mono {
    font-family: var(--font-mono, "JetBrains Mono Variable", monospace);
  }

  .list {
    position: fixed;
    z-index: 1100;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: var(--space-2);
    background: var(--bg-overlay);
    border: 1px solid var(--line-overlay);
    border-radius: 10px;
    box-shadow: var(--shadow-menu);
    animation: dd-overlay 120ms var(--ease);
    transform-origin: top left;
    outline: none;
    font-size: 13.5px;
  }
  .list.up {
    transform-origin: bottom left;
  }
  .items {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    outline: none;
  }
  .search {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-shrink: 0;
    height: 32px;
    margin-bottom: var(--space-2);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--line);
    color: var(--icon);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-family: var(--font-sans);
    font-size: 13px;
    user-select: text;
  }
  .search .count {
    font-size: 11px;
    color: var(--text-faint);
  }
  .option {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-height: 32px;
    padding: 0 var(--space-5) 0 var(--space-3);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }
  .option.active {
    background: var(--bg-hover);
  }
  .option.selected {
    font-weight: 600;
  }
  .option.disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .check {
    display: grid;
    place-items: center;
    width: 16px;
    flex-shrink: 0;
    color: var(--accent-fg);
  }
  .label {
    flex: 1;
  }
  .empty {
    padding: var(--space-4) var(--space-5);
    color: var(--text-muted);
  }
</style>

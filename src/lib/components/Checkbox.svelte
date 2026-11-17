<script lang="ts">
  import Icon from "./Icon.svelte";

  // DevDock's own check box and switch. A <button> with role checkbox/switch, so
  // a surrounding <label> still toggles it when its text is clicked.
  let {
    checked = $bindable(false),
    disabled = false,
    variant = "check",
    ariaLabel,
    title,
    onchange,
  }: {
    checked?: boolean;
    disabled?: boolean;
    /** `check` for options inside a sentence or form; `switch` for settings rows. */
    variant?: "check" | "switch";
    ariaLabel?: string;
    title?: string;
    onchange?: (checked: boolean) => void;
  } = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class={variant}
  class:on={checked}
  role={variant === "switch" ? "switch" : "checkbox"}
  aria-checked={checked}
  aria-label={ariaLabel}
  {title}
  {disabled}
  onclick={toggle}
>
  {#if variant === "check"}
    <span class="tick"><Icon name="check" size={11} /></span>
  {:else}
    <span class="knob"></span>
  {/if}
</button>

<style>
  button {
    flex-shrink: 0;
    padding: 0;
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  button:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 2px;
  }
  button:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .check {
    display: inline-grid;
    place-items: center;
    width: 17px;
    height: 17px;
    border: 1.5px solid var(--line-strong);
    border-radius: 5px;
    background: var(--bg-app);
    vertical-align: -3px;
  }
  .check:hover:not(:disabled) {
    border-color: var(--accent-fg);
  }
  .check.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .tick {
    display: grid;
    color: var(--accent-ink);
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .check.on .tick {
    opacity: 1;
    transform: none;
  }

  .switch {
    position: relative;
    width: 34px;
    height: 20px;
    border: 1px solid var(--line-strong);
    border-radius: 999px;
    background: var(--bg-app);
  }
  .switch:hover:not(:disabled) {
    border-color: var(--text-faint);
  }
  .switch.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--icon);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.25);
    transition:
      transform var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease);
  }
  .switch.on .knob {
    transform: translateX(14px);
    background: var(--accent-ink);
  }
</style>

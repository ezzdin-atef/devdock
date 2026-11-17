<script lang="ts">
  import { untrack } from "svelte";
  // A small colour picker: saturation/brightness square, hue bar, hex field.
  // `onpick` fires when the user lets go (or presses Enter), not on every move.
  let { value = "#3b82f6", onpick }: { value?: string; onpick: (hex: string) => void } = $props();

  type Hsv = { h: number; s: number; v: number };

  function hexToHsv(hex: string): Hsv {
    const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
    const n = m ? parseInt(m[1], 16) : 0x3b82f6;
    const r = ((n >> 16) & 255) / 255;
    const g = ((n >> 8) & 255) / 255;
    const b = (n & 255) / 255;
    const max = Math.max(r, g, b);
    const d = max - Math.min(r, g, b);
    let h = 0;
    if (d) {
      if (max === r) h = ((g - b) / d) % 6;
      else if (max === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      h = (h * 60 + 360) % 360;
    }
    return { h, s: max ? d / max : 0, v: max };
  }

  function hsvToHex({ h, s, v }: Hsv): string {
    const f = (n: number) => {
      const k = (n + h / 60) % 6;
      return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255);
    };
    return "#" + [f(5), f(3), f(1)].map((x) => x.toString(16).padStart(2, "0")).join("");
  }

  // Starts from the colour it was opened with; after that the picker owns it.
  let hsv = $state<Hsv>(untrack(() => hexToHsv(value)));
  let hex = $state(untrack(() => hsvToHex(hexToHsv(value))));
  const current = $derived(hsvToHex(hsv));
  $effect(() => {
    hex = current;
  });

  function drag(el: HTMLElement, set: (x: number, y: number) => void) {
    const update = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      set(
        Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
        Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
      );
    };
    const down = (e: PointerEvent) => {
      e.preventDefault();
      el.setPointerCapture(e.pointerId);
      update(e);
      const move = (m: PointerEvent) => update(m);
      const up = () => {
        el.removeEventListener("pointermove", move);
        el.removeEventListener("pointerup", up);
        onpick(current);
      };
      el.addEventListener("pointermove", move);
      el.addEventListener("pointerup", up);
    };
    el.addEventListener("pointerdown", down);
    return { destroy: () => el.removeEventListener("pointerdown", down) };
  }

  const square = (el: HTMLElement) =>
    drag(el, (x, y) => {
      hsv = { ...hsv, s: x, v: 1 - y };
    });
  const hueBar = (el: HTMLElement) =>
    drag(el, (x) => {
      hsv = { ...hsv, h: x * 359.9 };
    });

  function commitHex() {
    if (/^#?[0-9a-f]{6}$/i.test(hex.trim())) {
      const h = hex.trim().startsWith("#") ? hex.trim() : `#${hex.trim()}`;
      hsv = hexToHsv(h);
      onpick(h.toLowerCase());
    } else {
      hex = current;
    }
  }

  function nudge(e: KeyboardEvent, which: "square" | "hue") {
    const step = e.shiftKey ? 0.1 : 0.02;
    const d = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] }[e.key];
    if (!d) return;
    e.preventDefault();
    const clamp = (n: number) => Math.min(1, Math.max(0, n));
    if (which === "square") hsv = { ...hsv, s: clamp(hsv.s + d[0]), v: clamp(hsv.v + d[1]) };
    else hsv = { ...hsv, h: (hsv.h + d[0] * 360 + 360) % 360 };
  }
</script>

<div class="picker">
  <div
    class="square"
    use:square
    style:--hue="hsl({hsv.h} 100% 50%)"
    role="slider"
    tabindex="0"
    aria-label="Saturation and brightness"
    aria-valuetext="{Math.round(hsv.s * 100)}% saturation, {Math.round(hsv.v * 100)}% brightness"
    aria-valuenow={Math.round(hsv.s * 100)}
    onkeydown={(e) => nudge(e, "square")}
    onkeyup={(e) => e.key.startsWith("Arrow") && onpick(current)}
  >
    <span class="knob" style:left="{hsv.s * 100}%" style:top="{(1 - hsv.v) * 100}%" style:background={current}></span>
  </div>
  <div
    class="hue"
    use:hueBar
    role="slider"
    tabindex="0"
    aria-label="Hue"
    aria-valuemin={0}
    aria-valuemax={360}
    aria-valuenow={Math.round(hsv.h)}
    onkeydown={(e) => nudge(e, "hue")}
    onkeyup={(e) => e.key.startsWith("Arrow") && onpick(current)}
  >
    <span class="knob bar" style:left="{(hsv.h / 360) * 100}%" style:background="hsl({hsv.h} 100% 50%)"></span>
  </div>
  <div class="row">
    <span class="preview" style:background={current}></span>
    <input
      class="mono"
      bind:value={hex}
      maxlength="7"
      spellcheck="false"
      aria-label="Hex colour"
      onkeydown={(e) => {
        if (e.key === "Enter") commitHex();
      }}
      onblur={commitHex}
    />
  </div>
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5) var(--space-4);
  }
  .square {
    position: relative;
    height: 120px;
    border-radius: var(--radius-button-sm);
    background:
      linear-gradient(to top, #000, transparent),
      linear-gradient(to right, #fff, var(--hue));
    cursor: crosshair;
    touch-action: none;
  }
  .hue {
    position: relative;
    height: 12px;
    border-radius: 6px;
    background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    cursor: ew-resize;
    touch-action: none;
  }
  .square:focus-visible,
  .hue:focus-visible {
    outline: 2px solid var(--accent-fg);
    outline-offset: 2px;
  }
  .knob {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.35);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .knob.bar {
    top: 50%;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .preview {
    width: 26px;
    height: 26px;
    border-radius: var(--radius-badge);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 var(--space-4);
    background: var(--bg-app);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-button-sm);
    color: var(--text);
    font-size: 12.5px;
    user-select: text;
  }
  input:focus {
    outline: none;
    border-color: var(--accent-fg);
  }
</style>

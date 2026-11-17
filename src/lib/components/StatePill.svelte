<script lang="ts">
  import type { RunInfo } from "$lib/types";

  // State word + dot; status is never colour alone (DESIGN.md principle 3).
  let { run }: { run: RunInfo | null | undefined } = $props();

  const label = $derived.by(() => {
    if (!run) return "IDLE";
    switch (run.state) {
      case "exited":
        return `EXITED ${run.code ?? 0}`;
      case "failed":
        return run.code == null ? "FAILED TO START" : `FAILED ${run.code}`;
      default:
        return run.state.toUpperCase();
    }
  });
  const tone = $derived(
    !run
      ? "idle"
      : run.state === "running" || run.state === "starting"
        ? "ok"
        : run.state === "failed"
          ? "bad"
          : run.state === "stopping"
            ? "warn"
            : "idle",
  );
</script>

<span class="pill mono {tone}"><span class="dot"></span>{label}</span>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 22px;
    padding: 0 var(--space-5);
    border-radius: var(--radius-pill);
    font-size: 11px;
    letter-spacing: 0.06em;
    white-space: nowrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }
  .ok {
    background: var(--success-pill);
    color: var(--success);
  }
  .ok .dot {
    background: var(--success);
  }
  .bad {
    background: var(--error-pill);
    color: var(--error-text);
  }
  .bad .dot {
    background: var(--error);
  }
  .warn {
    background: var(--warning-panel);
    color: var(--accent-fg);
  }
  .warn .dot {
    background: var(--accent);
  }
  .idle {
    background: var(--bg-raised);
    color: var(--text-muted);
  }
  .idle .dot {
    border: 1.5px solid var(--idle);
  }
</style>

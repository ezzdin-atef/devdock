<script lang="ts">
  // DevDock mark: three jetties reaching out from the shore (square land end,
  // rounded water end) over a waterline, on the graphite app-icon tile. The tile
  // stays graphite in both themes so the mark is the same object everywhere:
  // title bar, taskbar, installer. Source art: docs/brand/devdock-icon.svg.
  let { size = 24 }: { size?: number } = $props();
</script>

<svg class="mark" width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
  <rect class="tile" x="0.5" y="0.5" width="31" height="31" rx="7.5" />
  <path class="plank" d="M7 7h16.7a1.8 1.8 0 0 1 0 3.6H7z" />
  <path class="plank" d="M7 12.6h11.7a1.8 1.8 0 0 1 0 3.6H7z" />
  <path class="plank" d="M7 18.2h6.7a1.8 1.8 0 0 1 0 3.6H7z" />
  <path class="water" d="M7.2 25.4q2.4-2 4.8 0t4.8 0t4.8 0t4.1 0" />
</svg>

<style>
  .mark {
    flex-shrink: 0;
  }
  /* Brand constants, deliberately not theme tokens. */
  .tile {
    fill: #16171a;
    stroke: #33353b;
    stroke-width: 1;
  }
  .plank {
    fill: #e8c547;
  }
  .water {
    fill: none;
    stroke: #e8c547;
    stroke-opacity: 0.55;
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>

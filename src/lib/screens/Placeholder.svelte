<script lang="ts">
  let { title, milestone }: { title: string; milestone: string } = $props();
</script>

<section class="placeholder">
  <div class="eyebrow">{milestone}</div>
  <h1>{title}</h1>
  <div class="panel">
    <strong>Not built yet</strong>
    <p>This screen arrives in {milestone}.</p>
  </div>
</section>

<style>
  .placeholder {
    padding: var(--space-12) var(--space-14);
  }
  h1 {
    margin: var(--space-1) 0 var(--space-8);
    font-size: 28px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .panel {
    padding: var(--space-8);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-panel);
  }
  p {
    margin: var(--space-2) 0 0;
    color: var(--text-muted);
  }
</style>

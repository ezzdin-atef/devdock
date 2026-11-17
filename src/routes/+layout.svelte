<script lang="ts">
  import "@fontsource-variable/schibsted-grotesk";
  import "@fontsource-variable/jetbrains-mono";
  import "../app.css";

  let { children } = $props();

  // No browser menu (Reload, Inspect...) on right-click: the app shows its own menus
  // where they make sense. Text fields keep theirs, for copy and paste.
  function onContextMenu(e: MouseEvent) {
    const t = e.target as HTMLElement | null;
    const editable = t?.closest("input, textarea, [contenteditable=''], [contenteditable='true']");
    if (!editable) e.preventDefault();
  }
</script>

<svelte:window oncontextmenu={onContextMenu} />

{@render children()}

<script lang="ts">
  import { onMount } from "svelte";
  import { EditorSelection } from "@codemirror/state";
  import { EditorView } from "@codemirror/view";
  import { files } from "$lib/files.svelte";
  import type { ProjectId } from "$lib/types";
  import { createState, languageFor, languageSlot } from "./setup";

  // One CodeMirror view for the project's open file. The state outlives the view:
  // it's handed back to the store on unmount and picked up again on mount.
  let { projectId }: { projectId: ProjectId } = $props();

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  let shownVersion = -1;

  const pf = $derived(files.of(projectId));
  const open = $derived(pf.open);

  function mount() {
    const b = files.buffer(projectId);
    if (!open || !b) return;
    const state =
      b.state ??
      createState(b.text, {
        save: () => files.save(projectId),
        change: (s) => files.changed(projectId, s),
      });
    if (view) view.setState(state);
    else view = new EditorView({ state, parent: host });
    shownVersion = open.version;
    const path = open.path;
    languageFor(path).then((lang) => {
      if (view && files.of(projectId).open?.path === path) {
        view.dispatch({ effects: languageSlot.reconfigure(lang) });
      }
    });
  }

  onMount(() => {
    mount();
    return () => {
      if (view) {
        files.keep(projectId, view.state);
        view.destroy();
        view = null;
      }
    };
  });

  // A different file (or the same one re-read from disk): fresh state.
  $effect(() => {
    if (open && open.version !== shownVersion) mount();
  });

  // Jump to a search hit.
  $effect(() => {
    const r = open?.reveal;
    if (!r || !view || open.version !== shownVersion) return;
    const doc = view.state.doc;
    const line = doc.line(Math.min(Math.max(1, r.line), doc.lines));
    const from = Math.min(line.from + r.from, line.to);
    const to = Math.min(line.from + r.to, line.to);
    view.dispatch({
      selection: EditorSelection.single(from, to),
      effects: EditorView.scrollIntoView(from, { y: "center" }),
    });
    view.focus();
    open.reveal = null;
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  .editor :global(.cm-editor) {
    height: 100%;
  }
</style>

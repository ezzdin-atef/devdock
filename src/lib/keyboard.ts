// Window-level keys (T8.3, DESIGN.md section 9): Ctrl K palette, F5 refresh the
// current screen, arrows move through the project / workspace list, Enter shows the
// selected project's Overview. Overlays (palette, dialogs) handle their own keys.

import { agents } from "./agents.svelte";
import { app } from "./app.svelte";
import { palette } from "./palette.svelte";
import { ports } from "./ports.svelte";
import { runs } from "./runs.svelte";
import { security } from "./security.svelte";
import { spaces } from "./spaces.svelte";

function typingIn(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest("input, textarea, select, [contenteditable]");
}

function overlayOpen(): boolean {
  return palette.open || runs.closeRequested !== null || ports.inspecting !== null || security.logOpen !== null;
}

/** F5: refresh whatever the current screen shows. */
export async function refreshCurrent() {
  switch (app.view) {
    case "projects":
      await app.refreshOnFocus();
      break;
    case "ports":
      await ports.refresh();
      break;
    case "spaces":
      await spaces.load();
      break;
    case "running":
      await runs.refreshList();
      break;
    default:
      await agents.recheck();
  }
  agents.showToast("Refreshed", "ok");
}

function step<T>(items: T[], current: T | null, delta: number): T | undefined {
  if (!items.length) return undefined;
  const i = current === null ? -1 : items.indexOf(current);
  const next = i < 0 ? (delta > 0 ? 0 : items.length - 1) : Math.min(items.length - 1, Math.max(0, i + delta));
  return items[next];
}

export function keyboard(e: KeyboardEvent) {
  if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "k") {
    e.preventDefault();
    palette.toggle();
    return;
  }
  if (e.key === "F5") {
    // Without this the webview reloads the whole page.
    e.preventDefault();
    if (!overlayOpen()) refreshCurrent();
    return;
  }
  if (overlayOpen() || typingIn(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;

  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    const delta = e.key === "ArrowDown" ? 1 : -1;
    if (app.view === "projects") {
      // With a project open, arrows step to the previous/next one; on the cards
      // page they scroll as usual.
      if (!app.selectedId) return;
      const ids = (app.lib?.projects ?? []).map((p) => p.id);
      const next = step(ids, app.selectedId, delta);
      if (next) {
        e.preventDefault();
        app.select(next);
      }
    } else if (app.view === "spaces") {
      const next = step(spaces.list.map((w) => w.id), spaces.selectedId, delta);
      if (next) {
        e.preventDefault();
        spaces.selectedId = next;
      }
    }
    return;
  }

  // Esc from an open project: back to all projects.
  if (e.key === "Escape" && app.view === "projects" && app.selectedId) {
    e.preventDefault();
    app.select(null);
    return;
  }

  // Enter on the page itself (not on a focused button): open the project's Overview.
  if (e.key === "Enter" && app.view === "projects" && app.selectedId && e.target === document.body) {
    e.preventDefault();
    app.tab = "overview";
  }
}

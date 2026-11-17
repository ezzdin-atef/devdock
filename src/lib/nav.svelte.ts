// Back / forward like a browser: the mouse's side buttons (and Alt+Left/Right)
// step through where you've been: which screen, which project, which group or
// library filter, which tab.

import { app } from "./app.svelte";
import type { DetailTab, Library, ProjectId, View } from "./types";

interface Place {
  view: View;
  project: ProjectId | null;
  tab: DetailTab;
  group: string | null;
  library: Library;
}

const LIMIT = 50;

function here(): Place {
  const project = app.view === "projects" ? app.selectedId : null;
  return {
    view: app.view,
    project,
    // The tab only means something while a project is open.
    tab: project ? app.tab : "overview",
    group: app.query.group,
    library: app.query.library,
  };
}

const same = (a: Place, b: Place) =>
  a.view === b.view && a.project === b.project && a.tab === b.tab && a.group === b.group && a.library === b.library;

class Nav {
  private back: Place[] = [];
  private forward: Place[] = [];
  private last: Place | null = null;
  /** Set while we move ourselves, so that move isn't recorded as a new step. */
  private moving = false;
  canBack = $state(false);
  canForward = $state(false);

  private started = false;

  /** Watches the app and records each change of place. Safe to call twice. */
  init() {
    if (this.started) return;
    this.started = true;
    $effect.root(() => {
      $effect(() => {
        const now = here();
        if (this.moving || !this.last) {
          this.last = now;
          return;
        }
        if (same(now, this.last)) return;
        this.back.push(this.last);
        if (this.back.length > LIMIT) this.back.shift();
        this.forward = [];
        this.last = now;
        this.sync();
      });
    });

    // Mouse buttons 3 and 4 are Back and Forward. Handle them on mouseup and stop
    // the webview from treating them as page navigation.
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3 || e.button === 4) {
        e.preventDefault();
        if (e.type === "mouseup") (e.button === 3 ? this.goBack() : this.goForward());
      }
    };
    window.addEventListener("mousedown", onMouse);
    window.addEventListener("mouseup", onMouse);
    window.addEventListener("keydown", (e) => {
      if (e.altKey && !e.ctrlKey && !e.shiftKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        if (e.key === "ArrowLeft") this.goBack();
        else this.goForward();
      }
    });
  }

  goBack() {
    const to = this.back.pop();
    if (!to) return;
    this.forward.push(here());
    this.go(to);
  }

  goForward() {
    const to = this.forward.pop();
    if (!to) return;
    this.back.push(here());
    this.go(to);
  }

  private async go(to: Place) {
    this.moving = true;
    try {
      app.view = to.view;
      if (to.group !== app.query.group || to.library !== app.query.library) {
        app.setQuery({ group: to.group, library: to.library });
      }
      // A project removed meanwhile can't be shown; land on the cards instead.
      const exists = !to.project || app.lib?.projects.some((p) => p.id === to.project);
      if (to.view === "projects") {
        if (to.project) app.tab = to.tab;
        await app.select(exists ? to.project : null);
      }
    } finally {
      this.last = here();
      this.sync();
      // Let the effect see the new place before recording again.
      queueMicrotask(() => (this.moving = false));
    }
  }

  private sync() {
    this.canBack = this.back.length > 0;
    this.canForward = this.forward.length > 0;
  }
}

export const nav = new Nav();

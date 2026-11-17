// Drag and drop on the Projects page. Pointer events, not HTML drag-and-drop: the
// desktop window handles file drops itself, which blocks HTML drag events on Windows.
//
// Two kinds of drag:
// - projects onto a group: targets carry `data-drop-group="<id>"` ("-" = ungrouped,
//   "+" = make a new group with them);
// - a group to a new place: targets carry `data-group-slot="<id>"` and
//   `data-axis="x" | "y"` (which way the list runs), and get a line before or after.

import { app } from "./app.svelte";
import { UNGROUPED, type ProjectId } from "./types";

export const NEW_GROUP = "+";
const THRESHOLD = 5;
/** Distance from a scroll area's edge where auto-scroll starts, and its top speed. */
const EDGE = 64;
const MAX_SPEED = 18;

export type Payload = { kind: "projects"; ids: ProjectId[] } | { kind: "group"; id: string };

class DragUi {
  payload = $state<Payload | null>(null);
  label = $state("");
  /** Group colour for the ghost when dragging a group. */
  color = $state<string | undefined>(undefined);
  x = $state(0);
  y = $state(0);
  active = $state(false);
  /** Projects: the group under the pointer. */
  over = $state<string | null>(null);
  /** Groups: where it would land. */
  slot = $state<{ id: string; after: boolean } | null>(null);
  /** Projects dropped between projects: next to which one, and that one's group. */
  place = $state<{ id: ProjectId; after: boolean; group: string | undefined } | null>(null);

  /** For templates: is this project being dragged? */
  has(id: ProjectId) {
    return this.active && this.payload?.kind === "projects" && this.payload.ids.includes(id);
  }
  get ids(): ProjectId[] {
    return this.payload?.kind === "projects" ? this.payload.ids : [];
  }
  /** The group being dragged, if a group is. */
  get group(): string | null {
    return this.payload?.kind === "group" ? this.payload.id : null;
  }

  /** Call from pointerdown. Starts only after a short move, so clicks stay clicks. */
  begin(e: PointerEvent, payload: Payload, label: string, color?: string) {
    if (e.button !== 0) return;
    const sx = e.clientX;
    const sy = e.clientY;
    let scrollEl: HTMLElement | null = null;
    let raf = 0;

    const scrollLoop = () => {
      raf = 0;
      if (!this.active || !scrollEl) return;
      const r = scrollEl.getBoundingClientRect();
      const speed = (d: number) => Math.ceil(((EDGE - d) / EDGE) * MAX_SPEED);
      let dy = 0;
      if (this.y < r.top + EDGE) dy = -speed(this.y - r.top);
      else if (this.y > r.bottom - EDGE) dy = speed(r.bottom - this.y);
      if (dy) {
        scrollEl.scrollTop += dy;
        this.hitTest();
        raf = requestAnimationFrame(scrollLoop);
      }
    };

    const move = (m: PointerEvent) => {
      if (!this.active) {
        if (Math.hypot(m.clientX - sx, m.clientY - sy) < THRESHOLD) return;
        this.active = true;
        this.payload = payload;
        this.label = label;
        this.color = color;
        document.body.classList.add("dragging", payload.kind === "group" ? "dragging-group" : "dragging-projects");
      }
      this.x = m.clientX;
      this.y = m.clientY;
      scrollEl = scrollableAt(m.clientX, m.clientY);
      this.hitTest();
      if (!raf) raf = requestAnimationFrame(scrollLoop);
    };

    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key, true);
      if (raf) cancelAnimationFrame(raf);
    };

    const up = () => {
      stop();
      if (!this.active) return;
      const done = { payload: this.payload, over: this.over, slot: this.slot, place: this.place };
      this.end();
      // The click that follows a drag must not open whatever is under it.
      const swallow = (c: MouseEvent) => {
        c.stopPropagation();
        c.preventDefault();
      };
      window.addEventListener("click", swallow, { capture: true, once: true });
      setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
      this.drop(done.payload, done.over, done.slot, done.place);
    };

    const key = (k: KeyboardEvent) => {
      if (k.key === "Escape" && this.active) {
        k.preventDefault();
        k.stopPropagation();
        stop();
        this.end();
      }
    };

    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", key, true);
  }

  /** What's under the pointer right now. */
  private hitTest() {
    const el = document.elementFromPoint(this.x, this.y) as HTMLElement | null;
    if (this.payload?.kind === "projects") {
      // Over another project: drop next to it. Otherwise: into the group under it.
      const card = el?.closest<HTMLElement>("[data-project-slot]");
      const id = card?.dataset.projectSlot;
      if (card && id && !this.payload.ids.includes(id)) {
        const r = card.getBoundingClientRect();
        const after = card.dataset.axis === "x" ? this.x > r.left + r.width / 2 : this.y > r.top + r.height / 2;
        this.place = { id, after, group: card.dataset.inGroup };
        this.over = null;
        document.body.classList.add("placing");
        return;
      }
      this.place = null;
      document.body.classList.remove("placing");
      this.over = el?.closest<HTMLElement>("[data-drop-group]")?.dataset.dropGroup ?? null;
      return;
    }
    const target = el?.closest<HTMLElement>("[data-group-slot]");
    const id = target?.dataset.groupSlot;
    if (!target || !id || id === this.group) {
      this.slot = null;
      return;
    }
    const r = target.getBoundingClientRect();
    const after =
      target.dataset.axis === "x" ? this.x > r.left + r.width / 2 : this.y > r.top + r.height / 2;
    this.slot = { id, after };
  }

  private drop(
    payload: Payload | null,
    over: string | null,
    slot: { id: string; after: boolean } | null,
    place: { id: ProjectId; after: boolean; group: string | undefined } | null,
  ) {
    if (!payload) return;
    if (payload.kind === "projects" && place) {
      // Inside a group's section the project joins that group; elsewhere
      // (folders, types, one list) only its place changes.
      const regroup = place.group !== undefined;
      const group = place.group === UNGROUPED ? null : (place.group ?? null);
      app.placeProjects(payload.ids, place.id, place.after, regroup, group);
      return;
    }
    if (payload.kind === "projects") {
      if (over === NEW_GROUP) app.createGroup("New group", payload.ids);
      else if (over !== null) app.moveToGroup(payload.ids, over === UNGROUPED ? null : over);
      return;
    }
    if (!slot) return;
    const others = (app.lib?.groups ?? []).map((g) => g.id).filter((id) => id !== payload.id);
    const at = others.indexOf(slot.id);
    if (at >= 0) app.reorderGroup(payload.id, at + (slot.after ? 1 : 0));
  }

  private end() {
    this.active = false;
    this.payload = null;
    this.over = null;
    this.slot = null;
    this.place = null;
    document.body.classList.remove("dragging", "dragging-group", "dragging-projects", "placing");
  }
}

/** The nearest scrolling area under the pointer (the page, the sidebar...). */
function scrollableAt(x: number, y: number): HTMLElement | null {
  let el = document.elementFromPoint(x, y) as HTMLElement | null;
  while (el && el !== document.body) {
    const s = getComputedStyle(el).overflowY;
    if ((s === "auto" || s === "scroll") && el.scrollHeight > el.clientHeight) return el;
    el = el.parentElement;
  }
  return null;
}

export const drag = new DragUi();

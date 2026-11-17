// Light / dark / follow-Windows. A UI-only preference, so it lives in localStorage
// (read before first paint by the inline script in app.html) rather than in Rust.

import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemePref = "system" | "light" | "dark";
export type Theme = "light" | "dark";

const KEY = "devdock.theme";
const media = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: light)") : null;

function readPref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

class ThemeUi {
  pref = $state<ThemePref>(readPref());
  private systemLight = $state(media?.matches ?? false);
  resolved = $derived<Theme>(this.pref === "system" ? (this.systemLight ? "light" : "dark") : this.pref);

  init() {
    media?.addEventListener("change", (e) => {
      this.systemLight = e.matches;
      if (this.pref === "system") this.apply(true);
    });
    this.apply(false);
    if (this.pref !== "system") getCurrentWindow().setTheme(this.pref).catch(() => {});
  }

  set(pref: ThemePref) {
    if (pref === this.pref) return;
    this.pref = pref;
    try {
      if (pref === "system") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, pref);
    } catch {
      /* storage unavailable: the choice still holds for this session */
    }
    // Native title bar follows; `null` hands it back to Windows.
    getCurrentWindow()
      .setTheme(pref === "system" ? null : pref)
      .catch(() => {});
    this.apply(true);
  }

  /** Quick toggle from the title bar: flips what you see now. */
  toggle() {
    this.set(this.resolved === "dark" ? "light" : "dark");
  }

  private apply(animate: boolean) {
    const root = document.documentElement;
    if (animate) {
      root.classList.add("theme-switching");
      setTimeout(() => root.classList.remove("theme-switching"), 260);
    }
    root.dataset.theme = this.resolved;
  }
}

export const theme = new ThemeUi();

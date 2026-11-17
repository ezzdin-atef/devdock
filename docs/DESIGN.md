# DevDock — Design

> Visual language: **"Harbor control panel."** Quiet graphite surfaces, one dock-yellow signal colour, monospace for anything a machine produced, and a status strip like an instrument panel along the bottom of every screen.

The reference mockup is the **DevDock UI** canvas (page "Full app"). This file is the source of truth for tokens and rules when building it (Tauri + Svelte front end, Rust back end).

---

## 1. Design principles

1. **Machine text looks like machine text.** Paths, branches, PIDs, ports, commands, hashes and logs always use the mono typeface. Human words use the sans.
2. **One accent, used sparingly.** Yellow marks *the* primary action, the current selection, and DevDock ownership. If everything is yellow, nothing is.
3. **Status is never colour alone.** Every coloured state also has a word (`RUNNING`, `FAILED`, `EXTERNAL`) or a symbol (●, ↑2).
4. **Calm by default, loud only when it matters.** Conflicts and failures get a tinted panel; everything else stays neutral.
5. **Show the truth, then the action.** Each panel states what is happening, then offers the next safe step.
6. **No decoration.** No gradients, glows, emoji, or coloured stripes along the left edge of cards. Structure comes from 1 px hairlines and spacing.

---

## 2. Color tokens

### Surfaces
| Token | Hex | Use |
|---|---|---|
| `bg.chrome` | `#0E0F11` | Title bar, rail, status strip |
| `bg.sidebar` | `#0F1012` | List columns |
| `bg.app` | `#121315` | Main background |
| `bg.panel` | `#18191C` | Cards, tables, inputs |
| `bg.raised` | `#202125` | Secondary buttons |
| `bg.hover` | `#26272B` | Selected segment, hovered row |
| `bg.overlay` | `#1C1D20` | Dialogs, palette, menus |
| `bg.console` | `#0B0C0D` | Logs, terminal |
| `scrim` | `rgba(8,9,10,0.62)` | Behind dialogs |

### Lines
| Token | Hex | Use |
|---|---|---|
| `line` | `#2A2C31` | Default hairline borders, dividers |
| `line.strong` | `#34363C` | Inputs, dashed "add" targets |
| `line.overlay` | `#3A3C42` | Dialog border |

### Text
| Token | Hex | Contrast on `bg.app` | Use |
|---|---|---|---|
| `text` | `#ECE9E2` | ~15:1 | Primary |
| `text.soft` | `#D4CFC6` | ~12:1 | Body on tinted panels |
| `text.muted` | `#9A988F` | ~6.3:1 | Labels, metadata, paths |
| `text.faint` | `#6E6D67` | ~3.4:1 | Log timestamps only (decorative) |

### Accent (tweakable)
| Token | Hex | Use |
|---|---|---|
| `accent` | `#E8C547` dock yellow | Primary button, selection, DevDock-owned badge, "ahead" count |
| `accent.ink` | `#121315` | Text on accent |
| `accent.tint` | `#26241A` | Active rail item, active chip |
| `accent.panel` | `#1E1E1B` / border `#3A3A33` | Selected list row |

Alternative accents explored: `#8FB8FF` (blue), `#7BD3A8` (mint), `#FF9F5A` (orange). All work with `accent.ink`.

### Status
| State | Fg | Tint bg | Border |
|---|---|---|---|
| Success / running / healthy | `#7BD3A8` | `#1F3A2E` (pill), `#172420` (panel) | `#2E4A3C` |
| Error / failed / destructive | `#FF8A6B` (dot, primary button) · `#FFB7A3` (text) | `#2A1E1A` · `#221816` · `#24161A` (log line) | `#5A3328` |
| Warning / conflict | `accent` | `#211E14` | `#5C4E22` |
| Info / experimental | `#8FB8FF` | — | — |
| Idle / stopped | ring `#6B6A64` | — | — |

Log service prefixes (combined logs): postgres `#8FB8FF`, api `#FFB27A`, web `text`. These always appear next to the service name.

---

### Light theme
Same harbor in daylight: neutral light greys with a faint cool cast (no cream), white work surfaces, near-black ink. Chrome `#EBEDF0`, list columns `#F3F4F6`, app `#F8F9FA`, panels `#FFFFFF`. Every token above has a light value in `src/app.css` under `:root[data-theme="light"]`; components never branch on theme. Two rules carry the difference:
- **`accent` vs `accent.fg`.** `accent` (`#E8C547`) is for fills (primary button, `DEVDOCK` badge, health bars on dark). `accent.fg` is the accent used as text, icon, border or focus ring: the same yellow in dark, deep mustard `#7F5F00` in light, where yellow text would fail contrast.
- **`icon`** is its own token, a step stronger than `text.muted` (dark `#BCB9B0`, light `#3E434B`), because thin line glyphs read weaker than text of the same colour. Icon-only controls use it.
- **Status colours darken** (success `#1A7148`, error `#C8492B` / text `#A33A1F`, info `#2C62C9`) and tints become pale washes of the same hue.

Theme preference is System (follow Windows) / Light / Dark, set in Settings > Appearance or flipped from the title bar; stored in `localStorage` and applied before first paint. `pnpm contrast` checks every pair in both themes.

## 3. Typography

| Role | Family | Size / weight | Notes |
|---|---|---|---|
| Display (screen title) | Schibsted Grotesk | 26–28 / 700, tracking −0.02em | One per screen |
| Section title | Schibsted Grotesk | 14–15 / 600–700 | Panel headers |
| Body | Schibsted Grotesk | 13–13.5 / 400–500 | Buttons, rows, prose |
| Small | Schibsted Grotesk | 12–12.5 / 400 | Helper text |
| Mono data | JetBrains Mono | 11–12 / 400–500 | Paths, PIDs, branches, commands |
| Mono label | JetBrains Mono | 10 / 400, UPPERCASE, tracking 0.08–0.1em | Section eyebrows, table headers, badges |
| Big numbers | JetBrains Mono | 22–34 / 400, line-height 1 | Git counts, port tile |
| Log | JetBrains Mono | 11–11.5, line-height 1.9 | Console |

Fallbacks: `system-ui, sans-serif` and `ui-monospace, monospace`. Both fonts are bundled through the `@fontsource-variable` npm packages (SIL OFL), so the app never loads fonts from the network.

---

## 4. Spacing, radius, elevation

- **Spacing scale (px):** 2 · 4 · 6 · 8 · 10 · 12 · 14 · 16 · 18 · 20 · 24 · 28.
  - Screen padding: 24–28 horizontal, 20–24 top.
  - Gap between panels: 18. Inside panels: 12–16.
  - Table cells: 11–12 vertical, 16 horizontal.
- **Radius:** 4 (badges, key caps) · 6 (small buttons) · 7–8 (buttons, inputs) · 9–10 (list rows, command cards) · 12 (panels) · 14 (dialogs) · 99 (pills, dots).
- **Elevation:** flat by default (hairlines only). Overlays only: menus `0 12px 32px rgba(0,0,0,.45)`, dialogs and palette `0 24px 64px rgba(0,0,0,.55)`.

---

## 5. Layout

```
┌──────────────────────────── Title bar 48 ───────────────────────────┐
│ ▤ DevDock      [ ⌕ Jump to a project, command or port…  Ctrl K ]  ● 5 running  — ☐ × │
├────┬──────────────┬─────────────────────────────────────────────────┤
│Rail│ List column  │ Main                                            │
│ 76 │ 300–360      │ header · tabs · content                         │
│    │ (Projects /  │                                                 │
│    │  Spaces only)│                                                 │
├────┴──────────────┴─────────────────────────────────────────────────┤
│ Status strip 30 · mono 11 · counts left, tool detection right       │
└─────────────────────────────────────────────────────────────────────┘
```

- Minimum comfortable window: 1100 × 720; designed at 1280 × 900.
- The list column appears only on screens that have a list (Projects, Spaces). Running, Ports and Settings use the full width.
- Settings content is capped at 760 px wide and centred.
- At narrow widths panels wrap: grids fall to fewer columns, and tables scroll sideways inside their panel.

---

## 6. Components

### Window chrome
- **Title bar:** custom, replacing the Windows one (`decorations: false`, native shadow kept). Three columns so the palette trigger is centred on the window: logo mark (22 px tile) + wordmark 14/700 + breadcrumb (`/ Projects / devchrono`, 13 px `text.muted`, current item `text.soft`) | palette trigger | running pill, theme toggle, window controls. Empty space is a drag region (double-click maximizes). Window controls are 46 px wide and full height, flush right behind a hairline, drawn as 1 px line glyphs; hover `bg.hover`, close hover is Windows red `#C42B1C` with white in both themes. Close goes through the normal close request, so the "stop running processes?" dialog still applies. When the window loses focus, title-bar contents fade to ~55%.
- **Logo:** three jetties reaching out from the shore over a waterline. Planks are square at the land (left) end and rounded at the water end, decreasing in length; a soft accent wave sits underneath. Always drawn in dock yellow on the graphite tile (`#16171A`, 1 px `#33353B` edge) in both themes, so the title-bar mark, taskbar icon and installer are one object. Source art: `docs/brand/devdock-icon.svg` (1024) and `docs/brand/devdock-icon-small.svg` (hinted for 16-32 px, used for the small `.ico` sizes and the favicon).
- **Status strip:** mono 11, `text.muted`; values coloured by status, always with a word.

### Rail item
60 × ~56, icon 18 above label 10.5. Active: `accent.tint` bg, `accent` fg, 600 weight. Settings is pinned to the bottom.

### Buttons
| Variant | Style |
|---|---|
| Primary | bg `accent`, text `accent.ink`, 600, h 36–40, r 7–8. **At most one per view.** |
| Secondary | bg `bg.raised`, border `line`, text `text`, h 32–38 |
| Destructive (soft) | bg `#2A1E1A` or transparent, border `#5A3328`, text `#FFB7A3` |
| Destructive (confirm) | bg `#FF8A6B`, text `#1A0E0A`, 600 (dialogs only) |
| Ghost / link | transparent, `accent` text, 600 |
| Icon button | 32 × 32, needs an `aria-label` / tooltip |
| Add target | dashed `line.strong`, `text.muted`, full width |
Hover: brighten ~15%. Focus: 2 px `accent` outline, offset 2.

### Inputs
h 36–40, bg `bg.panel`, border `line.strong`, r 8, leading icon in `text.muted`, optional key-cap hint (`Ctrl K`). Focus border = `accent`. Inline rename uses the same input, with an accent border.

### Chips & segmented controls
- **Filter chip:** mono 10–11, h 26–30, pill. Active: accent border + `accent.tint` + accent text.
- **Segmented:** container `bg.app` / `bg.panel`, padding 3, r 8–9; active segment `bg.hover`.

### Badges
Mono 10, padding 3×7, r 4.
- `DEVDOCK` → solid accent.
- `EXTERNAL`, `SYSTEM` → outline `#4A4C52`, `text.soft`.
- Type (`NEXT.JS`, `RUST`, `NODE`) → bg `#23252A`, or plain mono `text.muted` in lists.
- Small state words (`RUNNING`, `DETECTED`, `NOT INSTALLED`, `NEW`) → mono 10 in the status colour.

### Status dot
8 px. Running = filled green (with a 3 px tint ring when selected); failed = filled `#FF8A6B`; idle = 1.5 px ring `#6B6A64`; missing = dashed ring `#FF8A6B`.

### List row (project / workspace)
Padding 10×12, r 9, transparent border. Contents: dot · name 14/600 · type (mono, right) · one-line mono subline (truncated). Selected = `accent.panel`.
Workspace rows replace the dot with a **health bar**: one 18 × 5 bar per service.

### Panel
bg `bg.panel`, 1 px `line`, r 12. Optional header row: 14/600 title, hairline below, action on the right.

### Stat tile
Mono eyebrow (10, uppercase) → main value (mono 14 or big number) → muted 12.5 detail. Tiles are clickable and lead to their detail. A tile with a running state uses the success panel tint.

### Key–value list
`dl` grid: label column 92–150 in `text.muted`, value in `text`, mono for machine values. Hairline between rows in large panels.

### Tables
Header mono 10 uppercase `text.muted`; rows 13 px with hairlines between them; numbers and IDs in mono; actions right-aligned. A row needing attention gets a tint (`#1E1C16` conflict, `#201719` failed).

### Tabs
h 40, label 13.5; active = 2 px `accent` underline + `text` 600; optional count after the label (mono 10, accent or success pill).

### Command card (Run)
Row with name (mono 12.5) over the command and last result (mono 10.5, coloured by result), with a 34 × 34 ▶ button on the right. Running cards use the success panel tint and a `RUNNING` label instead of ▶.

### Console / log
bg `bg.console`, mono 11.5 / 1.9. Timestamps `text.faint`. A stderr line spans the full width with tint `#24161A`, text `#FFB7A3` and a small `ERR` tag. Blinking ▍ cursor at the end while following. Toolbar above: stream/service segmented control + "Follow" checkbox.

### Banners
- **Conflict:** border `#5C4E22`, bg `#211E14`; a 64 × 64 mono port tile on the left; title 15/600; explanation; actions — primary safe option, secondary inspect, destructive last.
- **Error:** border `#5A3328`, bg `#221816`, triangle icon, what happened + how to fix + Dismiss.
- **Success notice:** neutral panel with a green check.

### Dialog
Width up to 480, r 14, padding 22, `bg.overlay`, scrim behind. Structure: mono eyebrow (risk type) → title 19/700 → one-line explanation → details `dl` on `bg.app` → options (checkbox) → actions right-aligned: Cancel (secondary) then the confirm button.

### Command palette
Width up to 600, positioned 80 px from the top. Input row h 54 with an `Esc` hint. Grouped results under mono eyebrows (PROJECTS, COMMANDS). The matching part of each result is bold, and a right-hand mono hint shows the shortcut or state. Active result bg `#2A2A24`.

### Agent card
Mono glyph tile `>_` (38 px), state label at the top right, name 16/600, mono path, full-width launch button. Not installed = dashed border, transparent bg, install guide + Re-check.

### Workspace service card
Order number tile (mono, accent) + health label, name 15/600, mono source and command, 2×2 mono stat grid (port, uptime, check, restarts). Cards are joined by → arrows in `#6E6D67`.

### Context menu
`bg.overlay`, r 10, padding 6, items h 34. The destructive item is last, below a divider, in `#FFB7A3`, with a reassurance note underneath ("The folder on disk is not touched").

---

## 7. Iconography

Line icons on a 16 × 16 grid, 1.5–1.6 px stroke, round caps and joins, drawn in the current text colour. Shown at 13–18 px. The current set: folder, code `</>`, plus, search, star (outline / filled), branch, play (filled), stop (filled square), restart, terminal, alert triangle, lock, kebab (three dots), plug (ports), layers (workspaces), sliders (settings), check. Agents use the `>_` mono glyph, never vendor logos.

---

## 8. States and copy

| Situation | Pattern |
|---|---|
| Empty project list | Dashed add target: "Choose a folder to dock…" + "Add scan root" |
| Missing folder | Muted name, red mono subline "Folder not found", actions **Locate…** / **Remove** |
| Tool missing | Status strip shows `code missing`; the action shows an inline error with the fix |
| Not a Git repo | Subline "no git"; Git tab explains and hides the counters |
| No upstream | Shows "no upstream" instead of ↑/↓ |
| Loading / scanning | Muted inline progress text in the scan banner; never a spinner that blocks the UI |
| Process failed | `FAILED` + exit code + last error line; Restart is the primary action |
| Permission denied | Say which process, why, and the fix ("run DevDock as administrator") |

**Copy voice:** short, plain, specific. Say what DevDock did or didn't do ("DevDock didn't start this process, so it won't stop it unless you confirm"). Use the real command, path or port, not a vague noun. No exclamation marks, no blame.

---

## 9. Interaction

- **Keyboard:** `Ctrl K` palette · `↑/↓` lists · `Enter` open · `Esc` close overlay or cancel rename · `Ctrl Alt 1–9` per-project shortcuts · `F5` refresh current panel.
- **Selection:** clicking a list row updates the detail view in place; the tab you were on is kept when switching projects.
- **Destructive actions** never fire on the first click when they affect anything outside DevDock.
- **Live data:** logs stream; Git and ports refresh on focus and on demand, and show a "refreshed hh:mm" stamp.
- **Motion:** 120–160 ms ease-out on hover/selection, and 180 ms fade+scale (0.98→1) for overlays. No decorative animation. Honour the OS reduced-motion setting.

---

## 10. Accessibility

- Text contrast ≥ 4.5:1 (all `text.*` tokens except `text.faint`, which is only used for decorative timestamps).
- Status always comes with text or a symbol, never colour alone. Success and error also differ clearly in lightness.
- Every icon-only button has an accessible name and a tooltip.
- Visible focus ring on every interactive element; full keyboard reachability.
- Hit targets ≥ 32 px in dense desktop rows, ≥ 40 px for primary actions.

---

## 11. Implementation notes (Rust UI)

- Put every token in `src/app.css` as CSS custom properties (`--bg-app`, `--line`, `--accent`, `--space-4`, `--radius-panel`, `--font-sans`, `--font-mono`) and keep raw hex values out of components.
- **Svelte:** one component per DESIGN.md component (`Button` with variants, `Panel`, `Banner`, `Badge`, `StatusDot`, ...) in `src/lib/components/`; scoped `<style>` blocks only reference the variables.
- Two themes: dark (base `:root`) and light (`:root[data-theme="light"]`). Raw hex values stay out of components; add a token to both blocks instead.
- Motion uses CSS transitions (120-160 ms ease-out) and respects `prefers-reduced-motion`.
- Keep the layout regions (rail, list, main, status strip) as separate view functions so screens can be swapped without touching the chrome.

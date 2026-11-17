# DevDock — Build Plan

> How to get from an empty Tauri project to the full app described in `PRODUCT.md` and `DESIGN.md`, one working release at a time.

Companion files: `PRODUCT.md` (what), `DESIGN.md` (how it looks), `TASKS.md` (the checklist).

---

## 1. Strategy

- **Vertical slices, not layers.** Each milestone ships a usable app: real UI, real logic, real persistence for that slice. No "build the whole core first."
- **Learning comes first.** Every milestone targets a specific set of Rust concepts. If a feature fights the learning goal, cut the feature.
- **Logic stays in Rust, the UI only renders.** All behaviour lives in plain Rust modules with no Tauri types, so it can be tested without a window. Svelte calls Rust through typed `invoke` wrappers and listens to events; it never touches the file system or processes itself.
- **Read before write.** Git stays read-only. Process control starts with processes DevDock owns; external processes come later and always need confirmation.
- **Rule from the roadmap:** if a milestone becomes complicated, remove features rather than growing the architecture.

## 2. Milestones

| # | Version | Theme | Exit criteria (demo) |
|---|---|---|---|
| M0 | — | Setup | `pnpm tauri dev` opens an empty, themed DevDock window; CI is green |
| M1 | v0.1.0 | Launcher | Add a folder, see it in the list, open it in Explorer / VS Code; a missing `code` shows a helpful error |
| M2 | v0.2.0 | Explorer | Projects survive restart; scanning `D:\dev` finds projects with types, no duplicates; search, sort, favorites, rename, remove, missing-folder handling |
| M3 | v0.3.0 | Git | Branch, upstream, ahead/behind and file counts match `git status` for normal repos, monorepo subfolders and worktrees |
| M4 | v0.4.0 | Run | Start `next dev` from DevDock, stream logs, stop and restart the whole process tree, UI never freezes; Running screen |
| M5 | v0.5.0 | Ports | Ports screen lists listeners with PID, process and owner; conflict detected before launch with safe options; confirmation for external processes |
| M6 | v0.6.0 | Agents | Detect `claude` / `codex` / `opencode`; launch in the chosen terminal and folder; shortcuts per project |
| M7 | v0.7.0 | Workspaces | Start a frontend + API + DB in order with health checks, restart rules and combined logs; stop all cleanly |
| M8 | v0.8.0 | Polish | Command palette, keyboard navigation, settings export/import, Overview tab, final pass against DESIGN.md |

The full app (all screens in the canvas) is complete at M8. The longer-term ideas in PRODUCT.md come after that.

## 3. Architecture

### 3.1 Layout (grows over time)

```
devdock/
├── package.json  svelte.config.js  vite.config.js   # pnpm + SvelteKit (SPA, adapter-static)
├── src/                        # Svelte UI: renders state, sends commands
│   ├── app.css                 # tokens from DESIGN.md as CSS variables
│   ├── lib/
│   │   ├── api.ts              # typed wrappers around invoke() / listen()
│   │   ├── types.ts            # TS mirrors of the Rust DTOs
│   │   ├── chrome/             # TitleBar, Rail, StatusStrip
│   │   ├── components/         # Button, Panel, Banner, Badge, StatusDot, ...
│   │   └── screens/            # projects, git, run, running, ports, agents, workspaces, settings
│   └── routes/+page.svelte     # single page; the rail switches screens
├── src-tauri/                  # Rust: the Tauri app crate
│   ├── tauri.conf.json  capabilities/
│   └── src/
│       ├── main.rs  lib.rs     # builder, plugins, managed state
│       ├── commands.rs         # #[tauri::command] adapters: thin, no logic
│       └── ...                 # domain modules (M1-M2), moved to devdock-core in M3
├── Cargo.toml                  # workspace (from M3)
├── crates/                     # from M3
│   ├── devdock-core/           # domain + services, no Tauri, no OS-specific code
│   │   └── src/
│   │       ├── project.rs      # Project, ProjectId, ProjectKind
│   │       ├── store.rs        # load/save, migrations
│   │       ├── detect.rs       # marker-file detection
│   │       ├── scan.rs         # scan roots, walkdir, ignore list
│   │       ├── git.rs          # GitStatus via git CLI + parser
│   │       ├── runner.rs       # process supervision, log channel
│   │       ├── ports.rs        # PortEntry, ownership matching
│   │       ├── agents.rs       # tool discovery, launch specs
│   │       ├── workspace.rs    # services, ordering, health, restart policy
│   │       └── error.rs        # DevDockError (thiserror)
│   └── devdock-platform/       # OS boundary behind traits (from M4)
│       └── src/{mod.rs, windows.rs}
└── docs/  PRODUCT.md DESIGN.md PLAN.md TASKS.md decisions.md
```

**Done in M4:** `crates/devdock-platform` holds the Windows Job Object code (the only `unsafe` in the project) and implements `devdock_core::run::ProcessTree`. **Done in M3:** the root `Cargo.toml` is a workspace; domain modules live in `crates/devdock-core` (no Tauri dependency) and `src-tauri` only holds `commands.rs` and `lib.rs`. Before M3, (`project.rs`, `launcher.rs`, `tools.rs`, `error.rs`, then `store.rs`, `detect.rs`, `scan.rs`). At the start of M3 a root `Cargo.toml` workspace is added and they move to `crates/devdock-core`, once there is enough logic to make it worth it.

### 3.2 Core ideas

- **UI state vs domain state.** Svelte (`$state` runes) holds UI state only: selected view, tab, filters, open dialog. Rust owns domain data (projects, process handles, statuses) in `tauri::State<Mutex<AppState>>`.
- **Commands in, events out.** The UI calls `#[tauri::command]`s (request/response, `Result<T, ErrorDto>`). Background work (scan, git, processes, port polling, health checks) runs on worker threads and pushes updates with `app.emit("scan:found", payload)`; the UI subscribes with `listen()`. Inside Rust, workers still talk over `std::sync::mpsc` where it helps.
- **One DTO boundary.** Everything crossing to the UI is a `serde::Serialize` struct with `#[serde(rename_all = "camelCase")]`, mirrored by hand in `src/lib/types.ts`. Commands are thin: parse input, call the domain function, map the error.
- **Platform traits.** `ProcessTree`, `PortTable`, `Launcher` (open folder, open editor, open terminal). `windows.rs` implements them; tests use fakes.
- **Errors.** `thiserror` for typed errors in Rust. Each error maps to an `ErrorDto { kind, message, fix }`, so the "what happened + what to do" copy from DESIGN.md §8 lives in Rust next to the error and is unit-tested; the UI only shows it. No `unwrap()` outside tests.
- **Persistence.** JSON (`serde_json`) in `%APPDATA%\DevDock\devdock.json`, with a `version` field and atomic writes (write to a temp file, then rename). Move to SQLite only when an open question in PRODUCT.md forces it (log history, many projects).

### 3.3 Threading by milestone

| Milestone | Concurrency |
|---|---|
| M1 | None. Launching is a quick `Command::spawn` without waiting; Tauri runs sync commands off the UI thread |
| M2 | One scan thread + channel |
| M3 | Git refresh on a worker thread per request |
| M4 | One supervisor thread per process; stdout/stderr reader threads; log ring buffer |
| M5 | Port poller on a timer |
| M7 | Workspace orchestrator (state machine) + health-check timers. Consider `tokio` only if threads get messy |

## 4. Tooling and workflow

- **Toolchain:** stable Rust, `rustfmt`, `clippy -D warnings`; Node LTS + pnpm, `svelte-check`.
- **JS packages:** `@tauri-apps/api`, `@tauri-apps/plugin-dialog` (M1), `@fontsource-variable/*` fonts (M0), `vitest` (when UI logic needs tests), `xterm.js` (M6, experimental).
- **Crates (add only when the milestone needs them):** `tauri` (M0), `tauri-plugin-dialog` (M1), `serde`, `serde_json`, `walkdir` (M2), `thiserror` (M1), `sysinfo` (M4–M5), `windows` crate for Job Objects / IP Helper (M4–M5), `crossbeam-channel` (optional), `portable-pty` (M6, experimental), `tokio` (M7, only if needed).
- **Git:** `main` stays releasable. One branch per task ID (`m2/T2.4-scan`). Tag `v0.x.0` at each milestone exit.
- **CI (GitHub Actions, windows-latest):** `pnpm install`, `pnpm check` (svelte-check), `cargo fmt --check`, `cargo clippy`, `cargo test`, `pnpm tauri build`. Attach the installer and `.exe` to the tag.
- **Docs:** update `TASKS.md` checkboxes in the same PR as the code. Write down every decision in a short `docs/decisions.md` entry.

## 5. Testing strategy

| Layer | How |
|---|---|
| Core logic | Unit tests: detection on temp folders, path normalisation, dedupe, store round-trip, git output parser with captured fixtures, port-owner matching, workspace ordering and restart policy |
| Integration | `tests/` creating real temp Git repos (`git init`, commits, worktrees) and checking `GitStatus`; spawning a tiny test binary to check supervision, the process tree and exit codes |
| Platform | Fakes for `ProcessTree` / `PortTable` / `Launcher` in core tests; manual checklist on Windows for the real implementations |
| UI | `svelte-check` in CI; `vitest` for pure TS helpers if any appear; manual demo script per milestone (the exit criteria above) plus a visual check against the canvas |
| Safety | Dedicated tests proving that DevDock never signals a PID it doesn't own without the confirmation flag |

## 6. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Ending a shell leaves orphan children on Windows (e.g. `npm` → `node`) | Ports stay taken, confusing state | Put every spawned process in a **Job Object** with kill-on-close; stop the job, not the PID |
| Port → PID mapping needs admin or is incomplete | Wrong or missing owner | Use IP Helper (`GetExtendedTcpTable`); show "unknown" honestly; never guess the owner from port or name |
| UI freezes on slow Git / scans | Bad feel | All I/O off the UI thread from M2; timeouts on git commands |
| Logic leaks into Svelte | Rust learning goal lost; untested behaviour | Rule: components only call `api.ts`; any branching on domain data goes into a Rust function with a test |
| Rust and TS types drift | Runtime `undefined`s | Keep DTOs few and flat; mirror them in `types.ts`; consider `ts-rs` / `specta` if they grow past ~15 |
| Scope creep (Git writes, terminal tabs) | Never ships | Non-goals in PRODUCT.md; new ideas go to a "Later" list, not the current milestone |
| Executable discovery differs (`.cmd` shims, PATHEXT) | Agent / editor "not found" | Resolve with PATHEXT; show the resolved path in Settings |
| JSON corruption on crash | Lost projects | Atomic write + keep a `.bak` of the previous save |

## 7. Definition of done (every task)

1. Behaviour matches the PRODUCT.md section and the DESIGN.md component.
2. Errors are handled with a user-facing message; no panics on bad input.
3. Core logic has tests; `fmt`, `clippy` and `test` pass in CI.
4. UI work never blocks the window: long work runs on a thread and reports through events.
5. Checkbox ticked in `TASKS.md`; notes added to `decisions.md` if something was decided.

## 8. Order of work inside each milestone

1. Domain types + tests (core).
2. Service / function + tests (core).
3. Platform implementation if needed.
4. UI screen using the theme components.
5. Errors and empty states.
6. Demo script → tag.

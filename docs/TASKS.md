# DevDock — Tasks

> Detailed checklist for `PLAN.md`. Each task has an ID, a size (**S** ≈ one sitting, **M** ≈ a few sittings, **L** ≈ break it down further before starting), what to build, and how you know it's done (**Done when**).
> Tick a box in the same PR as the code.

Legend: 🦀 = the main Rust concept this task teaches.

---

## M0 — Setup

> Redone on 2026-11-16 for Tauri + Svelte (the first pass used egui). See `decisions.md`.

- [x] **T0.1 Create the project** · S
  `create-tauri-app` (SvelteKit + TS, pnpm); window titled "DevDock", 1280×900, min 1100×720.
  Done when: `pnpm tauri dev` opens an empty window. 🦀 Cargo, crates, `tauri::Builder`
- [x] **T0.2 Theme tokens** · M
  `src/app.css` with every colour, space, radius and font token from DESIGN.md §2–4 as CSS variables; dark only.
  Done when: the window background, panels and buttons use DevDock colours.
- [x] **T0.3 Fonts** · S
  Schibsted Grotesk + Martian Mono via `@fontsource-variable` (OFL); `--font-sans` / `--font-mono`.
  Done when: body text uses the sans and machine text uses Martian, with no network requests.
- [x] **T0.4 App shell** · M
  `TitleBar` (logo mark + wordmark), `Rail`, `StatusStrip`, main area; each its own component.
  Done when: the layout matches DESIGN.md §5 with placeholder content.
- [ ] **T0.5 Tooling** · S
  `rustfmt.toml`, `clippy` config, `[lints]`, GitHub Actions on `windows-latest` (pnpm check, fmt, clippy, test, tauri build).
  Done when: CI is green on `main`.
  Status: workflow written; needs a GitHub remote and a first push.
- [x] **T0.6 Docs in repo** · S
  Add `docs/` with PRODUCT, DESIGN, PLAN, TASKS and an empty `decisions.md`.

---

## M1 — Launcher (v0.1.0)

- [x] **T1.1 `Project` type** · S
  `struct Project { id, name: String, path: PathBuf }`; `name` defaults to the folder name.
  Done when: unit test — `Project::from_path("D:\\dev\\devchrono")` has name `devchrono`. 🦀 `struct`, `String` vs `&str`, `PathBuf`
- [x] **T1.2 Project list in Rust state** · S
  `AppState { projects: Vec<Project> }` behind a `Mutex` in `tauri::State`; `list_projects` command; Svelte renders rows (slot number, name, mono path) and the empty state.
  Done when: rows look like the Phase 1 artboard. 🦀 `Vec`, `Mutex`, borrowing in a loop
- [x] **T1.3 Folder picker** · S
  "Add project" → `@tauri-apps/plugin-dialog` `open({ directory: true })` → `add_project(path)` command (rejects a missing folder and duplicates); highlight it with a "just added" tag.
  Done when: picking a folder adds it to the list; cancelling does nothing. 🦀 `Option`, `if let`
- [x] **T1.4 `Launcher` functions** · M
  `open_in_explorer(&Path)`, `open_in_editor(&Path, editor)` using `std::process::Command` (`explorer.exe`, `code`). Return `Result<(), LaunchError>`.
  Done when: both buttons open the right folder. 🦀 `Result`, `?`, `Command`
- [x] **T1.5 Error type + UI** · M
  `LaunchError { NotFound(path), EditorMissing(cmd), Io(io::Error) }` with `thiserror`, mapped to `ErrorDto { kind, message, fix }`; show the error banner (DESIGN.md "Error" banner) with the fix text and Dismiss.
  Done when: renaming `code` off PATH shows "code wasn't found on PATH…" and Explorer still works. 🦀 `enum` with data, `match`, `From`
- [x] **T1.6 Status strip** · S
  Show the project count and whether `explorer.exe` / `code` were found (check once at startup).
- [ ] **T1.7 Release** · S
  Manual demo script; tag `v0.1.0`; attach the `.exe`.
  Status: waiting on the first commit and push.

**Out of scope:** saving, scanning, Git, processes.

---

## M2 — Explorer (v0.2.0)

- [x] **T2.1 Store** · M
  `store.rs`: `load()` / `save(&Saved)`; file `%APPDATA%\DevDock\devdock.json` via Tauri's path API (`DEVDOCK_DATA_DIR` overrides it for testing); `version: u32` field; atomic write + `.bak`.
  Done when: tests round-trip; a corrupt file falls back to `.bak` with a warning. 🦀 serde derive, `std::fs`, error propagation
- [x] **T2.2 Save on change** · S
  Save after add / rename / remove / favorite (debounced 500 ms). Load on start.
  Done when: restarting keeps every change.
- [x] **T2.3 Path normalisation + dedupe** · S
  Canonicalise and lowercase (Windows), strip `\\?\`, trailing slash. `ProjectId` = hash of the normalised path.
  Done when: tests — `D:\Dev\x\` and `d:\dev\x` are the same project. 🦀 traits (`Hash`, `Eq`)
- [x] **T2.4 Detection** · M
  `detect(&Path) -> Detected { kind: ProjectKind, markers: Vec<Marker>, is_git }`. Rules: `next.config.*` + `package.json` → NextJs; `package.json` → Node; `Cargo.toml` → Rust; `composer.json` → Php; `docker-compose.yml` → Compose; `.git` (dir **or** file for worktrees) → git.
  Done when: tests run on temp folders for each rule. 🦀 enums, `match`, iterators
- [x] **T2.5 Scan roots** · L → split:
  - [x] T2.5a `scan(root, depth, ignore) -> Vec<Candidate>` with `walkdir`; stop going deeper once a folder is detected as a project; skip `node_modules`, `target`, `.next`, `vendor`, `.git`. 🦀 iterator adapters, closures
  - [x] T2.5b Run the scan on a worker thread; emit `ScanEvent::{Found, Done{new, dupes, ignored}}` as Tauri events. 🦀 `std::thread`, `mpsc`, `move` closures
  - [x] T2.5c Scan banner UI ("3 new, 2 duplicates skipped…") + "Review new" filter.
  Done when: scanning a 15-project folder takes no noticeable UI time and finds every project exactly once.
- [x] **T2.6 Sidebar** · M
  Library filters (All, Favorites, Recently opened, Missing) with counts; scan roots list; "Saved to …" footer.
- [x] **T2.7 Search, sort, chips** · M
  Case-insensitive search on name + path; sort by recent / name / type; type chips with counts. Filtering is a pure function `filter(&[Project], &Query) -> Vec<&Project>`.
  Done when: tests for the filter function. 🦀 lifetimes on returned references
- [x] **T2.8 Favorites + recent** · S
  Star toggle; record `last_opened` when any "open" action succeeds.
- [x] **T2.9 Rename + remove** · M
  Row menu (Rename, Copy path, Remove); inline rename (Enter saves, Esc cancels); Remove never touches disk — say so in the menu.
- [x] **T2.10 Missing folders** · S
  Check existence on load and on focus; muted row + "Folder not found" + Locate… (re-pick a path, keep the id's metadata) / Remove.
- [ ] **T2.11 Release** · S — demo, tag `v0.2.0`.

---

## M3 — Git dashboard (v0.3.0)

- [x] **T3.0 Split into a workspace** · M
  Add a root `Cargo.toml` workspace; move domain modules from `src-tauri` into `crates/devdock-core`. No behaviour change.
  Done when: CI is green, and `devdock-core` has no `tauri` dependency. 🦀 workspaces, `pub` visibility
- [x] **T3.1 Run git** · S
  `git_cmd(dir, args) -> Result<String, GitError>` with a timeout; `GitError::{NotInstalled, NotARepo, Failed(stderr)}`.
- [x] **T3.2 Parse status** · M
  Use `git status --porcelain=v2 --branch`; parse the branch, upstream, ahead/behind and counts of staged / modified / untracked files into `GitStatus`.
  Done when: fixture tests cover: clean, dirty, detached HEAD, no upstream, renamed files. 🦀 string parsing, `split`, `parse::<i32>()`, `Option` chaining
- [x] **T3.3 Repo facts** · S
  Root (`rev-parse --show-toplevel`), last commit (`log -1 --format=%h%x00%s%x00%an%x00%cr`), remotes (`remote -v`), worktrees (`worktree list --porcelain`).
- [x] **T3.4 Shared repos + worktrees** · M
  Project path ≠ repo root → show "inside shared repo <root>"; `.git` file → mark as a worktree.
  Done when: integration tests with real temp repos (`git init`, `git worktree add`).
- [x] **T3.5 Background refresh** · S
  Refresh on select, on window focus and via the button; worker thread + `GitEvent`; "refreshed hh:mm" stamp.
- [x] **T3.6 Git UI** · M
  Branch strip, three counters, key–value facts panel, read-only note; branch + change count in list rows; "no git" / "no upstream" states.
- [ ] **T3.7 Release** · S — demo against `git status` in 3 repos, tag `v0.3.0`.

---

## M4 — Dev server manager (v0.4.0)

- [x] **T4.1 Commands model** · S
  `Command { name, program, args, cwd, source: Script | Custom }`; read `scripts` from `package.json` (serde `Value`); custom commands saved per project.
- [x] **T4.2 Platform: process tree** · L → split:
  - [x] T4.2a `trait ProcessTree` / `TreeHandle` in `devdock-core::run::process`; `devdock-platform` implements it. 🦀 traits, trait objects vs generics
  - [x] T4.2b Windows impl: create a Job Object with `KILL_ON_JOB_CLOSE`, spawn through `cmd /C` (or resolve `npm.cmd`), assign it to the job.
  - [x] T4.2c Test: spawn a script that starts a child; `kill_tree` leaves no children (checked with the job's own active-process count, no `sysinfo`).
- [x] **T4.3 Supervisor** · M
  One thread per run: spawn, read stdout/stderr on two threads into a `LogLine { ts, stream, text }` channel, wait for exit → `RunEvent::{Started{pid}, Line, Exited{code}}`. 🦀 `Child`, `BufReader::lines`, `Arc<Mutex<_>>`, `JoinHandle`
- [x] **T4.4 Run state machine** · S
  `Idle → Starting → Running → (Stopping) → Exited(code) | Failed(code)`; restart = stop then start.
  Done when: unit tests for the transitions. 🦀 enums as state machines
- [x] **T4.5 Log buffer** · S
  Ring buffer (e.g. 5 000 lines per run); filter by stream; "follow" auto-scroll.
- [x] **T4.6 Run tab UI** · M
  Command cards (▶, last result), process header (state pill, PID, tree size, uptime, port), Stop / Restart, console with stderr highlighting.
- [x] **T4.7 Running screen** · M
  Rail item + table of every owned process; Stop everything; running count in the title bar.
- [x] **T4.8 Shutdown** · S
  On window close: stop owned processes (setting, default on), or warn first if any are running.
  Status: warns first, then stops everything. No setting: with kill-on-close jobs the processes can't outlive DevDock (see decisions.md).
- [x] **T4.9 Port hint** · S
  Pick up `localhost:NNNN` from logs to show the port (only a hint until M5).
- [ ] **T4.10 Release** · S — demo `next dev` + `cargo run`, tag `v0.4.0`.
  Status: demoed with a dependency-free Node dev server (via `npm run dev`) and `cargo run`; a real `next dev` run and the tag are still to do.

---

## M5 — Port inspector (v0.5.0)

- [x] **T5.1 Platform: port table** · M
  `trait SystemProbe` (listeners, processes, terminate) in `devdock-core::ports`; Windows impl via `GetExtendedTcpTable` (IPv4 + IPv6) in `devdock-platform`. 🦀 FFI / `windows` crate, `unsafe` boundaries
- [x] **T5.2 Process info** · S
  PID → name, command line, cwd, start time, children (`sysinfo`); handle access denied → `Unknown`.
- [x] **T5.3 Ownership** · M
  Owner = `DevDock` only if the PID is in one of our job objects; `System` for PID 0/4 and protected; otherwise `External`. Match a project by cwd prefix, **never** by name or port.
  Done when: tests with fakes — a same-name process not in our job is External.
- [x] **T5.4 Poller** · S
  Refresh every 3 s while Ports is visible, and on demand; "refreshed Ns ago".
- [x] **T5.5 Pre-launch check** · M
  Before starting a command with a known port, check the table; if taken → conflict banner with: alternative port (if a port flag/env is configured), Show process, End PID….
- [x] **T5.6 Confirmation for external processes** · M
  Dialog with command, folder, start time, children, a "also end children" checkbox; only then terminate. Errors (access denied) are shown as described in DESIGN.md §8.
  Done when: a test proves `terminate_external` can't be called without a `Confirmed` token type.
  Status: three `compile_fail` doctests (forged `Confirmed`, forged `Permit`, bare PID) plus runtime checks for PID reuse, protected and owned processes. 🦀 newtype pattern for safety
- [x] **T5.7 Ports UI** · M
  Table with owner badges, filter (All / DevDock / External), Stop for owned, Inspect for external, Protected for system.
- [ ] **T5.8 Release** · S — demo the port 3000 conflict, tag `v0.5.0`.
  Status: conflict demoed live on port 4870 (3000 was avoided to stay clear of other local servers); the tag waits on a commit.

---

## M6 — AI workspace launcher (v0.6.0)

- [x] **T6.1 Executable discovery** · M
  Resolve `claude`, `codex`, `opencode`, `code`, `wt`, `pwsh`, `git`, `docker` via PATH + PATHEXT (`.exe`, `.cmd`, `.bat`); cache; Re-check button.
  Done when: tests with a fake PATH. 🦀 `std::env`, `OsString`
  Also: `git-bash.exe` is found next to Git, and `wt.exe` in `%LOCALAPPDATA%\Microsoft\WindowsApps` when that folder isn't on PATH.
- [x] **T6.2 Terminal launcher** · M
  `LaunchSpec { terminal, cwd, program }` → Windows Terminal (`wt -d <cwd> <program>`), PowerShell, Git Bash.
  Plus Command Prompt. `agents::plan` is pure and unit-tested per terminal; opt-in tests (`--test terminals -- --ignored`) open each terminal and check the folder.
- [x] **T6.3 Agents tab UI** · M
  Agent cards (detected / not installed + install guide), terminal + working-folder selects, launch-history line.
- [x] **T6.4 Shortcuts** · M
  Per-project `Ctrl Alt 1–9` bindings to a launch spec (while DevDock has focus); edit list in the tab. (Global hotkeys = an open question.)
- [x] **T6.5 Settings screen (first pass)** · S
  Editor, terminal, default agent; detected tools list.
- [ ] **T6.6 Built-in PTY (experimental, optional)** · L
  Status: skipped for M6 as the task allows; external terminals cover the launch use case.
  `portable-pty` in Rust streaming to `xterm.js` in a panel; behind an "experimental" setting. Skip if it slows M6.
- [ ] **T6.7 Release** · S — demo launching Claude Code in a project folder, tag `v0.6.0`.
  Status: launches verified with a stand-in `claude` (PowerShell, Command Prompt, shortcut) and a `touch`/`New-Item` probe (Git Bash, Windows Terminal); the demo with the real Claude Code and the tag are still to do.

---

## M7 — Workspaces (v0.7.0)

- [x] **T7.1 Model** · S
  `Workspace { name, services: Vec<Service> }`, `Service { project, command, port, health: Option<Health>, order }`, `Health::{Tcp(port), Http(url, expect)}`, `RestartPolicy { max, window }`.
- [x] **T7.2 Ordering** · S
  Sort by `order`; reject cycles if `depends_on` is added later.
  Done when: unit tests. 🦀 sorting, `Ord`
- [x] **T7.3 Health checks** · M
  TCP connect / HTTP GET with timeout; interval 10 s; 3 misses = unhealthy.
- [x] **T7.4 Orchestrator** · L
  State machine per workspace: start service *n* → wait for healthy (with timeout) → start *n+1*; on crash apply the restart policy; Stop all in reverse order.
  Done when: tests with fake services covering: happy path, slow start, crash loop hitting the limit, stop while starting. 🦀 state machines, timers, `select`-style loops
- [x] **T7.5 Combined logs** · S
  Merge service log streams, add a prefix label and colour; per-service filter.
- [x] **T7.6 Docker Compose services** · M
  Service kind `Compose { file, service }` → `docker compose up <svc>` / `stop`; health via port.
  Status: command lines and validation are unit-tested; not run live (Docker isn't installed on the dev machine).
- [x] **T7.7 Worktree option** · S
  Service can target a project worktree path from M3 data.
  Status: validated against `git worktree list`; run keys and folder rebasing unit-tested; not run live.
- [x] **T7.8 Workspaces UI** · M
  List with health bars, service cards joined by arrows, Start all / Stop all, rules panel, "open VS Code + agent after start" preset.
  Status: preset reuses the tested editor/agent launch paths; not triggered live (it would open windows).
- [ ] **T7.9 Release** · S — demo the Wallet stack, tag `v0.7.0`.
  Status: demoed live with a 3-service Node stack (db → api → web): ordered start, crash restart, user stop, reverse Stop all. Tag waits on a commit.

---

## M8 — Polish → full app (v0.8.0)

- [x] **T8.1 Overview tab** · M — summary tiles linking to Git / Run / Ports / Agents; detected facts; recent activity (needs an activity log in the store).
- [x] **T8.2 Command palette** · L — `Ctrl K`; fuzzy matcher over projects, commands, agents, workspaces, ports; grouped results; Enter runs. 🦀 trait for "palette source", boxed iterators
- [x] **T8.3 Keyboard navigation** · M — arrows in lists, Enter, Esc, F5, visible focus ring everywhere.
- [x] **T8.4 Settings complete** · M — scan roots & depth, safety toggles (external confirmation locked on), export / import JSON.
- [x] **T8.5 Empty & error states audit** · S — walk DESIGN.md §8 table; every state implemented.
- [x] **T8.6 Visual pass** · M — compare every screen with the canvas; fix spacing, colours, type.
- [x] **T8.7 Accessibility pass** · S — tooltips on icon buttons, contrast check, status always has text.
- [x] **T8.8 Performance pass** · S — start-up time, idle CPU (repaint only on events), memory with 5 runs streaming.
- [ ] **T8.9 Release** · S — tag `v0.8.0`; update README with screenshots. README done; tag waits for the first commit.

---

## Later (not scheduled)

Git branch create/switch · visual diffs · terminal tabs · Docker container monitoring · environment profiles per project · CPU/memory per process · start-up history · Linux & macOS · global hotkeys · persisted log history (SQLite).

---

## Progress

| Milestone | Tasks | Done |
|---|---|---|
| M0 Setup | 6 | 5 |
| M1 Launcher | 7 | 6 |
| M2 Explorer | 11 | 10 |
| M3 Git | 8 | 7 |
| M4 Run | 10 | 9 |
| M5 Ports | 8 | 7 |
| M6 Agents | 7 | 5 |
| M7 Workspaces | 9 | 8 |
| M8 Polish | 9 | 8 |

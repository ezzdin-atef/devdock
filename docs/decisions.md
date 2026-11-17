# Decisions

Short entries, newest first. Format: date, decision, why.

## 2026-11-16 - M8: performance baseline (release build)
Measured on this machine with `target/release/devdock.exe` (5.8 MB): start-up to the first rendered
project list 1.5-2.1 s; settled idle 0.05-0.5 % of one core, 383 MB across 7 processes (WebView2
included); 5 runs streaming about 250 lines/s in total with the console visible: 32 % of one core
(2.7 % of the machine), 502 MB, and a `library` round trip stays at 4.8 ms median, 6.9 ms max.
Nothing repaints without an event, so no polling was removed; memory is mostly WebView2.

## 2026-11-16 - M8: the CSP allows the IPC origin
The release build blocked `ipc.localhost` under `default-src 'self'`, so Tauri fell back to
`postMessage` with console errors on every call. `connect-src 'self' ipc: http://ipc.localhost` is
the fix. Dev never showed it because Vite serves the page; check release builds for CSP errors.

## 2026-11-16 - M8: palette items are built once per opening
`palette_open` gathers projects, commands, agents, workspaces and ports (the slow part) into a
`Vec<PaletteItem>` behind a mutex; `palette_search` only scores it, so each keystroke is cheap.
Each kind is a `PaletteSource` returning a boxed iterator. The fuzzy matcher is a small DP that
rewards word starts and runs of letters, so "run dev chr" ranks "Run dev in devchrono" first.

## 2026-11-16 - M8: activity is part of the project, capped at 50
Opens, run starts/ends and agent launches are stored on the project (newest first, 50 kept) so
the Overview tab can show them after a restart. Run end is recorded once per run, from the
state-change event, not from the UI.

## 2026-11-16 - M8: import is refused while anything runs
Replacing the project list while runs or workspaces point at old ids would orphan them. Import
says "Stop everything, then import again" instead. The current file is kept as `.bak` first.

## 2026-11-16 - M8: contrast is checked in CI
`scripts/check-contrast.mjs` reads the tokens from `app.css` and checks 25 text/background pairs
against 4.5:1, so a colour tweak can't silently fail WCAG AA.

## 2026-11-16 - M7: the orchestrator is a pure state machine driven by `tick`
`Orchestrator::tick(now, driver)` does one step; the app calls it every 250 ms on a thread, tests
call it with a fake driver and a fake clock (start order, 60 s timeout, crash loop hitting the
limit, stop while starting, user stop, 3 missed checks) in well under a second. No tokio.

## 2026-11-16 - M7: the ticking thread owns its orchestrator
The first version shared it behind a mutex. Health checks block (a refused localhost connection
takes about a second per address on Windows), so reading workspace state from the UI waited up
to 4 s. Now the thread owns the orchestrator, publishes a snapshot after each tick and receives
Stop as a flag; reads take ~16 ms. TCP/HTTP check timeouts are 800 ms.

## 2026-11-16 - M7: services run through the same Runner as the Run tab
A workspace's `dev` uses the same run key as the project's Run tab, so it appears there, on
Running and as DEVDOCK on Ports, and starting a workspace adopts an already-running `dev`
instead of starting a second copy. Worktree services get their own key suffix; Compose services
use `compose:<file>:<service>`.

## 2026-11-16 - M7: a user stop is not a crash
If the user stops a workspace service elsewhere (Run tab, Running screen), the orchestrator marks
it Stopped instead of restarting it. Only exits DevDock didn't ask for count against the policy.

## 2026-11-16 - M7: health checks on std::net; http only
A TCP connect or a hand-written `GET` request is enough for local dev servers, so no HTTP client
crate. https URLs are refused with a clear message rather than half-supported.

## 2026-11-16 - M6: agents run by name inside the user's shell
DevDock opens the chosen terminal in the project (or a subfolder) and runs `claude` / `codex` /
`opencode` there, exactly as if the user typed it. It doesn't run the resolved path or talk to the
agent: the user's own shell setup (PATH order, profile, aliases) decides, and the agent outlives
DevDock like any window the user opened. Program names are checked against a strict character set
because they end up inside shell command lines.

## 2026-11-16 - M6: Git Bash needs CHERE_INVOKING=1
`git-bash.exe` starts a login shell, and Git for Windows' `/etc/profile` changes to `$HOME`
unless `CHERE_INVOKING` is set, so agents started in the home folder instead of the project.
Found in live testing; an opt-in test now checks the folder for every terminal.

## 2026-11-16 - M6: Windows Terminal is found off PATH too
`wt.exe` is an app execution alias in `%LOCALAPPDATA%\Microsoft\WindowsApps`, which some
accounts don't have on PATH, and `Path::is_file` can't follow those aliases. Discovery accepts any
non-directory entry and checks that folder explicitly.

## 2026-11-16 - M6: shortcuts are window-level, not global hotkeys
Ctrl+Alt+1..9 work while DevDock has focus (matched on the physical key, so keyboard layouts
don't matter). Global hotkeys stay an open question in PRODUCT.md. Note: on layouts where
AltGr = Ctrl+Alt, these combinations can collide with typed characters.

## 2026-11-16 - M6: under `tauri dev`, launched terminals close when the app restarts
The dev runner ends DevDock's whole process tree on rebuild, taking terminals with it. A normal
start from Explorer has no such job. If this ever matters in production (DevDock started from a
shell that uses a kill-on-close job), spawn with `CREATE_BREAKAWAY_FROM_JOB`.

## 2026-11-16 - M6: the built-in PTY (T6.6) is skipped for now
External terminals cover launching agents; an embedded terminal adds `portable-pty`, `xterm.js`
and input handling for little gain at this point.

## 2026-11-16 - M5: ending an external process needs a token only core can mint
`SystemProbe::terminate` takes a `Permit`; `Permit` and `Confirmed` have private fields, and the
only constructor path is `ports::confirm`, which re-reads the process and refuses if it exited,
if the PID now belongs to a different process (start time changed), if it is a system process,
or if it is DevDock's own (those are stopped through their job). `terminate_external` consumes
the `Confirmed`, so one confirmation ends one process (tree). `compile_fail` doctests pin this.

## 2026-11-16 - M5: "System" includes processes whose session Windows won't reveal
Non-admin users can't query the session of services (even PID 4 returns access denied). Such a
process belongs to another account or is elevated, so DevDock couldn't end it anyway; it is
shown as SYSTEM / Protected instead of offering an End button that would fail. A process whose
details vanished between two reads stays EXTERNAL.

## 2026-11-16 - M5: ports are learned from output, and only from normal starts
When a run prints `localhost:NNNN`, the port is saved per command (`Project.ports`) and checked
before the next start. Runs started with "Start on port N" (`PORT=N`) are not learned, so the
next plain start is still checked against the command's usual port.

## 2026-11-16 - M5: sysinfo for process details, Win32 for sessions and termination
`sysinfo` reads names, parents, start times and (only for listening PIDs) command lines and
folders. Session ids (`ProcessIdToSessionId`) and `TerminateProcess` are direct calls, because
DevDock needs the precise access-denied error to explain it (DESIGN.md section 8).

## 2026-11-16 - M4: one Job Object per run, shell created suspended
`cmd /D /S /C "<line>"` is spawned with `CREATE_SUSPENDED`, assigned to a fresh job with
`KILL_ON_JOB_CLOSE`, then resumed (its main thread is found with a Toolhelp snapshot, since `std`
doesn't expose the thread handle). Nothing the shell starts can escape the job, Stop is one
`TerminateJobObject`, and if DevDock exits or crashes Windows kills every tree. Verified: after
Stop and after Quit, the demo servers' grandchildren were gone and their ports free.

## 2026-11-16 - M4: Stop is a hard kill of the tree
No Ctrl+C first: delivering a console control event to a windowless process group needs console
juggling that isn't worth it for dev servers. Revisit if a tool needs graceful shutdown (e.g. to
flush a database); Docker Compose services in M7 will use `docker compose stop` instead.

## 2026-11-16 - M4: the ProcessTree trait lives in core, `devdock-platform` implements it
Dependency points inwards: core defines what it needs, the platform crate provides it, the app
wires `devdock_platform::process_tree()` into the `Runner`. Core's supervisor tests use a plain
`cmd` + `taskkill` tree, so they don't depend on the Win32 code.

## 2026-11-16 - M4: `unsafe` is confined to `devdock-platform/src/windows.rs`
The workspace forbids `unsafe`; the platform crate sets its own lints (`unsafe_code = "deny"`,
allowed in that one module) and `clippy::undocumented_unsafe_blocks = "deny"`, so every block
carries a SAFETY comment.

## 2026-11-16 - M4: no "leave processes running on close" setting
T4.8 mentioned a setting. With kill-on-close jobs, processes can't outlive DevDock, so the only
honest behaviour is: if anything is running, ask first ("Stop all and quit" / Cancel).

## 2026-11-16 - M4: logs are pulled, change notices are pushed (coalesced)
The supervisor appends to a 5 000-line ring buffer per run; a single thread emits at most one
`run:changed` event per run every 80 ms, and the console fetches only lines after its last
sequence number. A chatty process can't flood the webview with events.

## 2026-11-16 - M4: stderr is shown as ERR even when it's just progress
`cargo` and many bundlers print progress on stderr. DevDock shows streams truthfully (tinted,
`ERR` tag) and offers an Output/Errors filter rather than guessing which stderr lines matter.

## 2026-11-16 - M3: workspace split; launcher and tool lookup stay in core for now
`crates/devdock-core` holds all domain code and has no Tauri dependency (checked with
`cargo tree`). `launcher.rs` and `tools.rs` are Windows-flavoured but tiny; they move to
`devdock-platform` with the process-tree work in M4 rather than creating a crate for two files.

## 2026-11-16 - M3: git is read through the CLI, read-only by construction
Every call passes `--no-optional-locks` (plus `GIT_OPTIONAL_LOCKS=0`), so even `git status`
never rewrites the index; an integration test checks the index bytes are unchanged. Calls have a
timeout and `GIT_TERMINAL_PROMPT=0`, so a credential prompt can't hang DevDock.

## 2026-11-16 - M3: list rows use `git status` only; the Git tab fetches full facts
One git call per project keeps a refresh of every project cheap (4 at a time on worker threads).
The selected project also gets rev-parse, log, remote and worktree calls. Results are a
session cache in Rust; "not a repo" is a normal state (`noRepo`), not an error.

## 2026-11-16 - M3: monorepo subfolders show whole-repo counts
`git status` run in a subfolder reports the whole repository, and the exit criterion is "match
`git status`", so DevDock does the same and says so ("SHARED REPO ... counts are for the whole
repository").

## 2026-11-16 - M3: UI git caches are keyed by project id
The first version kept one "current details" slot in Svelte. Under slow first git calls it showed
one project's facts under another. Keying details and loading flags by project id makes that
impossible; a 60-check rapid-switching run on a cold app found no mismatches.

## 2026-11-16 - M2: filtering, sorting and counts live in Rust
`library::filter(&[Project], &Query, &Flags) -> Vec<&Project>` runs on every keystroke via the
`library` command. IPC is fast enough for hundreds of projects, the logic stays unit-tested, and
it keeps the lifetimes lesson from T2.7 in Rust. Svelte keeps only the query object.

## 2026-11-16 - M2: project ids are FNV-1a hashes of the normalised path, as hex strings
`std`'s `DefaultHasher` is not stable across Rust versions, and ids are saved to disk, so a small
hand-written FNV-1a is used. Ids are 16-hex-digit strings because a u64 doesn't fit a JS number.
An id follows its path, so **Locate…** gives the project a new id and carries the metadata over.

## 2026-11-16 - M2: data folder via Tauri, not the `directories` crate
`app.path().config_dir()` already gives `%APPDATA%`; one dependency fewer. Setting
`DEVDOCK_DATA_DIR` points DevDock at another folder, so tests and demos never touch the real list.

## 2026-11-16 - M2: damaged saves are moved aside, never overwritten
If `devdock.json` can't be read (corrupt, or written by a newer DevDock), it is renamed to
`devdock.json.corrupt-<ms>`, the `.bak` is loaded, a notice is shown once, and a fresh file is
written straight away.

## 2026-11-16 - M2: "New from scan" is session-only
Projects added by the latest scan get a `NEW` tag and their own library filter until the next scan
or restart. Opening a project clears its tag. Persisting it wasn't worth a schema field.

## 2026-11-16 - UI moves from egui to Tauri 2 + Svelte 5
The UI is a SvelteKit SPA (adapter-static) inside a Tauri 2 window; all logic and domain state stay
in Rust. A web UI matches DESIGN.md (hairlines, type, motion, tables) far more cheaply than custom
egui styling. To keep Rust as the learning focus, components may only call typed wrappers in
`src/lib/api.ts`, and user-facing error copy is produced in Rust (`ErrorDto`), not in Svelte.
The egui M0 shell was discarded (never committed).

## 2026-11-16 - Fonts from @fontsource-variable
Schibsted Grotesk and Martian Mono come from the `@fontsource-variable` npm packages (SIL OFL) and
are bundled by Vite, so there are no binary font files in the repo and no network loads at runtime.

## 2026-11-16 - Native window decorations for now
The DevDock title bar is an in-app 48 px region under the OS title bar. A frameless window
(`decorations: false` + `data-tauri-drag-region`) is deferred to M8 polish.

## 2026-11-16 - No persistence in M1
Projects live in `Mutex<AppState>` for the session only; saving arrives with M2 (T2.1). The list
starts empty and shows the "Choose a folder to dock" empty state.

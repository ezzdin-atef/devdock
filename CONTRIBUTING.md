# Contributing to DevDock

Thanks for taking the time to help. This guide covers building DevDock from source, the checks
every change must pass, how the code is laid out, and how releases are cut.

- [Ground rules](#ground-rules)
- [Build from source](#build-from-source)
- [Checks and tests](#checks-and-tests)
- [Architecture](#architecture)
- [Commit messages](#commit-messages)
- [Releasing](#releasing)

## Ground rules

- Open an [issue](https://github.com/ezzdin-atef/devdock/issues) before large changes so the
  direction can be agreed first.
- Keep the [safety rules](README.md#safety-rules) intact - they are product invariants, not
  options.
- Rust owns all state and all user-facing error text; the UI only renders and sends commands.
- Read [docs/decisions.md](docs/decisions.md) before reworking an existing design choice.

## Build from source

### Prerequisites

- Windows 10/11
- [Rust](https://rustup.rs) stable with the MSVC toolchain (edition 2024)
- Node 20+ and [pnpm](https://pnpm.io) (the version is pinned in `package.json`)
- WebView2 runtime (built into Windows 11)
- Visual Studio Build Tools with "Desktop development with C++" (needed by the MSVC toolchain)

### Commands

Install JS dependencies:

```bash
pnpm install
```

Run in development (hot reload for the UI, rebuilds Rust on change):

```bash
pnpm tauri dev
```

Build a release binary and the NSIS installer:

```bash
pnpm tauri build
```

The binary lands in `target/release/devdock.exe`, the installer in
`target/release/bundle/nsis/`.

To develop without touching your real project list, point DevDock at a scratch data folder:

```powershell
$env:DEVDOCK_DATA_DIR = "C:\temp\devdock-demo"; pnpm tauri dev
```

## Checks and tests

The same checks CI runs on `windows-latest` for every push to `main` and every PR:

```bash
cargo fmt --all --check
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

```bash
pnpm check
```

```bash
pnpm contrast
```

- `cargo test` includes integration tests that create real Git repositories
  (`crates/devdock-core/tests/git_repos.rs`) and check terminal launch plans
  (`tests/terminals.rs`), so `git` must be on `PATH`.
- `pnpm check` runs `svelte-check` (TypeScript + Svelte).
- `pnpm contrast` verifies theme colours meet WCAG AA in both themes.

Lint policy: `unsafe_code` is forbidden workspace-wide (except the platform crate),
`clippy::unwrap_used` is denied and `expect_used` warns.

If a change affects the UI, recapture the README images with demo data:

```bash
pnpm screenshots
```

## Architecture

```
+-------------------------------+
|  Svelte 5 UI (src/)           |  renders DTOs, sends commands
|  src/lib/api.ts (typed calls) |
+---------------+---------------+
                | Tauri commands + events
+---------------v---------------+
|  src-tauri/                   |  thin command/event layer
+---------------+---------------+
                |
+---------------v---------------+     +------------------------------+
|  crates/devdock-core          |---->|  crates/devdock-platform     |
|  all domain logic and state   |     |  Windows FFI: Job Objects,   |
|  (no Tauri dependency)        |     |  TCP table, process details  |
+-------------------------------+     +------------------------------+
```

Platform-specific code is isolated behind traits so the core stays testable and portable.

| Path | What it is |
|---|---|
| `crates/devdock-core` | Domain logic: projects, detection, scanning, git, runs, ports, agents, workspaces, palette, persistence. |
| `crates/devdock-platform` | Windows FFI (Job Objects, TCP table, process details). The only crate allowed `unsafe`. |
| `src-tauri` | Tauri app: commands, events, window, updater, installer config. |
| `src` | Svelte 5 (SvelteKit, SPA mode) UI. Screens in `src/lib/screens`, shared components in `src/lib/components`. |
| `scripts` | Release, changelog, contrast-check and screenshot scripts (`scripts/demo` holds the demo backend). |
| `site` | The landing page published to GitHub Pages. |
| `docs` | Product spec, design system, plan, tasks, decisions, states audit, release guide. |
| `.github/workflows` | `ci.yml` (checks + unsigned build), `release.yml` (signed release on `v*` tags), `pages.yml` (site). |

Key modules in `devdock-core`:

| Module | Responsibility |
|---|---|
| `detect`, `scan`, `library`, `project` | Project types, scan roots, the project list |
| `git/` | Read-only Git facts via the CLI, with caching |
| `run/` | Command specs, process supervision, logs, port hints |
| `ports/` | Port ownership, conflicts, end-process confirmation |
| `agents`, `launcher`, `tools` | Tool detection on `PATH`, terminal and agent launch plans |
| `workspace/` | Service model, start order, health checks, restart policy, combined logs |
| `palette` | Command palette search |
| `store`, `saver`, `state` | In-memory state and atomic JSON persistence |

Further reading: [docs/PLAN.md](docs/PLAN.md) (architecture and plan),
[docs/DESIGN.md](docs/DESIGN.md) (design system and UI rules),
[docs/states-audit.md](docs/states-audit.md) (empty, loading and error states).

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org) subjects (`feat:`, `fix:`,
`docs:`, `chore:`, ...). The changelog is generated from them at release time, so write the
subject for a reader of `CHANGELOG.md`.

## Releasing

```bash
pnpm release patch
```

Bumps the version (`Cargo.toml` is the single source of truth), writes `CHANGELOG.md` from
conventional commit subjects, commits, tags and pushes. GitHub Actions then builds, signs and
publishes the installer and the in-app update.

| Command | Result |
|---|---|
| `pnpm release patch` / `minor` / `major` | Semver bump |
| `pnpm release beta` | Pre-release (skipped by the updater) |
| `pnpm release 0.5.0` | Exact version |
| `pnpm release:dry` | Preview the next patch release, change nothing |
| `pnpm build:installer` | Build an installer locally without publishing |
| `pnpm screenshots` | Recapture the README images from the UI with demo data, then frame them |

One-time setup (signing key, secrets) and details: [docs/RELEASING.md](docs/RELEASING.md).

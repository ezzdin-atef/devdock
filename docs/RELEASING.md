# Releasing DevDock

One command cuts a release. GitHub Actions builds the installer, signs the update,
and publishes it; installed copies of DevDock find it on their own.

```
pnpm release patch      0.3.0 -> 0.3.1   bug fixes
pnpm release minor      0.3.0 -> 0.4.0   new features
pnpm release major      0.3.0 -> 1.0.0   breaking changes
pnpm release beta       0.3.0 -> 0.3.1-beta.0 (published as a pre-release)
pnpm release 0.5.0      an exact version
pnpm release current    tag the version already in Cargo.toml (use for the very first release)
pnpm release:dry        preview the next patch release; changes nothing
```

Flags: `--dry-run` (show checks and notes, change nothing), `--no-push` (commit and tag
locally only), `--yes` (skip the confirmation prompt).

## What `pnpm release` does

1. **Checks** you're on `main`, the tree is clean, you're up to date with `origin/main`, the
   tag doesn't exist yet, and the updater key and endpoint are configured. Any failure
   stops it before anything is written, with the command that fixes it.
2. **Bumps the version** in `Cargo.toml` (`[workspace.package]`, the single source of
   truth; Tauri reads it from there), `package.json` and `Cargo.lock`.
3. **Writes release notes** to `CHANGELOG.md` from commit subjects since the last tag,
   grouped by prefix: `feat:` → Added, `fix:` → Fixed, `perf:`/`refactor:` → Changed,
   `docs:` → Docs, `chore:`/`ci:`/`build:`/`test:` → Maintenance, anything else → Other.
   To write the notes by hand instead, add a `## [x.y.z] - date` section to `CHANGELOG.md`
   before running; the script keeps it.
4. **Commits** `release: vX.Y.Z`, creates an annotated tag, and pushes both in one atomic
   push.

## What GitHub Actions does (`.github/workflows/release.yml`)

On every `v*` tag, on `windows-latest`:

- Fails early if the tag doesn't match the version in `Cargo.toml`, or the signing secret is missing.
- Builds the NSIS installer with `src-tauri/tauri.release.conf.json`, which turns on signed
  updater artifacts.
- Publishes the GitHub Release `DevDock vX.Y.Z` with the `CHANGELOG.md` section as its
  notes, the installer `DevDock_X.Y.Z_x64-setup.exe`, its `.sig`, and `latest.json`.
  Versions with a `-` (e.g. `0.4.0-beta.1`) are marked pre-release, so the updater skips them.

To rebuild an existing tag (e.g. after fixing a secret), run the workflow from the Actions tab
with **Run workflow** and the tag name.

`ci.yml` still runs checks and an unsigned build on every push to `main` and every PR.

## The installer

NSIS, per-machine: installs to `C:\Program Files\DevDock` (asks for admin once), adds a
Start menu entry in a `DevDock` folder, registers `devdock` under App Paths (so it starts
from Win+R), and installs WebView2 if it's missing. The uninstaller offers to delete
DevDock's data in `%APPDATA%\DevDock`. Branding: `src-tauri/installer/` (`sidebar.bmp`
164×314, `header.bmp` 150×57, both 24-bit BMP) and `hooks.nsh` for extra install steps.

Build one locally without publishing: `pnpm build:installer` → `target/release/bundle/nsis/`.

## Updates inside the app

DevDock checks `https://github.com/<repo>/releases/latest/download/latest.json` 8 seconds
after start and every 6 hours (installed builds only, never `pnpm tauri dev`). A new version
shows an **Update x.y.z** pill in the title bar; it opens the notes with **Install and
restart**, **Later** and **Skip this version**. Settings → DevDock has **Check for updates**.
The download is verified against the public key in `tauri.conf.json` before anything runs.

## One-time setup

1. Make sure the repo is **public** (the updater downloads release files without signing in).
2. Run `pnpm release:setup <owner>/<repo>`. It points the updater at your repo, creates the
   signing key at `~/.tauri/devdock.key` (asks for a password, which may be empty), writes
   the public key into `tauri.conf.json`, and prints the two commands below.
3. Add the private key and its password as Actions secrets:

   ```
   gh secret set TAURI_SIGNING_PRIVATE_KEY --repo <owner>/<repo> < ~/.tauri/devdock.key
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo <owner>/<repo>
   ```

4. Commit, then `pnpm release current` for the first release.

**Back up `~/.tauri/devdock.key`.** Without it you can't sign updates that installed copies
accept, and everyone would have to reinstall manually once.

## Not covered yet

- **Code signing (Authenticode).** The installer isn't signed with a certificate, so
  Windows SmartScreen shows "Windows protected your PC" on first download (More info → Run
  anyway). Updates through the app aren't affected. To sign, get a code-signing certificate
  (or Azure Trusted Signing) and add `bundle.windows.certificateThumbprint` or a `signCommand`.

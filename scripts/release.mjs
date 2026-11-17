#!/usr/bin/env node
// Cut a DevDock release: bump the version, write the changelog, commit, tag, push.
// Pushing the tag starts .github/workflows/release.yml, which builds the signed
// installer and publishes the GitHub Release the in-app updater reads.
//
//   pnpm release patch            0.3.0 -> 0.3.1
//   pnpm release minor            0.3.0 -> 0.4.0
//   pnpm release major            0.3.0 -> 1.0.0
//   pnpm release beta             0.3.0 -> 0.3.1-beta.0, then -beta.1, ...
//   pnpm release 0.5.0            exact version
//   pnpm release current          tag the version already in Cargo.toml (first release)
//
// Flags: --dry-run (show everything, change nothing)  --no-push (commit + tag locally)
//        --yes (skip the confirmation prompt)
//
// The version lives in one place: [workspace.package] in Cargo.toml. Tauri reads it
// from there; package.json and Cargo.lock are kept in step by this script.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { createInterface } from "node:readline/promises";

const root = new URL("..", import.meta.url);
const file = (p) => new URL(p, root);
const args = process.argv.slice(2);
const flag = (f) => args.includes(f);
const bump = args.find((a) => !a.startsWith("--"));
const DRY = flag("--dry-run");
const PUSH = !flag("--no-push");
const BRANCH = "main";

const c = {
  bold: (s) => `\x1b[1m${s}\x1b[0m`,
  dim: (s) => `\x1b[2m${s}\x1b[0m`,
  ok: (s) => `\x1b[32m${s}\x1b[0m`,
  warn: (s) => `\x1b[33m${s}\x1b[0m`,
  bad: (s) => `\x1b[31m${s}\x1b[0m`,
};
const die = (msg, fix) => {
  console.error(`${c.bad("✖")} ${msg}${fix ? `\n  ${c.dim("→")} ${fix}` : ""}`);
  process.exit(1);
};
const git = (...a) => execFileSync("git", a, { cwd: root, encoding: "utf8" }).trim();
const tryGit = (...a) => {
  try {
    return execFileSync("git", a, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return null;
  }
};

if (!bump || flag("--help") || flag("-h")) {
  console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(1, 18).map((l) => l.replace(/^\/\/ ?/, "")).join("\n"));
  process.exit(bump ? 0 : 1);
}

// ---------- current version ----------
const cargoPath = file("Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const wsMatch = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/.exec(cargo);
if (!wsMatch) die("Couldn't find `version` under [workspace.package] in Cargo.toml.");
const current = wsMatch[2];

const SEMVER = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;
function next(cur, kind) {
  if (kind === "current") return cur;
  const m = SEMVER.exec(cur);
  if (!m) die(`Current version "${cur}" isn't semver.`);
  let [maj, min, pat] = [+m[1], +m[2], +m[3]];
  const pre = m[4];
  switch (kind) {
    case "major":
      return `${maj + 1}.0.0`;
    case "minor":
      return `${maj}.${min + 1}.0`;
    case "patch":
      return pre ? `${maj}.${min}.${pat}` : `${maj}.${min}.${pat + 1}`;
    case "beta":
    case "prerelease": {
      const pm = pre && /^beta\.(\d+)$/.exec(pre);
      return pm ? `${maj}.${min}.${pat}-beta.${+pm[1] + 1}` : `${maj}.${min}.${pat + 1}-beta.0`;
    }
    default:
      if (SEMVER.test(kind)) return kind;
      die(`Unknown bump "${kind}".`, "Use patch, minor, major, beta, or an exact version like 1.2.3.");
  }
}
const version = next(current, bump);
const tag = `v${version}`;
const cmp = (a, b) => {
  const pa = SEMVER.exec(a), pb = SEMVER.exec(b);
  for (let i = 1; i <= 3; i++) if (+pa[i] !== +pb[i]) return +pa[i] - +pb[i];
  if (!pa[4] !== !pb[4]) return pa[4] ? -1 : 1;
  return (pa[4] ?? "").localeCompare(pb[4] ?? "", undefined, { numeric: true });
};
const sameVersion = version === current;
if (cmp(version, current) < 0) die(`${version} is older than the current ${current}.`);

// ---------- preflight ----------
console.log(`\n${c.bold("DevDock release")}  ${current} → ${c.bold(version)}${DRY ? c.warn("  (dry run)") : ""}\n`);
const problems = [];
const check = (ok, msg, fix) => {
  console.log(`  ${ok ? c.ok("✔") : c.bad("✖")} ${msg}`);
  if (!ok) problems.push(fix ? `${msg}\n    → ${fix}` : msg);
};

const branch = tryGit("rev-parse", "--abbrev-ref", "HEAD");
check(branch !== null, "inside a git repository with at least one commit", "Make a first commit: git add -A && git commit -m \"Initial commit\"");
check(branch === BRANCH, `on ${BRANCH} (now: ${branch ?? "?"})`, `git switch ${BRANCH}`);
const dirty = tryGit("status", "--porcelain") ?? "";
check(dirty === "", "working tree is clean", "Commit or stash your changes first.");
check(!tryGit("rev-parse", "-q", "--verify", `refs/tags/${tag}`), `tag ${tag} doesn't exist yet`);

const conf = JSON.parse(readFileSync(file("src-tauri/tauri.conf.json"), "utf8"));
const upd = conf.plugins?.updater ?? {};
check(Boolean(upd.pubkey), "updater public key is set in tauri.conf.json", "Run: pnpm release:setup <owner>/<repo>");
check(!(upd.endpoints ?? []).some((e) => e.includes("/OWNER/")), "updater endpoint points at your GitHub repo", "Run: pnpm release:setup <owner>/<repo>");

if (PUSH) {
  const remote = tryGit("remote", "get-url", "origin");
  check(Boolean(remote), `remote "origin" is set${remote ? ` (${remote})` : ""}`, "git remote add origin https://github.com/<owner>/<repo>.git");
  if (remote && !DRY) {
    const fetched = tryGit("fetch", "--quiet", "origin", BRANCH) !== null;
    if (fetched) {
      const behind = +(tryGit("rev-list", "--count", `HEAD..origin/${BRANCH}`) ?? "0");
      check(behind === 0, `up to date with origin/${BRANCH}`, `git pull --rebase origin ${BRANCH}`);
      check(!tryGit("ls-remote", "--tags", "origin", tag), `tag ${tag} isn't on origin yet`);
    }
  }
}

if (problems.length && !DRY) {
  console.error(`\n${c.bad("Not releasing:")}\n  ${problems.join("\n  ")}\n`);
  process.exit(1);
}

// ---------- changelog ----------
const lastTag = tryGit("describe", "--tags", "--abbrev=0", "--match", "v*");
const range = lastTag ? `${lastTag}..HEAD` : "HEAD";
const log = (tryGit("log", range, "--no-merges", "--pretty=format:%s\x1f%h") ?? "")
  .split("\n")
  .filter(Boolean)
  .map((l) => l.split("\x1f"))
  .filter(([s]) => !/^(release|chore\(release\))\b/i.test(s));

const GROUPS = [
  ["Added", /^feat(\(.+?\))?!?:\s*/i],
  ["Fixed", /^fix(\(.+?\))?!?:\s*/i],
  ["Changed", /^(perf|refactor|style|ui)(\(.+?\))?!?:\s*/i],
  ["Docs", /^docs(\(.+?\))?!?:\s*/i],
  ["Maintenance", /^(chore|ci|build|test|deps)(\(.+?\))?!?:\s*/i],
];
const grouped = new Map();
for (const [subject, hash] of log) {
  const g = GROUPS.find(([, re]) => re.test(subject));
  const name = g ? g[0] : "Other";
  const text = g ? subject.replace(g[1], "") : subject;
  const line = `- ${text.charAt(0).toUpperCase()}${text.slice(1)} (${hash})`;
  grouped.set(name, [...(grouped.get(name) ?? []), line]);
}
const order = [...GROUPS.map(([n]) => n), "Other"];
const today = new Date().toISOString().slice(0, 10);
const body =
  [...grouped.keys()].length === 0
    ? "- Maintenance release."
    : order.filter((n) => grouped.has(n)).map((n) => `### ${n}\n\n${grouped.get(n).join("\n")}`).join("\n\n");
const section = `## [${version}] - ${today}\n\n${body}\n`;

// A section you already wrote by hand for this version wins over generated notes.
const clPath = file("CHANGELOG.md");
const existingLog = existsSync(clPath) ? readFileSync(clPath, "utf8") : "";
const handWritten = existingLog.includes(`## [${version}]`);

if (handWritten) {
  const notes = execFileSync("node", ["scripts/changelog-section.mjs", version], { cwd: root, encoding: "utf8" });
  console.log(`\n${c.bold("Release notes")} ${c.dim("(your existing CHANGELOG.md section is kept)")}\n`);
  console.log(notes.trimEnd().replace(/^/gm, "  ") + "\n");
} else {
  console.log(`\n${c.bold("Release notes")} ${c.dim(`(${lastTag ? `since ${lastTag}` : "all commits"}, written to CHANGELOG.md)`)}\n`);
  console.log(section.replace(/^/gm, "  "));
}

if (DRY) {
  if (problems.length) console.log(c.warn(`${problems.length} preflight problem(s) above would block a real release.\n`));
  console.log(c.dim("Dry run: nothing was changed."));
  process.exit(0);
}

if (!flag("--yes")) {
  const rl = createInterface({ input: process.stdin, output: process.stdout });
  const answer = await rl.question(
    `Commit, tag ${c.bold(tag)}${PUSH ? " and push (this starts the release build)" : ""}? [y/N] `,
  );
  rl.close();
  if (!/^y(es)?$/i.test(answer.trim())) die("Cancelled. Nothing was changed.");
}

// ---------- write ----------
if (!sameVersion) {
  writeFileSync(cargoPath, cargo.replace(wsMatch[0], `${wsMatch[1]}${version}${wsMatch[3]}`));

  const pkgPath = file("package.json");
  const pkg = readFileSync(pkgPath, "utf8");
  writeFileSync(pkgPath, pkg.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`));

  // Refresh the workspace members' entries in Cargo.lock (no dependency changes).
  try {
    execFileSync("cargo", ["update", "--workspace", "--offline"], { cwd: root, stdio: "ignore" });
  } catch {
    execFileSync("cargo", ["update", "--workspace"], { cwd: root, stdio: "inherit" });
  }
}

if (!handWritten) {
  const HEAD = "# Changelog\n\nAll notable changes to DevDock. Versions follow [semver](https://semver.org).\n\n";
  const old = existingLog.replace(/^# Changelog[\s\S]*?(?=^## |(?![\s\S]))/m, "");
  writeFileSync(clPath, `${HEAD}${section}\n${old}`.trimEnd() + "\n");
}

git("add", "Cargo.toml", "Cargo.lock", "package.json", "CHANGELOG.md");
// Nothing staged happens for `pnpm release current` with a hand-written changelog.
if (tryGit("diff", "--cached", "--quiet") === null) git("commit", "-m", `release: ${tag}`);
git("tag", "-a", tag, "-m", `DevDock ${tag}`);
console.log(`\n${c.ok("✔")} Committed and tagged ${c.bold(tag)}`);

if (PUSH) {
  execFileSync("git", ["push", "--atomic", "origin", BRANCH, tag], { cwd: root, stdio: "inherit" });
  const remote = git("remote", "get-url", "origin").replace(/\.git$/, "").replace(/^git@github\.com:/, "https://github.com/");
  console.log(`\n${c.ok("✔")} Pushed. The release build is running:`);
  console.log(`  ${remote}/actions`);
  console.log(`  ${remote}/releases/tag/${tag}  ${c.dim("(appears when the build finishes)")}\n`);
} else {
  console.log(c.dim(`\nNot pushed. When ready: git push --atomic origin ${BRANCH} ${tag}\n`));
}

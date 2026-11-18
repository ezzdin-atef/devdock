// Turns the raw app captures in docs/screenshots/raw/ into the framed README images in
// docs/screenshots/. Renders an HTML template with headless Edge (or Chrome).
//
//   node scripts/frame-screenshots.mjs            # all shots
//   node scripts/frame-screenshots.mjs ports run  # just these
//
// Raw captures are expected at 1280x900 (the app window at 100% scale).

import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const rawDir = join(root, 'docs', 'screenshots', 'raw');
const outDir = join(root, 'docs', 'screenshots');

const SHOTS = {
  overview: {
    eyebrow: 'Projects',
    title: 'Every local project in one place, organized into your own groups',
  },
  cards: {
    eyebrow: 'Your view',
    title: 'Folders, cards or rows - size, details, sort and grouping are yours',
  },
  project: {
    eyebrow: 'Overview',
    title: 'Git, dev server, packages and a portless address for each project',
  },
  git: {
    eyebrow: 'Git',
    title: 'Switch branches, fetch, pull and push - pull only fast-forwards',
  },
  files: {
    eyebrow: 'Files',
    title: "Browse, search and edit a project's files without leaving DevDock",
  },
  security: {
    eyebrow: 'Security',
    title: 'Scan for vulnerabilities, secrets and risky settings - with the fix',
  },
  run: {
    eyebrow: 'Run',
    title: 'Dev servers on saved ports - stop ends the whole process tree',
  },
  logs: {
    eyebrow: 'Logs',
    title: 'Full-height logs with search, filters and one-click clear',
  },
  ports: {
    eyebrow: 'Ports',
    title: 'See who holds every port before something else grabs it',
  },
  palette: {
    eyebrow: 'Ctrl K',
    title: 'Jump to any project, command, agent or port',
  },
  settings: {
    eyebrow: 'Settings',
    title: 'Hidden ports, local HTTPS and your tools - all in one place',
  },
};

const WIDTH = 1600;
const HEIGHT = 1172;

const font = (pkg, file) =>
  pathToFileURL(join(root, 'node_modules', '@fontsource-variable', pkg, 'files', file)).href;

function findBrowser() {
  const candidates = [
    process.env.BROWSER_PATH,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
  ].filter(Boolean);
  const found = candidates.find((p) => existsSync(p));
  if (!found) throw new Error('No Edge/Chrome found; set BROWSER_PATH.');
  return found;
}

function html({ eyebrow, title }, imageUrl, logoSvg) {
  return `<!doctype html>
<html><head><meta charset="utf-8"><style>
@font-face { font-family: 'Schibsted'; src: url('${font('schibsted-grotesk', 'schibsted-grotesk-latin-wght-normal.woff2')}') format('woff2'); font-weight: 100 900; }
@font-face { font-family: 'JBMono'; src: url('${font('jetbrains-mono', 'jetbrains-mono-latin-wght-normal.woff2')}') format('woff2'); font-weight: 100 900; }
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { width: ${WIDTH}px; height: ${HEIGHT}px; overflow: hidden; }
body {
  position: relative;
  background:
    radial-gradient(900px 520px at 50% -8%, rgba(232, 197, 71, 0.16), transparent 70%),
    radial-gradient(700px 500px at 100% 100%, rgba(123, 211, 168, 0.06), transparent 70%),
    linear-gradient(180deg, #17181b 0%, #0f1012 100%);
  font-family: 'Schibsted', 'Segoe UI', sans-serif;
  color: #ecebe6;
}
.grid {
  position: absolute; inset: 0;
  background-image: radial-gradient(rgba(255, 255, 255, 0.07) 1px, transparent 1px);
  background-size: 24px 24px;
  -webkit-mask-image: radial-gradient(1000px 600px at 50% 0%, #000 0%, transparent 75%);
}
header {
  position: relative;
  height: 196px;
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px;
}
.eyebrow {
  display: inline-flex; align-items: center; gap: 10px;
  padding: 6px 14px 6px 8px;
  border: 1px solid #3a3a33; border-radius: 999px;
  background: rgba(38, 36, 26, 0.7);
  font-family: 'JBMono', monospace; font-size: 13px; font-weight: 600;
  letter-spacing: 0.14em; text-transform: uppercase; color: #e8c547;
}
.eyebrow svg { width: 22px; height: 22px; display: block; }
h1 { font-size: 34px; font-weight: 650; letter-spacing: -0.015em; text-align: center; max-width: 1200px; }
.window {
  position: relative;
  margin: 0 auto;
  width: 1282px; height: 902px;
  border-radius: 14px;
  border: 1px solid #33353b;
  overflow: hidden;
  box-shadow:
    0 0 0 1px rgba(0, 0, 0, 0.6),
    0 2px 0 0 rgba(255, 255, 255, 0.03) inset,
    0 30px 60px -10px rgba(0, 0, 0, 0.7),
    0 60px 140px -20px rgba(0, 0, 0, 0.55),
    0 -20px 120px -40px rgba(232, 197, 71, 0.25);
}
.window img { display: block; width: 1280px; height: 900px; }
.window::after {
  content: ''; position: absolute; inset: 0; border-radius: 13px; pointer-events: none;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
}
</style></head>
<body>
  <div class="grid"></div>
  <header>
    <div class="eyebrow">${logoSvg}${eyebrow}</div>
    <h1>${title}</h1>
  </header>
  <div class="window"><img src="${imageUrl}"></div>
</body></html>`;
}

const browser = findBrowser();
const logoSvg = readFileSync(join(root, 'docs', 'brand', 'devdock-icon-small.svg'), 'utf8')
  .replace(/<\?xml[^>]*\?>/, '')
  .trim();
const names = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(SHOTS);
const work = mkdtempSync(join(tmpdir(), 'devdock-frame-'));

try {
  for (const name of names) {
    const shot = SHOTS[name];
    if (!shot) throw new Error(`Unknown shot "${name}". Known: ${Object.keys(SHOTS).join(', ')}`);
    const raw = join(rawDir, `${name}.png`);
    if (!existsSync(raw)) throw new Error(`Missing raw capture: ${raw}`);

    const page = join(work, `${name}.html`);
    writeFileSync(page, html(shot, pathToFileURL(raw).href, logoSvg));
    const out = join(outDir, `${name}.png`);
    const before = existsSync(out) ? statSync(out).mtimeMs : 0;
    execFileSync(browser, [
      '--headless=new',
      '--disable-gpu',
      '--hide-scrollbars',
      '--allow-file-access-from-files',
      '--force-device-scale-factor=1',
      '--default-background-color=00000000',
      `--user-data-dir=${join(work, 'profile')}`,
      `--window-size=${WIDTH},${HEIGHT}`,
      '--virtual-time-budget=2000',
      `--screenshot=${out}`,
      pathToFileURL(page).href,
    ], { stdio: 'ignore' });
    // On some machines msedge.exe hands off to a background process and returns at
    // once; wait for the image so the page isn't deleted before it's rendered.
    const deadline = Date.now() + 20_000;
    while (!(existsSync(out) && statSync(out).mtimeMs > before)) {
      if (Date.now() > deadline) throw new Error(`Timed out waiting for ${out}`);
      await new Promise((r) => setTimeout(r, 200));
    }
    await new Promise((r) => setTimeout(r, 300)); // let the write finish
    console.log(`framed ${name} -> ${out}`);
  }
} finally {
  // The background browser can hold its profile for a moment after the last shot.
  rmSync(work, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
}

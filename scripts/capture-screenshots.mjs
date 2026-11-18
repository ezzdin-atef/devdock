// Captures the raw README and website screenshots (docs/screenshots/raw/*.png, 1280x900)
// from the real Svelte UI running against the demo backend in scripts/demo/mock-ipc.js. No
// Tauri, no personal data. `pnpm screenshots` runs this and then frames the images.
//
// Scenes: overview (Projects by group), cards, palette, project (overview tab), run, logs,
// git, files (editor + search), ports, settings (local addresses).
//
//   node scripts/capture-screenshots.mjs

import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const outDir = join(root, 'docs', 'screenshots', 'raw');
const mock = readFileSync(join(root, 'scripts', 'demo', 'mock-ipc.js'), 'utf8');
const VITE_PORT = 1437;
const W = 1280;
const H = 900;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findBrowser() {
  const found = [
    process.env.BROWSER_PATH,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
  ]
    .filter(Boolean)
    .find((p) => existsSync(p));
  if (!found) throw new Error('No Edge/Chrome found; set BROWSER_PATH.');
  return found;
}

async function waitFor(fn, what, timeout = 60_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    try {
      const v = await fn();
      if (v) return v;
    } catch {
      /* not yet */
    }
    await sleep(250);
  }
  throw new Error(`Timed out waiting for ${what}`);
}

class Cdp {
  constructor(url) {
    this.ws = new WebSocket(url);
    this.id = 0;
    this.pending = new Map();
    this.ws.onmessage = (m) => {
      const msg = JSON.parse(m.data);
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      msg.error ? p.reject(new Error(msg.error.message)) : p.resolve(msg.result);
    };
  }
  open() {
    return new Promise((res, rej) => {
      this.ws.onopen = res;
      this.ws.onerror = rej;
    });
  }
  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }
  async eval(expr) {
    const r = await this.send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? 'eval failed');
    return r.result.value;
  }
  /** Mouse-clicks the innermost visible element whose own text is exactly `text`. */
  async click(text, scope = 'body') {
    const at = await this.eval(`(() => {
      // Ignore a trailing count badge ("Run 1").
      const label = (e) => e.textContent.trim().replace(/[\\s\\d]+$/, "");
      const els = [...document.querySelector(${JSON.stringify(scope)}).querySelectorAll('*')]
        .filter((e) => e.offsetParent !== null && label(e) === ${JSON.stringify(text)});
      const el = els.find((e) => ![...e.children].some((c) => label(c) === ${JSON.stringify(text)})) ?? els.at(-1);
      if (!el) return null;
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    })()`);
    if (!at) throw new Error(`Nothing to click with text "${text}"`);
    for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) {
      await this.send('Input.dispatchMouseEvent', { type, x: at.x, y: at.y, button: 'left', clickCount: 1 });
    }
    // Park the pointer away from the UI so no hover state is captured.
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1279, y: 899 });
    await sleep(700);
  }
  async key(key, code, keyCode, modifiers = 0) {
    for (const type of ['rawKeyDown', 'keyUp']) {
      await this.send('Input.dispatchKeyEvent', { type, key, code, windowsVirtualKeyCode: keyCode, modifiers });
    }
  }
  async shot(name) {
    await sleep(600);
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    writeFileSync(join(outDir, `${name}.png`), Buffer.from(data, 'base64'));
    console.log(`captured ${name}`);
  }
}

const work = mkdtempSync(join(tmpdir(), 'devdock-capture-'));
const vite = spawn(process.execPath, [join(root, 'node_modules', 'vite', 'bin', 'vite.js'), 'dev', '--port', String(VITE_PORT), '--strictPort'], {
  cwd: root,
  stdio: 'ignore',
});
const debugPort = 9400 + Math.floor(Math.random() * 400);
const browser = spawn(findBrowser(), [
  '--headless=new',
  '--disable-gpu',
  '--hide-scrollbars',
  '--no-first-run',
  `--remote-debugging-port=${debugPort}`,
  `--user-data-dir=${join(work, 'profile')}`,
  `--window-size=${W},${H}`,
  'about:blank',
], { stdio: 'ignore' });

try {
  const appUrl = `http://127.0.0.1:${VITE_PORT}/`;
  await waitFor(async () => (await fetch(appUrl)).ok, 'vite');
  const target = await waitFor(async () => {
    const list = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();
    return list.find((t) => t.type === 'page');
  }, 'browser');

  const cdp = new Cdp(target.webSocketDebuggerUrl);
  await cdp.open();
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: W, height: H, deviceScaleFactor: 1, mobile: false });
  // Reduced motion: no capture lands mid-animation.
  await cdp.send('Emulation.setEmulatedMedia', {
    features: [
      { name: 'prefers-color-scheme', value: 'dark' },
      { name: 'prefers-reduced-motion', value: 'reduce' },
    ],
  });
  // Start on the Projects page split into the demo's groups.
  const view = { layout: 'folders', grouping: 'groups', size: 'm', sort: 'recent' };
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `try {
      localStorage.setItem("devdock.view", ${JSON.stringify(JSON.stringify(view))});
      localStorage.removeItem("devdock.collapsed");
    } catch {}
    ${mock}`,
  });
  await cdp.send('Page.navigate', { url: appUrl });
  await waitFor(() => cdp.eval(`document.body.innerText.includes('storefront')`), 'app to render');
  await sleep(1500);

  await cdp.shot('overview');

  // Every project as a card, no sections.
  await cdp.click('Cards', 'main');
  await cdp.eval(`import('/src/lib/viewPrefs.svelte.ts').then(({ view }) => view.set({ grouping: 'none' }))`);
  await sleep(500);
  await cdp.shot('cards');

  await cdp.key('k', 'KeyK', 75, 2 /* ctrl */);
  await sleep(500);
  await cdp.send('Input.insertText', { text: 'api' });
  await cdp.shot('palette');
  await cdp.key('Escape', 'Escape', 27);
  await sleep(300);

  await cdp.click('storefront', 'main');
  await cdp.shot('project');

  await cdp.click('Run', 'main');
  await sleep(800);
  await cdp.shot('run');

  await cdp.click('Logs', 'main');
  await sleep(800);
  await cdp.shot('logs');

  // billing-api is behind its remote: open its Git tab and pull.
  await cdp.eval(`import('/src/lib/app.svelte.ts').then(async ({ app }) => { app.tab = 'git'; await app.select('p2'); })`);
  await sleep(1200);
  await cdp.eval(`[...document.querySelectorAll('.sync button')].find((b) => b.textContent.includes('Pull'))?.click()`);
  await sleep(800);
  await cdp.shot('git');

  // storefront's Files tab: search across files, with a hit open in the editor.
  await cdp.eval(`import('/src/lib/app.svelte.ts').then(async ({ app }) => { app.tab = 'files'; await app.select('p1'); })`);
  await sleep(1000);
  await cdp.eval(`[...document.querySelectorAll('.switch button')].find((b) => b.textContent.includes('Search'))?.click()`);
  await sleep(400);
  await cdp.send('Input.insertText', { text: 'formatPrice' });
  await sleep(1000);
  await cdp.eval(`[...document.querySelectorAll('.hit')].find((h) => h.textContent.includes('price ??'))?.click()`);
  await sleep(1200);
  await cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: 1279, y: 899 });
  await cdp.shot('files');

  await cdp.click('Ports', 'nav.rail');
  await cdp.shot('ports');

  await cdp.click('Settings', 'nav.rail');
  await cdp.eval(`(() => {
    const title = [...document.querySelectorAll('.title')].find((t) => t.textContent.trim() === 'Local addresses');
    title?.closest('section')?.scrollIntoView({ block: 'center' });
    return true;
  })()`);
  await sleep(500);
  await cdp.shot('settings');

  cdp.ws.close();
} finally {
  browser.kill();
  vite.kill();
  await sleep(500);
  rmSync(work, { recursive: true, force: true, maxRetries: 15, retryDelay: 400 });
}

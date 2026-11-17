// Assembles the landing page into _site/ for GitHub Pages:
// site/* plus the app screenshots and brand assets that already live in docs/.
// Uses the raw 1280x900 captures (docs/screenshots/raw/) when present, since the page
// draws its own window frames; falls back to docs/screenshots/.
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "_site");
const shotsRaw = join(root, "docs", "screenshots", "raw");
const shots = existsSync(shotsRaw) ? shotsRaw : join(root, "docs", "screenshots");

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "screenshots"), { recursive: true });
cpSync(join(root, "site"), out, { recursive: true });
for (const f of readdirSync(shots).filter((f) => f.endsWith(".png"))) {
  cpSync(join(shots, f), join(out, "screenshots", f));
}
cpSync(join(root, "docs", "brand"), join(out, "brand"), { recursive: true });

// The version the page shows before (or without) asking GitHub for the latest one.
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const page = join(out, "index.html");
writeFileSync(page, readFileSync(page, "utf8").replaceAll("__VERSION__", version));

console.log(`Site built in ${out} for v${version} (screenshots from ${shots})`);

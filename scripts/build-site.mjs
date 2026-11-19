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
// A PNG icon for search results and apple-touch-icon, which don't all take SVG.
cpSync(join(root, "src-tauri", "icons", "128x128.png"), join(out, "brand", "icon-128.png"));

const SITE_URL = "https://ezzdin-atef.github.io/devdock/";

// The version the page shows before (or without) asking GitHub for the latest one.
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const page = join(out, "index.html");
let html = readFileSync(page, "utf8").replaceAll("__VERSION__", version);

// FAQPage structured data, built from the page's own <details> so it can't drift.
const text = (s) =>
  s.replace(/<[^>]+>/g, "").replace(/&amp;/g, "&").replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();
const faqBlock = html.slice(html.indexOf('<section id="faq">'), html.indexOf("</section>", html.indexOf('<section id="faq">')));
const faq = [...faqBlock.matchAll(/<details><summary>([\s\S]*?)<\/summary><p>([\s\S]*?)<\/p><\/details>/g)].map(
  ([, q, a]) => ({ "@type": "Question", name: text(q), acceptedAnswer: { "@type": "Answer", text: text(a) } }),
);
const faqLd = JSON.stringify({ "@context": "https://schema.org", "@type": "FAQPage", mainEntity: faq }, null, 2);
html = html.replace("<!--FAQ_JSONLD-->", `<script type="application/ld+json">\n${faqLd.replace(/</g, "\\u003c")}\n  </script>`);
writeFileSync(page, html);

// Sitemap for Google Search Console. (A robots.txt would have to live at the
// github.io root, which this project site doesn't own, so there is none.)
const today = new Date().toISOString().slice(0, 10);
writeFileSync(
  join(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>${SITE_URL}</loc><lastmod>${today}</lastmod></url>
</urlset>
`,
);

console.log(`Site built in ${out} for v${version} (screenshots from ${shots})`);

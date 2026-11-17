// Checks text/background token pairs from src/app.css against WCAG AA (T8.7).
// `text-faint` is excluded on purpose: DESIGN.md allows it only for decorative
// log timestamps. Run: pnpm contrast
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");
const read = (block) =>
  Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})\b/g)].map((m) => [m[1], m[2]]));
// Dark is the base :root block; light overrides it under :root[data-theme="light"].
const lightAt = css.indexOf(':root[data-theme="light"]');
const dark = read(css.slice(0, lightAt));
const light = { ...dark, ...read(css.slice(lightAt, css.indexOf("}", lightAt))) };

const lum = (hex) => {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const ratio = (a, b) => {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

// [foreground, background] pairs as used in components.
const pairs = [
  ["text", "bg-app"], ["text", "bg-panel"], ["text", "bg-overlay"], ["text", "bg-chrome"],
  ["text-soft", "bg-app"], ["text-soft", "bg-panel"], ["text-soft", "error-panel"], ["text-soft", "warning-panel"],
  ["text-muted", "bg-app"], ["text-muted", "bg-panel"], ["text-muted", "bg-chrome"], ["text-muted", "bg-sidebar"],
  ["text-muted", "bg-overlay"], ["text-muted", "bg-console"],
  ["accent-fg", "bg-app"], ["accent-fg", "bg-chrome"], ["accent-fg", "accent-tint"], ["accent-fg", "accent-panel"],
  ["accent-ink", "accent"], ["error-ink", "error"],
  ["success", "bg-panel"], ["success", "success-pill"], ["success", "success-panel"],
  ["error-text", "error-panel"], ["error-text", "error-pill"], ["error-text", "bg-console"],
  ["info", "bg-panel"],
  ["icon", "bg-app"], ["icon", "bg-chrome"], ["icon", "bg-panel"], ["icon", "bg-hover"],
];

let failed = 0;
for (const [theme, tokens] of [["dark", dark], ["light", light]]) {
console.log(`
${theme}`);
for (const [fg, bg] of pairs) {
  if (!tokens[fg] || !tokens[bg]) {
    console.log(`?  ${fg} on ${bg}: token missing`);
    failed++;
    continue;
  }
  const r = ratio(tokens[fg], tokens[bg]);
  const ok = r >= 4.5;
  if (!ok) failed++;
  console.log(`${ok ? "ok" : "FAIL"} ${r.toFixed(2).padStart(5)}  ${fg} on ${bg}`);
}
}
if (failed) {
  console.error(`\n${failed} pair(s) below 4.5:1`);
  process.exit(1);
}
console.log(`\nall ${pairs.length} pairs meet 4.5:1`);

// Which icon (and tint) a file gets in the Files tab, from its name. Tints reuse
// the app's semantic tokens, which have checked contrast in both themes.

import type { IconName } from "$lib/components/Icon.svelte";

export interface FileIcon {
  icon: IconName;
  /** CSS colour for the icon. */
  tint: string;
}

const CODE = new Set([
  "ts", "tsx", "js", "jsx", "mjs", "cjs", "svelte", "vue", "rs", "go", "py", "rb", "php", "java",
  "kt", "cs", "c", "h", "cpp", "hpp", "swift", "dart", "lua", "sh", "bash", "ps1", "bat", "cmd",
  "sql", "html", "css", "scss", "sass", "less", "astro",
]);
const CONFIG = new Set(["json", "jsonc", "json5", "toml", "yaml", "yml", "xml", "ini", "conf", "config", "env"]);
const TEXT = new Set(["md", "mdx", "txt", "rst", "log", "csv", "adoc"]);
const IMAGE = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp", "avif"]);
const LOCKS = new Set(["pnpm-lock.yaml", "package-lock.json", "yarn.lock", "Cargo.lock", "bun.lockb", "composer.lock", "poetry.lock", "Gemfile.lock"]);

export function fileIcon(name: string): FileIcon {
  if (LOCKS.has(name)) return { icon: "lock", tint: "var(--text-faint)" };
  const lower = name.toLowerCase();
  if (lower.startsWith(".env") || lower.startsWith(".git") || lower.startsWith(".editorconfig") || lower.startsWith(".prettierrc"))
    return { icon: "braces", tint: "var(--svc-orange)" };
  if (lower === "dockerfile" || lower === "makefile") return { icon: "fileCode", tint: "var(--info)" };
  const ext = lower.includes(".") ? lower.slice(lower.lastIndexOf(".") + 1) : "";
  if (CODE.has(ext)) return { icon: "fileCode", tint: "var(--info)" };
  if (CONFIG.has(ext)) return { icon: "braces", tint: "var(--svc-orange)" };
  if (TEXT.has(ext)) return { icon: "fileText", tint: "var(--text-muted)" };
  if (IMAGE.has(ext)) return { icon: "image", tint: "var(--svc-violet)" };
  return { icon: "file", tint: "var(--text-muted)" };
}

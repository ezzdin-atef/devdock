import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import { readFileSync } from "node:fs";
const host = process.env.TAURI_DEV_HOST;

// The Files tab's editor loads a language the first time a file needs it. Without
// pre-bundling them up front, Vite discovers each one mid-session, re-bundles, and
// reloads the page (dropping unsaved edits) or fails with "new version of the
// pre-bundle". The list is read from @codemirror/language-data so it stays in step.
const languageData = readFileSync(
  new URL("./node_modules/@codemirror/language-data/dist/index.js", import.meta.url),
  "utf8",
);
const lazyLanguages = [...new Set([...languageData.matchAll(/import\('([^']+)'\)/g)].map((m) => m[1]))].map(
  (id) => `@codemirror/language-data > ${id}`,
);

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

  optimizeDeps: {
    include: [
      "codemirror",
      "@codemirror/state",
      "@codemirror/view",
      "@codemirror/language",
      "@codemirror/language-data",
      "@codemirror/autocomplete",
      "@codemirror/commands",
      "@codemirror/search",
      "@lezer/highlight",
      ...lazyLanguages,
    ],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));

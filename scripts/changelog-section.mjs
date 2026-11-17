#!/usr/bin/env node
// Prints the CHANGELOG.md section for one version (used as the GitHub Release body
// and the in-app "what's new" notes).   node scripts/changelog-section.mjs 0.4.0
import { readFileSync, existsSync } from "node:fs";

const version = (process.argv[2] ?? "").replace(/^v/, "");
const path = new URL("../CHANGELOG.md", import.meta.url);
const text = existsSync(path) ? readFileSync(path, "utf8") : "";
const esc = version.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const m = new RegExp(`^## \\[${esc}\\][^\\n]*\\n([\\s\\S]*?)(?=^## \\[|(?![\\s\\S]))`, "m").exec(text);
process.stdout.write((m ? m[1].trim() : `DevDock ${version}`) + "\n");

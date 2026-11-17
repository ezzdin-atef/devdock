#!/usr/bin/env node
// One-time release setup.   pnpm release:setup <owner>/<repo>
//
// 1. Points the updater endpoint and homepage in tauri.conf.json at your GitHub repo.
// 2. Creates the updater signing key (if you don't have one) at ~/.tauri/devdock.key
//    and writes its PUBLIC half into tauri.conf.json. The private key never leaves
//    your machine unless you add it to GitHub yourself (step 3).
// 3. Prints the commands that store the private key as GitHub Actions secrets.
//
// Safe to run again: it only fills in what is missing (pass --new-key to replace the key,
// which makes already-installed copies unable to verify future updates).

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const root = new URL("..", import.meta.url);
const repo = process.argv.slice(2).find((a) => !a.startsWith("--"));
const NEW_KEY = process.argv.includes("--new-key");

if (!repo || !/^[\w.-]+\/[\w.-]+$/.test(repo)) {
  console.error("Usage: pnpm release:setup <owner>/<repo>   e.g. pnpm release:setup octocat/devdock");
  process.exit(1);
}

const confPath = new URL("src-tauri/tauri.conf.json", root);
const conf = JSON.parse(readFileSync(confPath, "utf8"));

// 1. Repo URLs.
conf.bundle.homepage = `https://github.com/${repo}`;
conf.plugins.updater.endpoints = [`https://github.com/${repo}/releases/latest/download/latest.json`];
console.log(`✔ Updater endpoint → https://github.com/${repo}/releases/latest/download/latest.json`);

// 2. Signing key.
const dir = join(homedir(), ".tauri");
const key = join(dir, "devdock.key");
if (NEW_KEY || !existsSync(`${key}.pub`)) {
  mkdirSync(dir, { recursive: true });
  console.log(`\nCreating the updater signing key at ${key}`);
  console.log("Choose a password (or leave it empty). You'll store it as a GitHub secret next.\n");
  const r = spawnSync("pnpm", ["tauri", "signer", "generate", "-w", key, ...(NEW_KEY ? ["--force"] : [])], {
    cwd: root,
    stdio: "inherit",
    shell: true,
  });
  if (r.status !== 0 || !existsSync(`${key}.pub`)) {
    console.error("✖ Key generation didn't finish. Nothing about the key was changed.");
    process.exit(1);
  }
} else {
  console.log(`✔ Using the existing signing key ${key}`);
}
conf.plugins.updater.pubkey = readFileSync(`${key}.pub`, "utf8").trim();
writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");
console.log("✔ Public key written to src-tauri/tauri.conf.json");

// Remote hint.
let remote = null;
try {
  remote = execFileSync("git", ["remote", "get-url", "origin"], { cwd: root, encoding: "utf8" }).trim();
} catch {}

console.log(`
Next, once per repository:

  1. Store the private key and its password as GitHub Actions secrets
     (needs the GitHub CLI, signed in: gh auth login):

       gh secret set TAURI_SIGNING_PRIVATE_KEY --repo ${repo} < "${key}"
       gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo ${repo}

     (press Enter at the second prompt if you left the password empty)
${remote ? "" : `
  2. Connect this folder to the repository:

       git remote add origin https://github.com/${repo}.git
`}
  ${remote ? "2" : "3"}. Commit this change, then release with:  pnpm release patch

Back up ${key}. If it's lost, installed copies can't verify new updates
and everyone has to reinstall from the website once.
`);

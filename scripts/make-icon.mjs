#!/usr/bin/env node
// Rebuild platform icons and transparent UI logos from the approved artwork.
// Usage: node scripts/make-icon.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const icons = join(root, "src-tauri", "icons");

execFileSync(process.execPath, [
  join(root, "node_modules", "@tauri-apps", "cli", "tauri.js"),
  "icon", join(root, "icon-source.png"), "--output", icons,
], { cwd: root, stdio: "inherit" });

mkdirSync(join(root, "src", "assets"), { recursive: true });
mkdirSync(join(root, "public"), { recursive: true });
copyFileSync(join(icons, "32x32.png"), join(root, "public", "favicon.png"));

execFileSync(process.execPath, [
  join(root, "node_modules", "@tauri-apps", "cli", "tauri.js"),
  "icon", join(root, "logo-monochrome-source.png"),
  "--output", join(root, "src", "assets", "brand"), "--png", "64", "--png", "128",
], { cwd: root, stdio: "inherit" });

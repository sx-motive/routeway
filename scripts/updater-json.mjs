#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const tag = process.argv[2];
if (!tag) {
  console.error("Usage: node scripts/updater-json.mjs <tag>");
  process.exit(1);
}

const gh = (...args) => execFileSync("gh", args, { encoding: "utf8" });
const release = JSON.parse(gh("release", "view", tag, "--json", "body,assets,publishedAt"));
const assets = release.assets.map((a) => a.name);
const dir = mkdtempSync(join(tmpdir(), "updater-json-"));

function entry(pattern) {
  const file = assets.find((name) => pattern.test(name) && !name.endsWith(".sig"));
  if (!file) throw new Error(`no asset matching ${pattern} in ${tag}`);
  if (!assets.includes(`${file}.sig`)) throw new Error(`missing ${file}.sig in ${tag}`);
  gh("release", "download", tag, "-p", `${file}.sig`, "-D", dir, "--clobber");
  return {
    signature: readFileSync(join(dir, `${file}.sig`), "utf8").trim(),
    url: release.assets.find((a) => a.name === file).url,
  };
}

const nsis = entry(/_x64-setup\.exe$/);
const msi = entry(/_x64_en-US\.msi$/);
const mac = entry(/\.app\.tar\.gz$/);

const manifest = {
  version: tag.replace(/^v/, ""),
  notes: release.body,
  pub_date: release.publishedAt,
  platforms: {
    "windows-x86_64": nsis,
    "windows-x86_64-nsis": nsis,
    "windows-x86_64-msi": msi,
    "darwin-aarch64": mac,
    "darwin-x86_64": mac,
    "darwin-aarch64-app": mac,
    "darwin-x86_64-app": mac,
  },
};

const out = join(dir, "latest.json");
writeFileSync(out, JSON.stringify(manifest, null, 2));
gh("release", "upload", tag, out, "--clobber");
console.log(`latest.json uploaded to ${tag}`);

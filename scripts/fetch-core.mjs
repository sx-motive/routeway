#!/usr/bin/env node
// Downloads the pinned sing-box build and places it where Tauri expects a sidecar:
// src-tauri/binaries/sing-box-<target-triple>[.exe]
//
// Usage:
//   node scripts/fetch-core.mjs              # current host (dev)
//   node scripts/fetch-core.mjs --universal  # both macOS arches + lipo (release)

import { execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, rm, chmod, copyFile, access } from "node:fs/promises";
import { pipeline } from "node:stream/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";

export const CORE_VERSION = "1.14.3";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = join(ROOT, "src-tauri", "binaries");

// asset slug -> rust target triple
const BUILDS = {
  "darwin-arm64": { triple: "aarch64-apple-darwin", archive: "tar.gz", exe: "" },
  "darwin-amd64": { triple: "x86_64-apple-darwin", archive: "tar.gz", exe: "" },
  "windows-amd64": { triple: "x86_64-pc-windows-msvc", archive: "zip", exe: ".exe" },
  "windows-arm64": { triple: "aarch64-pc-windows-msvc", archive: "zip", exe: ".exe" },
};

function hostSlug() {
  const arch = process.arch === "arm64" ? "arm64" : "amd64";
  if (process.platform === "darwin") return `darwin-${arch}`;
  if (process.platform === "win32") return `windows-${arch}`;
  throw new Error(`unsupported host platform: ${process.platform}`);
}

async function exists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

async function fetchBuild(slug) {
  const build = BUILDS[slug];
  if (!build) throw new Error(`unknown build: ${slug}`);

  const dest = join(OUT_DIR, `sing-box-${build.triple}${build.exe}`);
  if (await exists(dest)) {
    console.log(`· sing-box ${CORE_VERSION} ${slug} already present`);
    return dest;
  }

  const name = `sing-box-${CORE_VERSION}-${slug}`;
  const url = `https://github.com/SagerNet/sing-box/releases/download/v${CORE_VERSION}/${name}.${build.archive}`;
  const work = join(tmpdir(), `routeway-core-${slug}-${process.pid}`);
  await mkdir(work, { recursive: true });

  console.log(`↓ ${url}`);
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`download failed (${res.status}) for ${url}`);
  const archive = join(work, `core.${build.archive}`);
  await pipeline(res.body, createWriteStream(archive));

  // bsdtar ships with macOS and Windows 10+ and reads both tar.gz and zip.
  execFileSync("tar", ["-xf", archive, "-C", work], { stdio: "inherit" });

  await mkdir(OUT_DIR, { recursive: true });
  await copyFile(join(work, name, `sing-box${build.exe}`), dest);
  if (!build.exe) await chmod(dest, 0o755);
  await rm(work, { recursive: true, force: true });

  console.log(`✓ ${dest}`);
  return dest;
}

async function main() {
  const universal = process.argv.includes("--universal");

  if (!universal) {
    await fetchBuild(hostSlug());
    return;
  }

  if (process.platform === "darwin") {
    const arm = await fetchBuild("darwin-arm64");
    const intel = await fetchBuild("darwin-amd64");
    const fat = join(OUT_DIR, "sing-box-universal-apple-darwin");
    execFileSync("lipo", ["-create", "-output", fat, arm, intel], { stdio: "inherit" });
    await chmod(fat, 0o755);
    console.log(`✓ ${fat} (universal)`);
    return;
  }

  // Windows release runners build per-arch, no fat binary exists.
  await fetchBuild(hostSlug());
}

main().catch((err) => {
  console.error(`✗ ${err.message}`);
  process.exit(1);
});

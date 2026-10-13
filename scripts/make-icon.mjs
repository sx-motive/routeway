#!/usr/bin/env node
// Draws the Routeway source icon (a split path: one branch through the server,
// one staying direct) and writes it as a PNG. No image deps — the shapes are
// simple enough to rasterize by signed distance.
//
//   node scripts/make-icon.mjs            # writes icon-source.png
//   npx tauri icon icon-source.png        # expands it into every platform size

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SIZE = 1024;
const OUT = join(dirname(fileURLToPath(import.meta.url)), "..", "icon-source.png");

const BG = [18, 18, 20, 255];
const ACCENT = [245, 71, 75, 255]; // matches --color-accent
const MUTED = [120, 120, 124, 255];

// Segments as fractions of SIZE: [x1, y1, x2, y2, halfWidth, color]
// The muted branch is thinner and stops short of the dots, so the accent path
// reads as the chosen route rather than as half of a symmetric Y.
const STEM = [0.5, 0.84, 0.5, 0.54, 0.052, ACCENT];
const BRANCH_PROXY = [0.5, 0.54, 0.75, 0.3, 0.052, ACCENT];
const BRANCH_DIRECT = [0.5, 0.54, 0.27, 0.32, 0.04, MUTED];
const SEGMENTS = [BRANCH_DIRECT, STEM, BRANCH_PROXY];

const DOTS = [
  [0.75, 0.3, 0.092, ACCENT],
  [0.25, 0.3, 0.062, MUTED],
];

function distanceToSegment(px, py, x1, y1, x2, y2) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lengthSq = dx * dx + dy * dy;
  let t = lengthSq === 0 ? 0 : ((px - x1) * dx + (py - y1) * dy) / lengthSq;
  t = Math.max(0, Math.min(1, t));
  const cx = x1 + t * dx;
  const cy = y1 + t * dy;
  return Math.hypot(px - cx, py - cy);
}

function blend(dst, src, alpha) {
  for (let i = 0; i < 3; i++) dst[i] = Math.round(dst[i] * (1 - alpha) + src[i] * alpha);
  dst[3] = 255;
}

function render() {
  // 2x supersampling, then box-average — cheap anti-aliasing.
  const ss = 2;
  const dim = SIZE * ss;
  const acc = new Float64Array(SIZE * SIZE * 4);

  for (let y = 0; y < dim; y++) {
    const fy = (y + 0.5) / dim;
    for (let x = 0; x < dim; x++) {
      const fx = (x + 0.5) / dim;
      const pixel = [...BG];

      for (const [x1, y1, x2, y2, half, color] of SEGMENTS) {
        if (distanceToSegment(fx, fy, x1, y1, x2, y2) <= half) blend(pixel, color, 1);
      }
      for (const [cx, cy, r, color] of DOTS) {
        const d = Math.hypot(fx - cx, fy - cy);
        if (d <= r) blend(pixel, color, 1);
        else if (d <= r * 1.6 && color === ACCENT) {
          blend(pixel, color, 0.18 * (1 - (d - r) / (r * 0.6)));
        }
      }

      const ox = Math.floor(x / ss);
      const oy = Math.floor(y / ss);
      const base = (oy * SIZE + ox) * 4;
      for (let i = 0; i < 4; i++) acc[base + i] += pixel[i] / (ss * ss);
    }
  }

  // PNG scanlines: one filter byte (0 = none) per row.
  const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
  let offset = 0;
  for (let y = 0; y < SIZE; y++) {
    raw[offset++] = 0;
    for (let x = 0; x < SIZE; x++) {
      const base = (y * SIZE + x) * 4;
      for (let i = 0; i < 4; i++) raw[offset++] = Math.round(acc[base + i]);
    }
  }
  return raw;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(render(), { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync(OUT, png);
console.log(`✓ ${OUT} (${SIZE}×${SIZE})`);

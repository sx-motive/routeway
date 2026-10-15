import { readdir, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { extname, join } from "node:path";
import { gzipSync } from "node:zlib";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const totals = new Map();
for (const file of await readdir(dist, { recursive: true, withFileTypes: true })) {
  if (!file.isFile()) continue;
  const bytes = await readFile(join(file.parentPath, file.name));
  const type = extname(file.name).slice(1);
  const total = totals.get(type) ?? { bytes: 0, gzip: 0, files: 0 };
  total.bytes += bytes.length;
  total.gzip += gzipSync(bytes).length;
  total.files++;
  totals.set(type, total);
}

console.table([...totals].map(([type, total]) => ({
  type, files: total.files,
  "size (kB)": (total.bytes / 1000).toFixed(2),
  "gzip (kB)": (total.gzip / 1000).toFixed(2),
})));
console.log(`Total dist: ${([...totals.values()].reduce((sum, row) => sum + row.bytes, 0) / 1000).toFixed(2)} kB`);

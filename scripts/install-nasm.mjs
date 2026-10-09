import { createWriteStream, existsSync, mkdirSync, readdirSync, statSync, unlinkSync } from "node:fs";
import { pipeline } from "node:stream/promises";
import { execSync } from "node:child_process";
import path from "node:path";
import https from "node:https";
import http from "node:http";

const url =
  "https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/win64/nasm-2.16.03-win64.zip";
const zipPath = path.resolve("tools/nasm.zip");
const dest = path.resolve("tools/nasm");

mkdirSync(path.dirname(zipPath), { recursive: true });

function get(u, redirects = 0) {
  return new Promise((resolve, reject) => {
    const mod = u.startsWith("https") ? https : http;
    mod
      .get(u, (res) => {
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location &&
          redirects < 5
        ) {
          res.resume();
          resolve(get(res.headers.location, redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode}`));
          return;
        }
        resolve(res);
      })
      .on("error", reject);
  });
}

function findNasm(dir) {
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) {
      const hit = findNasm(p);
      if (hit) return hit;
    } else if (name.toLowerCase() === "nasm.exe") {
      return p;
    }
  }
  return null;
}

const existing = existsSync(dest) ? findNasm(dest) : null;
if (existing) {
  console.log(existing);
  process.exit(0);
}

console.log("Downloading NASM...");
const res = await get(url);
await pipeline(res, createWriteStream(zipPath));
console.log("Extracting...");
execSync(
  `powershell -NoProfile -Command "Expand-Archive -LiteralPath '${zipPath.replace(/'/g, "''")}' -DestinationPath '${dest.replace(/'/g, "''")}' -Force"`,
  { stdio: "inherit" },
);
try {
  unlinkSync(zipPath);
} catch {
  // ignore
}
const nasm = findNasm(dest);
if (!nasm) {
  console.error("nasm.exe not found after extract");
  process.exit(1);
}
console.log(nasm);

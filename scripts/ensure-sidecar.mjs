/**
 * Ensure the Tauri externalBin sidecar exists and is not older than sidecar sources
 * before `tauri dev`. Matches the Node.js sidecar guide: package first, then run tauri.
 * https://v2.tauri.app/learn/sidecar-nodejs/
 */
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
const extension = triple.includes("windows") ? ".exe" : "";
const binary = resolve(root, "src-tauri", "binaries", `convertzz-sidecar-${triple}${extension}`);
const linuxResource = resolve(root, "src-tauri", "binaries", "convertzz-sidecar-linux-resource.gz");
const linuxChecksum = resolve(
  root,
  "src-tauri",
  "binaries",
  "convertzz-sidecar-linux-resource.sha256",
);

const missing =
  !existsSync(binary) ||
  (triple.includes("linux") && (!existsSync(linuxResource) || !existsSync(linuxChecksum)));

const stale = !missing && isSidecarStale(binary);

if (!missing && !stale) {
  console.log(`Sidecar already present: ${binary}`);
  process.exit(0);
}

console.log(
  missing
    ? "Sidecar binary missing; running pnpm run sidecar:build…"
    : "Sidecar binary older than sources; running pnpm run sidecar:build…",
);
const result = spawnSync("pnpm", ["run", "sidecar:build"], {
  cwd: root,
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);

function isSidecarStale(binaryPath) {
  const binaryMtime = statSync(binaryPath).mtimeMs;
  const sourceRoots = [
    resolve(root, "sidecar", "src"),
    resolve(root, "shared", "contracts.ts"),
    resolve(root, "scripts", "build-sidecar.mjs"),
    resolve(root, "scripts", "compile-sidecar.mjs"),
    resolve(root, "package.json"),
  ];
  const newestSource = newestMtime(sourceRoots);
  return newestSource > binaryMtime;
}

function newestMtime(paths) {
  let newest = 0;
  for (const path of paths) {
    if (!existsSync(path)) continue;
    const info = statSync(path);
    if (info.isDirectory()) {
      newest = Math.max(newest, walkDirectoryNewest(path));
    } else {
      newest = Math.max(newest, info.mtimeMs);
    }
  }
  return newest;
}

function walkDirectoryNewest(directory) {
  let newest = 0;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.name === "node_modules" || entry.name.startsWith(".")) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) newest = Math.max(newest, walkDirectoryNewest(path));
    else if (entry.isFile() && /\.(ts|js|mjs|cjs|json)$/u.test(entry.name)) {
      newest = Math.max(newest, statSync(path).mtimeMs);
    }
  }
  return newest;
}

function hostTriple() {
  try {
    const tuple = execFileSync("rustc", ["--print", "host-tuple"], {
      encoding: "utf8",
    }).trim();
    if (tuple) return tuple;
  } catch {
    // Fall through for older rustc.
  }
  const output = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const match = output.match(/^host:\s*(.+)$/m);
  if (!match) throw new Error("Unable to determine the Rust host triple.");
  return match[1].trim();
}

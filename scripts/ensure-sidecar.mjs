/**
 * Ensure the Tauri externalBin sidecar exists before `tauri dev`.
 * Matches the Node.js sidecar guide: package first, then run tauri.
 * https://v2.tauri.app/learn/sidecar-nodejs/
 */
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

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

if (!missing) {
  console.log(`Sidecar already present: ${binary}`);
  process.exit(0);
}

console.log("Sidecar binary missing; running pnpm run sidecar:build…");
const result = spawnSync("pnpm", ["run", "sidecar:build"], {
  cwd: root,
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);

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

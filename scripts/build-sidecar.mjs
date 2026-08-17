/**
 * Package the Node.js sidecar the way Tauri expects:
 * https://v2.tauri.app/develop/sidecar/
 * https://v2.tauri.app/learn/sidecar-nodejs/
 *
 * 1. Build a self-contained binary with @yao-pkg/pkg.
 * 2. Place it at src-tauri/binaries/convertzz-sidecar-$TARGET_TRIPLE.
 * 3. On Linux, also emit gzip + SHA-256 resources (AppImage-safe; see tauri.linux.conf.json).
 *
 * pkg writes the large binary outside src-tauri first, then publishes once, so `tauri dev`
 * file watching is not thrashed by progressive writes under binaries/.
 */
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { gzipSync } from "node:zlib";

const root = resolve(import.meta.dirname, "..");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
const target = pkgTarget(triple);
const extension = triple.includes("windows") ? ".exe" : "";
const stagingDir = resolve(root, "sidecar", ".build");
const stagingOutput = resolve(stagingDir, `convertzz-sidecar${extension}`);
const output = resolve(root, "src-tauri", "binaries", `convertzz-sidecar-${triple}${extension}`);
const executable = resolve(
  root,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "pkg.cmd" : "pkg",
);

mkdirSync(stagingDir, { recursive: true });
mkdirSync(dirname(output), { recursive: true });
if (existsSync(stagingOutput)) rmSync(stagingOutput);

const result = spawnSync(
  executable,
  ["package.json", "--targets", target, "--output", stagingOutput, "--compress", "GZip"],
  {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32",
  },
);

if (result.status !== 0) process.exit(result.status ?? 1);

// Official guide renames into src-tauri/binaries/$name-$TARGET_TRIPLE after packaging.
publishFile(stagingOutput, output);
chmodSync(output, 0o755);
console.log(`Sidecar ready: ${output}`);

if (triple.includes("linux")) {
  const staleResource = resolve(
    root,
    "src-tauri",
    "binaries",
    "convertzz-sidecar-linux-resource.bin",
  );
  const resource = resolve(root, "src-tauri", "binaries", "convertzz-sidecar-linux-resource.gz");
  const checksum = resolve(
    root,
    "src-tauri",
    "binaries",
    "convertzz-sidecar-linux-resource.sha256",
  );
  const stagedResource = resolve(stagingDir, "convertzz-sidecar-linux-resource.gz");
  const stagedChecksum = resolve(stagingDir, "convertzz-sidecar-linux-resource.sha256");
  const sidecar = readFileSync(output);
  if (existsSync(staleResource)) rmSync(staleResource);
  writeFileSync(stagedResource, gzipSync(sidecar, { level: 9 }));
  writeFileSync(stagedChecksum, `${createHash("sha256").update(sidecar).digest("hex")}\n`);
  publishFile(stagedResource, resource);
  publishFile(stagedChecksum, checksum);
  chmodSync(resource, 0o644);
  chmodSync(checksum, 0o644);
  console.log(`Linux sidecar resource ready: ${resource}`);
}

function publishFile(source, destination) {
  const temporary = `${destination}.${process.pid}.tmp`;
  try {
    copyFileSync(source, temporary);
    renameSync(temporary, destination);
  } catch (error) {
    try {
      rmSync(temporary, { force: true });
    } catch {
      // ignore cleanup failures
    }
    throw error;
  }
}

function hostTriple() {
  // Prefer the flag from the Tauri sidecar guide (Rust 1.84+).
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

function pkgTarget(value) {
  const os = value.includes("windows") ? "win" : value.includes("linux") ? "linux" : "macos";
  const arch = value.startsWith("aarch64") ? "arm64" : "x64";
  return `node24-${os}-${arch}`;
}

import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { gzipSync } from "node:zlib";

const root = resolve(import.meta.dirname, "..");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
const target = pkgTarget(triple);
const extension = triple.includes("windows") ? ".exe" : "";
const output = resolve(root, "src-tauri", "binaries", `convertzz-sidecar-${triple}${extension}`);
const executable = resolve(
  root,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "pkg.cmd" : "pkg",
);

mkdirSync(dirname(output), { recursive: true });
if (existsSync(output)) rmSync(output);

const result = spawnSync(
  executable,
  ["package.json", "--targets", target, "--output", output, "--compress", "GZip"],
  {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32",
  },
);

if (result.status !== 0) process.exit(result.status ?? 1);
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
  const sidecar = readFileSync(output);
  if (existsSync(staleResource)) rmSync(staleResource);
  writeFileSync(resource, gzipSync(sidecar, { level: 9 }));
  writeFileSync(checksum, `${createHash("sha256").update(sidecar).digest("hex")}\n`);
  chmodSync(resource, 0o644);
  chmodSync(checksum, 0o644);
  console.log(`Linux sidecar resource ready: ${resource}`);
}

function hostTriple() {
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

import { spawnSync } from "node:child_process";
import { existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { build } from "esbuild";

const root = resolve(import.meta.dirname, "..");
const output = resolve(root, "sidecar", "dist");
const executable = resolve(
  root,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tsc.cmd" : "tsc",
);

if (existsSync(output)) {
  rmSync(output, { recursive: true });
}

const result = spawnSync(executable, ["-p", "sidecar/tsconfig.json", "--noEmit"], {
  cwd: root,
  stdio: "inherit",
  shell: process.platform === "win32",
});

if (result.status !== 0) process.exit(result.status ?? 1);

await build({
  entryPoints: [resolve(root, "sidecar", "src", "index.ts")],
  outfile: resolve(output, "convertzz-sidecar.cjs"),
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node12",
  minify: true,
  define: {
    "import.meta.url": JSON.stringify("file:///convertzz/convertzz-sidecar.cjs"),
  },
  external: ["cjk-conv", "deepmerge-plus", "novel-segment", "segment-dict"],
});

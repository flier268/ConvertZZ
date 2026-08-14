import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  createReadStream,
  createWriteStream,
  lstatSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { pipeline } from "node:stream/promises";
import { createGunzip } from "node:zlib";

const [appImageArgument, apeArgument, oggArgument] = process.argv.slice(2);
if (!appImageArgument || !apeArgument || !oggArgument) {
  throw new Error(
    "Usage: node scripts/verify-linux-appimage.mjs <AppImage> <APE fixture> <OGG fixture>",
  );
}

const appImage = resolve(appImageArgument);
const apeFixture = resolve(apeArgument);
const oggFixture = resolve(oggArgument);
const extractionDirectory = mkdtempSync(join(tmpdir(), "convertzz-appimage-"));

try {
  run(appImage, ["--appimage-extract"], { cwd: extractionDirectory });

  const appDir = join(extractionDirectory, "squashfs-root");
  const resourceDirectory = join(appDir, "usr/lib/ConvertZZ");
  const packagedSidecar = join(resourceDirectory, "convertzz-sidecar.gz");
  const packagedChecksum = join(resourceDirectory, "convertzz-sidecar.sha256");
  const runtimeSidecar = join(extractionDirectory, "convertzz-sidecar");
  const dictionary = join(resourceDirectory, "Dictionary.csv");
  const wasm = join(resourceDirectory, "taglib-wasi.wasm");

  if ((statSync(packagedSidecar).mode & 0o111) !== 0) {
    throw new Error("AppImage 內的 sidecar 資源不應具有執行權限。");
  }

  await pipeline(
    createReadStream(packagedSidecar),
    createGunzip(),
    createWriteStream(runtimeSidecar, { mode: 0o755 }),
  );
  chmodSync(runtimeSidecar, 0o755);
  const runtimeHash = sha256(runtimeSidecar);
  const declaredHash = readFileSync(packagedChecksum, "utf8").trim().toLowerCase();
  if (declaredHash !== runtimeHash) {
    throw new Error(
      `AppImage 解壓後的 sidecar 不符。解壓後 ${runtimeHash}，宣告 ${declaredHash}。`,
    );
  }

  const sidecarArguments = ["--dictionary", dictionary, "--wasm", wasm];
  const health = request(runtimeSidecar, sidecarArguments, {
    id: "appimage-health",
    operation: "health",
    payload: {},
  });
  if (
    !health.ok ||
    typeof health.result?.node !== "string" ||
    !health.result.node.startsWith("24.")
  ) {
    throw new Error(`AppImage sidecar 健康檢查失敗：${JSON.stringify(health)}`);
  }

  const conversion = request(runtimeSidecar, sidecarArguments, {
    id: "appimage-conversion",
    operation: "convert.preview",
    payload: { text: "里面开发头发", direction: "s2t", engine: "segmented" },
  });
  if (!conversion.ok || conversion.result?.text !== "裡面開發頭髮") {
    throw new Error(`AppImage sidecar 文字轉換失敗：${JSON.stringify(conversion)}`);
  }

  const audio = request(runtimeSidecar, sidecarArguments, {
    id: "appimage-audio",
    operation: "audio.scan",
    payload: { paths: [apeFixture, oggFixture] },
  });
  const scanned = Array.isArray(audio.result) ? audio.result : [];
  const formats = scanned.map((file) => file?.format).sort();
  if (!audio.ok || scanned.some((file) => file?.warning) || formats.join(",") !== "ape,ogg") {
    throw new Error(`AppImage 離線音訊掃描失敗：${JSON.stringify(audio)}`);
  }

  process.stdout.write(
    `${JSON.stringify(
      {
        appImage,
        sidecarSha256: runtimeHash,
        node: health.result.node,
        convertedText: conversion.result.text,
        audioFormats: formats,
      },
      null,
      2,
    )}\n`,
  );
} finally {
  removeOwnedTemporaryDirectory(extractionDirectory);
}

function request(executable, args, body) {
  const result = spawnSync(executable, args, {
    encoding: "utf8",
    input: `${JSON.stringify(body)}\n`,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `sidecar 結束碼 ${result.status ?? "unknown"}：${result.stderr || result.stdout}`,
    );
  }
  const messages = result.stdout
    .split(/\r?\n/u)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
  const response = messages.findLast(
    (message) => message.id === body.id && message.type === "response",
  );
  if (!response) throw new Error(`sidecar 沒有回傳 ${body.id} 的完成回應。`);
  return response;
}

function run(executable, args, options = {}) {
  const result = spawnSync(executable, args, {
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(`${basename(executable)} 執行失敗：${result.stderr || result.stdout}`);
  }
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function removeOwnedTemporaryDirectory(path) {
  const resolvedRoot = realpathSync(tmpdir());
  const resolvedPath = realpathSync(path);
  const entry = lstatSync(resolvedPath);
  if (!entry.isDirectory() || entry.isSymbolicLink()) throw new Error("拒絕清除非一般暫存目錄。");
  if (
    dirname(resolvedPath) !== resolvedRoot ||
    !basename(resolvedPath).startsWith("convertzz-appimage-")
  ) {
    throw new Error(`拒絕清除非 ConvertZZ 暫存目錄：${resolvedPath}`);
  }
  rmSync(resolvedPath, { recursive: true });
}

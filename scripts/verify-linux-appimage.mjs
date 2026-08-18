import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

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
  const binary = join(appDir, "usr/bin/convertzz");
  const resourceDirectory = join(appDir, "usr/lib/ConvertZZ");
  const dictionary = join(resourceDirectory, "Dictionary.csv");
  const segmentDict = join(resourceDirectory, "segment-dict");

  if (!existsSync(binary) || (statSync(binary).mode & 0o111) === 0) {
    throw new Error("AppImage 缺少可執行的 usr/bin/convertzz。");
  }
  if (!existsSync(dictionary)) {
    throw new Error("AppImage 缺少 Dictionary.csv。");
  }
  if (!existsSync(join(segmentDict, "segment"))) {
    throw new Error("AppImage 缺少 segment-dict 分詞字典。");
  }
  if (existsSync(join(resourceDirectory, "convertzz-sidecar.gz"))) {
    throw new Error("AppImage 不應再包含 Node.js sidecar 資源。");
  }
  if (existsSync(join(resourceDirectory, "taglib-wasi.wasm"))) {
    throw new Error("AppImage 不應再包含 taglib-wasi.wasm。");
  }
  if (!existsSync(apeFixture) || !existsSync(oggFixture)) {
    throw new Error("缺少音訊驗收樣本。");
  }

  process.stdout.write(
    `${JSON.stringify(
      {
        appImage,
        binary,
        dictionary,
        segmentDict,
        apeFixture,
        oggFixture,
      },
      null,
      2,
    )}\n`,
  );
} finally {
  rmSync(extractionDirectory, { recursive: true, force: true });
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} 結束碼 ${result.status ?? "unknown"}：${result.stderr || result.stdout}`,
    );
  }
  return result;
}

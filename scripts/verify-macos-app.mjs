import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const [appArgument, apeArgument, oggArgument] = process.argv.slice(2);
if (!appArgument || !apeArgument || !oggArgument) {
  throw new Error(
    "Usage: node scripts/verify-macos-app.mjs <ConvertZZ.app> <APE fixture> <OGG fixture>",
  );
}

const app = resolve(appArgument);
const apeFixture = resolve(apeArgument);
const oggFixture = resolve(oggArgument);
const macosDir = join(app, "Contents", "MacOS");
const resourceDirectory = join(app, "Contents", "Resources");
const dictionary = join(resourceDirectory, "Dictionary.csv");
const segmentDict = join(resourceDirectory, "segment-dict");

if (!existsSync(macosDir)) {
  throw new Error("macOS 應用程式包缺少 Contents/MacOS。");
}

const binaries = readdirSync(macosDir)
  .map((name) => join(macosDir, name))
  .filter((path) => {
    const stats = statSync(path);
    return stats.isFile() && (stats.mode & 0o111) !== 0;
  });
if (binaries.length === 0) {
  throw new Error("macOS 應用程式包缺少可執行檔。");
}

if (!existsSync(dictionary)) {
  throw new Error("macOS 應用程式包缺少 Dictionary.csv。");
}
if (!existsSync(join(segmentDict, "segment"))) {
  throw new Error("macOS 應用程式包缺少 segment-dict 分詞字典。");
}
if (existsSync(join(resourceDirectory, "taglib-wasi.wasm"))) {
  throw new Error("macOS 應用程式包不應再包含 taglib-wasi.wasm。");
}
if (!existsSync(apeFixture) || !existsSync(oggFixture)) {
  throw new Error("缺少音訊驗收樣本。");
}

const binary = binaries[0];
const lipo = spawnSync("lipo", ["-archs", binary], { encoding: "utf8" });
if (lipo.status !== 0) {
  throw new Error(`lipo 無法讀取 ${binary}：${lipo.stderr || lipo.stdout}`);
}
const archs = new Set((lipo.stdout || "").trim().split(/\s+/u).filter(Boolean));
if (!archs.has("arm64") || !archs.has("x86_64")) {
  throw new Error(`預期 Universal 二進位（arm64 + x86_64），實際：${[...archs].join(" ")}`);
}

process.stdout.write(
  `${JSON.stringify(
    {
      app,
      binary,
      dictionary,
      segmentDict,
      archs: [...archs],
      apeFixture,
      oggFixture,
    },
    null,
    2,
  )}\n`,
);

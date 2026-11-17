import { afterEach, describe, expect, it } from "vitest";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
const ape = join(projectRoot, "tests/fixtures/mac-399.ape");
const ogg = join(projectRoot, "tests/fixtures/test.ogg");
const unix = process.platform !== "win32";
const directories: string[] = [];

afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});

function temporary(): string {
  const directory = mkdtempSync(join(tmpdir(), "convertzz-verify-"));
  directories.push(directory);
  return directory;
}

function executable(path: string, content: string) {
  writeFileSync(path, content);
  chmodSync(path, 0o755);
}

function runScript(script: string, args: string[], env: NodeJS.ProcessEnv = process.env) {
  return spawnSync(process.execPath, [join(projectRoot, "scripts", script), ...args], {
    encoding: "utf8",
    env,
  });
}

/** 假 AppImage：`--appimage-extract` 時在工作目錄建立 squashfs-root。 */
function fakeAppImage(): string {
  const directory = temporary();
  const appImage = join(directory, "ConvertZZ.AppImage");
  const resources = "squashfs-root/usr/lib/ConvertZZ";
  executable(
    appImage,
    [
      "#!/bin/sh",
      "set -e",
      "mkdir -p squashfs-root/usr/bin " + `${resources}/segment-dict/segment`,
      "printf '#!/bin/sh\\n' > squashfs-root/usr/bin/convertzz",
      "chmod +x squashfs-root/usr/bin/convertzz",
      "",
    ].join("\n"),
  );
  return appImage;
}

/** 假 macOS 應用程式包，並以假 `lipo` 回報 Universal 架構。 */
function fakeMacApp(): { app: string; env: NodeJS.ProcessEnv } {
  const directory = temporary();
  const app = join(directory, "ConvertZZ.app");
  mkdirSync(join(app, "Contents/MacOS"), { recursive: true });
  mkdirSync(join(app, "Contents/Resources/segment-dict/segment"), { recursive: true });
  executable(join(app, "Contents/MacOS/convertzz"), "#!/bin/sh\n");
  const bin = join(directory, "bin");
  mkdirSync(bin);
  executable(join(bin, "lipo"), "#!/bin/sh\necho 'x86_64 arm64'\n");
  return { app, env: { ...process.env, PATH: `${bin}${delimiter}${process.env.PATH ?? ""}` } };
}

describe.runIf(unix)("發行包離線驗證腳本", () => {
  it("AppImage 驗證可跑完", () => {
    const result = runScript("verify-linux-appimage.mjs", [fakeAppImage(), ape, ogg]);
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toHaveProperty("segmentDict");
  });

  it("macOS 應用程式包驗證可跑完", () => {
    const { app, env } = fakeMacApp();
    const result = runScript("verify-macos-app.mjs", [app, ape, ogg], env);
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout).archs).toEqual(["x86_64", "arm64"]);
  });
});

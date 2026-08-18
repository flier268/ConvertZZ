import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildLatestJson, collectFiles } from "../scripts/write-latest-json.mjs";

describe("latest.json", () => {
  it("組出 Windows 與 Linux 的簽署更新清單", () => {
    const root = mkdtempSync(join(tmpdir(), "convertzz-latest-"));
    writeFileSync(join(root, "ConvertZZ_2.1.0_x64-setup.exe"), "exe");
    writeFileSync(join(root, "ConvertZZ_2.1.0_x64-setup.exe.sig"), "windows-sig\n");
    writeFileSync(join(root, "ConvertZZ_2.1.0_amd64.AppImage"), "appimage");
    writeFileSync(join(root, "ConvertZZ_2.1.0_amd64.AppImage.sig"), "linux-sig\n");
    writeFileSync(join(root, "ConvertZZ_2.1.0_amd64.deb"), "deb");

    expect(
      buildLatestJson({
        files: collectFiles(root),
        tag: "v2.1.0",
        repo: "flier268/ConvertZZ",
        pubDate: "2026-08-15T00:00:00.000Z",
      }),
    ).toEqual({
      version: "2.1.0",
      notes: "ConvertZZ v2.1.0",
      pub_date: "2026-08-15T00:00:00.000Z",
      platforms: {
        "windows-x86_64": {
          signature: "windows-sig",
          url: "https://github.com/flier268/ConvertZZ/releases/download/v2.1.0/ConvertZZ_2.1.0_x64-setup.exe",
        },
        "linux-x86_64": {
          signature: "linux-sig",
          url: "https://github.com/flier268/ConvertZZ/releases/download/v2.1.0/ConvertZZ_2.1.0_amd64.AppImage",
        },
      },
    });
  });

  it("預發佈標籤會保留 SemVer 預發佈字串", () => {
    const root = mkdtempSync(join(tmpdir(), "convertzz-latest-beta-"));
    writeFileSync(join(root, "ConvertZZ_2.0.0-beta1_x64-setup.exe"), "exe");
    writeFileSync(join(root, "ConvertZZ_2.0.0-beta1_x64-setup.exe.sig"), "windows-sig\n");
    writeFileSync(join(root, "ConvertZZ_2.0.0-beta1_amd64.AppImage"), "appimage");
    writeFileSync(join(root, "ConvertZZ_2.0.0-beta1_amd64.AppImage.sig"), "linux-sig\n");

    expect(
      buildLatestJson({
        files: collectFiles(root),
        tag: "v2.0.0-beta1",
        repo: "flier268/ConvertZZ",
        pubDate: "2026-08-15T00:00:00.000Z",
      }),
    ).toMatchObject({
      version: "2.0.0-beta1",
      notes: "ConvertZZ v2.0.0-beta1",
      platforms: {
        "windows-x86_64": {
          url: "https://github.com/flier268/ConvertZZ/releases/download/v2.0.0-beta1/ConvertZZ_2.0.0-beta1_x64-setup.exe",
        },
        "linux-x86_64": {
          url: "https://github.com/flier268/ConvertZZ/releases/download/v2.0.0-beta1/ConvertZZ_2.0.0-beta1_amd64.AppImage",
        },
      },
    });
  });

  it("缺少簽章時拒絕產生更新清單", () => {
    expect(() =>
      buildLatestJson({
        files: ["/tmp/ConvertZZ_2.1.0_x64-setup.exe"],
        tag: "v2.1.0",
        repo: "flier268/ConvertZZ",
        pubDate: "2026-08-15T00:00:00.000Z",
      }),
    ).toThrow("找不到 ConvertZZ_2.1.0_x64-setup.exe 的簽章檔。");
  });
});

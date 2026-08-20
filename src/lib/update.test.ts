import { describe, expect, it, vi } from "vitest";
import {
  checkLatestRelease,
  compareVersions,
  isPreReleaseVersion,
  isUpdateVersionSkipped,
  normalizeVersion,
  resolveUpdate,
} from "./update";

describe("版本檢查", () => {
  it("比較正式版本與預發佈版本", () => {
    expect(compareVersions("2.1.0", "2.0.9")).toBe(1);
    expect(compareVersions("2.0.0", "2.0.0")).toBe(0);
    expect(compareVersions("1.9.9", "2.0.0")).toBe(-1);
    expect(compareVersions("2.0.0", "2.0.0-beta5")).toBe(1);
    expect(compareVersions("2.0.0-beta5", "2.0.0-beta4")).toBe(1);
    expect(compareVersions("2.0.0-beta1", "2.0.0-alpha9")).toBe(1);
    expect(compareVersions("2.0.0-alpha9", "2.0.0-beta1")).toBe(-1);
    expect(compareVersions("v2.0.0-beta5", "2.0.0-beta4")).toBe(1);
    expect(compareVersions("2.0.0-rc.1", "2.0.0-beta.9")).toBe(1);
    expect(compareVersions("2.0.0-beta.2", "2.0.0-beta.10")).toBe(-1);
  });

  it("辨識並保留預發佈版本字串", () => {
    expect(normalizeVersion("v2.0.0-beta1")).toBe("2.0.0-beta1");
    expect(normalizeVersion("2.0.0-beta1+build.1")).toBe("2.0.0-beta1");
    expect(isPreReleaseVersion("2.0.0")).toBe(false);
    expect(isPreReleaseVersion("2.0.0-beta1")).toBe(true);
    expect(isPreReleaseVersion("v2.0.0-rc.1")).toBe(true);
  });

  it("預設只使用正式版 latest", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            tag_name: "v2.1.0",
            html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
            prerelease: false,
            draft: false,
          }),
          { status: 200 },
        ),
    );
    await expect(checkLatestRelease("2.0.0", fetcher as typeof fetch)).resolves.toMatchObject({
      latestVersion: "2.1.0",
      updateAvailable: true,
    });
    expect(fetcher).toHaveBeenCalledWith(
      "https://api.github.com/repos/flier268/ConvertZZ/releases/latest",
      expect.any(Object),
    );
  });

  it("開啟開發版檢查時會選出較新的預發佈版本", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify([
            {
              tag_name: "v2.0.0-beta4",
              html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.0.0-beta4",
              prerelease: true,
              draft: false,
            },
            {
              tag_name: "v2.0.0-beta5",
              html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.0.0-beta5",
              prerelease: true,
              draft: false,
            },
            {
              tag_name: "v1.9.0",
              html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v1.9.0",
              prerelease: false,
              draft: false,
            },
          ]),
          { status: 200 },
        ),
    );
    await expect(
      checkLatestRelease("2.0.0-beta4", fetcher as typeof fetch, { includePreRelease: true }),
    ).resolves.toMatchObject({
      latestVersion: "2.0.0-beta5",
      updateAvailable: true,
      url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.0.0-beta5",
    });
    expect(fetcher).toHaveBeenCalledWith(
      "https://api.github.com/repos/flier268/ConvertZZ/releases?per_page=30",
      expect.any(Object),
    );
  });

  it("有可安裝更新且版本與目標一致時優先就地安裝", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        checkInstallable: async () => ({ currentVersion: "2.0.0", version: "2.1.0" }),
        checkRelease: async () => ({
          currentVersion: "2.0.0",
          latestVersion: "2.1.0",
          updateAvailable: true,
          url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
        }),
      }),
    ).resolves.toMatchObject({
      kind: "install",
      latestVersion: "2.1.0",
    });
  });

  it("GitHub 檢查失敗時仍可使用簽署更新通道", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        checkInstallable: async () => ({ currentVersion: "2.0.0", version: "2.1.0" }),
        checkRelease: async () => {
          throw new Error("GitHub 無法連線");
        },
      }),
    ).resolves.toMatchObject({
      kind: "install",
      latestVersion: "2.1.0",
    });
  });

  it("預設通道會忽略可安裝的預發佈版本", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        includePreRelease: false,
        checkInstallable: async () => ({ currentVersion: "2.0.0", version: "2.1.0-beta1" }),
        checkRelease: async () => ({
          currentVersion: "2.0.0",
          latestVersion: "2.0.0",
          updateAvailable: false,
          url: "https://github.com/flier268/ConvertZZ/releases",
        }),
      }),
    ).resolves.toMatchObject({ kind: "none", latestVersion: "2.0.0" });
  });

  it("開發版通道在簽署清單沒有對應預發佈時改開下載頁", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        includePreRelease: true,
        checkInstallable: async () => null,
        checkRelease: async () => ({
          currentVersion: "2.0.0",
          latestVersion: "2.1.0-beta1",
          updateAvailable: true,
          url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0-beta1",
        }),
      }),
    ).resolves.toMatchObject({
      kind: "open",
      latestVersion: "2.1.0-beta1",
    });
  });

  it("簽署通道失敗時改開啟 GitHub Release", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        checkInstallable: async () => {
          throw new Error("latest.json 不存在");
        },
        checkRelease: async () => ({
          currentVersion: "2.0.0",
          latestVersion: "2.1.0",
          updateAvailable: true,
          url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
        }),
      }),
    ).resolves.toMatchObject({
      kind: "open",
      latestVersion: "2.1.0",
    });
  });

  it("啟動檢查會略過已記錄版本，較新版本仍會詢問", () => {
    expect(isUpdateVersionSkipped("2.1.0", "")).toBe(false);
    expect(isUpdateVersionSkipped("2.1.0", undefined)).toBe(false);
    expect(isUpdateVersionSkipped("2.1.0", "2.1.0")).toBe(true);
    expect(isUpdateVersionSkipped("2.1.0", "2.2.0")).toBe(true);
    expect(isUpdateVersionSkipped("2.2.0", "2.1.0")).toBe(false);
    expect(isUpdateVersionSkipped("2.1.0-beta2", "2.1.0-beta1")).toBe(false);
    expect(isUpdateVersionSkipped("2.1.0-beta1", "2.1.0-beta1")).toBe(true);
  });

  it("沒有新版本時回傳 none", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        checkInstallable: async () => null,
        checkRelease: async () => ({
          currentVersion: "2.0.0",
          latestVersion: "2.0.0",
          updateAvailable: false,
          url: "https://github.com/flier268/ConvertZZ/releases",
        }),
      }),
    ).resolves.toMatchObject({ kind: "none", latestVersion: "2.0.0" });
  });
});

import { describe, expect, it, vi } from "vitest";
import {
  checkLatestRelease,
  compareVersions,
  isUpdateVersionSkipped,
  resolveUpdate,
} from "./update";

describe("版本檢查", () => {
  it("比較正式版本", () => {
    expect(compareVersions("2.1.0", "2.0.9")).toBe(1);
    expect(compareVersions("2.0.0", "2.0.0")).toBe(0);
    expect(compareVersions("1.9.9", "2.0.0")).toBe(-1);
  });

  it("只使用模擬回應判斷 GitHub Release", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            tag_name: "v2.1.0",
            html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
          }),
          { status: 200 },
        ),
    );
    await expect(checkLatestRelease("2.0.0", fetcher as typeof fetch)).resolves.toMatchObject({
      latestVersion: "2.1.0",
      updateAvailable: true,
    });
    expect(fetcher).toHaveBeenCalledOnce();
  });

  it("有可安裝更新時優先就地安裝", async () => {
    await expect(
      resolveUpdate("2.0.0", {
        checkInstallable: async () => ({ currentVersion: "2.0.0", version: "2.1.0" }),
        checkRelease: async () => {
          throw new Error("不應查詢 GitHub");
        },
      }),
    ).resolves.toMatchObject({
      kind: "install",
      latestVersion: "2.1.0",
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

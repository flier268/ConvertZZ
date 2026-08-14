import { describe, expect, it, vi } from "vitest";
import { checkLatestRelease, compareVersions } from "./update";

describe("版本檢查", () => {
  it("比較正式版本", () => {
    expect(compareVersions("2.1.0", "2.0.9")).toBe(1);
    expect(compareVersions("2.0.0", "2.0.0")).toBe(0);
    expect(compareVersions("1.9.9", "2.0.0")).toBe(-1);
  });

  it("只使用模擬回應判斷 GitHub Release", async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({ tag_name: "v2.1.0", html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0" }), { status: 200 }));
    await expect(checkLatestRelease("2.0.0", fetcher as typeof fetch)).resolves.toMatchObject({ latestVersion: "2.1.0", updateAvailable: true });
    expect(fetcher).toHaveBeenCalledOnce();
  });
});

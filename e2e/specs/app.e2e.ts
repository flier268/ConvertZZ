import { expect, test } from "@playwright/test";
import { e2eState, openApp, openPage } from "../helpers";

test.describe("ConvertZZ 前端", () => {
  test("啟動後顯示快速轉換並連上轉換核心", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "快速轉換" })).toBeVisible();
    await expect(page.locator(".status-dot.online")).toBeVisible();
    await expect(page.locator(".runtime-status")).toContainText("核心");
  });

  test("首次導覽在匯入前會詢問，而不是直接寫入", async ({ page }) => {
    await openApp(page, {
      onboardingCompleted: false,
      legacySettingsPath: "/tmp/ConvertZZ.json",
    });
    await expect(page.getByText("歡迎使用 ConvertZZ 2.0")).toBeVisible();
    for (let index = 0; index < 7; index += 1)
      await page.getByRole("button", { name: "下一步" }).click();
    await expect(page.getByText("來源檔不會被修改")).toBeVisible();
    await expect(page.getByRole("button", { name: "匯入找到的設定" })).toBeVisible();
    await expect(page.getByRole("button", { name: "略過匯入" })).toBeVisible();
  });

  test("覆寫需要第二次確認", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#tour-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "衝突策略" }).locator(".el-select").click();
    await page.getByRole("option", { name: "覆寫" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await page.getByRole("button", { name: "確認執行" }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual(
        expect.arrayContaining([
          expect.stringContaining("將執行"),
          "覆寫會取代既有的同名檔案。是否確定繼續？",
        ]),
      );
  });

  test("無法自動安裝時改開啟 GitHub Release", async ({ page }) => {
    await page.route("https://api.github.com/**", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          tag_name: "v2.1.0",
          html_url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
        }),
      });
    });
    await openApp(page);
    await openPage(page, "#tour-about", "ConvertZZ 2.0");
    await page.getByRole("button", { name: "檢查更新" }).click();
    await expect(page.getByText("發現新版本 2.1.0")).toBeVisible();
    await expect(page.getByText("此安裝方式無法自動更新")).toBeVisible();
    await page.getByRole("button", { name: "開啟下載頁" }).click();
    await expect
      .poll(async () => (await e2eState(page)).lastOpenedUrl ?? "")
      .toBe("https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0");
  });

  test("可略過此版本並在設定頁看到紀錄", async ({ page }) => {
    await openApp(page, { update: "install" });
    await openPage(page, "#tour-about", "ConvertZZ 2.0");
    await page.getByRole("button", { name: "檢查更新" }).click();
    await expect(page.getByText("是否下載並安裝")).toBeVisible();
    await page.getByText("不再詢問此版本").click();
    await page.getByRole("button", { name: "稍後再說" }).click();
    await openPage(page, "#tour-settings", "設定");
    await expect(page.locator(".settings-note")).toContainText("已略過 2.1.0");
  });

  test("G-15 可安裝更新會先確認再下載", async ({ page }) => {
    await openApp(page, { update: "install" });
    await openPage(page, "#tour-about", "ConvertZZ 2.0");
    await page.getByRole("button", { name: "檢查更新" }).click();
    await expect(page.getByText("發現新版本 2.1.0")).toBeVisible();
    await expect(page.getByText("是否下載並安裝")).toBeVisible();
    await expect(page.getByRole("button", { name: "下載並安裝" })).toBeVisible();
    await page.getByRole("button", { name: "稍後再說" }).click();
  });

  test("E-02 檔名作業預覽會顯示來源與輸出", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#tour-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "作業" }).locator(".el-select").click();
    await page.getByRole("option", { name: "轉換檔名" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByText("變更預覽")).toBeVisible();
    await expect(page.getByRole("cell", { name: "/tmp/里面.txt", exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "/tmp/裡面.txt", exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "里面.txt", exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "裡面.txt", exact: true })).toBeVisible();
  });

  test("H-06 Linux 設定頁不顯示 SendTo，並可匯入舊設定", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#tour-settings", "設定");
    await expect(page.getByRole("heading", { name: "設定" })).toBeVisible();
    await expect(page.getByRole("button", { name: "匯入 ConvertZZ.json" })).toBeVisible();
    await expect(page.getByText("SendTo 捷徑")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "建立 SendTo 捷徑" })).toHaveCount(0);
  });

  test("I-08 關於頁顯示平台差異表", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#tour-about", "ConvertZZ 2.0");
    await expect(page.getByText("平台差異")).toBeVisible();
    await expect(page.getByText("全域快捷鍵")).toBeVisible();
    await expect(page.getByText("SendTo 捷徑")).toBeVisible();
    await expect(page.getByText("需 AppIndicator；使用選單開啟").first()).toBeVisible();
    await expect(page.getByRole("columnheader", { name: "Linux Wayland" })).toBeVisible();
  });

  test("D-11 儲存字典前會詢問", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/Dictionary.csv" });
    await openPage(page, "#tour-dictionary", "舊版字典");
    await page.getByRole("button", { name: "選取可寫字典" }).click();
    await expect(page.getByRole("button", { name: "儲存變更", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "新增" }).click();
    await page.getByRole("button", { name: "儲存變更", exact: true }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual(expect.arrayContaining([expect.stringContaining("將先備份字典，再寫入")]));
  });
});

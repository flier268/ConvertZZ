import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { openApp, openPage } from "../helpers";

const shots = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "e2e-screenshots");

test.describe("音訊標籤審閱", () => {
  test("篩選、統計、詳細面板、設定抽屜與確認筆數", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await page.getByRole("button", { name: "建立標籤預覽" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();
    await expect(page.getByText("已勾選 233 個檔案・467 個欄位")).toBeVisible();
    await expect(page.getByText("只看有變更 233")).toBeVisible();
    await expect(page.getByRole("button", { name: "有變更 233" })).toBeVisible();
    await expect(page.getByRole("button", { name: "錯誤 1" })).toBeVisible();
    await expect.poll(async () => page.locator(".audio-row").count()).toBeLessThan(40);
    await expect(page.locator(".audio-row").first()).toBeVisible();
    await expect(page.getByText("track-000.mp3")).toHaveCount(0);

    await page.getByText("全部 250").click();
    await page.getByRole("button", { name: "全不選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 0 個檔案" })).toBeDisabled();
    await page.getByRole("button", { name: "全選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();
    await expect(page.getByText("已勾選 233 個檔案・467 個欄位")).toBeVisible();

    await page.getByText("只看有變更 233").click();
    await page.getByPlaceholder("搜尋檔名、專輯、標題").fill("same");
    await expect(page.locator(".audio-detail")).toContainText("這個篩選沒有檔案");
    await page.getByText("全部 250").click();
    await expect(page.locator(".audio-name", { hasText: "same.ogg" })).toBeVisible();
    await page.getByPlaceholder("搜尋檔名、專輯、標題").fill("");

    await page.getByText("只看錯誤 1").click();
    await page.locator(".audio-name", { hasText: "broken.mp3" }).click();
    await expect(page.locator(".audio-detail")).toContainText("無法讀取標籤");
    await expect(page.locator(".audio-detail")).toContainText("不能勾選");
    await page.screenshot({ path: join(shots, "audio-error-selected.png") });

    await page.getByRole("button", { name: "編輯設定" }).click();
    const drawer = page.getByRole("dialog");
    await expect(drawer.getByRole("heading", { name: "一般" })).toBeVisible();
    await expect(drawer.getByRole("heading", { name: "備份" })).toBeVisible();
    await expect(drawer.getByRole("heading", { name: "ID3v1" })).toBeVisible();
    await expect(drawer.getByRole("heading", { name: "ID3v2" })).toBeVisible();
    await expect(drawer.getByRole("button", { name: "套用並重新建立預覽" })).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-settings-drawer.png") });
    await page.keyboard.press("Escape");

    await page.getByText("全部 250").click();
    await expect(page.locator(".audio-name", { hasText: "same.ogg" })).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-all-filter.png") });

    await page.getByText("只看有變更 233").click();
    await page.locator(".audio-name", { hasText: "song.mp3" }).click();
    await expect(page.locator(".audio-detail .diff-ins").first()).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-list-detail.png") });

    await page.getByText("全部 250").click();
    await expect(page.locator(".audio-folder", { hasText: "/music/專輯甲" })).toBeVisible();
    await expect(page.locator(".audio-folder", { hasText: "/music/專輯乙" })).toBeVisible();
    await expect(page.locator(".audio-folder", { hasText: "/music/現場" })).toBeVisible();
    const list = page.locator(".audio-file-pane .virtual-list");
    await list.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    await expect(page.locator(".audio-folder", { hasText: "/music/合輯" })).toBeVisible();
    await list.evaluate((element) => {
      element.scrollTop = 0;
    });
    const album = page.locator(".audio-folder", { hasText: "/music/專輯甲" });
    await expect(album).toContainText("1 檔");
    await album.click();
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.screenshot({ path: join(shots, "audio-folder-collapsed.png") });
    await page.locator(".audio-file-pane").focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(".audio-folder.active")).toContainText("/music/專輯乙");
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(".audio-row.active .audio-name")).toHaveText("same.ogg");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("Enter");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toBeVisible();

    await page.getByRole("button", { name: "全不選" }).click();
    await album.click();
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.getByRole("button", { name: "全選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();

    await page.getByRole("button", { name: "確認寫入 233 個檔案" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.lastAudioApplyPaths?.length ?? -1))
      .toBe(233);
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.lastAudioApplyPaths ?? []))
      .not.toContain("/music/專輯乙/same.ogg");
  });
});

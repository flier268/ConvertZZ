import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import {
  openApp,
  openPage,
  scrollAudioFolder,
  scrollAudioListToEnd,
  scrollAudioRow,
} from "../helpers";

const shots = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "e2e-screenshots");

function segment(page: import("@playwright/test").Page, name: string) {
  return page.locator(".audio-segments .el-radio-button", { hasText: name });
}

test.describe("音訊標籤審閱", () => {
  test("篩選、統計、詳細面板、設定抽屜與確認筆數", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();
    await expect(page.getByText("已勾選 233 個檔案・467 個欄位")).toBeVisible();
    await expect(page.getByRole("radio", { name: "有變更 233" })).toBeChecked();
    await expect(segment(page, "有變更 233")).toHaveAttribute("aria-pressed", "true");
    await expect(segment(page, "不變 16")).toHaveAttribute("aria-pressed", "false");
    await expect(page.locator(".page-header")).toContainText("1 個路徑 · 全部 250 個檔案");
    await expect(page.getByRole("radio", { name: "不變 16" })).toBeVisible();
    await expect(page.getByRole("radio", { name: "錯誤 1" })).toBeVisible();
    await expect(page.getByRole("radio", { name: "全部 250" })).toBeVisible();
    await expect(page.getByText("只看有變更")).toHaveCount(0);
    await expect(page.getByText("只看錯誤")).toHaveCount(0);
    await expect.poll(async () => page.locator(".audio-row").count()).toBeLessThan(40);
    await expect(page.locator(".audio-row").first()).toBeVisible();
    await expect(page.getByText("track-000.mp3")).toHaveCount(0);

    await segment(page, "全部 250").click();
    await page.getByRole("button", { name: "全不選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 0 個檔案" })).toBeDisabled();
    await page.getByRole("button", { name: "全選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();
    await expect(page.getByText("已勾選 233 個檔案・467 個欄位")).toBeVisible();

    await segment(page, "有變更 233").click();
    await page.getByPlaceholder("搜尋檔名、專輯、標題").fill("same");
    await expect(page.locator(".audio-file-pane")).toContainText("沒有符合的檔案");
    await expect(page.locator(".audio-detail")).toContainText("沒有符合的檔案");
    await segment(page, "全部 250").click();
    await expect(page.locator(".audio-name", { hasText: "same.ogg" })).toBeVisible();
    await page.getByPlaceholder("搜尋檔名、專輯、標題").fill("");

    await segment(page, "錯誤 1").click();
    await expect(page.getByRole("radio", { name: "錯誤 1" })).toBeChecked();
    await expect(page.getByRole("radio", { name: "有變更 233" })).not.toBeChecked();
    const errorSegment = segment(page, "錯誤 1");
    await expect(errorSegment).toHaveClass(/is-filled/);
    await expect(errorSegment).toHaveClass(/is-alert/);
    const errorFill = errorSegment.locator(".el-radio-button__inner");
    await expect(errorFill).toHaveCSS("background-color", "rgb(192, 57, 43)");
    await expect(errorFill).toHaveCSS("color", "rgb(255, 255, 255)");
    await expect(errorFill).toHaveCSS("border-top-color", "rgb(192, 57, 43)");
    const errorBox = page.locator(".audio-row", { hasText: "broken.mp3" }).locator("input");
    await expect(errorBox).toBeDisabled();
    await expect(errorBox).not.toBeChecked();
    await page.getByRole("button", { name: "全選" }).click();
    await page.getByRole("button", { name: "反選目前篩選" }).click();
    await expect(errorBox).toBeDisabled();
    await expect(errorBox).not.toBeChecked();
    await page.locator(".audio-name", { hasText: "broken.mp3" }).click();
    const errorDetail = await page.locator(".audio-detail").innerText();
    expect(errorDetail.split("無法讀取標籤").length - 1).toBe(1);
    await expect(page.locator(".audio-detail")).toContainText("/music/現場/broken.mp3");
    await expect(page.locator(".audio-detail")).toContainText("不能勾選");
    await expect(page.getByRole("button", { name: "重新掃描這個檔案" })).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-error-selected.png") });

    await expect(page.getByText("APEv2／Vorbis 方向")).toBeVisible();
    await expect(page.getByText("ID3v1", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("ID3v2", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("轉換檔名")).toBeVisible();
    await expect(page.getByRole("button", { name: "編輯設定" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "套用並重新建立預覽" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "取消計畫" })).toHaveCount(0);
    await page.screenshot({ path: join(shots, "audio-settings-drawer.png") });

    await segment(page, "全部 250").click();
    await scrollAudioRow(page, "same.ogg");
    await expect(page.locator(".audio-name", { hasText: "same.ogg" })).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-all-filter.png") });

    await segment(page, "有變更 233").click();
    await scrollAudioRow(page, "song.mp3");
    await page.locator(".audio-name", { hasText: "song.mp3" }).click();
    await expect(page.getByRole("button", { name: "隱藏不變欄位" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(page.locator(".audio-detail")).toContainText("4 個變更");
    await expect(page.locator(".audio-id3v1-truncation")).toContainText(
      "ID3v1 標題超過 30 位元組，已截斷為 裡面的很長標題",
    );
    await expect(page.locator(".audio-detail .diff-ins").first()).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-list-detail.png") });
    await page.getByRole("button", { name: "快捷鍵說明" }).hover();
    await expect(page.getByText("↑/↓ 切換檔案")).toBeVisible();
    await expect(page.getByText("Enter / ←→ 摺疊或展開資料夾")).toBeVisible();
    await expect(page.getByText("空白 勾選")).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-shortcuts-tooltip.png") });

    await segment(page, "全部 250").click();
    const list = page.locator(".audio-file-pane .virtual-list");
    await expect(page.locator(".audio-folder", { hasText: "/music/合輯" })).toBeVisible();
    await scrollAudioListToEnd(page);
    await expect(page.locator(".audio-folder", { hasText: "/music/現場" })).toBeVisible();
    await scrollAudioFolder(page, "/music/專輯甲");
    const album = page.locator(".virtual-list-row .audio-folder", { hasText: "/music/專輯甲" });
    await expect(album).toContainText("1 檔");
    await album.dispatchEvent("click");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.screenshot({ path: join(shots, "audio-folder-collapsed.png") });
    await list.focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(".audio-folder.active")).toContainText("/music/現場");
    await page.keyboard.press("ArrowDown");
    await expect(page.locator(".audio-row.active .audio-name")).toHaveText("broken.mp3");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("ArrowUp");
    await page.keyboard.press("Enter");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toBeVisible();
    await page.keyboard.press("ArrowDown");
    const songBox = page.locator(".audio-row", { hasText: "song.mp3" }).locator("input");
    await expect(songBox).toBeChecked();
    await page.keyboard.press("Space");
    await expect(songBox).not.toBeChecked();
    await page.keyboard.press("Space");
    await expect(songBox).toBeChecked();
    await page.getByRole("button", { name: "快捷鍵說明" }).focus();
    await page.keyboard.press("Space");
    await expect(songBox).toBeChecked();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();

    await page.evaluate(() => {
      if (window.__convertzzE2e) window.__convertzzE2e.holdAudioRescan = true;
    });
    await segment(page, "錯誤 1").click();
    await page.getByRole("button", { name: "重新掃描這個檔案" }).click();
    await expect(page.getByRole("button", { name: "重新掃描這個檔案" })).toHaveClass(/is-loading/);
    await page.evaluate(() => window.__convertzzE2e?.releaseAudioRescan?.());
    await expect(page.getByRole("radio", { name: "錯誤 0" })).toBeVisible();
    await segment(page, "有變更 234").click();
    await expect(page.getByRole("radio", { name: "有變更 234" })).toBeChecked();
    await expect(page.locator(".audio-detail")).toContainText("夜曲繁");
    await expect(page.locator(".audio-detail")).not.toContainText("不能勾選");
    await page.screenshot({ path: join(shots, "audio-after-rescan.png") });
    await segment(page, "全部 250").click();

    await page.getByRole("button", { name: "全不選" }).click();
    await scrollAudioFolder(page, "/music/專輯甲");
    const folderCheck = page.locator(".virtual-list-row [aria-label='選取資料夾 /music/專輯甲']");
    await folderCheck.evaluate((element) => {
      const input = element as HTMLInputElement;
      input.checked = true;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.getByRole("button", { name: "確認寫入 1 個檔案" })).toBeEnabled();
    await page.screenshot({ path: join(shots, "audio-folder-selectall.png") });
    await album.dispatchEvent("click");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toHaveCount(0);
    await page.getByRole("button", { name: "全選" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 234 個檔案" })).toBeEnabled();

    await page.getByRole("button", { name: "確認寫入 234 個檔案" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.lastAudioApplyPaths?.length ?? -1))
      .toBe(234);
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.lastAudioApplyPaths ?? []))
      .not.toContain("/music/專輯乙/same.ogg");
  });

  test("預覽後取消勾選的檔案與欄位不會寫入", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await expect(page.getByRole("button", { name: "確認寫入 233 個檔案" })).toBeEnabled();

    await scrollAudioRow(page, "song.mp3");
    const song = page.locator(".audio-row", { hasText: "song.mp3" });
    await song.locator(".el-checkbox").click();
    await expect(page.getByRole("button", { name: "確認寫入 232 個檔案" })).toBeEnabled();

    await scrollAudioRow(page, "track-001.mp3");
    await page.locator(".audio-name", { hasText: "track-001.mp3" }).click();
    const album = page.locator(".audio-detail .el-table__row", { hasText: "專輯" });
    await album.locator(".el-checkbox").click();
    await expect(page.getByRole("button", { name: "確認寫入 232 個檔案" })).toBeEnabled();
    await expect(page.getByText("已勾選 232 個檔案・463 個欄位")).toBeVisible();

    await page.getByRole("button", { name: "確認寫入 232 個檔案" }).click();
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.lastAudioApply?.paths ?? []))
      .not.toContain("/music/專輯甲/song.mp3");
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            window.__convertzzE2e?.lastAudioApply?.selectedFields?.["/music/現場/track-001.mp3"] ??
            [],
        ),
      )
      .toEqual(["id3v2:title"]);
    await expect
      .poll(() => page.evaluate(() => window.__convertzzE2e?.confirms?.[0] ?? ""))
      .toContain("232");
  });

  test("尚未掃描、篩選是空的，以及掃描失敗", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page);
    await openPage(page, "#tour-audio", "音訊標籤");
    await expect(page.getByText("尚未掃描音訊檔案。請選取 MP3、APE、OGG 或 Opus。")).toBeVisible();
    await page.screenshot({ path: join(shots, "audio-empty.png") });

    await openApp(page, { failAudioScan: true, selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    const failure = page.locator(".el-result");
    await expect(failure).toContainText("掃描失敗");
    await expect(failure).toContainText("讀不到音訊標籤");
    await page.screenshot({ path: join(shots, "audio-error-state.png") });
  });

  test("可以依變更數排序資料夾裡的檔案", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await segment(page, "全部 250").click();
    await page.locator(".audio-sort").click();
    await page.getByRole("option", { name: "變更數" }).click();
    await expect(page.locator(".audio-folder").first()).toContainText("/music/合輯");
    await page.screenshot({ path: join(shots, "audio-sorted.png") });
  });

  test("即時預覽：隱藏不變欄位、串流、自動重建與設定抽屜", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await openApp(page, { selectedFiles: "/tmp/song.mp3", holdAudioScan: true });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await expect(page.locator(".audio-name", { hasText: "live-00.mp3" })).toBeVisible();
    await expect.poll(async () => page.locator(".audio-name").count()).toBeGreaterThan(5);
    await expect(page.locator(".job-progress")).toBeVisible();
    await expect(page.getByRole("button", { name: "建立標籤預覽" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "選取音訊檔案" })).toBeEnabled();
    await expect(page.getByRole("button", { name: "確認寫入" })).toHaveCount(0);
    await page.screenshot({ path: join(shots, "live-streaming.png") });
    await page.evaluate(() => window.__convertzzE2e?.releaseAudioScan?.());
    await expect(page.getByRole("button", { name: "重新載入" })).toBeEnabled();

    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await scrollAudioRow(page, "song.mp3");
    await page.locator(".audio-name", { hasText: "song.mp3" }).click();
    await expect(page.getByRole("button", { name: "隱藏不變欄位" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await expect(page.locator(".audio-detail")).not.toContainText("未知字幕");
    await page.screenshot({ path: join(shots, "live-hide-unchanged-on.png") });
    await page.getByRole("button", { name: "隱藏不變欄位" }).click();
    await expect(page.locator(".audio-detail")).toContainText("未知字幕");
    await page.screenshot({ path: join(shots, "live-hide-unchanged-off.png") });

    await page.screenshot({ path: join(shots, "live-drawer.png") });
    await page
      .locator(".el-form-item", { hasText: "APEv2／Vorbis 方向" })
      .locator(".el-select")
      .click();
    await page.getByRole("option", { name: "繁轉簡" }).click();
    await expect(page.locator(".el-form-item", { hasText: "APEv2／Vorbis 方向" })).toContainText(
      "繁轉簡",
    );
    await expect(page.getByRole("button", { name: "確認寫入" })).toBeEnabled();
    await page.screenshot({ path: join(shots, "live-auto-rebuild.png") });
  });
});

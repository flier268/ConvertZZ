import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { openApp, openPage } from "../helpers";

const shots = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "e2e-screenshots");

test("標籤操作規則、詳細來源與警告", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openApp(page, { selectedFiles: "/tmp/song.mp3" });
  await openPage(page, "#tour-audio", "音訊標籤");
  await page.getByRole("button", { name: "選取音訊檔案" }).click();
  await expect(page.getByRole("button", { name: /確認寫入/ })).toBeEnabled();

  await page.getByRole("button", { name: "編輯設定" }).click();
  const drawer = page.getByRole("dialog");
  await drawer.getByRole("button", { name: "舊播放器相容" }).click();
  await drawer.getByRole("button", { name: "清理多餘標籤" }).click();
  await expect(drawer.getByText("把 ID3v2 複製到 ID3v1")).toBeVisible();
  await expect(drawer.getByText("有的播放器偏好 APEv2")).toBeVisible();
  await expect(page.getByText("標籤操作 2 條")).toBeVisible();
  await expect(drawer.getByText("規則由上到下執行")).toBeVisible();
  await page.screenshot({ path: join(shots, "ops-rules-drawer.png") });

  await drawer.getByRole("button", { name: "完成", exact: true }).click();
  await expect(page.getByText("移除標籤").first()).toBeVisible();
  await expect(page.getByText("規則 1").first()).toBeVisible();
  await expect(page.getByText("1 字 Big5 無法表示")).toBeVisible();
  await page.screenshot({ path: join(shots, "ops-rules-in-detail.png") });

  await page.locator(".audio-segments .el-radio-button", { hasText: "警告 2" }).click();
  await expect(page.getByText("1 個檔案有截斷或缺字")).toBeVisible();
  await page.locator(".audio-name", { hasText: "ape-only.mp3" }).click();
  const removalBox = page.locator(".audio-detail .el-checkbox").first();
  await removalBox.click();
  await expect(page.getByRole("button", { name: "確認寫入 1 個檔案" })).toBeVisible();
  await page.screenshot({ path: join(shots, "ops-warnings.png") });
});

test("並排標籤欄挑選來源與移除確認", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openApp(page, { selectedFiles: "/tmp/song.mp3" });
  await openPage(page, "#tour-audio", "音訊標籤");
  await page.getByRole("button", { name: "選取音訊檔案" }).click();
  await expect(page.getByRole("button", { name: /確認寫入/ })).toBeEnabled();

  await page.locator(".audio-view-switch").getByText("並排標籤欄").click();
  await expect(page.getByRole("columnheader", { name: /APEv2/ })).toBeVisible();
  await page.getByRole("button", { name: "ID3v2 欄位選單" }).click();
  await expect(page.getByRole("menuitem", { name: "複製到… ID3v1" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "以此標籤覆蓋其他標籤" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "移除這個標籤…" })).toBeVisible();
  await page.screenshot({ path: join(shots, "ops-tag-columns.png") });

  await page.getByRole("radio", { name: "標題 採用 ID3v1" }).check();
  await expect(page.getByText("ID3v1 當來源時只補空白欄位。")).toBeVisible();
  await expect
    .poll(async () =>
      page.evaluate(
        () =>
          window.__convertzzE2e?.lastAudioPlan?.tagOperations?.some((operation) =>
            operation.paths?.includes("/music/專輯甲/song.mp3"),
          ) ?? false,
      ),
    )
    .toBe(true);

  await page.getByRole("button", { name: "APEv2 欄位選單" }).click();
  await page.getByRole("menuitem", { name: "移除這個標籤…" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("含 1 個隱藏欄位")).toBeVisible();
  await expect(dialog.getByText("只有這個檔案")).toBeVisible();
  await expect(dialog.getByRole("radio", { name: /已勾選的檔案/ })).toBeDisabled();
  await expect(dialog.getByRole("checkbox", { name: "移除前先補到 ID3v2" })).toBeEnabled();
  await page.screenshot({ path: join(shots, "ops-remove-confirm.png") });
  await dialog.getByRole("button", { name: "加入計畫：移除 APEv2" }).click();
  await expect
    .poll(async () =>
      page.evaluate(
        () =>
          window.__convertzzE2e?.lastAudioPlan?.tagOperations?.some(
            (operation) => operation.action === "remove" && operation.source === "apev2",
          ) ?? false,
      ),
    )
    .toBe(true);
});

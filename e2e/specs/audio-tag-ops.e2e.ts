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
  await page.getByRole("button", { name: "建立標籤預覽" }).click();
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

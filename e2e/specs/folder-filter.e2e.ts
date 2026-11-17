import { expect, test, type Page } from "@playwright/test";
import { openApp, openPage } from "../helpers";

const dialog = (page: Page) => page.locator(".folder-wizard");

async function pickThroughWizard(page: Page) {
  await page.getByRole("button", { name: "選取來源資料夾" }).click();
  await expect(dialog(page)).toBeVisible();
  await dialog(page).getByLabel("來源資料夾路徑").fill("/tmp/samples");
  await dialog(page).getByRole("button", { name: "下一步" }).click();
}

test("步驟式流程：資料夾 → profile → 預覽統計（含無副檔名）→ 加入，列表套用白名單", async ({
  page,
}) => {
  await openApp(page, { selectedDirectory: "/tmp/samples" });
  await openPage(page, "#nav-files", "檔案與檔名");
  await pickThroughWizard(page);
  await expect(dialog(page).getByTestId("profile-row")).toHaveCount(3);
  await expect(dialog(page).getByTestId("profile-row").first()).toContainText("常用文字檔案");
  await page.screenshot({ path: "test-results/screens/wizard-step2.png", fullPage: true });
  await dialog(page).getByRole("button", { name: "下一步" }).click();
  const table = dialog(page).getByRole("table", { name: "副檔名統計" });
  await expect(table).toContainText("（無副檔名）");
  await expect(table).toContainText(".bin");
  await page.screenshot({ path: "test-results/screens/wizard-step3.png", fullPage: true });
  await dialog(page).getByRole("button", { name: "加入 1 個檔案" }).click();
  const list = page.locator(".file-plan-list");
  await expect(list.getByText("/tmp/samples/里面.txt").first()).toBeVisible();
  await expect(list.getByText("DATA.BIN")).toHaveCount(0);
  await expect(page.locator(".folder-filter")).toContainText("副檔名篩選");
  await expect(page.locator(".folder-filter textarea")).toHaveCount(0);
});

test("記住上次 profile：第二次選資料夾直接到預覽，更換 profile 後列出全部檔案", async ({
  page,
}) => {
  await openApp(page, { selectedDirectory: "/tmp/samples" });
  await openPage(page, "#nav-files", "檔案與檔名");
  await pickThroughWizard(page);
  await dialog(page).getByRole("button", { name: "下一步" }).click();
  await dialog(page).getByRole("button", { name: "加入 1 個檔案" }).click();
  await expect(page.locator(".file-plan-list").getByText("里面.txt").first()).toBeVisible();

  await page.getByRole("button", { name: "選取來源資料夾" }).click();
  await expect(dialog(page)).toContainText("已套用上次的 profile「常用文字檔案」");
  await page.screenshot({ path: "test-results/screens/wizard-quick.png", fullPage: true });
  await dialog(page).getByRole("button", { name: "更換 profile" }).first().click();
  await dialog(page).getByTestId("profile-row").filter({ hasText: "所有檔案" }).click();
  await dialog(page).getByRole("button", { name: "下一步" }).click();
  await dialog(page).getByRole("button", { name: "加入 3 個檔案" }).click();
  const list = page.locator(".file-plan-list");
  await expect(list.getByText("/tmp/samples/DATA.BIN").first()).toBeVisible();
  await expect(list.getByText("/tmp/samples/README").first()).toBeVisible();
});

test("在流程中新增黑名單 profile：貼上 *.bin 後排除 .BIN", async ({ page }) => {
  await openApp(page, { selectedDirectory: "/tmp/samples" });
  await openPage(page, "#nav-files", "檔案與檔名");
  await pickThroughWizard(page);
  await dialog(page).getByRole("button", { name: "＋ 新增 profile" }).click();
  const editor = page.locator(".profile-editor-dialog");
  await editor.getByLabel("profile 名稱").fill("排除二進位");
  await editor.getByText("黑名單（排除這些）").click();
  await editor.getByLabel("新增副檔名").fill("*.bin;");
  await expect(editor.locator(".ext-tags .el-tag")).toHaveText([".bin"]);
  await page.screenshot({ path: "test-results/screens/profile-editor.png", fullPage: true });
  await editor.getByRole("button", { name: "儲存" }).click();
  await dialog(page).getByRole("button", { name: "下一步" }).click();
  await expect(dialog(page).getByRole("table", { name: "副檔名統計" })).toContainText("排除");
  await dialog(page).getByRole("button", { name: "加入 2 個檔案" }).click();
  const list = page.locator(".file-plan-list");
  await expect(list.getByText("/tmp/samples/README").first()).toBeVisible();
  await expect(list.getByText("DATA.BIN")).toHaveCount(0);
});

test("設定 › 檔案以 profile 表格取代 textarea，內建 profile 沒有刪除", async ({ page }) => {
  await openApp(page);
  await openPage(page, "#nav-settings", "設定");
  await page.getByRole("tab", { name: "檔案" }).click();
  const panel = page.getByTestId("filter-profiles");
  await expect(panel).toContainText("常用文字檔案");
  await expect(panel.locator("textarea")).toHaveCount(0);
  await expect(panel.locator("tr", { hasText: "常用文字檔案" }).getByText("刪除")).toHaveCount(0);
  await expect(panel.locator("tr", { hasText: "常用網頁文件" }).getByText("刪除")).toHaveCount(1);
  await expect(panel.getByText("每次都走完整流程")).toBeVisible();
  await page.screenshot({ path: "test-results/screens/settings-profiles.png", fullPage: true });
});

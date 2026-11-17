import { expect, test } from "@playwright/test";
import { emitAppEvent, e2eState, openApp, openPage, scrollAudioRow } from "../helpers";

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

  test("拖入檔案後會詢問動作並可進入檔案預覽", async ({ page }) => {
    await openApp(page);
    await emitAppEvent(page, "app://file-drop", {
      paths: ["/tmp/里面.txt", "/tmp/notes.txt"],
    });
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText("選擇轉換動作")).toBeVisible();
    await expect(dialog.getByText("已拖入 2 項")).toBeVisible();
    await expect(dialog.getByText("檔案與檔名轉換")).toBeVisible();
    await expect(dialog.getByText("音訊標籤")).toBeVisible();
    await dialog.getByText("轉換檔名", { exact: true }).click();
    await dialog.getByRole("button", { name: "繼續" }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("heading", { name: "檔案與檔名" })).toBeVisible();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "確認執行" })).toBeVisible();
  });

  test("切換到設定再回來會保留檔案預覽狀態", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "作業" }).locator(".el-select").click();
    await page.getByRole("option", { name: "轉換檔名" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();
    await openPage(page, "#nav-settings", "設定");
    await openPage(page, "#nav-files", "檔案與檔名");
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "確認執行" })).toBeVisible();
  });

  test("檔案清單與預覽可用分隔線調整寬度，表格欄也可拖曳", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "作業" }).locator(".el-select").click();
    await page.getByRole("option", { name: "轉換檔名" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();

    const statusHeader = page.locator(".el-table-v2__header-cell", { hasText: "狀態" });
    const statusResizer = statusHeader.locator(".file-plan-col-resizer");
    await expect(statusResizer).toBeAttached();
    const beforeHeader = await statusHeader.boundingBox();
    expect(beforeHeader).toBeTruthy();
    expect(beforeHeader!.width).toBeGreaterThan(60);
    const resized = await page.evaluate(async () => {
      const findStatus = () =>
        [...document.querySelectorAll(".el-table-v2__header-cell")].find((el) =>
          el.textContent?.includes("狀態"),
        );
      const status = findStatus();
      const resizer = status?.querySelector(".file-plan-col-resizer");
      if (!status || !resizer) return { before: 0, after: 0 };
      const before = status.getBoundingClientRect().width;
      const rect = resizer.getBoundingClientRect();
      const cx = rect.left + rect.width / 2;
      const cy = rect.top + rect.height / 2;
      resizer.dispatchEvent(
        new MouseEvent("mousedown", { bubbles: true, clientX: cx, clientY: cy }),
      );
      window.dispatchEvent(
        new MouseEvent("mousemove", { bubbles: true, clientX: cx + 48, clientY: cy }),
      );
      window.dispatchEvent(
        new MouseEvent("mouseup", { bubbles: true, clientX: cx + 48, clientY: cy }),
      );
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return { before, after: findStatus()?.getBoundingClientRect().width ?? 0 };
    });
    expect(resized.after).toBeGreaterThan(resized.before + 20);

    const layoutSplitter = page
      .locator(".file-plan-splitter .el-splitter-bar__dragger-horizontal")
      .first();
    await expect(layoutSplitter).toBeVisible();
    const listBox = page.locator(".file-plan-list");
    const beforeList = await listBox.boundingBox();
    expect(beforeList).toBeTruthy();
    const barBox = await layoutSplitter.boundingBox();
    expect(barBox).toBeTruthy();
    await page.mouse.move(barBox!.x + barBox!.width / 2, barBox!.y + barBox!.height / 2);
    await page.mouse.down();
    await page.mouse.move(barBox!.x + barBox!.width / 2 - 80, barBox!.y + barBox!.height / 2, {
      steps: 6,
    });
    await page.mouse.up();
    const afterList = await listBox.boundingBox();
    expect(afterList!.width).toBeLessThan(beforeList!.width - 20);

    await expect(
      page.locator(".file-plan-preview .el-splitter-bar__dragger-horizontal").first(),
    ).toBeVisible();
  });

  test("檔案頁窄視窗不橫向溢出，清單會填滿剩餘高度", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByText("變更預覽")).toBeVisible();
    await expect(page.getByRole("button", { name: "建立預覽" })).toBeVisible();
    await expect(page.locator(".el-form-item", { hasText: "作業" })).toBeVisible();

    const layoutMetrics = async () =>
      page.evaluate(() => {
        const content = document.querySelector(".content");
        const card = document.querySelector(".el-card");
        const grid = document.querySelector(".option-grid");
        const layout = document.querySelector(".file-plan-layout");
        if (!content || !card || !grid || !layout) return null;
        const contentRect = content.getBoundingClientRect();
        const layoutRect = layout.getBoundingClientRect();
        return {
          contentOverflowX: content.scrollWidth - content.clientWidth,
          cardOverflowX: card.scrollWidth - card.clientWidth,
          gridOverflowX: grid.scrollWidth - grid.clientWidth,
          layoutHeight: layoutRect.height,
          gapBelowLayout: contentRect.bottom - layoutRect.bottom,
          overflowY: content.scrollHeight - content.clientHeight,
        };
      });

    const wide = await layoutMetrics();
    expect(wide).toBeTruthy();
    expect(wide!.contentOverflowX).toBeLessThanOrEqual(2);
    expect(wide!.overflowY).toBeLessThanOrEqual(2);
    expect(wide!.layoutHeight).toBeGreaterThan(300);
    expect(wide!.gapBelowLayout).toBeLessThan(40);

    await page.setViewportSize({ width: 960, height: 640 });
    const narrow = await layoutMetrics();
    expect(narrow).toBeTruthy();
    expect(narrow!.contentOverflowX).toBeLessThanOrEqual(2);
    expect(narrow!.cardOverflowX).toBeLessThanOrEqual(2);
    expect(narrow!.gridOverflowX).toBeLessThanOrEqual(2);
    expect(narrow!.overflowY).toBeLessThanOrEqual(2);
    expect(narrow!.layoutHeight).toBeGreaterThanOrEqual(160);
    expect(narrow!.gapBelowLayout).toBeLessThan(40);
  });

  test("各工作頁不讓整窗內容過長", async ({ page }) => {
    await openApp(page);
    const pages = [
      { nav: "#nav-quick", heading: "快速轉換" },
      { nav: "#nav-clipboard", heading: "剪貼簿" },
      { nav: "#nav-tools", heading: "文字工具" },
      { nav: "#nav-dictionary", heading: "使用者詞典" },
      { nav: "#nav-settings", heading: "設定" },
      { nav: "#nav-about", heading: "ConvertZZ 2.0" },
      { nav: "#nav-audio", heading: "音訊標籤" },
    ] as const;
    for (const item of pages) {
      await openPage(page, item.nav, item.heading);
      const metrics = await page.evaluate(() => {
        const content = document.querySelector(".content");
        const fill = document.querySelector(".page-fill-main");
        if (!content || !fill) return null;
        const contentRect = content.getBoundingClientRect();
        const fillRect = fill.getBoundingClientRect();
        return {
          overflowY: content.scrollHeight - content.clientHeight,
          overflowX: content.scrollWidth - content.clientWidth,
          gap: contentRect.bottom - fillRect.bottom,
          fillHeight: fillRect.height,
        };
      });
      expect(metrics, item.heading).toBeTruthy();
      expect(metrics!.overflowY, item.heading).toBeLessThanOrEqual(2);
      expect(metrics!.overflowX, item.heading).toBeLessThanOrEqual(2);
      const gapLimit = item.nav === "#nav-audio" ? 120 : 48;
      expect(metrics!.gap, item.heading).toBeLessThan(gapLimit);
      expect(metrics!.fillHeight, item.heading).toBeGreaterThan(160);
    }
  });

  test("檔案轉換進度文字在進度條下方且停止按鈕與建立預覽並排", async ({ page }) => {
    await openApp(page, { holdFileApply: true });
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await page.getByRole("button", { name: "確認執行" }).click();
    const progress = page.locator(".job-progress");
    await expect(progress).toBeVisible();
    await expect(progress.locator(".el-progress__text")).toContainText("正在轉換並寫入");
    try {
      const bar = await progress.locator(".el-progress-bar").boundingBox();
      const text = await progress.locator(".el-progress__text").boundingBox();
      expect(bar).toBeTruthy();
      expect(text).toBeTruthy();
      expect(text!.y).toBeGreaterThan(bar!.y + bar!.height - 1);
      expect(text!.x).toBeLessThan(bar!.x + 24);

      const actions = page.locator(".path-summary-actions");
      const createBtn = actions.getByRole("button", { name: "建立預覽" });
      const stopBtn = actions.getByRole("button", { name: "停止作業" });
      const createBox = await createBtn.boundingBox();
      const stopBox = await stopBtn.boundingBox();
      expect(createBox).toBeTruthy();
      expect(stopBox).toBeTruthy();
      expect(Math.abs(createBox!.y - stopBox!.y)).toBeLessThan(8);
      expect(stopBox!.x).toBeGreaterThan(createBox!.x);
    } finally {
      await page.evaluate(() => window.__convertzzE2e?.releaseFileApply?.());
    }
  });

  test("拖入檔案後可改選音訊標籤", async ({ page }) => {
    await openApp(page);
    await emitAppEvent(page, "app://file-drop", { paths: ["/tmp/song.mp3"] });
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await dialog.getByText("音訊標籤", { exact: true }).click();
    await dialog.getByRole("button", { name: "繼續" }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("heading", { name: "音訊標籤" })).toBeVisible();
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toBeVisible();
    await expect(page.getByRole("button", { name: "重新載入" })).toBeVisible();
  });

  test("拖放詢問會記住上次選項", async ({ page }) => {
    await openApp(page);
    await emitAppEvent(page, "app://file-drop", { paths: ["/tmp/remember.txt"] });
    let dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await dialog.getByText("轉換檔名", { exact: true }).click();
    await dialog.getByText("繁轉簡", { exact: true }).click();
    await dialog.getByRole("button", { name: "繼續" }).click();
    await expect(dialog).toBeHidden();

    await emitAppEvent(page, "app://file-drop", { paths: ["/tmp/remember-again.txt"] });
    dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("radio", { name: "轉換檔名" })).toBeChecked();
    await expect(dialog.getByRole("radio", { name: "繁轉簡" })).toBeChecked();
    await dialog.getByRole("button", { name: "取消" }).click();
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
          "覆寫會取代既有的同名檔案與 .bak 備份。是否確定繼續？",
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
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "作業" }).locator(".el-select").click();
    await page.getByRole("option", { name: "轉換檔名" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByText("變更預覽")).toBeVisible();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/裡面.txt", { exact: true }),
    ).toBeVisible();
    await expect(page.locator(".file-plan-preview .preview-diff-body").nth(0)).toContainText(
      "里面.txt",
    );
    await expect(page.locator(".file-plan-preview .preview-diff-body").nth(1)).toContainText(
      "裡面.txt",
    );
    await expect(page.locator(".file-plan-preview .diff-remove").first()).toBeVisible();
    await expect(page.locator(".file-plan-preview .diff-add").first()).toBeVisible();
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
    await expect(page.getByRole("columnheader", { name: "macOS" })).toBeVisible();
  });

  test("1.x 字典設定遷移後提示前往使用者詞典，關閉後記住", async ({ page }) => {
    await openApp(page, { legacyDictionaryNotice: true });
    const notice = page.getByText("舊版字典已改為使用者詞典");
    await expect(notice).toBeVisible();
    await notice.click();
    await expect(page.getByRole("heading", { level: 1, name: "使用者詞典" })).toBeVisible();
    await expect(notice).toBeHidden();
    await expect
      .poll(async () => (await e2eState(page)).savedSettings?.legacyDictionaryNotice)
      .toBe(false);
  });

  test("使用者詞典匯入 1.x 字典先警告，確認後標上來源並立即儲存", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/我的舊字典.csv" });
    await openPage(page, "#tour-dictionary", "使用者詞典");
    await expect(page.getByRole("button", { name: "儲存", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "匯入", exact: true }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual([expect.stringContaining("不建議直接匯入")]);
    const dialog = page.getByRole("dialog", { name: "匯入詞典" });
    await expect(dialog.getByText("來源詞只能是中日韓文字。")).toBeVisible();
    await expect(dialog.getByText("1.x 舊版字典，共")).toBeVisible();
    await dialog.getByRole("button", { name: "匯入並儲存" }).click();
    await expect
      .poll(async () => (await e2eState(page)).lastDictionarySave)
      .toEqual([
        { simplified: "优盘", traditional: "隨身碟", direction: "s2t" },
        {
          simplified: "信息",
          traditional: "資訊",
          direction: "both",
          sourceFile: "我的舊字典.csv",
          importedAt: "2026-11-17T12:00:00+08:00",
        },
      ]);
    await expect(
      page.locator(".dictionary-source span", { hasText: "我的舊字典.csv" }),
    ).toBeVisible();
    await expect(page.getByRole("button", { name: "儲存", exact: true })).toBeDisabled();
  });

  test("使用者詞典取消舊版警告時不匯入", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/Dictionary.csv", confirmResult: false });
    await openPage(page, "#tour-dictionary", "使用者詞典");
    await page.getByRole("button", { name: "匯入", exact: true }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual([expect.stringContaining("不建議直接匯入")]);
    await expect(page.getByRole("dialog", { name: "匯入詞典" })).toBeHidden();
    expect((await e2eState(page)).lastDictionarySave).toBeUndefined();
  });

  test("使用者詞典匯入本格式不警告，匯出寫到選擇的路徑", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/隨便的名字.txt", savePath: "/tmp/備份.tsv" });
    await openPage(page, "#tour-dictionary", "使用者詞典");
    await page.getByRole("button", { name: "匯入", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "匯入詞典" });
    await expect(dialog.getByText("使用者詞典，共")).toBeVisible();
    expect((await e2eState(page)).confirms).toEqual([]);
    await dialog.getByRole("button", { name: "匯入並儲存" }).click();
    await expect(
      page.locator(".dictionary-source span", { hasText: "隨便的名字.txt" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "匯出", exact: true }).click();
    await expect
      .poll(async () => (await e2eState(page)).lastDictionaryExport?.path)
      .toBe("/tmp/備份.tsv");
    expect((await e2eState(page)).lastDictionaryExport?.entries).toHaveLength(2);
  });

  test("使用者詞典全選只選篩選結果，可多選刪除", async ({ page }) => {
    await openApp(page, { dictionaryEntryCount: 200 });
    await openPage(page, "#tour-dictionary", "使用者詞典");
    await expect(page.getByText("201 / 201 筆")).toBeVisible();
    const deleteSelected = page.getByRole("button", { name: /^刪除選取/ });
    await expect(deleteSelected).toBeDisabled();

    await page.getByRole("combobox", { name: "來源篩選" }).click();
    await page.getByRole("option", { name: "大量.txt" }).click();
    await expect(page.getByText("100 / 201 筆")).toBeVisible();
    await page.locator(".dictionary-table-host .el-table-v2__header-cell .el-checkbox").click();
    await expect(deleteSelected).toHaveText("刪除選取（100）");
    await page.evaluate(() => {
      window.__convertzzE2e!.confirmResult = false;
    });
    await deleteSelected.click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual([expect.stringContaining("要從表格刪除選取的 100 筆詞條嗎")]);
    await expect(page.getByText("100 / 201 筆")).toBeVisible();
    await expect(deleteSelected).toHaveText("刪除選取（100）");
    await page.evaluate(() => {
      window.__convertzzE2e!.confirmResult = true;
    });
    await deleteSelected.click();
    // 該來源已無詞條，來源篩選回到全部；其他來源的詞條不受影響。
    await expect(page.getByText("101 / 101 筆")).toBeVisible();
    const rowChecks = page.locator(".dictionary-table-host .el-table-v2__row .el-checkbox");
    await rowChecks.nth(0).click();
    await rowChecks.nth(1).click();
    await expect(deleteSelected).toHaveText("刪除選取（2）");
    await page.getByPlaceholder("搜尋簡體或繁體").fill("词1");
    await expect(deleteSelected).toBeDisabled();
    await page.getByPlaceholder("搜尋簡體或繁體").fill("");
    await expect(page.getByText("101 / 101 筆")).toBeVisible();
    await page.getByRole("button", { name: "刪除", exact: true }).first().click();
    await expect
      .poll(async () => (await e2eState(page)).confirms?.at(-1))
      .toContain("要從表格刪除「优盘／隨身碟」嗎");
    await expect(page.getByText("100 / 100 筆")).toBeVisible();
    await expect(page.getByRole("button", { name: "儲存", exact: true })).toBeEnabled();
  });

  test("使用者詞典 5 萬筆只渲染可見列，並可依來源、方向與關鍵字篩選", async ({ page }) => {
    await openApp(page, { dictionaryEntryCount: 50_000 });
    await openPage(page, "#tour-dictionary", "使用者詞典");
    await expect(page.getByText("50,001 / 50,001 筆")).toBeVisible();
    const renderedRows = await page.locator(".dictionary-table-host .el-table-v2__row").count();
    expect(renderedRows).toBeGreaterThan(0);
    expect(renderedRows).toBeLessThan(100);

    await page.getByRole("combobox", { name: "來源篩選" }).click();
    await page.getByRole("option", { name: "手動加入" }).click();
    await expect(page.getByText("25,001 / 50,001 筆")).toBeVisible();
    await page.getByRole("combobox", { name: "來源篩選" }).click();
    await page.getByRole("option", { name: "大量.txt" }).click();
    await expect(page.getByText("25,000 / 50,001 筆")).toBeVisible();
    await page.getByPlaceholder("搜尋簡體或繁體").fill("詞4999");
    await expect(page.getByText("6 / 50,001 筆")).toBeVisible();

    const firstInput = page
      .locator(".dictionary-table-host .el-table-v2__row input.el-input__inner")
      .first();
    await firstInput.fill("詞改");
    await expect(page.getByText("6 / 50,001 筆")).toBeVisible();
    await expect(page.getByRole("button", { name: "儲存", exact: true })).toBeEnabled();
  });

  test("E-01 內容預覽會顯示來源與輸出文字", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByText("變更預覽")).toBeVisible();
    await expect(page.getByText("檔案預覽")).toBeVisible();
    await expect(page.locator(".file-plan-preview .preview-diff-body").nth(0)).toContainText(
      "里面开发头发",
    );
    await expect(page.locator(".file-plan-preview .preview-diff-body").nth(1)).toContainText(
      "裡面開發頭髮",
    );
    await expect(page.locator(".file-plan-preview .diff-remove").first()).toBeVisible();
    await expect(page.locator(".file-plan-preview .diff-add").first()).toBeVisible();
  });

  test("檔案預覽可開全視窗並翻頁、跳上下差異", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByRole("button", { name: "全視窗預覽" })).toBeEnabled();
    await page.getByRole("button", { name: "全視窗預覽" }).click();
    const dialog = page.getByRole("dialog", { name: "檔案差異預覽" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button", { name: "下一個差異" })).toBeVisible();
    await expect(dialog.getByText(/\d+ \/ \d+ 頁/)).toBeVisible();
    await dialog.getByRole("button", { name: "下一個差異" }).click();
    await expect(dialog.locator("mark.diff-change.is-active").first()).toBeVisible();
    await dialog.locator(".el-pager li").filter({ hasText: /^2$/ }).click();
    await expect(dialog.getByText(/2 \/ \d+ 頁/)).toBeVisible();
  });

  test("快速轉換固定並排差異檢視", async ({ page }) => {
    await openApp(page);
    await expect(page.getByRole("heading", { name: "快速轉換" })).toBeVisible();
    await expect(page.getByText("轉換差異")).toBeVisible();
    await page.getByPlaceholder("在此輸入或貼上文字").fill("里面开发头发");
    await page.getByRole("button", { name: "開始轉換" }).click();
    await expect(page.locator(".preview-diff-body").nth(1)).toContainText("裡面開發頭髮");
    await expect(page.locator(".diff-add").first()).toBeVisible();
    await expect(page.getByPlaceholder("在此輸入或貼上文字")).toHaveValue("里面开发头发");
  });

  test("剪貼簿轉換固定並排差異檢視", async ({ page }) => {
    await openApp(page, { clipboardText: "里面开发头发" });
    await openPage(page, "#tour-clipboard", "剪貼簿");
    await expect(page.getByText("轉換差異")).toBeVisible();
    await page.getByRole("button", { name: "立即讀取" }).click();
    await expect(page.getByPlaceholder("剪貼簿文字會顯示於此")).toHaveValue("里面开发头发");
    await expect(page.locator(".preview-diff-body").nth(1)).toContainText("裡面開發頭髮");
    await expect(page.locator(".diff-add").first()).toBeVisible();
  });

  test("E-03 取消計畫不會進入確認寫入", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#tour-files", "檔案與檔名");
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(page.getByRole("button", { name: "確認執行" })).toBeVisible();
    await page.getByRole("button", { name: "取消計畫" }).click();
    await expect(page.getByRole("button", { name: "確認執行" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "建立預覽" })).toBeVisible();
  });

  test("內容與檔名預覽會同時顯示路徑與內容", async ({ page }) => {
    await openApp(page);
    await openPage(page, "#nav-files", "檔案與檔名");
    await page.locator(".el-form-item", { hasText: "作業" }).locator(".el-select").click();
    await page.getByRole("option", { name: "內容與檔名" }).click();
    await page.getByRole("button", { name: "選取檔案" }).click();
    await page.getByRole("button", { name: "建立預覽" }).click();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/里面.txt", { exact: true }),
    ).toBeVisible();
    await expect(
      page.locator(".file-plan-list").getByText("/tmp/裡面.txt", { exact: true }),
    ).toBeVisible();
    const preview = page.locator(".file-plan-preview");
    await expect(preview.getByText("檔名", { exact: true })).toBeVisible();
    await expect(preview.getByText("內容", { exact: true })).toBeVisible();
    await expect(preview.locator(".preview-diff-body").nth(0)).toContainText("里面.txt");
    await expect(preview.locator(".preview-diff-body").nth(1)).toContainText("裡面.txt");
    await expect(preview.locator(".preview-diff-body").nth(2)).toContainText("里面开发头发");
    await expect(preview.locator(".preview-diff-body").nth(3)).toContainText("裡面開發頭髮");
  });

  test("音訊頁可掃描、預覽並在確認後寫入", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toBeVisible();
    await expect(page.getByText("含封面")).toBeVisible();
    await expect(page.getByRole("cell", { name: "里面", exact: true })).toBeVisible();
    await expect(page.getByRole("cell", { name: "未知字幕", exact: true })).toHaveCount(0);
    await expect(page.getByText("ID3v2 版本")).toBeVisible();
    await expect(page.getByRole("columnheader", { name: "轉換預覽" })).toBeVisible();
    await expect(page.locator(".audio-detail .diff-ins").first()).toContainText("裡");
    await expect(page.locator(".audio-detail .diff-del").first()).toContainText("里");
    await page.getByRole("button", { name: "確認寫入" }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual(expect.arrayContaining([expect.stringContaining("將寫入")]));
    await expect(page.getByRole("button", { name: "建立標籤預覽" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "重新掃描" })).toBeVisible();
  });

  test("音訊備份覆寫需要額外確認", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.locator(".el-form-item", { hasText: "備份衝突" }).locator(".el-select").click();
    await page.getByRole("option", { name: "覆寫" }).click();
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await page.getByRole("button", { name: "確認寫入" }).click();
    await expect
      .poll(async () => (await e2eState(page)).confirms ?? [])
      .toEqual(
        expect.arrayContaining([
          expect.stringContaining("將寫入"),
          "覆寫會取代既有的 .bak 備份。是否確定繼續？",
        ]),
      );
  });

  test("音訊頁沒有取消計畫，預覽會留著", async ({ page }) => {
    await openApp(page, { selectedFiles: "/tmp/song.mp3" });
    await openPage(page, "#tour-audio", "音訊標籤");
    await page.getByRole("button", { name: "選取音訊檔案" }).click();
    await expect(page.getByRole("button", { name: "確認寫入" })).toBeVisible();
    await expect(page.getByRole("button", { name: "取消計畫" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "重新載入" })).toBeVisible();
    await expect(page.getByRole("button", { name: "建立標籤預覽" })).toHaveCount(0);
    await scrollAudioRow(page, "song.mp3");
    await expect(page.locator(".audio-name", { hasText: "song.mp3" })).toBeVisible();
  });
});

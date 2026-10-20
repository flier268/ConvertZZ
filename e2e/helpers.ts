import { expect, type Page } from "@playwright/test";
import type { ConvertzzE2eConfig } from "./mocks/tauri";

export async function openApp(page: Page, config: ConvertzzE2eConfig = {}): Promise<void> {
  await page.addInitScript((value) => {
    window.__convertzzE2e = { confirms: [], ...value };
  }, config);
  await page.goto("/");
  await expect(page.locator(".runtime-status")).not.toHaveText("核心啟動中", { timeout: 15_000 });
}

export async function e2eState(page: Page): Promise<ConvertzzE2eConfig> {
  return page.evaluate(() => window.__convertzzE2e ?? {});
}

const navLabels: Record<string, string> = {
  "#nav-quick": "快速轉換",
  "#nav-files": "檔案與檔名",
  "#nav-clipboard": "剪貼簿",
  "#nav-audio": "音訊標籤",
  "#nav-tools": "文字工具",
  "#nav-dictionary": "舊版字典",
  "#nav-settings": "設定",
  "#nav-about": "關於與差異",
};

/** 畫面上的第一列已對上 scrollTop。Vue 還沒改寫列之前，舊視窗的列還留在 DOM。 */
async function audioWindowCaughtUp(list: ReturnType<Page["locator"]>): Promise<boolean> {
  return list.evaluate((element) => {
    const row = element.querySelector(".virtual-list-row");
    if (!(row instanceof HTMLElement)) return false;
    const match = /translateY\(([-\d.]+)px\)/.exec(row.style.transform);
    if (!match) return false;
    const y = Number(match[1]);
    return y <= element.scrollTop && y >= element.scrollTop - 280;
  });
}

async function audioRowInView(
  list: ReturnType<Page["locator"]>,
  row: ReturnType<Page["locator"]>,
): Promise<boolean> {
  if (!(await audioWindowCaughtUp(list))) return false;
  if ((await row.count()) === 0) return false;
  return row.first().evaluate((element) => {
    const host = element.closest(".virtual-list");
    if (!(host instanceof HTMLElement)) return false;
    const rowBox = element.getBoundingClientRect();
    const hostBox = host.getBoundingClientRect();
    return rowBox.bottom > hostBox.top + 1 && rowBox.top < hostBox.bottom - 1;
  });
}

/**
 * 虛擬清單只掛載可視列。每次捲動都要等該視窗畫出來再往下走，
 * 否則還停在上一屏的列會被誤認成已經找到。
 */
async function scrollVirtualUntil(
  list: ReturnType<Page["locator"]>,
  row: ReturnType<Page["locator"]>,
): Promise<void> {
  let wrapped = false;
  await expect
    .poll(
      async () => {
        if (await audioRowInView(list, row)) return true;
        if (!(await audioWindowCaughtUp(list))) return false;
        const action = await list.evaluate((element) => {
          const max = Math.max(0, element.scrollHeight - element.clientHeight);
          const step = Math.max(40, element.clientHeight - 80);
          if (element.scrollTop >= max - 1) {
            element.scrollTop = 0;
            return "wrapped";
          }
          element.scrollTop = Math.min(max, element.scrollTop + step);
          return "moved";
        });
        if (action === "wrapped") {
          if (wrapped) return false;
          wrapped = true;
        }
        return false;
      },
      { timeout: 15_000 },
    )
    .toBe(true);
}

export async function scrollAudioRow(page: Page, name: string): Promise<void> {
  const list = page.locator(".audio-file-pane .virtual-list");
  const row = page.locator(".virtual-list-row .audio-name", { hasText: name });
  await scrollVirtualUntil(list, row);
  await row.first().scrollIntoViewIfNeeded();
}

export async function scrollAudioFolder(page: Page, name: string): Promise<void> {
  const list = page.locator(".audio-file-pane .virtual-list");
  const row = page.locator(".virtual-list-row .audio-folder", { hasText: name });
  await scrollVirtualUntil(list, row);
  await expect(row).toBeVisible();
}

export async function scrollAudioListToEnd(page: Page): Promise<void> {
  const list = page.locator(".audio-file-pane .virtual-list");
  await list.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  await expect.poll(() => audioWindowCaughtUp(list), { timeout: 15_000 }).toBe(true);
}

export async function openPage(page: Page, id: string, heading: string): Promise<void> {
  const label = navLabels[id];
  if (label) await page.getByRole("menuitem", { name: label }).click();
  else await page.locator(id).click();
  await expect(page.getByRole("heading", { name: heading })).toBeVisible();
}

export async function emitAppEvent(page: Page, event: string, payload?: unknown): Promise<void> {
  await page.evaluate(
    async ([name, data]) => {
      const emit = window.__convertzzEmit;
      if (!emit) throw new Error("e2e Tauri emit mock is missing");
      await emit(name, data);
    },
    [event, payload] as const,
  );
}

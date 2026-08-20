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

export async function openPage(page: Page, id: string, heading: string): Promise<void> {
  await page.locator(id).click();
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

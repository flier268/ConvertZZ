import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  importStepIndex,
  importStepNextLabel,
  ONBOARDING_STEPS,
  pageForOnboardingStep,
} from "./onboarding";

describe("第一次啟動導覽", () => {
  it("包含匯入舊版設定的步驟", () => {
    expect(ONBOARDING_STEPS.map((step) => step.id)).toContain("import");
    expect(importStepIndex()).toBeGreaterThan(0);
    expect(pageForOnboardingStep(importStepIndex())).toBe("settings");
  });

  it("依步驟切換對應頁面", () => {
    expect(pageForOnboardingStep(1)).toBe("quick");
    expect(pageForOnboardingStep(2)).toBe("files");
    expect(pageForOnboardingStep(4)).toBe("audio");
    expect(pageForOnboardingStep(99)).toBe("settings");
  });

  it("匯入失敗時會在導覽畫面顯示錯誤", () => {
    const tour = readFileSync(
      fileURLToPath(new URL("../OnboardingTour.vue", import.meta.url)),
      "utf8",
    );
    expect(tour).toContain("importError");
    expect(tour).toContain("onboarding-import-error");
    expect(tour).toContain("importFailureMessage");
  });

  it("匯入成功後按鈕改為下一步，未匯入時維持略過", () => {
    expect(importStepNextLabel(false)).toBe("略過匯入");
    expect(importStepNextLabel(true)).toBe("下一步");
    const tour = readFileSync(
      fileURLToPath(new URL("../OnboardingTour.vue", import.meta.url)),
      "utf8",
    );
    expect(tour).toContain("importStepNextLabel(settingsImported)");
    expect(tour).not.toContain("children: '略過匯入'");
  });

  it("開始導覽時會顯示主視窗，避免開在已隱藏的視窗上", () => {
    const tour = readFileSync(
      fileURLToPath(new URL("../OnboardingTour.vue", import.meta.url)),
      "utf8",
    );
    const app = readFileSync(fileURLToPath(new URL("../App.vue", import.meta.url)), "utf8");
    expect(tour).toContain('invoke("show_main_window")');
    expect(app).toContain("showForOnboarding");
    expect(app).toContain("isOnboardingComplete");
  });
});

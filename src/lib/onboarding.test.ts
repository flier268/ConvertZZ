import { describe, expect, it } from "vitest";
import { importStepIndex, ONBOARDING_STEPS, pageForOnboardingStep } from "./onboarding";

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
});

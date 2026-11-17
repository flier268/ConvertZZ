export const ONBOARDING_STORE_KEY = "onboardingCompleted";

export const ONBOARDING_STEPS = [
  { id: "welcome", page: "quick", title: "歡迎使用 ConvertZZ 2.0" },
  { id: "quick", page: "quick", title: "快速轉換" },
  { id: "files", page: "files", title: "檔案與檔名" },
  { id: "clipboard", page: "clipboard", title: "剪貼簿" },
  { id: "audio", page: "audio", title: "音訊標籤" },
  { id: "tools", page: "tools", title: "文字工具" },
  { id: "desktop", page: "quick", title: "浮動球與托盤" },
  { id: "import", page: "settings", title: "匯入舊版設定" },
  { id: "settings", page: "settings", title: "設定與快捷鍵" },
] as const;

export type OnboardingPage = (typeof ONBOARDING_STEPS)[number]["page"];

export function pageForOnboardingStep(index: number): OnboardingPage {
  return (
    ONBOARDING_STEPS[Math.min(Math.max(index, 0), ONBOARDING_STEPS.length - 1)]?.page ?? "quick"
  );
}

export function importStepIndex(): number {
  return ONBOARDING_STEPS.findIndex((step) => step.id === "import");
}

export function importStepNextLabel(imported: boolean): string {
  return imported ? "下一步" : "略過匯入";
}

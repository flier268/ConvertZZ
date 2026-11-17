import { t } from "@/i18n";

export const ONBOARDING_STORE_KEY = "onboardingCompleted";

export const ONBOARDING_STEPS = [
  { id: "welcome", page: "quick" },
  { id: "quick", page: "quick" },
  { id: "files", page: "files" },
  { id: "clipboard", page: "clipboard" },
  { id: "audio", page: "audio" },
  { id: "tools", page: "tools" },
  { id: "desktop", page: "quick" },
  { id: "import", page: "settings" },
  { id: "settings", page: "settings" },
] as const;

export type OnboardingStepId = (typeof ONBOARDING_STEPS)[number]["id"];

export function onboardingStepTitle(id: OnboardingStepId): string {
  return t(`lib.onboarding.steps.${id}`);
}

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
  return imported ? t("lib.onboarding.next") : t("lib.onboarding.skipImport");
}

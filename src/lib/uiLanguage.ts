import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UiLanguage } from "@shared/contracts";
import { resolveLocale, setLocale, type Locale } from "@/i18n";

const UI_LANGUAGE_EVENT = "app://ui-language";

/** 主視窗套用介面語言：更新畫面、通知 Rust（托盤選單與核心訊息），並同步其他視窗。 */
export async function applyUiLanguage(language: UiLanguage | undefined): Promise<Locale> {
  const next = resolveLocale(language);
  setLocale(next);
  await invoke("set_ui_language", { locale: next }).catch(() => undefined);
  await emit(UI_LANGUAGE_EVENT, next).catch(() => undefined);
  return next;
}

export function listenUiLanguage(): Promise<UnlistenFn> {
  return listen<Locale>(UI_LANGUAGE_EVENT, ({ payload }) => {
    if (payload === "zh-TW" || payload === "en") setLocale(payload);
  });
}

import { ref } from "vue";
import type { UiLanguage } from "@shared/contracts";
import zhTW from "./zh-TW";
import en from "./en";
import type { MessagePath } from "./types";

export type Locale = "zh-TW" | "en";
export type MessageKey = MessagePath<typeof zhTW>;
export type MessageParams = Record<string, string | number>;

type MessageTree = { [key: string]: string | MessageTree };

const catalogs: Record<Locale, MessageTree> = { "zh-TW": zhTW, en };

/** 預設繁體中文；只有載入設定後才依 `language` 切換，測試與無設定情境維持繁中。 */
export const locale = ref<Locale>("zh-TW");

export function resolveLocale(
  language: UiLanguage | undefined,
  systemLanguages?: readonly string[],
): Locale {
  if (language === "zh-TW" || language === "en") return language;
  const languages = systemLanguages ?? systemUiLanguages();
  if (!languages.length) return "zh-TW";
  return languages[0].toLowerCase().startsWith("zh") ? "zh-TW" : "en";
}

function systemUiLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  if (navigator.languages?.length) return navigator.languages;
  return navigator.language ? [navigator.language] : [];
}

export function setLocale(next: Locale): void {
  locale.value = next;
  if (typeof document !== "undefined") document.documentElement.lang = next;
}

function lookup(tree: MessageTree, key: string): string | undefined {
  let node: string | MessageTree | undefined = tree;
  for (const part of key.split(".")) {
    if (node === undefined || typeof node === "string") return undefined;
    node = node[part];
  }
  return typeof node === "string" ? node : undefined;
}

function interpolate(template: string, params?: MessageParams): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

/** 取得目前語系的訊息；`{name}` 以 params 代入。在 template／computed 內呼叫會隨語系切換更新。 */
export function t(key: MessageKey, params?: MessageParams): string {
  const template = lookup(catalogs[locale.value], key) ?? lookup(catalogs["zh-TW"], key) ?? key;
  return interpolate(template, params);
}

/**
 * 依數量選擇單複數。訊息以「 | 」分隔單數與複數（英文用）；沒有分隔時兩者相同（中文）。
 * `{count}` 會自動代入。
 */
export function tn(key: MessageKey, count: number, params?: MessageParams): string {
  const forms = t(key, { count, ...params }).split(" | ");
  return forms.length > 1 && count !== 1 ? forms[1] : forms[0];
}

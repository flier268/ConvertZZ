import { afterEach, describe, expect, it } from "vitest";
import zhTW from "./zh-TW";
import en from "./en";
import { locale, resolveLocale, setLocale, t } from "./index";

type Tree = { [key: string]: string | Tree };

function keyPaths(tree: Tree, prefix = ""): string[] {
  return Object.entries(tree).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === "string" ? [path] : keyPaths(value, path);
  });
}

function placeholders(tree: Tree): Record<string, string[]> {
  return Object.fromEntries(
    keyPaths(tree).map((path) => {
      const text = path
        .split(".")
        .reduce<string | Tree>((node, part) => (node as Tree)[part], tree);
      return [path, [...new Set((text as string).match(/\{\w+\}/g) ?? [])].sort()];
    }),
  );
}

afterEach(() => setLocale("zh-TW"));

describe("i18n", () => {
  it("英文與繁中訊息鍵一致", () => {
    expect(keyPaths(en).sort()).toEqual(keyPaths(zhTW).sort());
  });

  it("英文與繁中使用相同的代入參數", () => {
    expect(placeholders(en)).toEqual(placeholders(zhTW));
  });

  it("跟隨系統時中文系統用繁體中文，其餘用英文", () => {
    expect(resolveLocale("system", ["zh-CN", "en-US"])).toBe("zh-TW");
    expect(resolveLocale("system", ["zh-Hant-TW"])).toBe("zh-TW");
    expect(resolveLocale("system", ["en-US", "zh-TW"])).toBe("en");
    expect(resolveLocale("system", ["ja-JP"])).toBe("en");
    expect(resolveLocale("en", ["zh-TW"])).toBe("en");
    expect(resolveLocale("zh-TW", ["en-US"])).toBe("zh-TW");
  });

  it("預設為繁體中文，切換後回傳英文並代入參數", () => {
    expect(locale.value).toBe("zh-TW");
    expect(t("common.cancel")).toBe("取消");
    setLocale("en");
    expect(t("common.cancel")).toBe("Cancel");
    expect(t("common.direction.s2t")).toBe("Simplified → Traditional");
  });
});

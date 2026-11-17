import { t } from "@/i18n";

/** 直接指定版本控制資料夾（.git 等）時，後端略過並在計畫的 `ignoredInputs` 列出。 */

const IGNORED_INPUT_WARNING_PREFIX_ZH = "已略過版本控制資料夾內的路徑：";

function isIgnoredInputWarning(warning: string): boolean {
  return (
    warning.startsWith(IGNORED_INPUT_WARNING_PREFIX_ZH) ||
    warning.startsWith(t("lib.ignoredInputs.warningPrefix"))
  );
}

export function ignoredInputsConfirmMessage(ignored: string[]): string {
  const listed = ignored.slice(0, 5).join("\n");
  const more = ignored.length > 5 ? t("lib.ignoredInputs.more", { count: ignored.length }) : "";
  return t("lib.ignoredInputs.confirm", { paths: listed, more });
}

export function withoutIgnoredInputs(paths: string[], ignored: string[]): string[] {
  const set = new Set(ignored);
  return paths.filter((path) => !set.has(path));
}

/** 使用者已在確認視窗取消時，不必再顯示後端的略過警告。 */
export function dropIgnoredInputWarnings(warnings: string[], ignored: string[]): string[] {
  return warnings.filter(
    (warning) =>
      !(isIgnoredInputWarning(warning) && ignored.some((path) => warning.includes(path))),
  );
}

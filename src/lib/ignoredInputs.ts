/** 直接指定版本控制資料夾（.git 等）時，後端略過並在計畫的 `ignoredInputs` 列出。 */

const IGNORED_INPUT_WARNING_PREFIX = "已略過版本控制資料夾內的路徑：";

export function ignoredInputsConfirmMessage(ignored: string[]): string {
  const listed = ignored.slice(0, 5).join("\n");
  const more = ignored.length > 5 ? `\n… 等 ${ignored.length} 項` : "";
  return (
    `下列路徑位於版本控制資料夾（.git、.svn、.hg、.bzr）內：\n${listed}${more}\n\n` +
    "轉換這些檔案可能損壞儲存庫。確定要轉換嗎？按「取消」會從來源清單移除這些路徑。"
  );
}

export function withoutIgnoredInputs(paths: string[], ignored: string[]): string[] {
  const set = new Set(ignored);
  return paths.filter((path) => !set.has(path));
}

/** 使用者已在確認視窗取消時，不必再顯示後端的略過警告。 */
export function dropIgnoredInputWarnings(warnings: string[], ignored: string[]): string[] {
  return warnings.filter(
    (warning) =>
      !(
        warning.startsWith(IGNORED_INPUT_WARNING_PREFIX) &&
        ignored.some((path) => warning.includes(path))
      ),
  );
}

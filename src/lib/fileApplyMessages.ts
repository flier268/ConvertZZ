const CONTENT_SKIP_MARK = "已略過內容轉換";

/** 多個內容略過警告合併成一則，避免批次轉換時每個檔案各跳一次。 */
export function summarizeFileApplyWarnings(warnings: string[]): string[] {
  const skipped = warnings.filter((warning) => warning.includes(CONTENT_SKIP_MARK));
  if (skipped.length < 2) return warnings;
  const rest = warnings.filter((warning) => !warning.includes(CONTENT_SKIP_MARK));
  return [`已略過 ${skipped.length} 個檔案的內容轉換。`, ...rest];
}

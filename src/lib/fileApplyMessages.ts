import { t } from "@/i18n";

const CONTENT_SKIP_MARK_ZH = "已略過內容轉換";

/** 核心警告依介面語系產生；同時比對繁中原文，語系切換前產生的警告也能合併。 */
function isContentSkipWarning(warning: string): boolean {
  return (
    warning.includes(CONTENT_SKIP_MARK_ZH) || warning.includes(t("lib.fileApply.contentSkipMark"))
  );
}

/** 多個內容略過警告合併成一則，避免批次轉換時每個檔案各跳一次。 */
export function summarizeFileApplyWarnings(warnings: string[]): string[] {
  const skipped = warnings.filter(isContentSkipWarning);
  if (skipped.length < 2) return warnings;
  const rest = warnings.filter((warning) => !isContentSkipWarning(warning));
  return [t("lib.fileApply.contentSkipped", { count: skipped.length }), ...rest];
}

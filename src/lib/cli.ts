import { nextTick, shallowRef } from "vue";
import type { ParsedCli } from "@shared/contracts";

interface CliInvocation {
  sequence: number;
  options: ParsedCli;
}

export const cliInvocation = shallowRef<CliInvocation>();

let sequence = 0;

export function setCliInvocation(options: ParsedCli): void {
  cliInvocation.value = { sequence: ++sequence, options };
}

export type CliPage = "audio" | "files" | "quick";

export interface AppliedCliNavigation {
  page: CliPage | null;
  /** 音訊標籤之外另建立了檔名預覽時為 true。 */
  filenamePreviewQueued: boolean;
  /** 命令列解析錯誤；有值時未導向、未送出 invocation。 */
  parseError?: string;
}

/**
 * 依解析結果導向頁面並送出 CLI invocation。
 * `--audio --filename` 會先掛載檔案頁建立檔名預覽，再切到音訊標籤。
 */
export async function applyParsedCliNavigation(
  parsed: ParsedCli,
  setPage: (page: CliPage) => void,
): Promise<AppliedCliNavigation> {
  if (parsed.parseErrors?.length) {
    return {
      page: null,
      filenamePreviewQueued: false,
      parseError: parsed.parseErrors.join("；"),
    };
  }
  if (parsed.mode === "audio") {
    if (parsed.operation === "filename") {
      setCliInvocation({ ...parsed, mode: "file", operation: "filename" });
      setPage("files");
      return { page: "files", filenamePreviewQueued: false };
    }
    if (parsed.operation === "both") {
      // 先掛載檔案頁（keep-alive），否則檔名 invocation 會在未掛載時遺失。
      setCliInvocation({ ...parsed, mode: "file", operation: "filename" });
      setPage("files");
      await nextTick();
      setCliInvocation({ ...parsed, operation: "content" });
      setPage("audio");
      return { page: "audio", filenamePreviewQueued: true };
    }
    setCliInvocation(parsed);
    setPage("audio");
    return { page: "audio", filenamePreviewQueued: false };
  }
  if (parsed.mode === "file") {
    setCliInvocation(parsed);
    setPage("files");
    return { page: "files", filenamePreviewQueued: false };
  }
  setCliInvocation(parsed);
  setPage("quick");
  return { page: "quick", filenamePreviewQueued: false };
}

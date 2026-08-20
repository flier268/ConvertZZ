import type { Direction, LastDropActionSettings, ParsedCli, SettingsV2 } from "@shared/contracts";

export type DropActionKind = "file" | "audio";

export interface DropActionChoice {
  kind: DropActionKind;
  operation: "content" | "filename" | "both";
  direction: Direction;
}

export interface FileDropPayload {
  paths: string[];
}

export const DEFAULT_DROP_ACTION: DropActionChoice = {
  kind: "file",
  operation: "content",
  direction: "s2t",
};

export function normalizeDropActionChoice(
  value: Partial<LastDropActionSettings> | undefined,
  fallbackDirection: Direction = DEFAULT_DROP_ACTION.direction,
): DropActionChoice {
  const kind = value?.kind === "audio" ? "audio" : "file";
  const operation =
    value?.operation === "filename" || value?.operation === "both" ? value.operation : "content";
  const direction =
    value?.direction === "s2t" || value?.direction === "t2s" || value?.direction === "none"
      ? value.direction
      : fallbackDirection === "none"
        ? "s2t"
        : fallbackDirection;
  return { kind, operation, direction };
}

export function buildDropCliInvocation(
  paths: string[],
  choice: DropActionChoice,
  settings: Pick<SettingsV2, "engine" | "autoBackupBeforeConversion">,
): ParsedCli {
  return {
    mode: choice.kind === "audio" ? "audio" : "file",
    paths: [...paths],
    inputEncoding: "auto",
    outputEncoding: "auto",
    direction: choice.direction,
    engine: settings.engine,
    operation: choice.operation,
    vocabularyCorrection: "settings",
    backup: settings.autoBackupBeforeConversion,
  };
}

export function dropTargetPage(kind: DropActionKind): "files" | "audio" {
  return kind === "audio" ? "audio" : "files";
}

/** 對話框路徑摘要；過多時截斷。 */
export function summarizeDropPaths(paths: string[], limit = 3): string {
  if (!paths.length) return "未選取檔案";
  const shown = paths.slice(0, limit).map((path) => {
    const parts = path.split(/[/\\]/u).filter(Boolean);
    return parts[parts.length - 1] ?? path;
  });
  const extra = paths.length - shown.length;
  return extra > 0 ? `${shown.join("、")} 等 ${paths.length} 項` : shown.join("、");
}

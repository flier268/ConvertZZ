import type { FilePlanItem } from "@shared/contracts";
import { t } from "@/i18n";

function filenameLabels() {
  return {
    title: t("lib.fileDiff.filename.title"),
    sourceLabel: t("lib.fileDiff.filename.sourceLabel"),
    outputLabel: t("lib.fileDiff.filename.outputLabel"),
  };
}

export interface DiffSection {
  title: string;
  sourceLabel: string;
  outputLabel: string;
  source: string;
  output: string;
}

export function baseName(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}

/** 清單用：改名時顯示「原檔名 → 新檔名」，否則只顯示檔名。 */
export function nameChangeLabel(item: FilePlanItem): string {
  const sourceName = baseName(item.sourcePath);
  const outputName = baseName(item.outputPath);
  return sourceName === outputName ? sourceName : `${sourceName} → ${outputName}`;
}

/** 依路徑與預覽內容組出檔名／內容差異區段，避免檔名模式重複顯示。 */
export function buildFileDiffSections(item: FilePlanItem): DiffSection[] {
  const sourceName = baseName(item.sourcePath);
  const outputName = baseName(item.outputPath);
  const renamed = sourceName !== outputName;
  const previewIsFilename = item.sourcePreview === sourceName && item.outputPreview === outputName;
  const sections: DiffSection[] = [];

  if (renamed) {
    sections.push({
      ...filenameLabels(),
      source: sourceName,
      output: outputName,
    });
  }

  // 內容尚未載入時不要塞空白內容區段。
  if (!item.previewLoaded && !previewIsFilename) {
    if (!renamed && (item.sourcePreview || item.outputPreview)) {
      sections.push({
        ...filenameLabels(),
        source: item.sourcePreview ?? "",
        output: item.outputPreview ?? "",
      });
    }
    return sections;
  }

  if (!previewIsFilename) {
    sections.push({
      title: t("lib.fileDiff.content.title"),
      sourceLabel: t("lib.fileDiff.content.sourceLabel"),
      outputLabel: t("lib.fileDiff.content.outputLabel"),
      source: item.sourcePreview ?? "",
      output: item.outputPreview ?? "",
    });
  } else if (!renamed) {
    sections.push({
      ...filenameLabels(),
      source: item.sourcePreview ?? "",
      output: item.outputPreview ?? "",
    });
  }

  return sections;
}

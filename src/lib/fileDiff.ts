import type { FilePlanItem } from "@shared/contracts";

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

/** 依路徑與預覽內容組出檔名／內容差異區段，避免檔名模式重複顯示。 */
export function buildFileDiffSections(item: FilePlanItem): DiffSection[] {
  const sourceName = baseName(item.sourcePath);
  const outputName = baseName(item.outputPath);
  const renamed = sourceName !== outputName;
  const previewIsFilename = item.sourcePreview === sourceName && item.outputPreview === outputName;
  const sections: DiffSection[] = [];

  if (renamed) {
    sections.push({
      title: "檔名",
      sourceLabel: "來源檔名",
      outputLabel: "輸出檔名",
      source: sourceName,
      output: outputName,
    });
  }

  if (!previewIsFilename) {
    sections.push({
      title: "內容",
      sourceLabel: "來源預覽",
      outputLabel: "輸出預覽",
      source: item.sourcePreview ?? "",
      output: item.outputPreview ?? "",
    });
  } else if (!renamed) {
    sections.push({
      title: "檔名",
      sourceLabel: "來源檔名",
      outputLabel: "輸出檔名",
      source: item.sourcePreview ?? "",
      output: item.outputPreview ?? "",
    });
  }

  return sections;
}

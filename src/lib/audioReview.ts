import type { AudioTagField, AudioTagFile, Direction } from "@shared/contracts";

export type AudioListFilter = "changed" | "unchanged" | "errors" | "all";

export interface AudioReviewStats {
  total: number;
  changed: number;
  unchanged: number;
  errors: number;
  /** 「只看有變更」實際列出的筆數（尚未建立預覽時含待審檔案）。 */
  listedChanged: number;
}

export interface AudioSelectionTotals {
  files: number;
  fields: number;
}

export interface AudioContainerFlags {
  id3v1: boolean;
  id3v2: boolean;
}

export interface DiffPiece {
  text: string;
  kind: "same" | "remove" | "add";
}

export interface AudioSettingsSnapshot {
  direction: Direction;
  vocabularyCorrection: boolean;
  backup: boolean;
  hasMp3: boolean;
  id3v1Enabled: boolean;
  id3v1SourceEncoding: string;
  id3v1OutputEncoding: string;
  id3v2Enabled: boolean;
  id3v2Direction: Direction;
  id3v2RepairSourceEncoding: boolean;
  id3v2SourceEncoding: string;
  id3v2Version: 3 | 4;
  id3v2Encoding: string;
}

const DIRECTION_LABEL: Record<Direction, string> = {
  s2t: "簡→繁",
  t2s: "繁→簡",
  none: "不轉換",
};

const TEXT_ENCODING_LABEL: Record<string, string> = {
  gbk: "GBK",
  big5: "Big5",
  "shift-jis": "Shift-JIS",
  utf8: "UTF-8",
};

const ID3V2_ENCODING_LABEL: Record<string, string> = {
  utf8: "UTF-8",
  utf16: "UTF-16",
  utf16be: "UTF-16BE",
  latin1: "Latin-1",
};

const TITLE_KEYS = new Set(["title", "tit2"]);
const ALBUM_KEYS = new Set(["album", "talb"]);

export function audioBaseName(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}

export function fieldValuesChanged(field: AudioTagField): boolean {
  if (!field.convertedValues) return false;
  return field.convertedValues.join("\u0000") !== field.values.join("\u0000");
}

export function changedFieldCount(file: AudioTagFile): number {
  return file.fields.filter(fieldValuesChanged).length;
}

export function audioPreviewReady(files: readonly AudioTagFile[]): boolean {
  return files.some((file) => file.fields.some((field) => field.convertedValues !== undefined));
}

export function audioFileBucket(
  file: AudioTagFile,
  previewReady: boolean,
): Exclude<AudioListFilter, "all"> {
  if (file.warning) return "errors";
  if (!previewReady) return "changed";
  return changedFieldCount(file) > 0 ? "changed" : "unchanged";
}

export function audioReviewStats(files: readonly AudioTagFile[]): AudioReviewStats {
  let changed = 0;
  let errors = 0;
  for (const file of files) {
    if (file.warning) errors += 1;
    else if (changedFieldCount(file) > 0) changed += 1;
  }
  const unchanged = files.length - changed - errors;
  const ready = audioPreviewReady(files);
  const listedChanged = files.filter((file) => audioFileBucket(file, ready) === "changed").length;
  return { total: files.length, changed, unchanged, errors, listedChanged };
}

function fieldBlob(file: AudioTagFile, keys: Set<string>, label: string): string {
  return file.fields
    .filter((field) => keys.has(field.key.toLowerCase()) || field.label.includes(label))
    .flatMap((field) => [...field.values, ...(field.convertedValues ?? [])])
    .join("\n");
}

export function matchesAudioQuery(file: AudioTagFile, query: string): boolean {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const haystack = [
    audioBaseName(file.path),
    fieldBlob(file, TITLE_KEYS, "標題"),
    fieldBlob(file, ALBUM_KEYS, "專輯"),
  ]
    .join("\n")
    .toLowerCase();
  return haystack.includes(needle);
}

export function filterAudioFiles(
  files: readonly AudioTagFile[],
  filter: AudioListFilter,
  query: string,
): AudioTagFile[] {
  const ready = audioPreviewReady(files);
  return files.filter((file) => {
    if (!matchesAudioQuery(file, query)) return false;
    if (filter === "all") return true;
    return audioFileBucket(file, ready) === filter;
  });
}

function containerEnabled(field: AudioTagField, containers: AudioContainerFlags): boolean {
  if (field.container === "id3v1") return containers.id3v1;
  if (field.container === "id3v2") return containers.id3v2;
  return true;
}

function countsTowardWrite(
  field: AudioTagField,
  containers: AudioContainerFlags,
  ready: boolean,
): boolean {
  if (!field.selected || !containerEnabled(field, containers)) return false;
  // 預覽完成前還不知道結果，已勾欄位先算進「建立預覽」。預覽後只算真的會變的欄位。
  if (!ready) return true;
  return fieldValuesChanged(field);
}

/** 至少有一個已勾、且（預覽後）數值會變的欄位。不變檔不計入、也不寫入。 */
export function fileHasWritableSelection(
  file: AudioTagFile,
  containers: AudioContainerFlags,
  previewReady: boolean,
): boolean {
  if (!file.selected || file.warning) return false;
  return file.fields.some((field) => countsTowardWrite(field, containers, previewReady));
}

export function selectionTotals(
  files: readonly AudioTagFile[],
  containers: AudioContainerFlags,
): AudioSelectionTotals {
  const ready = audioPreviewReady(files);
  let fileCount = 0;
  let fieldCount = 0;
  for (const file of files) {
    if (!file.selected || file.warning) continue;
    let selected = 0;
    for (const field of file.fields) {
      if (!countsTowardWrite(field, containers, ready)) continue;
      selected += 1;
    }
    if (selected > 0) {
      fileCount += 1;
      fieldCount += selected;
    }
  }
  return { files: fileCount, fields: fieldCount };
}

export function writableAudioPaths(
  files: readonly AudioTagFile[],
  containers: AudioContainerFlags,
): string[] {
  const ready = audioPreviewReady(files);
  return files
    .filter((file) => fileHasWritableSelection(file, containers, ready))
    .map((file) => file.path);
}

export function applyBulkSelection(
  files: readonly AudioTagFile[],
  visible: readonly AudioTagFile[],
  mode: "all" | "none" | "invert",
): void {
  const paths = new Set(visible.filter((file) => !file.warning).map((file) => file.path));
  for (const file of files) {
    if (!paths.has(file.path) || file.warning) continue;
    if (mode === "all") file.selected = true;
    else if (mode === "none") file.selected = false;
    else file.selected = !file.selected;
  }
}

function mergePieces(pieces: DiffPiece[]): DiffPiece[] {
  const merged: DiffPiece[] = [];
  for (const piece of pieces) {
    if (!piece.text) continue;
    const last = merged.at(-1);
    if (last && last.kind === piece.kind) last.text += piece.text;
    else merged.push({ ...piece });
  }
  return merged;
}

/** 字元級差異。過長字串整段標成刪除／新增，避免平方比對。 */
export function charDiff(
  before: string,
  after: string,
): { before: DiffPiece[]; after: DiffPiece[] } {
  if (before === after) {
    const same = before ? [{ text: before, kind: "same" as const }] : [];
    return { before: same, after: same.map((piece) => ({ ...piece })) };
  }
  const a = Array.from(before);
  const b = Array.from(after);
  if (a.length * b.length > 40_000) {
    return {
      before: [{ text: before, kind: "remove" }],
      after: [{ text: after, kind: "add" }],
    };
  }
  const rows = a.length + 1;
  const cols = b.length + 1;
  const dp = Array.from({ length: rows }, () => new Uint16Array(cols));
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const oldPieces: DiffPiece[] = [];
  const newPieces: DiffPiece[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      oldPieces.push({ text: a[i], kind: "same" });
      newPieces.push({ text: b[j], kind: "same" });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      oldPieces.push({ text: a[i], kind: "remove" });
      i += 1;
    } else {
      newPieces.push({ text: b[j], kind: "add" });
      j += 1;
    }
  }
  while (i < a.length) {
    oldPieces.push({ text: a[i], kind: "remove" });
    i += 1;
  }
  while (j < b.length) {
    newPieces.push({ text: b[j], kind: "add" });
    j += 1;
  }
  return { before: mergePieces(oldPieces), after: mergePieces(newPieces) };
}

function encodingLabel(value: string): string {
  return TEXT_ENCODING_LABEL[value] ?? value;
}

export function settingsSummaryChips(settings: AudioSettingsSnapshot): string[] {
  const chips: string[] = [];
  if (settings.hasMp3 && settings.id3v2Enabled) {
    chips.push(`ID3v2 ${DIRECTION_LABEL[settings.id3v2Direction]}`);
    if (settings.id3v2RepairSourceEncoding) {
      chips.push(`${encodingLabel(settings.id3v2SourceEncoding)} 錯碼修復`);
    }
  }
  if (settings.hasMp3 && settings.id3v1Enabled) {
    chips.push(
      `ID3v1 ${encodingLabel(settings.id3v1SourceEncoding)}→${encodingLabel(settings.id3v1OutputEncoding)}`,
    );
  }
  chips.push(`APE/Vorbis ${DIRECTION_LABEL[settings.direction]}`);
  if (settings.vocabularyCorrection) chips.push("詞彙修正");
  chips.push(settings.backup ? "備份 .bak" : "不備份");
  if (settings.hasMp3 && settings.id3v2Enabled) {
    chips.push(
      `ID3v2.${settings.id3v2Version}・${ID3V2_ENCODING_LABEL[settings.id3v2Encoding] ?? settings.id3v2Encoding}`,
    );
  }
  return chips;
}

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

/** 區段按鈕的筆數。與 `filterAudioFiles` 使用同一個分桶，搜尋不會改這些數字。 */
export function audioSegmentCounts(
  files: readonly AudioTagFile[],
): Record<AudioListFilter, number> {
  const ready = audioPreviewReady(files);
  const counts: Record<AudioListFilter, number> = {
    changed: 0,
    unchanged: 0,
    errors: 0,
    all: files.length,
  };
  for (const file of files) counts[audioFileBucket(file, ready)] += 1;
  return counts;
}

/** 用同一個路徑換掉清單裡的一筆。找不到就加在最後。 */
export function replaceAudioFile(
  files: readonly AudioTagFile[],
  updated: AudioTagFile,
): AudioTagFile[] {
  const index = files.findIndex((file) => file.path === updated.path);
  if (index < 0) return [...files, updated];
  return files.map((file, itemIndex) => (itemIndex === index ? updated : file));
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

export interface AudioFolderCounts {
  /** 目前篩選下這個資料夾的檔案數。 */
  files: number;
  /** 其中標籤數值有變更的檔案數。 */
  changed: number;
  /** 其中讀取失敗的檔案數。 */
  errors: number;
}

export type AudioListRow =
  | { type: "folder"; path: string; counts: AudioFolderCounts; collapsed: boolean }
  | { type: "file"; file: AudioTagFile };

/** 檔案所在資料夾。無分隔字元時歸在「.」。 */
export function audioParentDir(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const index = normalized.lastIndexOf("/");
  if (index < 0) return ".";
  if (index === 0) return "/";
  return normalized.slice(0, index);
}

/**
 * 把已篩選的檔案收成單一扁平列：資料夾標題＋檔案。
 * 摺疊的資料夾不輸出檔案列，虛擬清單因此不會畫到它們。
 * 資料夾順序依檔案首次出現，同資料夾維持原順序。
 */
export function buildAudioListRows(
  files: readonly AudioTagFile[],
  collapsed: ReadonlySet<string>,
): AudioListRow[] {
  const groups = new Map<string, AudioTagFile[]>();
  for (const file of files) {
    const dir = audioParentDir(file.path);
    const list = groups.get(dir);
    if (list) list.push(file);
    else groups.set(dir, [file]);
  }
  const rows: AudioListRow[] = [];
  for (const [path, group] of groups) {
    const isCollapsed = collapsed.has(path);
    let changed = 0;
    let errors = 0;
    for (const file of group) {
      if (file.warning) errors += 1;
      else if (changedFieldCount(file) > 0) changed += 1;
    }
    rows.push({
      type: "folder",
      path,
      collapsed: isCollapsed,
      counts: { files: group.length, changed, errors },
    });
    if (!isCollapsed) {
      for (const file of group) rows.push({ type: "file", file });
    }
  }
  return rows;
}

export function toggleFolderCollapsed(collapsed: ReadonlySet<string>, folder: string): Set<string> {
  const next = new Set(collapsed);
  if (next.has(folder)) next.delete(folder);
  else next.add(folder);
  return next;
}

/**
 * 在扁平可見列上移動。被摺疊拿掉的檔案不在 `length` 裡，所以 ↑/↓ 會直接跳過。
 * `index` 為 -1（例如選取落在隱藏列）時，往下從第一列開始、往上從最後一列開始。
 */
export function stepAudioRow(length: number, index: number, delta: number): number {
  if (length <= 0) return -1;
  if (index < 0) return delta < 0 ? length - 1 : 0;
  return Math.min(length - 1, Math.max(0, index + delta));
}

/** 這一列所屬的資料夾標題索引，供黏住目前資料夾。 */
export function audioRowSection(rows: readonly { type: string }[], index: number): number | null {
  if (index < 0 || index >= rows.length) return null;
  for (let cursor = index; cursor >= 0; cursor -= 1) {
    if (rows[cursor]?.type === "folder") return cursor;
  }
  return null;
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

/**
 * 全選、全不選、反選只改 `visible` 裡的檔案（目前篩選結果、且非錯誤檔）。
 * 資料夾摺疊只影響畫面列，不影響這份清單：呼叫端要傳入全部篩選檔，
 * 摺疊資料夾裡的檔案才會一起被勾選或取消。
 */
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

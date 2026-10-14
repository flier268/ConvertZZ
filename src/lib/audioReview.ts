import type {
  AudioContainer,
  AudioTagField,
  AudioTagFile,
  Direction,
  TagAction,
  TagOnConflict,
  TagOperation,
} from "@shared/contracts";

export type AudioListFilter = "changed" | "unchanged" | "errors" | "warnings" | "all";

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
  /** 啟用中的標籤操作條數。省略或 0 不顯示 chip。 */
  tagOperationCount?: number;
}

export const AUDIO_PREVIEW_DEBOUNCE_MS = 350;

export function createPreviewRace() {
  let generation = 0;
  return {
    start(): number {
      generation += 1;
      return generation;
    },
    current(): number {
      return generation;
    },
    isCurrent(token: number): boolean {
      return token === generation;
    },
  };
}

export interface AudioPreviewController {
  schedule(): void;
  flush(): Promise<void>;
  run(): Promise<void>;
  /** 讓進行中的預覽回應失效，例如使用者改去掃描。 */
  invalidate(): void;
  /**
   * 掃描等外部工作接手忙碌狀態。進行中的預覽結束時不得再 onSettled。
   * 沒有新工作時，invalidate 仍會讓該輪自己結束並 onSettled。
   */
  claimJob(): void;
  /** 寫入進行中時，預覽不得再排程或取消這次寫入。 */
  setApplying(applying: boolean): void;
}

/**
 * 設定變更先合併成一次預覽。後發出的要求蓋過先發出的；過期回應不會寫回。
 */
export function createAudioPreviewController(deps: {
  debounceMs?: number;
  plan: () => Promise<unknown>;
  cancelInFlight: () => Promise<void>;
  apply: (value: unknown) => void;
  onError?: (error: unknown) => void;
  isCancellation?: (error: unknown) => boolean;
  onStart?: () => void;
  onSettled?: () => void;
}): AudioPreviewController {
  const race = createPreviewRace();
  const debounceMs = deps.debounceMs ?? AUDIO_PREVIEW_DEBOUNCE_MS;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let applying = false;
  let running = 0;
  // 只有目前這一代工作可以放下 busy。掃描等外部工作用 claimJob 推進世代。
  let jobGeneration = 0;
  // 已開始的 run 串成一條。flush 要等它們全部結束，不能在沒有計時器時直接 return。
  let tail: Promise<void> = Promise.resolve();

  async function run(): Promise<void> {
    if (applying) return;
    const token = race.start();
    const job = ++jobGeneration;
    let settle!: () => void;
    const done = new Promise<void>((resolve) => {
      settle = resolve;
    });
    const previous = tail;
    tail = previous.then(() => done);
    running += 1;
    deps.onStart?.();
    try {
      // 開頭檢查之後、取消之前，套用可能才開始。此時取消會打斷進行中的寫入。
      if (applying) return;
      await deps.cancelInFlight();
      if (!race.isCurrent(token)) return;
      const value = await deps.plan();
      if (!race.isCurrent(token) || value == null) return;
      deps.apply(value);
    } catch (error) {
      if (!race.isCurrent(token) || deps.isCancellation?.(error)) return;
      deps.onError?.(error);
    } finally {
      running -= 1;
      // 停止作業只 invalidate、沒有新的 run 接手時，這輪仍要 onSettled，否則 busy 不會下來。
      // 已有下一輪在跑、外部工作已 claimJob，或正在套用，則留給那一輪自己結束。
      if (job === jobGeneration && (race.isCurrent(token) || (running === 0 && !applying))) {
        deps.onSettled?.();
      }
      settle();
    }
  }

  return {
    schedule() {
      if (applying) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = undefined;
        void run();
      }, debounceMs);
    },
    async flush() {
      if (timer) {
        clearTimeout(timer);
        timer = undefined;
        void run();
      }
      await tail;
    },
    run,
    invalidate() {
      if (timer) clearTimeout(timer);
      timer = undefined;
      race.start();
    },
    claimJob() {
      jobGeneration += 1;
    },
    setApplying(next: boolean) {
      applying = next;
      if (!next) return;
      if (timer) clearTimeout(timer);
      timer = undefined;
    },
  };
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

export function containerRewrites(
  file: AudioTagFile,
  container: AudioTagField["container"],
): boolean {
  return (file.rewriteContainers ?? []).includes(container);
}

/** 欄位文字有變，或所屬容器要重寫編碼／ID3 版本。分桶與寫入用這個。 */
export function fieldAffectsAudio(file: AudioTagFile, field: AudioTagField): boolean {
  return fieldValuesChanged(field) || containerRewrites(file, field.container);
}

/**
 * 「隱藏不變欄位」只看文字。編碼或 ID3 版本要重寫時，文字相同的欄位在預覽欄仍是「不變」。
 */
export function fieldVisibleWhenHidingUnchanged(field: AudioTagField): boolean {
  return (
    fieldValuesChanged(field) ||
    (field.warnings?.length ?? 0) > 0 ||
    Boolean(field.origin?.startsWith("rule "))
  );
}

/** 計畫上的容器重寫說明。隱藏不變欄位時仍然顯示。 */
export function detailContainerNotices(file: AudioTagFile): string[] {
  return file.notices ?? [];
}

/** ID3v1 固定寬度截斷說明，給詳細面板顯示。 */
export function id3v1TruncationNotes(file: AudioTagFile): string[] {
  return file.fields.flatMap((field) => (field.truncation ? [field.truncation] : []));
}

export function removalFieldId(container: AudioContainer): string {
  return `remove:${container}`;
}

export function activeRemovals(file: AudioTagFile) {
  return (file.removals ?? []).filter((removal) => removal.selected && !removal.refused);
}

export function changedFieldCount(file: AudioTagFile): number {
  return (
    file.fields.filter((field) => fieldAffectsAudio(file, field)).length +
    activeRemovals(file).length
  );
}

/** 截斷、缺字，或移除時帶有隱藏欄位。 */
export function fileHasTagWarning(file: AudioTagFile): boolean {
  const fieldWarning = file.fields.some(
    (field) =>
      field.truncated ||
      (field.warnings ?? []).some(
        (warning) =>
          warning.code === "truncated" ||
          warning.code === "unrepresentable" ||
          warning.code === "hidden",
      ),
  );
  if (fieldWarning) return true;
  return (file.removals ?? []).some((removal) => removal.hiddenFieldCount > 0);
}

/** 底部「n 個檔案有截斷或缺字」只算文字損失，不含隱藏欄位。 */
export function fileHasTextLoss(file: AudioTagFile): boolean {
  return file.fields.some(
    (field) =>
      field.truncated ||
      (field.warnings ?? []).some(
        (warning) => warning.code === "truncated" || warning.code === "unrepresentable",
      ),
  );
}

export function fieldOriginLabel(origin: string | undefined): string {
  if (!origin) return "";
  if (origin === "manual") return "手動";
  if (origin === "convert") return "轉換";
  const rule = /^rule (\d+)$/.exec(origin);
  if (rule) return `規則 ${rule[1]}`;
  return origin;
}

export function audioPreviewReady(files: readonly AudioTagFile[]): boolean {
  return files.some(
    (file) => file.previewed || file.fields.some((field) => field.convertedValues !== undefined),
  );
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
    warnings: files.filter((file) => fileHasTagWarning(file)).length,
    all: files.length,
  };
  for (const file of files) counts[audioFileBucket(file, ready)] += 1;
  return counts;
}

/**
 * 比對路徑用的鍵。尾端斜線與反斜線視為相同；
 * 磁碟機代號（`C:`）、UNC（`\\server\share`）與 `\\?\` 路徑不分大小寫，其餘維持大小寫。
 */
export function audioPathKey(path: string): string {
  let normalized = path.replaceAll("\\", "/");
  const drive = /^[A-Za-z]:/.test(normalized);
  const unc = normalized.startsWith("//");
  if (drive || unc) normalized = normalized.toLowerCase();
  const minimum = drive ? 3 : normalized.startsWith("/") ? 1 : 0;
  while (normalized.length > minimum && normalized.endsWith("/")) {
    normalized = normalized.slice(0, -1);
  }
  return normalized;
}

/** 依路徑併入一批掃描或預覽結果。已存在的路徑就地更新，新路徑加在最後。 */
export function mergeAudioFiles(
  current: readonly AudioTagFile[],
  incoming: readonly AudioTagFile[],
): AudioTagFile[] {
  if (!incoming.length) return [...current];
  const next = [...current];
  const indexByPath = new Map(next.map((file, index) => [audioPathKey(file.path), index]));
  for (const file of incoming) {
    const index = indexByPath.get(audioPathKey(file.path));
    if (index == null) {
      indexByPath.set(audioPathKey(file.path), next.length);
      next.push(file);
    } else {
      next[index] = file;
    }
  }
  return next;
}

/** 用同一個路徑換掉清單裡的一筆。找不到就加在最後。 */
export function replaceAudioFile(
  files: readonly AudioTagFile[],
  updated: AudioTagFile,
): AudioTagFile[] {
  const index = files.findIndex((file) => audioPathKey(file.path) === audioPathKey(updated.path));
  if (index < 0) return [...files, updated];
  return files.map((file, itemIndex) => (itemIndex === index ? updated : file));
}

export function audioReviewStats(files: readonly AudioTagFile[]): AudioReviewStats {
  const counts = audioSegmentCounts(files);
  return {
    total: counts.all,
    changed: counts.changed,
    unchanged: counts.unchanged,
    errors: counts.errors,
    listedChanged: counts.changed,
  };
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
    if (filter === "warnings") return fileHasTagWarning(file);
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
    const ready = audioPreviewReady(files);
    let changed = 0;
    let errors = 0;
    for (const file of group) {
      const bucket = audioFileBucket(file, ready);
      if (bucket === "errors") errors += 1;
      else if (bucket === "changed") changed += 1;
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

export type AudioSortKey = "name" | "changed" | "format";

function compareAudioFiles(left: AudioTagFile, right: AudioTagFile, key: AudioSortKey): number {
  if (key === "changed") {
    const delta = changedFieldCount(right) - changedFieldCount(left);
    if (delta !== 0) return delta;
  } else if (key === "format") {
    const delta = left.format.localeCompare(right.format);
    if (delta !== 0) return delta;
  }
  return folderCollator.compare(audioBaseName(left.path), audioBaseName(right.path));
}

const folderCollator = new Intl.Collator("zh-Hant");

/**
 * 資料夾先依 zh-Hant 排序，再排資料夾裡的檔案。
 * 掃描結果是路徑位元組順序，分組前要改成筆畫／拼音順序。
 */
export function sortAudioFilesWithinFolders(
  files: readonly AudioTagFile[],
  key: AudioSortKey,
): AudioTagFile[] {
  const groups = new Map<string, AudioTagFile[]>();
  for (const file of files) {
    const dir = audioParentDir(file.path);
    const list = groups.get(dir);
    if (list) list.push(file);
    else groups.set(dir, [file]);
  }
  const order = [...groups.keys()].sort((left, right) => folderCollator.compare(left, right));
  const sorted: AudioTagFile[] = [];
  for (const dir of order) {
    const group = groups.get(dir) ?? [];
    group.sort((left, right) => compareAudioFiles(left, right, key));
    sorted.push(...group);
  }
  return sorted;
}

export function fieldDiffKey(field: AudioTagField): string {
  return `${field.container}:${field.key}`;
}

/** 每個有預覽文字的欄位只算一次字元差異。 */
export function buildFieldDiffs(
  file: AudioTagFile | undefined,
): Map<string, { before: DiffPiece[]; after: DiffPiece[] }> {
  const map = new Map<string, { before: DiffPiece[]; after: DiffPiece[] }>();
  if (!file) return map;
  for (const field of file.fields) {
    if (!field.convertedValues) continue;
    map.set(
      fieldDiffKey(field),
      charDiff(field.values.join("\n"), field.convertedValues.join("\n")),
    );
  }
  return map;
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
  if (field.origin?.startsWith("rule ")) return true;
  if (field.container === "id3v1") return containers.id3v1;
  if (field.container === "id3v2") return containers.id3v2;
  return true;
}

function countsTowardWrite(
  file: AudioTagFile,
  field: AudioTagField,
  containers: AudioContainerFlags,
  ready: boolean,
): boolean {
  if (!field.selected || !containerEnabled(field, containers)) return false;
  // 預覽完成前還不知道結果，已勾欄位先算進「建立預覽」。預覽後只算真的會變的欄位。
  if (!ready) return true;
  return fieldAffectsAudio(file, field);
}

/** 至少有一個已勾、且（預覽後）數值會變的欄位。不變檔不計入、也不寫入。 */
export function fileHasWritableSelection(
  file: AudioTagFile,
  containers: AudioContainerFlags,
  previewReady: boolean,
): boolean {
  if (!file.selected || file.warning) return false;
  if (activeRemovals(file).length > 0) return true;
  return file.fields.some((field) => countsTowardWrite(file, field, containers, previewReady));
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
    let selected = activeRemovals(file).length;
    for (const field of file.fields) {
      if (!countsTowardWrite(file, field, containers, ready)) continue;
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
  return audioApplySelection(files, containers).paths;
}

/** 目前勾選。`audio.apply` 必須帶這份，預覽之後改勾選才會影響寫入。 */
export function audioApplySelection(
  files: readonly AudioTagFile[],
  containers: AudioContainerFlags,
): { paths: string[]; selectedFields: Record<string, string[]> } {
  const ready = audioPreviewReady(files);
  const selectedFields: Record<string, string[]> = {};
  const paths: string[] = [];
  for (const file of files) {
    if (!fileHasWritableSelection(file, containers, ready)) continue;
    const ids = file.fields
      .filter((field) => countsTowardWrite(file, field, containers, ready))
      .map((field) => `${field.container}:${field.key}`);
    for (const removal of activeRemovals(file)) ids.push(removalFieldId(removal.container));
    if (!ids.length) continue;
    paths.push(file.path);
    selectedFields[file.path] = ids;
  }
  return { paths, selectedFields };
}

/** 錯誤檔一律未勾選，全選與反選也不能把它算進去。 */
export function clearWarningSelection(files: readonly AudioTagFile[]): void {
  for (const file of files) {
    if (file.warning) file.selected = false;
  }
}

export const AUDIO_TAG_READ_ERROR = "無法讀取標籤";

/** 與標題相同的原因不重複顯示。 */
export function audioWarningDetail(warning: string | undefined): string | undefined {
  const text = warning?.trim();
  if (!text || text === AUDIO_TAG_READ_ERROR) return undefined;
  return text;
}

/**
 * 全選、全不選、反選只改 `visible` 裡的檔案（目前篩選結果、且非錯誤檔）。
 * 錯誤檔在這次操作裡一律清成未勾選。
 * 資料夾摺疊只影響畫面列，不影響這份清單：呼叫端要傳入全部篩選檔，
 * 摺疊資料夾裡的檔案才會一起被勾選或取消。
 */
/** 目前篩選裡、這個資料夾可勾選的檔案。錯誤檔不算。 */
export function selectableFolderFiles(
  visible: readonly AudioTagFile[],
  folder: string,
): AudioTagFile[] {
  return visible.filter((file) => !file.warning && audioParentDir(file.path) === folder);
}

export function folderSelectionState(
  visible: readonly AudioTagFile[],
  folder: string,
): "all" | "none" | "mixed" | "empty" {
  const group = selectableFolderFiles(visible, folder);
  if (!group.length) return "empty";
  const selected = group.filter((file) => file.selected).length;
  if (selected === 0) return "none";
  if (selected === group.length) return "all";
  return "mixed";
}

/** 與全選相同規則，範圍只限這個資料夾在目前篩選下的非錯誤檔。 */
export function toggleFolderSelection(
  files: readonly AudioTagFile[],
  visible: readonly AudioTagFile[],
  folder: string,
): void {
  const group = selectableFolderFiles(visible, folder);
  if (!group.length) return;
  const mode = group.every((file) => file.selected) ? "none" : "all";
  applyBulkSelection(files, group, mode);
}

/** `audio.plan` 只帶有勾選欄位的檔案。 */
export function planSelectedFields(files: readonly AudioTagFile[]): Record<string, string[]> {
  const selectedFields: Record<string, string[]> = {};
  for (const file of files) {
    const ids = file.fields
      .filter((field) => field.selected)
      .map((field) => `${field.container}:${field.key}`);
    if (ids.length) selectedFields[file.path] = ids;
  }
  return selectedFields;
}

export function applyBulkSelection(
  files: readonly AudioTagFile[],
  visible: readonly AudioTagFile[],
  mode: "all" | "none" | "invert",
): void {
  clearWarningSelection(files);
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
  if ((settings.tagOperationCount ?? 0) > 0) {
    chips.push(`標籤操作 ${settings.tagOperationCount} 條`);
  }
  return chips;
}

export const TAG_RULE_FIELDS: ReadonlyArray<readonly [string, string]> = [
  ["title", "標題"],
  ["artist", "演出者"],
  ["album", "專輯"],
  ["year", "年份"],
  ["track", "音軌"],
  ["comment", "註解"],
  ["genre", "類型"],
];

export interface TagRule {
  id: string;
  enabled: boolean;
  action: TagAction;
  source: AudioContainer;
  target: AudioContainer;
  fields: string[];
  onConflict: TagOnConflict;
}

let tagRuleSerial = 0;

export function createTagRule(partial: Partial<TagRule> & Pick<TagRule, "action">): TagRule {
  tagRuleSerial += 1;
  return {
    id: partial.id ?? `tag-rule-${tagRuleSerial}`,
    enabled: partial.enabled ?? true,
    action: partial.action,
    source: partial.source ?? "id3v2",
    target: partial.target ?? "id3v1",
    fields: partial.fields ?? TAG_RULE_FIELDS.map(([key]) => key),
    onConflict:
      partial.onConflict ??
      (partial.action === "merge" || partial.source === "id3v1" ? "fillEmpty" : "overwrite"),
  };
}

/** 有的播放器偏好 APEv2，移除後畫面上的標題可能改變。 */
export const CLEANUP_APE_NOTE =
  "移除 MP3 上多餘的 APEv2。有的播放器偏好 APEv2，移除後顯示的標題可能會改變。檔案只剩這一個標籤時不會刪。";

export function cleanupApeRule(): TagRule {
  return createTagRule({ action: "remove", source: "apev2", target: "apev2" });
}

export function tagRuleSummary(rule: TagRule): string {
  const source = containerName(rule.source);
  const target = containerName(rule.target);
  if (rule.action === "remove") return `移除 ${source}（MP3 才執行）`;
  const how = rule.onConflict === "fillEmpty" ? "只補空白欄位" : "覆蓋目標";
  return `以 ${source} 為主合併到 ${target}（${how}）`;
}

export function toTagOperations(rules: readonly TagRule[]): TagOperation[] {
  return rules.map((rule) => ({
    action: rule.action,
    source: rule.source,
    target: rule.action === "remove" ? undefined : rule.target,
    fields: rule.action === "remove" ? [] : [...rule.fields],
    onConflict: rule.action === "remove" ? undefined : rule.onConflict,
    enabled: rule.enabled,
  }));
}

export function containerName(container: AudioContainer): string {
  if (container === "id3v1") return "ID3v1";
  if (container === "id3v2") return "ID3v2";
  if (container === "apev2") return "APEv2";
  return "Vorbis";
}

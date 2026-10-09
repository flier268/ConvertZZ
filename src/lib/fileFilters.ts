export interface LegacyFileFilter {
  name: string;
  extensions: string[];
}

export const SUPPORTED_FILES_FILTER_NAME = "支援的檔案";
export const ALL_FILES_FILTER_NAME = "所有檔案";
/** 對話框篩選與掃描狀態用的所有檔案標記。不是副檔名。 */
export const ALL_FILES_EXTENSION = "*";
/** 使用者在篩選器字串裡加入所有檔案時的群組。預設字串不含這項。 */
export const ALL_FILES_TYPE_FILTER_GROUP = "<所有檔案|*.*>";

export const DEFAULT_FILE_TYPE_FILTER =
  "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>";

/** 2.0 早期預設把音訊副檔名放進文字轉換篩選；載入時改回不含音訊的預設。 */
export const LEGACY_FILE_TYPE_FILTER_WITH_AUDIO =
  "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音訊文件|*.mp3;*.ape;*.ogg;*.oga;*.opus>";

/**
 * C# `FileConvert` 預設。`origin/master` `ConvertZZ/Settings.cs` 第 141 行，commit `00c2e902`。
 * 群組名是「音頻」，而且只列 `*.mp3`。
 */
export const CSHARP_FILE_TYPE_FILTER_WITH_AUDIO =
  "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音頻文件|*.mp3>";

const LEGACY_BUILTIN_FILE_TYPE_FILTERS = [
  LEGACY_FILE_TYPE_FILTER_WITH_AUDIO,
  CSHARP_FILE_TYPE_FILTER_WITH_AUDIO,
];

/**
 * 空字串不是舊內建預設。非空且不含 `<`（C# 1.0.0.0–1.0.0.3）、已知舊內建預設，
 * 或拿掉音訊／音頻群組後與新預設相同。
 */
export function isLegacyBuiltinFileTypeFilter(value: string): boolean {
  if (!value) return false;
  if (!value.includes("<")) return true;
  return (
    LEGACY_BUILTIN_FILE_TYPE_FILTERS.includes(value) ||
    fileTypeFilterWithoutAudioGroups(value) === DEFAULT_FILE_TYPE_FILTER
  );
}

/**
 * 只移除格式完整、且名稱含「音訊」或「音頻」的 `<名稱|樣式>`。
 * 其餘 `<...>`（沒有 `|`、未閉合）留在比較字串。未閉合時從該 `<` 留到結尾。
 */
function fileTypeFilterWithoutAudioGroups(value: string): string {
  const groups: string[] = [];
  let rest = value;
  while (rest.length) {
    const start = rest.indexOf("<");
    if (start < 0) break;
    const relativeEnd = rest.indexOf(">", start);
    if (relativeEnd < 0) {
      groups.push(rest.slice(start));
      break;
    }
    const group = rest.slice(start, relativeEnd + 1);
    if (!isRemovableAudioGroup(group)) groups.push(group);
    rest = rest.slice(relativeEnd + 1);
  }
  return groups.join("/");
}

function isRemovableAudioGroup(group: string): boolean {
  const inner = group.slice(1, -1);
  const pipe = inner.indexOf("|");
  if (pipe < 0) return false;
  const name = inner.slice(0, pipe);
  return name.includes("音訊") || name.includes("音頻");
}

function extensionFromLegacyPattern(pattern: string): string | null {
  const trimmed = pattern.trim();
  if (trimmed === "*" || trimmed.toLowerCase() === "*.*") return ALL_FILES_EXTENSION;
  const extension = trimmed.replace(/^\*\.?/u, "").replace(/^\./u, "");
  if (!extension || extension === "*") return null;
  return extension;
}

export function parseLegacyFileFilters(value: string): LegacyFileFilter[] {
  const filters: LegacyFileFilter[] = [];
  for (const match of value.matchAll(/<([^|<>]+)\|([^<>]+)>/gu)) {
    const extensions = match[2]
      .split(";")
      .map((pattern) => extensionFromLegacyPattern(pattern))
      .filter((extension): extension is string => extension !== null);
    if (extensions.length)
      filters.push({ name: match[1].trim(), extensions: Array.from(new Set(extensions)) });
  }
  return filters;
}

/** 設定字串已有 `*`／`*.*` 時不重複加入。空字串只寫入所有檔案群組。 */
export function appendAllFilesTypeFilter(value: string): string {
  if (
    parseLegacyFileFilters(value).some((filter) => filter.extensions.includes(ALL_FILES_EXTENSION))
  ) {
    return value;
  }
  const trimmed = value.trim();
  if (!trimmed) return ALL_FILES_TYPE_FILTER_GROUP;
  const separator = trimmed.endsWith("/") ? "" : "/";
  return `${trimmed}${separator}${ALL_FILES_TYPE_FILTER_GROUP}`;
}

/** 執行時在對話框篩選最前方加上「支援的檔案」聯集；設定字串本身不必也不應寫入此項。 */
export function ensureSupportedFilesFilter(filters: LegacyFileFilter[]): LegacyFileFilter[] {
  const categories = filters.filter((filter) => filter.name !== SUPPORTED_FILES_FILTER_NAME);
  const extensions = Array.from(
    new Set(
      (categories.length ? categories : filters).flatMap((filter) =>
        filter.extensions
          .filter((extension) => extension !== ALL_FILES_EXTENSION)
          .map((extension) => extension.toLowerCase()),
      ),
    ),
  );
  if (!extensions.length) return filters;
  return [{ name: SUPPORTED_FILES_FILTER_NAME, extensions }, ...categories];
}

/**
 * 檔案對話框用。固定在「支援的檔案」之後加上「所有檔案」。
 * 這項不寫入設定字串，也不應進入資料夾掃描的副檔名聯集。
 */
export function dialogFileFilters(filters: LegacyFileFilter[]): LegacyFileFilter[] {
  const withSupported = ensureSupportedFilesFilter(filters);
  const supportedIndex = withSupported.findIndex(
    (filter) => filter.name === SUPPORTED_FILES_FILTER_NAME,
  );
  const alreadyListed = withSupported.some(
    (filter, index) =>
      index > supportedIndex &&
      filter.name === ALL_FILES_FILTER_NAME &&
      filter.extensions.includes(ALL_FILES_EXTENSION),
  );
  if (alreadyListed) return withSupported;
  const allFiles = { name: ALL_FILES_FILTER_NAME, extensions: [ALL_FILES_EXTENSION] };
  const insertAt = supportedIndex >= 0 ? supportedIndex + 1 : 0;
  return [...withSupported.slice(0, insertAt), allFiles, ...withSupported.slice(insertAt)];
}

/** 資料夾掃描。`*` 是所有檔案；沒有任何副檔名時是空清單，不是所有檔案。 */
export type FolderScanExtensions = { kind: "all" } | { kind: "list"; extensions: string[] };

export function folderScanExtensions(filters: LegacyFileFilter[]): FolderScanExtensions {
  if (
    filters.some((filter) =>
      filter.extensions.some((extension) => extension === ALL_FILES_EXTENSION),
    )
  ) {
    return { kind: "all" };
  }
  const extensions = Array.from(
    new Set(
      filters
        .flatMap((filter) => filter.extensions)
        .filter((extension) => extension && extension !== ALL_FILES_EXTENSION)
        .map((extension) => `.${extension.toLowerCase()}`),
    ),
  );
  return { kind: "list", extensions };
}

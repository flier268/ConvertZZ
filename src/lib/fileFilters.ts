export interface LegacyFileFilter {
  name: string;
  extensions: string[];
}

export const SUPPORTED_FILES_FILTER_NAME = "支援的檔案";

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

/** 已知舊內建預設，或拿掉音訊／音頻群組後與新預設相同。 */
export function isLegacyBuiltinFileTypeFilter(value: string): boolean {
  return (
    LEGACY_BUILTIN_FILE_TYPE_FILTERS.includes(value) ||
    fileTypeFilterWithoutAudioGroups(value) === DEFAULT_FILE_TYPE_FILTER
  );
}

function fileTypeFilterWithoutAudioGroups(value: string): string {
  const groups = [...value.matchAll(/<([^|<>]*)\|[^<>]*>/gu)].filter(
    (match) => !match[1].includes("音訊") && !match[1].includes("音頻"),
  );
  return groups.map((match) => match[0]).join("/");
}

export function parseLegacyFileFilters(value: string): LegacyFileFilter[] {
  const filters: LegacyFileFilter[] = [];
  for (const match of value.matchAll(/<([^|<>]+)\|([^<>]+)>/gu)) {
    const extensions = match[2]
      .split(";")
      .map((pattern) =>
        pattern
          .trim()
          .replace(/^\*\.?/u, "")
          .replace(/^\./u, ""),
      )
      .filter((extension) => extension && extension !== "*");
    if (extensions.length)
      filters.push({ name: match[1].trim(), extensions: Array.from(new Set(extensions)) });
  }
  return filters;
}

/** 執行時在對話框篩選最前方加上「支援的檔案」聯集；設定字串本身不必也不應寫入此項。 */
export function ensureSupportedFilesFilter(filters: LegacyFileFilter[]): LegacyFileFilter[] {
  const categories = filters.filter((filter) => filter.name !== SUPPORTED_FILES_FILTER_NAME);
  const extensions = Array.from(
    new Set(
      (categories.length ? categories : filters).flatMap((filter) =>
        filter.extensions.map((extension) => extension.toLowerCase()),
      ),
    ),
  );
  if (!extensions.length) return filters;
  return [{ name: SUPPORTED_FILES_FILTER_NAME, extensions }, ...categories];
}

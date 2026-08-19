export interface LegacyFileFilter {
  name: string;
  extensions: string[];
}

export const SUPPORTED_FILES_FILTER_NAME = "支援的檔案";

export const DEFAULT_FILE_TYPE_FILTER =
  "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音訊文件|*.mp3;*.ape;*.ogg;*.oga;*.opus>";

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

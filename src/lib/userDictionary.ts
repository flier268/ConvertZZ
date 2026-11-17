import type { DictionaryImportPreview, UserDictionaryEntry } from "@shared/contracts";

/** 匯入與匯出接受的副檔名，與核心 `DICTIONARY_EXTENSIONS` 一致；檔名不限。 */
export const DICTIONARY_FILE_EXTENSIONS = ["txt", "tsv", "csv"];

export interface MergeResult {
  entries: UserDictionaryEntry[];
  added: number;
  /** 表格已涵蓋同樣的對應；方向也相同的詞條改標成這次匯入的檔名與時間。 */
  existing: number;
  /** 同方向同來源詞已對應到別的詞；保留表格原有詞條。 */
  conflicts: number;
}

function sourceKeys(entry: UserDictionaryEntry): string[] {
  const keys: string[] = [];
  if (entry.direction !== "t2s") keys.push(`s2t:${entry.simplified}`);
  if (entry.direction !== "s2t") keys.push(`t2s:${entry.traditional}`);
  return keys;
}

function targetOf(entry: UserDictionaryEntry, key: string): string {
  return key.startsWith("s2t:") ? entry.traditional : entry.simplified;
}

function mappingKey(entry: UserDictionaryEntry): string {
  return `${entry.direction}\t${entry.simplified}\t${entry.traditional}`;
}

/**
 * 把匯入預覽中可匯入的列併入目前表格並標上匯入檔名與時間；
 * 衝突時保留表格原有詞條，不覆寫。
 */
export function mergeImport(
  current: UserDictionaryEntry[],
  preview: Pick<DictionaryImportPreview, "rows" | "sourceFile" | "importedAt">,
): MergeResult {
  const entries = current.map((entry) => ({ ...entry }));
  const targets = new Map<string, string>();
  const mappings = new Map<string, UserDictionaryEntry>();
  for (const entry of entries) {
    mappings.set(mappingKey(entry), entry);
    for (const key of sourceKeys(entry)) {
      if (!targets.has(key)) targets.set(key, targetOf(entry, key));
    }
  }
  const provenance = { importedAt: preview.importedAt, sourceFile: preview.sourceFile };
  let added = 0;
  let existing = 0;
  let conflicts = 0;
  for (const row of preview.rows) {
    if (row.status !== "importable" || !row.direction) continue;
    const candidate: UserDictionaryEntry = {
      simplified: row.simplified,
      traditional: row.traditional,
      direction: row.direction,
      ...provenance,
    };
    const same = mappings.get(mappingKey(candidate));
    if (same) {
      Object.assign(same, provenance);
      existing += 1;
      continue;
    }
    const keys = sourceKeys(candidate);
    if (keys.every((key) => targets.get(key) === targetOf(candidate, key))) {
      existing += 1;
      continue;
    }
    if (
      keys.some((key) => {
        const target = targets.get(key);
        return target !== undefined && target !== targetOf(candidate, key);
      })
    ) {
      conflicts += 1;
      continue;
    }
    for (const key of keys) targets.set(key, targetOf(candidate, key));
    mappings.set(mappingKey(candidate), candidate);
    entries.push(candidate);
    added += 1;
  }
  return { entries, added, existing, conflicts };
}

/** 回傳同方向同來源詞對應到不同目標詞的來源詞，供儲存前提示。 */
export function findConflicts(entries: UserDictionaryEntry[]): string[] {
  const targets = new Map<string, string>();
  const conflicts = new Set<string>();
  for (const entry of entries) {
    for (const key of sourceKeys(entry)) {
      const target = targetOf(entry, key);
      const known = targets.get(key);
      if (known === undefined) targets.set(key, target);
      else if (known !== target) conflicts.add(key.slice(4));
    }
  }
  return [...conflicts];
}

export interface DictionaryFilter {
  keyword: string;
  direction: UserDictionaryEntry["direction"] | "";
  /** 空字串為全部；`MANUAL_SOURCE` 只看手動加入的詞。 */
  source: string;
}

export const MANUAL_SOURCE = "\u0000manual";

export function matchesFilter(entry: UserDictionaryEntry, filter: DictionaryFilter): boolean {
  if (filter.direction && entry.direction !== filter.direction) return false;
  if (filter.source === MANUAL_SOURCE) {
    if (entry.sourceFile) return false;
  } else if (filter.source && entry.sourceFile !== filter.source) return false;
  const keyword = filter.keyword.trim();
  return !keyword || entry.simplified.includes(keyword) || entry.traditional.includes(keyword);
}

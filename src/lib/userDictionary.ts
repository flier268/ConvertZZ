import type { LegacyImportRow, UserDictionaryEntry } from "@shared/contracts";

export interface MergeResult {
  entries: UserDictionaryEntry[];
  added: number;
  /** 表格已有相同簡體、繁體與方向。 */
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

/** 把舊版匯入預覽中可匯入的列併入目前表格；衝突時保留表格原有詞條，不覆寫。 */
export function mergeLegacyRows(
  current: UserDictionaryEntry[],
  rows: LegacyImportRow[],
): MergeResult {
  const entries = current.map((entry) => ({ ...entry }));
  const targets = new Map<string, string>();
  for (const entry of entries) {
    for (const key of sourceKeys(entry)) {
      if (!targets.has(key)) targets.set(key, targetOf(entry, key));
    }
  }
  let added = 0;
  let existing = 0;
  let conflicts = 0;
  for (const row of rows) {
    if (row.status !== "importable" || !row.direction) continue;
    const candidate: UserDictionaryEntry = {
      simplified: row.simplified,
      traditional: row.traditional,
      direction: row.direction,
    };
    const keys = sourceKeys(candidate);
    const known = keys.map((key) => targets.get(key));
    if (known.every((target, index) => target === targetOf(candidate, keys[index]!))) {
      existing += 1;
      continue;
    }
    if (
      known.some(
        (target, index) => target !== undefined && target !== targetOf(candidate, keys[index]!),
      )
    ) {
      conflicts += 1;
      continue;
    }
    for (const key of keys) targets.set(key, targetOf(candidate, key));
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

import { randomUUID } from "node:crypto";
import { COPYFILE_EXCL } from "node:constants";
import { basename, dirname, extname, join } from "node:path";
import { copyFile, readFile, rename, rm, writeFile } from "node:fs/promises";
import { ConvertZZError } from "../errors.js";
import { LegacyDictionary, readDictionaryEntries, type DictionaryEntry } from "../conversion/dictionary.js";
import type { Direction } from "../../../shared/contracts.js";
import { cn2tw, tw2cn } from "cjk-conv";

interface DictionaryChanges {
  updates?: Array<{ index: number; entry: DictionaryEntry }>;
  inserts?: DictionaryEntry[];
  deletes?: number[];
}

export class DictionaryService {
  constructor(private readonly defaultPath?: string) {}

  async read(payload: { path?: string; query?: string; offset?: number; limit?: number; sort?: "source" | "s2t" | "t2s" }) {
    const path = payload.path || this.defaultPath;
    if (!path) throw new ConvertZZError("DICTIONARY_MISSING", "找不到字典路徑。");
    const entries = await readDictionaryEntries(path);
    const query = payload.query?.trim().toLowerCase() ?? "";
    const filtered = query
      ? entries.filter((entry) => `${entry.type}\t${entry.simplified}\t${entry.traditional}`.toLowerCase().includes(query))
      : entries;
    const sorted = [...filtered].sort((left, right) => {
      if (payload.sort === "s2t") return right.simplifiedPriority - left.simplifiedPriority || right.simplified.length - left.simplified.length || left.index - right.index;
      if (payload.sort === "t2s") return right.traditionalPriority - left.traditionalPriority || right.traditional.length - left.traditional.length || left.index - right.index;
      return left.index - right.index;
    });
    const offset = Math.max(0, payload.offset ?? 0);
    const limit = Math.min(500, Math.max(1, payload.limit ?? 100));
    return { path, total: sorted.length, offset, entries: sorted.slice(offset, offset + limit) };
  }

  async update(payload: { path: string } & DictionaryChanges) {
    if (!payload.path) throw new ConvertZZError("DICTIONARY_PATH", "儲存字典前必須選取可寫入檔案。");
    const raw = (await readFile(payload.path, "utf8")).replace(/^\uFEFF/, "");
    const lines = raw.split(/\r?\n/);
    for (const update of payload.updates ?? []) {
      if (update.index < 0 || update.index >= lines.length) throw new ConvertZZError("DICTIONARY_INDEX", "字典項目索引已失效。請重新載入。");
      lines[update.index] = serializeEntry(update.entry);
    }
    for (const index of [...new Set(payload.deletes ?? [])].sort((left, right) => right - left)) {
      if (index < 0 || index >= lines.length) throw new ConvertZZError("DICTIONARY_INDEX", "字典項目索引已失效。請重新載入。");
      lines.splice(index, 1);
    }
    for (const entry of payload.inserts ?? []) lines.push(serializeEntry(entry));
    const temporary = join(dirname(payload.path), `.convertzz-dictionary-${randomUUID()}.csv`);
    const backup = dictionaryBackupPath(payload.path);
    const transactionBackup = join(dirname(payload.path), `.convertzz-dictionary-original-${randomUUID()}.csv`);
    try {
      await copyFile(payload.path, backup, COPYFILE_EXCL);
      await writeFile(temporary, `\uFEFF${lines.join("\n")}`, { flag: "wx" });
      await rename(payload.path, transactionBackup);
      try {
        await rename(temporary, payload.path);
      } catch (error) {
        await rename(transactionBackup, payload.path);
        throw error;
      }
      await rm(transactionBackup).catch(() => undefined);
    } catch (error) {
      await rm(temporary, { force: true });
      throw error;
    }
    return { updated: (payload.updates?.length ?? 0) + (payload.inserts?.length ?? 0) + (payload.deletes?.length ?? 0), backupPath: backup };
  }

  async preview(payload: { path?: string; text: string; direction: Direction } & DictionaryChanges) {
    const path = payload.path || this.defaultPath;
    if (!path) throw new ConvertZZError("DICTIONARY_MISSING", "找不到字典路徑。");
    const deleted = new Set(payload.deletes ?? []);
    const updates = new Map((payload.updates ?? []).map((update) => [update.index, update.entry]));
    const entries = (await readDictionaryEntries(path))
      .filter((entry) => !deleted.has(entry.index))
      .map((entry) => updates.get(entry.index) ?? entry)
      .concat(payload.inserts ?? []);
    const dictionary = LegacyDictionary.fromEntries(entries);
    const fallback = (text: string) => payload.direction === "s2t" ? cn2tw(text) : payload.direction === "t2s" ? tw2cn(text) : text;
    return { text: dictionary.replace(payload.text, payload.direction, fallback) };
  }
}

function serializeEntry(entry: DictionaryEntry): string {
  if (!entry.simplified || !entry.traditional) throw new ConvertZZError("DICTIONARY_ENTRY", "簡體與繁體欄位不能留空。");
  return [entry.enabled, entry.type, entry.simplified, entry.simplifiedPriority, entry.traditional, entry.traditionalPriority].join("\t");
}

function dictionaryBackupPath(path: string): string {
  const extension = extname(path) || ".csv";
  const stem = basename(path, extname(path));
  const timestamp = new Date().toISOString().replace(/[^0-9]/gu, "").slice(0, 14);
  return join(dirname(path), `${stem}.backup-${timestamp}-${randomUUID().slice(0, 8)}${extension}`);
}

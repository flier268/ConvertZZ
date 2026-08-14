import { readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import type { Direction } from "../../../shared/contracts.js";

export interface DictionaryEntry {
  enabled: boolean;
  type: string;
  simplified: string;
  simplifiedPriority: number;
  traditional: string;
  traditionalPriority: number;
}

export interface IndexedDictionaryEntry extends DictionaryEntry {
  index: number;
}

export async function readDictionaryEntries(path: string): Promise<IndexedDictionaryEntry[]> {
  const raw = (await readFile(path, "utf8")).replace(/^\uFEFF/, "");
  return raw
    .split(/\r?\n/)
    .map((line, index): IndexedDictionaryEntry | undefined => {
      if (!line) return undefined;
      const columns = line.split("\t");
      if (columns.length < 6) return undefined;
      return {
        index,
        enabled: /^(true|1)$/i.test(columns[0]),
        type: columns[1] ?? "",
        simplified: columns[2] ?? "",
        simplifiedPriority: Number(columns[3]) || 0,
        traditional: columns[4] ?? "",
        traditionalPriority: Number(columns[5]) || 0,
      };
    })
    .filter((entry): entry is IndexedDictionaryEntry => Boolean(entry));
}

interface TrieNode {
  children: Map<string, TrieNode>;
  replacement?: string;
  order?: number;
}

class ReplacementTrie {
  private readonly root: TrieNode = { children: new Map() };

  constructor(entries: Array<{ source: string; target: string; order: number }>) {
    for (const entry of entries) {
      if (!entry.source) continue;
      let node = this.root;
      for (const character of entry.source) {
        let child = node.children.get(character);
        if (!child) {
          child = { children: new Map() };
          node.children.set(character, child);
        }
        node = child;
      }
      if (node.order === undefined || entry.order < node.order) {
        node.replacement = entry.target;
        node.order = entry.order;
      }
    }
  }

  replace(input: string, fallback: (text: string) => string): string {
    const characters = Array.from(input);
    const output: string[] = [];
    let unmatched: string[] = [];
    const flushUnmatched = () => {
      if (!unmatched.length) return;
      output.push(fallback(unmatched.join("")));
      unmatched = [];
    };

    for (let index = 0; index < characters.length; ) {
      let node = this.root;
      let cursor = index;
      let match: { end: number; replacement: string; order: number } | undefined;

      while (cursor < characters.length) {
        const child = node.children.get(characters[cursor]);
        if (!child) break;
        node = child;
        cursor += 1;
        if (node.replacement !== undefined && node.order !== undefined) {
          if (!match || node.order < match.order) {
            match = { end: cursor, replacement: node.replacement, order: node.order };
          }
        }
      }

      if (match) {
        flushUnmatched();
        output.push(match.replacement);
        index = match.end;
      } else {
        unmatched.push(characters[index]);
        index += 1;
      }
    }

    flushUnmatched();
    return output.join("");
  }
}

export class LegacyDictionary {
  private constructor(
    readonly entries: DictionaryEntry[],
    private readonly s2t: ReplacementTrie,
    private readonly t2s: ReplacementTrie,
    private readonly protectedWords: string[],
  ) {}

  static async load(path: string): Promise<LegacyDictionary> {
    return LegacyDictionary.fromEntries(await readDictionaryEntries(path));
  }

  static fromEntries(input: DictionaryEntry[]): LegacyDictionary {
    const entries = input;

    const buildEntries = (direction: Direction) => {
      const seen = new Set<string>();
      return entries
        .map((entry) => ({
          source: direction === "s2t" ? entry.simplified : entry.traditional,
          target: direction === "s2t" ? entry.traditional : entry.simplified,
          priority:
            direction === "s2t" ? entry.simplifiedPriority : entry.traditionalPriority,
          enabled: entry.enabled,
        }))
        .filter((entry) => {
          if (!entry.source || seen.has(entry.source)) return false;
          seen.add(entry.source);
          return true;
        })
        .filter((entry) => entry.enabled)
        .sort((a, b) => b.priority - a.priority || b.source.length - a.source.length)
        .map((entry, order) => ({ ...entry, order }));
    };

    const protectedWords = entries
      .filter(
        (entry) =>
          entry.enabled &&
          entry.simplifiedPriority === 9999 &&
          entry.traditionalPriority === 9999 &&
          entry.simplified === entry.traditional,
      )
      .map((entry) => entry.simplified)
      .sort((a, b) => b.length - a.length);

    return new LegacyDictionary(
      entries.filter((entry) => entry.enabled),
      new ReplacementTrie(buildEntries("s2t")),
      new ReplacementTrie(buildEntries("t2s")),
      protectedWords,
    );
  }

  replace(input: string, direction: Direction, baseConvert: (text: string) => string): string {
    if (direction === "none") return input;
    const placeholders = new Map<string, string>();
    let protectedText = input;

    for (const word of this.protectedWords) {
      if (!protectedText.includes(word)) continue;
      const token = `__CONVERTZZ_${randomUUID().replaceAll("-", "")}_${placeholders.size}__`;
      placeholders.set(token, word);
      protectedText = protectedText.split(word).join(token);
    }

    let converted = direction === "s2t"
      ? this.s2t.replace(protectedText, baseConvert)
      : this.t2s.replace(protectedText, baseConvert);
    for (const [token, word] of placeholders) converted = converted.split(token).join(word);
    return converted;
  }
}

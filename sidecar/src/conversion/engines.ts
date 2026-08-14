import Segment from "novel-segment";
import { cjk2zht, cn2tw, tw2cn } from "cjk-conv";
import { performance } from "node:perf_hooks";
import { stat } from "node:fs/promises";
import type { ConversionRequest, ConversionResult, Direction } from "../../../shared/contracts.js";
import { LegacyDictionary } from "./dictionary.js";
import { ZhConvertClient } from "./zhconvert.js";
import { ConvertZZError } from "../errors.js";

const MAX_CHUNK_CHARACTERS = 8_192;

export class ConversionService {
  private readonly segmenter: InstanceType<typeof Segment>;
  private readonly dictionaries = new Map<
    string,
    { mtimeMs: number; value: Promise<LegacyDictionary> }
  >();

  constructor(
    private readonly defaultDictionaryPath: string | undefined,
    readonly zhconvert = new ZhConvertClient(),
  ) {
    this.segmenter = new Segment();
    this.segmenter.useDefault({ all_mod: true });
  }

  async convert(request: ConversionRequest): Promise<ConversionResult> {
    const startedAt = performance.now();
    const warnings: string[] = [];
    let text = request.text;

    if (request.direction !== "none" && text) {
      if (request.vocabularyCorrection === false) {
        text = baseConvert(text, request.direction);
        warnings.push("詞彙修正已停用。本次只執行 cjk-conv 字形轉換。");
      } else if (request.engine === "segmented") {
        text = this.segmentedConvert(text, request.direction);
      } else if (request.engine === "legacy") {
        const dictionaryPath = request.dictionaryPath ?? this.defaultDictionaryPath;
        if (!dictionaryPath) throw new ConvertZZError("DICTIONARY_MISSING", "找不到舊版字典。");
        const dictionary = await this.getDictionary(dictionaryPath);
        text = dictionary.replace(text, request.direction, (value) =>
          baseConvert(value, request.direction),
        );
        warnings.push("未命中字元使用跨平台 cjk-conv，結果可能與舊版 Windows 映射略有差異。");
      } else {
        text = await this.zhconvert.convert(text, request.direction, request.zhconvert);
      }
    }

    return {
      text,
      engine: request.engine,
      direction: request.direction,
      warnings,
      durationMs: Math.round((performance.now() - startedAt) * 100) / 100,
    };
  }

  private segmentedConvert(text: string, direction: Direction): string {
    return splitText(text)
      .map((chunk) => {
        const source =
          direction === "s2t"
            ? this.segmenter
                .doSegment(chunk, {
                  simple: true,
                  stripPunctuation: false,
                  stripStopword: false,
                  stripSpace: false,
                  convertSynonym: false,
                })
                .map((word) => cjk2zht(word))
                .join("")
            : chunk;
        const words = this.segmenter.doSegment(source, {
          simple: true,
          stripPunctuation: false,
          stripStopword: false,
          stripSpace: false,
          convertSynonym: direction === "s2t",
        });
        const segmented = words.join("");
        if (direction === "s2t") return cjk2zht(segmented);
        return baseConvert(segmented, direction);
      })
      .join("");
  }

  private async getDictionary(path: string): Promise<LegacyDictionary> {
    const mtimeMs = (await stat(path)).mtimeMs;
    let dictionary = this.dictionaries.get(path);
    if (!dictionary || dictionary.mtimeMs !== mtimeMs) {
      dictionary = { mtimeMs, value: LegacyDictionary.load(path) };
      this.dictionaries.set(path, dictionary);
    }
    return dictionary.value;
  }
}

function baseConvert(text: string, direction: Direction): string {
  if (direction === "s2t") return cn2tw(text);
  if (direction === "t2s") return tw2cn(text);
  return text;
}

function splitText(text: string): string[] {
  if (text.length <= MAX_CHUNK_CHARACTERS) return [text];
  const chunks: string[] = [];
  let remaining = text;
  while (remaining.length > MAX_CHUNK_CHARACTERS) {
    let end = MAX_CHUNK_CHARACTERS;
    const candidate = remaining.slice(0, end);
    const natural = Math.max(candidate.lastIndexOf("\n"), candidate.lastIndexOf("。"));
    if (natural > MAX_CHUNK_CHARACTERS / 2) end = natural + 1;
    if (/^[\uDC00-\uDFFF]$/u.test(remaining[end] ?? "")) end -= 1;
    chunks.push(remaining.slice(0, end));
    remaining = remaining.slice(end);
  }
  if (remaining) chunks.push(remaining);
  return chunks;
}

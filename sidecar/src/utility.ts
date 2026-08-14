import type { UtilityConvertRequest } from "../../shared/contracts.js";
import { reinterpretText } from "./encoding/codecs.js";

const SYMBOL_TABLE = new Map<string, string>([
  [",", "，"],
  ["~", "～"],
  ["!", "！"],
  ["#", "＃"],
  ["$", "＄"],
  ["%", "％"],
  ["^", "︿"],
  ["&", "＆"],
  ["*", "＊"],
  ["-", "－"],
  ["+", "＋"],
  ["{", "｛"],
  ["}", "｝"],
  [";", "；"],
  ["|", "｜"],
  ["?", "？"],
  ["(", "（"],
  [")", "）"],
  ["“", "「"],
  ["”", "」"],
  ["‘", "『"],
  ["’", "』"],
  ["[", "［"],
  ["]", "］"],
  [" ", "　"],
  [":", "："],
  [".", "。"],
  ['"', "、"],
  ["@", "＠"],
  ["<", "＜"],
  [">", "＞"],
  ["=", "＝"],
]);
const REVERSE_SYMBOL_TABLE = new Map(
  Array.from(SYMBOL_TABLE, ([source, target]) => [target, source]),
);

export function convertUtility(request: UtilityConvertRequest): string {
  switch (request.kind) {
    case "html-decimal-encode":
      return htmlEncode(request.text, 10);
    case "html-decimal-decode":
      return htmlDecode(request.text);
    case "html-hex-encode":
      return htmlEncode(request.text, 16);
    case "html-hex-decode":
      return htmlDecode(request.text);
    case "unicode-escape-encode":
      return unicodeEscapeEncode(request.text);
    case "unicode-escape-decode":
      return unicodeEscapeDecode(request.text);
    case "encoding":
      return reinterpretText(
        request.text,
        request.sourceEncoding ?? "utf8",
        request.targetEncoding ?? "utf8",
      );
    case "fullwidth":
      return replaceSymbols(request.text, SYMBOL_TABLE);
    case "halfwidth":
      return replaceSymbols(request.text, REVERSE_SYMBOL_TABLE);
  }
}

function htmlEncode(text: string, radix: 10 | 16): string {
  return Array.from(text, (character) => {
    if (character === "&") return "&amp;";
    if (character === "<") return "&lt;";
    if (character === ">") return "&gt;";
    const codePoint = character.codePointAt(0) ?? 0;
    if ((codePoint >= 0x20 && codePoint <= 0x7e) || character === "\r" || character === "\n")
      return character;
    return radix === 10 ? `&#${codePoint};` : `&#x${codePoint.toString(16).toUpperCase()};`;
  }).join("");
}

function htmlDecode(text: string): string {
  return text
    .replace(/&#x([\da-f]+);?/giu, (match, value: string) => safeCodePoint(match, value, 16))
    .replace(/&#(\d+);?/gu, (match, value: string) => safeCodePoint(match, value, 10))
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

function safeCodePoint(original: string, value: string, radix: 10 | 16): string {
  const codePoint = Number.parseInt(value, radix);
  return Number.isInteger(codePoint) && codePoint >= 0 && codePoint <= 0x10ffff
    ? String.fromCodePoint(codePoint)
    : original;
}

function unicodeEscapeEncode(text: string): string {
  return Array.from(text, (character) => {
    const codePoint = character.codePointAt(0) ?? 0;
    if (codePoint <= 0xffff) return `\\u${codePoint.toString(16).toUpperCase().padStart(4, "0")}`;
    const adjusted = codePoint - 0x10000;
    const high = 0xd800 + (adjusted >> 10);
    const low = 0xdc00 + (adjusted & 0x3ff);
    return `\\u${high.toString(16).toUpperCase()}\\u${low.toString(16).toUpperCase()}`;
  }).join("");
}

function unicodeEscapeDecode(text: string): string {
  return text
    .replace(/\\u\{([\da-f]{1,6})\}/giu, (match, value: string) => safeCodePoint(match, value, 16))
    .replace(/(?:\\u[\da-f]{4})+/giu, (sequence) => {
      const units =
        sequence.match(/[\da-f]{4}/giu)?.map((value) => Number.parseInt(value, 16)) ?? [];
      return String.fromCharCode(...units);
    });
}

function replaceSymbols(text: string, table: Map<string, string>): string {
  return Array.from(text, (character) => table.get(character) ?? character).join("");
}

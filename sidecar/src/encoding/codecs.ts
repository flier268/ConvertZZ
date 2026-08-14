import chardet from "chardet";
import iconv from "iconv-lite";
import EncodingJapanese from "encoding-japanese";
import type { TextEncoding } from "../../../shared/contracts.js";
import { ConvertZZError } from "../errors.js";

const aliases: Record<string, TextEncoding> = {
  UTF8: "utf8",
  "UTF-8": "utf8",
  UTF16LE: "utf16le",
  "UTF-16LE": "utf16le",
  UTF16BE: "utf16be",
  "UTF-16BE": "utf16be",
  BIG5: "big5",
  BIG5HKSCS: "big5",
  GB18030: "gbk",
  GB2312: "gbk",
  GBK: "gbk",
  SHIFTJIS: "shift-jis",
  "SHIFT-JIS": "shift-jis",
  SJIS: "shift-jis",
  EUCJP: "euc-jp",
  "EUC-JP": "euc-jp",
  ISO2022JP: "iso-2022-jp",
  "ISO-2022-JP": "iso-2022-jp",
};

export function detectEncoding(buffer: Buffer): TextEncoding {
  if (buffer.length >= 3 && buffer[0] === 0xef && buffer[1] === 0xbb && buffer[2] === 0xbf) return "utf8-bom";
  if (buffer.length >= 2 && buffer[0] === 0xff && buffer[1] === 0xfe) return "utf16le";
  if (buffer.length >= 2 && buffer[0] === 0xfe && buffer[1] === 0xff) return "utf16be";
  const declaration = buffer.subarray(0, Math.min(buffer.length, 16 * 1024)).toString("latin1")
    .match(/(?:charset\s*=\s*["']?|@charset\s+["'])([a-z\d_-]+)/i)?.[1];
  if (declaration) {
    const declared = aliases[declaration.toUpperCase().replaceAll("_", "-")];
    if (declared) return declared;
  }
  if (/~\{[!-~]{2}/.test(buffer.subarray(0, 8192).toString("ascii"))) return "hz-gb-2312";
  const detected = chardet.detect(buffer.subarray(0, Math.min(buffer.length, 128 * 1024)));
  if (!detected) return "utf8";
  return aliases[detected.toUpperCase().replaceAll("_", "-")] ?? "utf8";
}

export function decodeText(buffer: Buffer, requested: TextEncoding): { text: string; encoding: TextEncoding } {
  const encoding = requested === "auto" ? detectEncoding(buffer) : requested;
  const withoutBom = stripBom(buffer, encoding);

  if (encoding === "hz-gb-2312") return { text: decodeHz(withoutBom), encoding };
  if (encoding === "iso-2022-jp" || encoding === "euc-jp") {
    const from = encoding === "iso-2022-jp" ? "JIS" : "EUCJP";
    return { text: String(EncodingJapanese.convert(Array.from(withoutBom), { from, to: "UNICODE", type: "string" })), encoding };
  }
  return { text: iconv.decode(withoutBom, iconvName(encoding)), encoding };
}

export function encodeText(text: string, encoding: TextEncoding, addBom = false): Buffer {
  if (encoding === "auto") throw new ConvertZZError("ENCODING_AUTO_OUTPUT", "輸出編碼不能使用自動偵測。");
  if (encoding === "hz-gb-2312") return encodeHz(text);
  if (encoding === "iso-2022-jp" || encoding === "euc-jp") {
    const to = encoding === "iso-2022-jp" ? "JIS" : "EUCJP";
    const converted = EncodingJapanese.convert(text, { from: "UNICODE", to, type: "array" });
    return Buffer.from(converted as number[]);
  }
  const shouldAddBom = addBom || encoding === "utf8-bom";
  const normalized = encoding === "utf8-bom" ? "utf8" : encoding;
  return iconv.encode(text, iconvName(normalized), { addBOM: shouldAddBom });
}

export function reinterpretText(text: string, source: TextEncoding, target: TextEncoding): string {
  if (source === "auto" || target === "auto") throw new ConvertZZError("ENCODING_REQUIRED", "重新解讀文字時必須指定來源與目標編碼。");
  return decodeText(encodeText(text, source), target).text;
}

function iconvName(encoding: TextEncoding): "utf8" | "utf16-le" | "utf16-be" | "big5" | "gbk" | "shift_jis" {
  switch (encoding) {
    case "utf8":
    case "utf8-bom":
      return "utf8";
    case "utf16le":
      return "utf16-le";
    case "utf16be":
      return "utf16-be";
    case "big5":
      return "big5";
    case "gbk":
      return "gbk";
    case "shift-jis":
      return "shift_jis";
    default:
      throw new ConvertZZError("ENCODING_UNSUPPORTED", `不支援編碼 ${encoding}。`);
  }
}

function stripBom(buffer: Buffer, encoding: TextEncoding): Buffer {
  if ((encoding === "utf8" || encoding === "utf8-bom") && buffer.subarray(0, 3).equals(Buffer.from([0xef, 0xbb, 0xbf]))) return buffer.subarray(3);
  if (encoding === "utf16le" && buffer.subarray(0, 2).equals(Buffer.from([0xff, 0xfe]))) return buffer.subarray(2);
  if (encoding === "utf16be" && buffer.subarray(0, 2).equals(Buffer.from([0xfe, 0xff]))) return buffer.subarray(2);
  return buffer;
}

function decodeHz(buffer: Buffer): string {
  const bytes: number[] = [];
  let ascii = "";
  let chinese = false;
  const flushAscii = () => {
    if (!ascii) return;
    bytes.push(...Buffer.from(ascii, "ascii"));
    ascii = "";
  };

  for (let index = 0; index < buffer.length; index += 1) {
    const value = buffer[index];
    if (value === 0x7e && index + 1 < buffer.length) {
      const next = buffer[index + 1];
      if (next === 0x7b) {
        flushAscii();
        chinese = true;
        index += 1;
        continue;
      }
      if (next === 0x7d) {
        chinese = false;
        index += 1;
        continue;
      }
      if (next === 0x7e) {
        ascii += "~";
        index += 1;
        continue;
      }
      if (next === 0x0a) {
        index += 1;
        continue;
      }
    }
    if (chinese) {
      if (index + 1 >= buffer.length) break;
      flushAscii();
      bytes.push(value | 0x80, buffer[++index] | 0x80);
    } else {
      ascii += String.fromCharCode(value);
    }
  }
  flushAscii();
  return iconv.decode(Buffer.from(bytes), "gb2312");
}

function encodeHz(text: string): Buffer {
  let output = "";
  let chinese = false;
  for (const character of text) {
    const encoded = iconv.encode(character, "gb2312");
    const isAscii = encoded.length === 1 && encoded[0] < 0x80;
    if (isAscii) {
      if (chinese) {
        output += "~}";
        chinese = false;
      }
      output += character === "~" ? "~~" : character;
      continue;
    }
    if (encoded.length !== 2) throw new ConvertZZError("HZ_CHARACTER", `字元「${character}」無法使用 HZ-GB-2312 表示。`);
    if (!chinese) {
      output += "~{";
      chinese = true;
    }
    output += String.fromCharCode(encoded[0] & 0x7f, encoded[1] & 0x7f);
  }
  if (chinese) output += "~}";
  return Buffer.from(output, "ascii");
}

import { describe, expect, it } from "vitest";
import type { TextEncoding } from "../../../shared/contracts.js";
import { decodeText, detectEncoding, encodeText } from "./codecs.js";

describe("文字編碼", () => {
  it.each<TextEncoding>(["utf8", "utf8-bom", "utf16le", "utf16be", "big5", "gbk", "shift-jis", "euc-jp", "iso-2022-jp", "hz-gb-2312"])(
    "往返 %s",
    (encoding) => {
      const source = encoding === "hz-gb-2312" ? "中文 ABC~" : "中文 テスト ABC";
      const encoded = encodeText(source, encoding, encoding === "utf8-bom");
      expect(decodeText(encoded, encoding).text).toBe(source);
    },
  );

  it("先判斷 BOM", () => {
    expect(detectEncoding(encodeText("測試", "utf8-bom"))).toBe("utf8-bom");
    expect(detectEncoding(encodeText("測試", "utf16le", true))).toBe("utf16le");
    expect(detectEncoding(encodeText("測試", "utf16be", true))).toBe("utf16be");
  });
});

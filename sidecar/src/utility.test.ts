import { describe, expect, it } from "vitest";
import { convertUtility } from "./utility.js";

describe("舊版文字工具", () => {
  it("只將非 ASCII 文字與特殊 HTML 字元轉為十進位實體", () => {
    expect(convertUtility({ kind: "html-decimal-encode", text: "A<&裡\n" }))
      .toBe("A&lt;&amp;&#35041;\n");
  });

  it("可混合解碼十進位、十六進位與具名實體", () => {
    expect(convertUtility({ kind: "html-hex-decode", text: "A&#35041; &#x958B; &lt;&amp;&gt;" }))
      .toBe("A裡 開 <&>");
  });

  it("可往返 Unicode 跳脫與代理對", () => {
    const encoded = convertUtility({ kind: "unicode-escape-encode", text: "A裡😀" });
    expect(encoded).toBe("\\u0041\\u88E1\\uD83D\\uDE00");
    expect(convertUtility({ kind: "unicode-escape-decode", text: encoded })).toBe("A裡😀");
  });

  it("沿用舊版標點與全半形對照", () => {
    expect(convertUtility({ kind: "fullwidth", text: "A, .\"“”" })).toBe("A，　。、「」");
    expect(convertUtility({ kind: "halfwidth", text: "A，　。、「」" })).toBe("A, .\"“”");
  });
});

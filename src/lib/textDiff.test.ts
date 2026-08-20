import { describe, expect, it } from "vitest";
import { buildSideBySideDiff, diffText, escapeHtml, sideBySideToHtml } from "./textDiff";
import { buildInterleavedDiffPair } from "./textDiff.fixtures";

describe("textDiff", () => {
  it("相同文字只回傳 equal", () => {
    expect(diffText("測試", "測試")).toEqual([{ value: "測試", kind: "equal" }]);
  });

  it("標出簡繁字元差異", () => {
    expect(diffText("里面开发头发", "裡面開發頭髮")).toEqual([
      { value: "里", kind: "delete" },
      { value: "裡", kind: "insert" },
      { value: "面", kind: "equal" },
      { value: "开", kind: "delete" },
      { value: "開", kind: "insert" },
      { value: "发", kind: "delete" },
      { value: "發", kind: "insert" },
      { value: "头", kind: "delete" },
      { value: "頭", kind: "insert" },
      { value: "发", kind: "delete" },
      { value: "髮", kind: "insert" },
    ]);
  });

  it("並排檢視把刪除放左側、插入放右側", () => {
    expect(buildSideBySideDiff("简体字", "簡體字")).toEqual({
      left: [
        { text: "简体", kind: "change" },
        { text: "字", kind: "equal" },
      ],
      right: [
        { text: "簡體", kind: "change" },
        { text: "字", kind: "equal" },
      ],
    });
  });

  it("保留換行並處理單側空白", () => {
    expect(diffText("甲\n乙", "甲\n丙")).toEqual([
      { value: "甲\n", kind: "equal" },
      { value: "乙", kind: "delete" },
      { value: "丙", kind: "insert" },
    ]);
    expect(diffText("", "新增")).toEqual([{ value: "新增", kind: "insert" }]);
    expect(diffText("刪除", "")).toEqual([{ value: "刪除", kind: "delete" }]);
  });

  it("大量交錯錨點仍能標出每一處差異", () => {
    const { source, output } = buildInterleavedDiffPair(200);
    const sides = buildSideBySideDiff(source, output);
    expect(sides.left.filter((span) => span.kind === "change")).toHaveLength(200);
    expect(sides.right.filter((span) => span.kind === "change")).toHaveLength(200);
  });

  it("等長長文以位置對齊標出逐字差異", () => {
    const source = `前綴${"简".repeat(50)}後綴`;
    const output = `前綴${"簡".repeat(50)}後綴`;
    const sides = buildSideBySideDiff(source, output);
    expect(sides.left[0]).toEqual({ text: "前綴", kind: "equal" });
    expect(sides.left.some((span) => span.kind === "change" && span.text.includes("简"))).toBe(
      true,
    );
    expect(sides.right.some((span) => span.kind === "change" && span.text.includes("簡"))).toBe(
      true,
    );
    expect(sides.left.at(-1)).toEqual({ text: "後綴", kind: "equal" });
  });

  it("v-html 輸出會跳脫特殊字元", () => {
    expect(escapeHtml(`a<b>&"'`)).toBe("a&lt;b&gt;&amp;&quot;&#39;");
    expect(sideBySideToHtml([{ text: "<x>", kind: "change" }], "diff-add")).toBe(
      '<mark class="diff-change diff-add">&lt;x&gt;</mark>',
    );
  });
});

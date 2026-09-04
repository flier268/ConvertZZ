import { describe, expect, it } from "vitest";
import {
  buildPagedSideBySideDiff,
  buildSideBySideDiff,
  diffText,
  escapeHtml,
  sideBySideToHtml,
} from "./textDiff";
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

  it("長文會依碼點預算分頁且保留變更標記", () => {
    const source = `${"甲".repeat(30)}简体${"乙".repeat(30)}开发${"丙".repeat(30)}`;
    const output = `${"甲".repeat(30)}簡體${"乙".repeat(30)}開發${"丙".repeat(30)}`;
    const pages = buildPagedSideBySideDiff(source, output, 40);
    expect(pages.length).toBeGreaterThan(1);
    expect(pages.some((page) => page.hasChanges)).toBe(true);
    const leftText = pages.flatMap((page) => page.left.map((span) => span.text)).join("");
    const rightText = pages.flatMap((page) => page.right.map((span) => span.text)).join("");
    expect(leftText).toBe(source);
    expect(rightText).toBe(output);
  });

  it("超過字元差限的單行長文仍只標出真正變更", () => {
    const source = `${"甲".repeat(2200)}里面开发头发${"乙".repeat(2200)}`;
    const output = `${"甲".repeat(2200)}裡面開發頭髮${"乙".repeat(2200)}`;
    const sides = buildSideBySideDiff(source, output);
    const leftChanges = sides.left
      .filter((span) => span.kind === "change")
      .map((span) => span.text);
    const rightChanges = sides.right
      .filter((span) => span.kind === "change")
      .map((span) => span.text);
    // 「面」簡繁相同，不會進變更；其餘簡繁字元應逐一標出。
    expect(leftChanges.join("")).toBe("里开发头发");
    expect(rightChanges.join("")).toBe("裡開發頭髮");
    expect(sides.left.some((span) => span.kind === "equal" && span.text.includes("面"))).toBe(true);
    expect(sides.left[0]).toEqual({ text: "甲".repeat(2200), kind: "equal" });
    expect(sides.left.at(-1)).toEqual({ text: "乙".repeat(2200), kind: "equal" });
  });

  it("成對簡繁變更盡量留在同一頁", () => {
    const source = `${"前".repeat(18)}里面`;
    const output = `${"前".repeat(18)}裡面`;
    const pages = buildPagedSideBySideDiff(source, output, 20);
    const changePages = pages.filter((page) => page.hasChanges);
    expect(changePages).toHaveLength(1);
    expect(changePages[0]?.left.some((span) => span.kind === "change")).toBe(true);
    expect(changePages[0]?.right.some((span) => span.kind === "change")).toBe(true);
  });
});

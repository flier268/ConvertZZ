import { describe, expect, it } from "vitest";
import {
  dropIgnoredInputWarnings,
  ignoredInputsConfirmMessage,
  withoutIgnoredInputs,
} from "./ignoredInputs";

describe("版本控制資料夾輸入", () => {
  it("確認訊息列出路徑並提醒可能損壞儲存庫", () => {
    const message = ignoredInputsConfirmMessage(["/repo/.git"]);
    expect(message).toContain("/repo/.git");
    expect(message).toContain("版本控制資料夾");
    expect(message).toContain("損壞儲存庫");
  });

  it("超過五項時只列前五項並附總數", () => {
    const paths = Array.from({ length: 7 }, (_, index) => `/r${index}/.git`);
    const message = ignoredInputsConfirmMessage(paths);
    expect(message).toContain("/r4/.git");
    expect(message).not.toContain("/r5/.git");
    expect(message).toContain("等 7 項");
  });

  it("取消時從來源清單移除被略過的路徑", () => {
    expect(withoutIgnoredInputs(["/a.txt", "/repo/.git", "/b"], ["/repo/.git"])).toEqual([
      "/a.txt",
      "/b",
    ]);
  });

  it("取消後移除對應的略過警告，保留其他警告", () => {
    const warnings = [
      "已略過版本控制資料夾內的路徑：/repo/.git。轉換可能損壞儲存庫；確定要處理請明確允許。",
      "資料夾「/empty」中沒有符合副檔名篩選器的檔案（隱藏資料夾與版本控制資料夾不會掃描）。",
    ];
    expect(dropIgnoredInputWarnings(warnings, ["/repo/.git"])).toEqual([warnings[1]]);
  });
});

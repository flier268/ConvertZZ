import { describe, expect, it } from "vitest";
import { summarizeFileApplyWarnings } from "./fileApplyMessages";

describe("檔案套用警告", () => {
  it("多個內容略過合併成一則", () => {
    expect(
      summarizeFileApplyWarnings([
        "a.mp3：此檔案為二進位內容，已略過內容轉換。",
        "b.mp3：解碼時發生錯誤，已略過內容轉換。",
        "輸出路徑已存在。",
      ]),
    ).toEqual(["已略過 2 個檔案的內容轉換。", "輸出路徑已存在。"]);
  });

  it("只有一則內容略過時保留原警告", () => {
    const warnings = ["a.mp3：此檔案為二進位內容，已略過內容轉換。"];
    expect(summarizeFileApplyWarnings(warnings)).toEqual(warnings);
  });
});

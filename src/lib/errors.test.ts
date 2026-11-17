import { describe, expect, it } from "vitest";
import { formatUnknownError } from "./errors";

describe("formatUnknownError", () => {
  it("保留一般 Error 訊息", () => {
    expect(formatUnknownError(new Error("boom"))).toBe("boom");
  });

  it("空字串與空 Error 訊息要可診斷", () => {
    expect(formatUnknownError("")).toBe("未知錯誤（空字串）");
    expect(formatUnknownError(new Error(""))).toBe("未知錯誤（空訊息）");
  });

  it("Tauri CoreError 形狀要帶出 code 與 details", () => {
    expect(
      formatUnknownError({
        message: "失敗",
        code: "IO_ERROR",
        details: { path: "C:\\a" },
      }),
    ).toBe('失敗 · code=IO_ERROR · details={"path":"C:\\\\a"}');
  });

  it("只有 code 的物件也要可讀", () => {
    expect(formatUnknownError({ message: "", code: "SEGMENTER" })).toBe("code=SEGMENTER");
  });

  it("純字串與 null 要可讀", () => {
    expect(formatUnknownError("plugin missing")).toBe("plugin missing");
    expect(formatUnknownError(null)).toBe("未知錯誤（空值）");
    expect(formatUnknownError(undefined)).toBe("未知錯誤（空值）");
  });
});

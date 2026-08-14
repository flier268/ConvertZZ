import { describe, expect, it } from "vitest";
import { isDialogCancelled } from "./appUpdate";

describe("更新對話框", () => {
  it("辨識使用者取消", () => {
    expect(isDialogCancelled("cancel")).toBe(true);
    expect(isDialogCancelled("close")).toBe(true);
    expect(isDialogCancelled(new Error("網路失敗"))).toBe(false);
  });
});

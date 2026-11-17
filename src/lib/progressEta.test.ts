import { describe, expect, it } from "vitest";
import {
  estimateRemainingSeconds,
  formatDurationSeconds,
  formatProgressLabel,
  progressPercentage,
} from "./progressEta";

describe("progressEta", () => {
  it("estimates remaining time from throughput", () => {
    const remaining = estimateRemainingSeconds(
      { current: 25, total: 100, message: "轉換中" },
      1_000,
      6_000,
    );
    expect(remaining).toBeCloseTo(15, 5);
  });

  it("formats durations and progress labels", () => {
    expect(formatDurationSeconds(65)).toBe("1 分 5 秒");
    expect(progressPercentage({ current: 1, total: 4, message: "" })).toBe(25);
    expect(
      formatProgressLabel({ current: 25, total: 100, message: "正在轉換" }, 1_000, 6_000),
    ).toContain("約剩");
  });
});

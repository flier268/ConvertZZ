import { describe, expect, it } from "vitest";
import { buildSideBySideDiff } from "./textDiff";
import { buildInterleavedDiffPair } from "./textDiff.fixtures";

describe("textDiff 效能", () => {
  it("3000 個獨立差異點的計算耗時", () => {
    const { source, output } = buildInterleavedDiffPair(3000);
    const started = performance.now();
    const sides = buildSideBySideDiff(source, output);
    const elapsedMs = performance.now() - started;
    const leftChanges = sides.left.filter((span) => span.kind === "change").length;
    const rightChanges = sides.right.filter((span) => span.kind === "change").length;

    expect(leftChanges).toBe(3000);
    expect(rightChanges).toBe(3000);
    expect(sides.left.length).toBe(6000);
    expect(sides.right.length).toBe(6000);
    expect(elapsedMs).toBeLessThan(100);
    console.info(
      JSON.stringify({
        case: "textDiff-3000-independent-changes",
        elapsedMs: Math.round(elapsedMs * 100) / 100,
        sourceChars: Array.from(source).length,
        leftSpans: sides.left.length,
        rightSpans: sides.right.length,
        leftChanges,
        rightChanges,
      }),
    );
  });

  it("對照：100／1000／3000 差異點耗時曲線", () => {
    const samples = [100, 1000, 3000].map((changes) => {
      const { source, output } = buildInterleavedDiffPair(changes);
      const started = performance.now();
      const sides = buildSideBySideDiff(source, output);
      const elapsedMs = performance.now() - started;
      return {
        changes,
        elapsedMs: Math.round(elapsedMs * 100) / 100,
        leftChanges: sides.left.filter((span) => span.kind === "change").length,
      };
    });

    for (const sample of samples) expect(sample.leftChanges).toBe(sample.changes);
    expect(samples[2]!.elapsedMs).toBeLessThan(100);
    console.info(JSON.stringify({ case: "textDiff-scaling", samples }));
  });
});

import { afterEach, describe, expect, it, vi } from "vitest";
import { AUDIO_PREVIEW_DEBOUNCE_MS, createAudioPreviewController } from "./audioReview";

afterEach(() => {
  vi.useRealTimers();
});

describe("音訊預覽自動重建", () => {
  it("防抖把連續變更收成一次，後完成的舊要求不會蓋過新結果", async () => {
    vi.useFakeTimers();
    const resolvers: Array<(value: { planId: string }) => void> = [];
    const applied: string[] = [];
    const cancels: number[] = [];
    const controller = createAudioPreviewController({
      plan: () =>
        new Promise((resolve) => {
          resolvers.push(resolve);
        }),
      cancelInFlight: async () => {
        cancels.push(cancels.length + 1);
      },
      apply: (value) => {
        applied.push((value as { planId: string }).planId);
      },
    });

    controller.schedule();
    controller.schedule();
    await vi.advanceTimersByTimeAsync(AUDIO_PREVIEW_DEBOUNCE_MS - 1);
    expect(resolvers).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    expect(resolvers).toHaveLength(1);
    expect(cancels).toHaveLength(1);

    controller.schedule();
    await vi.advanceTimersByTimeAsync(AUDIO_PREVIEW_DEBOUNCE_MS);
    expect(resolvers).toHaveLength(2);
    expect(cancels).toHaveLength(2);

    resolvers[1]!({ planId: "new" });
    resolvers[0]!({ planId: "old" });
    await vi.runAllTimersAsync();
    await Promise.resolve();
    expect(applied).toEqual(["new"]);
  });

  it("排程後開始套用時，不得再取消進行中的套用", async () => {
    vi.useFakeTimers();
    const cancels: number[] = [];
    const plans: number[] = [];
    const controller = createAudioPreviewController({
      plan: async () => {
        plans.push(plans.length + 1);
        return { planId: "preview" };
      },
      cancelInFlight: async () => {
        cancels.push(cancels.length + 1);
      },
      apply: () => undefined,
    });

    controller.schedule();
    // 套用開始：進行中的 audio.apply 不可被稍後觸發的預覽取消。
    controller.invalidate();
    await vi.advanceTimersByTimeAsync(AUDIO_PREVIEW_DEBOUNCE_MS);
    expect(cancels).toEqual([]);
    expect(plans).toEqual([]);
  });
});

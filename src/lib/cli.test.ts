import { beforeEach, describe, expect, it } from "vitest";
import { applyParsedCliNavigation, cliInvocation } from "./cli";
import type { ParsedCli } from "@shared/contracts";

function baseCli(overrides: Partial<ParsedCli> = {}): ParsedCli {
  return {
    mode: "file",
    paths: ["a.txt"],
    inputEncoding: "auto",
    outputEncoding: "auto",
    direction: "s2t",
    engine: "segmented",
    operation: "content",
    vocabularyCorrection: "settings",
    backup: true,
    headless: false,
    confirmWrite: false,
    outputEncodingExplicit: false,
    directionExplicit: false,
    ...overrides,
  };
}

describe("applyParsedCliNavigation", () => {
  beforeEach(() => {
    cliInvocation.value = undefined;
  });

  it("audio + both 先掛載檔案頁再切音訊，並佇列檔名與標籤兩次 invocation", async () => {
    const pages: string[] = [];
    const snapshots: Array<{ mode: string; operation: string }> = [];

    await applyParsedCliNavigation(
      baseCli({ mode: "audio", operation: "both", paths: ["s.mp3"] }),
      (page) => {
        pages.push(page);
        const options = cliInvocation.value?.options;
        if (options) snapshots.push({ mode: options.mode, operation: options.operation });
      },
    );

    expect(pages).toEqual(["files", "audio"]);
    expect(snapshots).toContainEqual({ mode: "file", operation: "filename" });
    expect(snapshots).toContainEqual({ mode: "audio", operation: "content" });
    expect(cliInvocation.value?.options.mode).toBe("audio");
    expect(cliInvocation.value?.options.operation).toBe("content");
  });

  it("純 audio 只開音訊頁", async () => {
    const pages: string[] = [];
    await applyParsedCliNavigation(baseCli({ mode: "audio", paths: ["s.mp3"] }), (page) => {
      pages.push(page);
    });
    expect(pages).toEqual(["audio"]);
    expect(cliInvocation.value?.options.mode).toBe("audio");
  });

  it("parseErrors 時不導向、不送出 invocation", async () => {
    const pages: string[] = [];
    const result = await applyParsedCliNavigation(
      baseCli({ parseErrors: ["無效的 --direction 值：sideways"] }),
      (page) => {
        pages.push(page);
      },
    );
    expect(pages).toEqual([]);
    expect(result.page).toBeNull();
    expect(result.parseError).toContain("--direction");
    expect(cliInvocation.value).toBeUndefined();
  });
});

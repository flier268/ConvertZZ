import { invoke } from "@tauri-apps/api/core";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { ConversionRequest, ConversionResult, Direction, EngineKind, ZhConvertOptions } from "@shared/contracts";
import { sidecar } from "./sidecar";

export async function convertText(text: string, direction: Direction, engine: EngineKind, vocabularyCorrection = true, zhconvert?: ZhConvertOptions, dictionaryPath?: string): Promise<ConversionResult> {
  return sidecar.request<ConversionResult>("convert.preview", { text, direction, engine, vocabularyCorrection, zhconvert, dictionaryPath } satisfies ConversionRequest);
}

export async function convertClipboard(
  direction: Direction,
  engine: EngineKind,
  automation: { copy: boolean; paste: boolean } = { copy: false, paste: false },
  vocabularyCorrection = true,
  zhconvert?: ZhConvertOptions,
  dictionaryPath?: string,
): Promise<ConversionResult> {
  if (automation.copy) {
    await invoke("simulate_copy_paste", { action: "copy" });
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  const source = await readText();
  const result = await convertText(source, direction, engine, vocabularyCorrection, zhconvert, dictionaryPath);
  await writeText(result.text);
  if (automation.paste) await invoke("simulate_copy_paste", { action: "paste" });
  return result;
}

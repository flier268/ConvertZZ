import { invoke } from "@tauri-apps/api/core";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type {
  ConversionRequest,
  ConversionResult,
  Direction,
  EngineKind,
  ZhConvertOptions,
} from "@shared/contracts";
import { core } from "./coreClient";

export async function convertText(
  text: string,
  direction: Direction,
  engine: EngineKind,
  vocabularyCorrection = true,
  zhconvert?: ZhConvertOptions,
  dictionaryPath?: string,
): Promise<ConversionResult> {
  return core.request<ConversionResult>("convert.preview", {
    text,
    direction,
    engine,
    vocabularyCorrection,
    zhconvert,
    dictionaryPath,
  } satisfies ConversionRequest);
}

export async function convertClipboard(
  direction: Direction,
  engine: EngineKind,
  automation: { copy: boolean; paste: boolean } = { copy: false, paste: false },
  vocabularyCorrection = true,
  zhconvert?: ZhConvertOptions,
  dictionaryPath?: string,
): Promise<ConversionResult> {
  const source = automation.copy ? await invoke<string>("capture_selection") : await readText();
  const result = await convertText(
    source,
    direction,
    engine,
    vocabularyCorrection,
    zhconvert,
    dictionaryPath,
  );
  await writeText(result.text);
  if (automation.paste) await invoke("replace_selection", { text: result.text });
  return result;
}

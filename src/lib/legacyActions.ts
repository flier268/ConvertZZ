import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getAllWindows } from "@tauri-apps/api/window";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { SettingsV2, TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { convertText } from "./actions";
import { resolveShellAction } from "./appMenu";
import { sidecar } from "./sidecar";
import { zhConvertOptions } from "./settings";
import { ElMessage } from "element-plus";

export const LEGACY_ACTIONS = [
  { label: "無", value: "0" },
  { label: "隱藏或顯示浮動球", value: "1" },
  { label: "GBK → Big5 並簡轉繁", value: "a1" },
  { label: "Big5 → GBK 並繁轉簡", value: "a2" },
  { label: "Unicode 簡轉繁", value: "a3" },
  { label: "Unicode 繁轉簡", value: "a4" },
  { label: "Unicode → HTML 十進位", value: "za1" },
  { label: "Unicode → HTML 十六進位", value: "za2" },
  { label: "HTML → Unicode", value: "za3" },
  { label: "Unicode → GBK 重新解讀", value: "zb1" },
  { label: "Unicode → Big5 重新解讀", value: "zb2" },
  { label: "Unicode → Shift-JIS 重新解讀", value: "zb3" },
  { label: "GBK → Unicode 重新解讀", value: "zb4" },
  { label: "Big5 → Unicode 重新解讀", value: "zb5" },
  { label: "Shift-JIS → Unicode 重新解讀", value: "zb6" },
  { label: "Shift-JIS → GBK", value: "zc1" },
  { label: "Shift-JIS → Big5", value: "zc2" },
  { label: "GBK → Shift-JIS", value: "zc3" },
  { label: "Big5 → Shift-JIS", value: "zc4" },
  { label: "HZ → GBK", value: "zd1" },
  { label: "HZ → Big5", value: "zd2" },
  { label: "GBK → HZ", value: "zd3" },
  { label: "Big5 → HZ", value: "zd4" },
  { label: "半形 → 全形", value: "ze1" },
  { label: "全形 → 半形", value: "ze2" },
] as const;

const ENCODING_ACTIONS: Record<string, [TextEncoding, TextEncoding]> = {
  zb1: ["gbk", "utf8"],
  zb2: ["big5", "utf8"],
  zb3: ["shift-jis", "utf8"],
  zb4: ["utf8", "gbk"],
  zb5: ["utf8", "big5"],
  zb6: ["utf8", "shift-jis"],
  zc1: ["gbk", "shift-jis"],
  zc2: ["big5", "shift-jis"],
  zc3: ["shift-jis", "gbk"],
  zc4: ["shift-jis", "big5"],
  zd1: ["gbk", "hz-gb-2312"],
  zd2: ["big5", "hz-gb-2312"],
  zd3: ["hz-gb-2312", "gbk"],
  zd4: ["hz-gb-2312", "big5"],
};

export async function executeLegacyAction(
  action: string,
  settings: SettingsV2,
  input?: string,
  automation: { copy: boolean; paste: boolean } = { copy: false, paste: false },
): Promise<{ text?: string; durationMs: number }> {
  const started = performance.now();
  if (!action || action === "0") return { durationMs: 0 };
  if (action === "1") {
    const floating = (await getAllWindows()).find((window) => window.label === "floating");
    if (!floating) return { durationMs: Math.round(performance.now() - started) };
    if (await floating.isVisible()) await floating.hide();
    else await floating.show();
    return { durationMs: Math.round(performance.now() - started) };
  }

  const shell = resolveShellAction(action);
  if (shell) {
    if (shell.type === "navigate") {
      await invoke("show_main_window");
      await emit("app://navigate", shell.page);
    } else if (shell.type === "open-url") {
      await openUrl(shell.url);
    } else {
      await invoke("quit_app");
    }
    return { durationMs: Math.round(performance.now() - started) };
  }

  if (automation.copy && input === undefined) {
    await invoke("simulate_copy_paste", { action: "copy" });
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  let text = input ?? await readText();

  if (action === "a1") {
    text = await utility(text, "encoding", "big5", "gbk");
    text = (await convertText(text, "s2t", settings.engine, settings.vocabularyCorrection, zhConvertOptions(settings, "s2t"), settings.dictionaryPath)).text;
  } else if (action === "a2") {
    text = (await convertText(text, "t2s", settings.engine, settings.vocabularyCorrection, zhConvertOptions(settings, "t2s"), settings.dictionaryPath)).text;
    text = await utility(text, "encoding", "gbk", "big5");
  } else if (action === "a3" || action === "a4") {
    const direction = action === "a3" ? "s2t" : "t2s";
    text = (await convertText(text, direction, settings.engine, settings.vocabularyCorrection, zhConvertOptions(settings, direction), settings.dictionaryPath)).text;
  } else if (action === "za1") text = await utility(text, "html-decimal-encode");
  else if (action === "za2") text = await utility(text, "html-hex-encode");
  else if (action === "za3") text = await utility(text, "html-decimal-decode");
  else if (action === "ze1") text = await utility(text, "fullwidth");
  else if (action === "ze2") text = await utility(text, "halfwidth");
  else if (ENCODING_ACTIONS[action]) {
    const [sourceEncoding, targetEncoding] = ENCODING_ACTIONS[action];
    text = await utility(text, "encoding", sourceEncoding, targetEncoding);
  } else {
    throw new Error(`不支援的舊版動作：${action}`);
  }

  await writeText(text);
  if (automation.paste && input === undefined) await invoke("simulate_copy_paste", { action: "paste" });
  const durationMs = Math.round(performance.now() - started);
  if (settings.promptAfterConversion && !automation.copy && !automation.paste) ElMessage.success(`轉換完成。耗時 ${durationMs} ms。`);
  return { text, durationMs };
}

async function utility(
  text: string,
  kind: UtilityConvertRequest["kind"],
  sourceEncoding?: TextEncoding,
  targetEncoding?: TextEncoding,
): Promise<string> {
  const result = await sidecar.request<{ text: string }>("utility.convert", { text, kind, sourceEncoding, targetEncoding } satisfies UtilityConvertRequest);
  return result.text;
}

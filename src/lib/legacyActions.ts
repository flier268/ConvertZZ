import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getAllWindows } from "@tauri-apps/api/window";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { SettingsV2, TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { convertText } from "./actions";
import { resolveShellAction } from "./appMenu";
import { core } from "./coreClient";
import { zhConvertOptions } from "./settings";
import { showAppToast } from "./toast";
import { t } from "@/i18n";

/** 標籤依目前語系產生；在 template 或 computed 內呼叫。 */
export function legacyActions() {
  return [
    { label: t("lib.legacyActions.none"), value: "0" },
    { label: t("lib.legacyActions.toggleFloatingBall"), value: "1" },
    { label: t("lib.legacyActions.a1"), value: "a1" },
    { label: t("lib.legacyActions.a2"), value: "a2" },
    { label: t("lib.legacyActions.a3"), value: "a3" },
    { label: t("lib.legacyActions.a4"), value: "a4" },
    { label: t("lib.legacyActions.za1"), value: "za1" },
    { label: t("lib.legacyActions.za2"), value: "za2" },
    { label: "HTML → Unicode", value: "za3" },
    { label: t("lib.legacyActions.zb1"), value: "zb1" },
    { label: t("lib.legacyActions.zb2"), value: "zb2" },
    { label: t("lib.legacyActions.zb3"), value: "zb3" },
    { label: t("lib.legacyActions.zb4"), value: "zb4" },
    { label: t("lib.legacyActions.zb5"), value: "zb5" },
    { label: t("lib.legacyActions.zb6"), value: "zb6" },
    { label: "Shift-JIS → GBK", value: "zc1" },
    { label: "Shift-JIS → Big5", value: "zc2" },
    { label: "GBK → Shift-JIS", value: "zc3" },
    { label: "Big5 → Shift-JIS", value: "zc4" },
    { label: "HZ → GBK", value: "zd1" },
    { label: "HZ → Big5", value: "zd2" },
    { label: "GBK → HZ", value: "zd3" },
    { label: "Big5 → HZ", value: "zd4" },
    { label: t("lib.legacyActions.ze1"), value: "ze1" },
    { label: t("lib.legacyActions.ze2"), value: "ze2" },
  ] as const;
}

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

  let text =
    automation.copy && input === undefined
      ? await invoke<string>("capture_selection")
      : (input ?? (await readText()));

  if (action === "a1") {
    text = await utility(text, "encoding", "big5", "gbk");
    text = (
      await convertText(
        text,
        "s2t",
        settings.engine,
        settings.vocabularyCorrection,
        zhConvertOptions(settings, "s2t"),
      )
    ).text;
  } else if (action === "a2") {
    text = (
      await convertText(
        text,
        "t2s",
        settings.engine,
        settings.vocabularyCorrection,
        zhConvertOptions(settings, "t2s"),
      )
    ).text;
    text = await utility(text, "encoding", "gbk", "big5");
  } else if (action === "a3" || action === "a4") {
    const direction = action === "a3" ? "s2t" : "t2s";
    text = (
      await convertText(
        text,
        direction,
        settings.engine,
        settings.vocabularyCorrection,
        zhConvertOptions(settings, direction),
      )
    ).text;
  } else if (action === "za1") text = await utility(text, "html-decimal-encode");
  else if (action === "za2") text = await utility(text, "html-hex-encode");
  else if (action === "za3") text = await utility(text, "html-decimal-decode");
  else if (action === "ze1") text = await utility(text, "fullwidth");
  else if (action === "ze2") text = await utility(text, "halfwidth");
  else if (ENCODING_ACTIONS[action]) {
    const [sourceEncoding, targetEncoding] = ENCODING_ACTIONS[action];
    text = await utility(text, "encoding", sourceEncoding, targetEncoding);
  } else {
    throw new Error(t("lib.legacyActions.unsupported", { action }));
  }

  await writeText(text);
  if (automation.paste && input === undefined) await invoke("replace_selection", { text });
  const durationMs = Math.round(performance.now() - started);
  if (settings.promptAfterConversion && !automation.copy && !automation.paste) {
    await showAppToast(t("lib.legacyActions.done", { duration: durationMs }));
  }
  return { text, durationMs };
}

async function utility(
  text: string,
  kind: UtilityConvertRequest["kind"],
  sourceEncoding?: TextEncoding,
  targetEncoding?: TextEncoding,
): Promise<string> {
  const result = await core.request<{ text: string }>("utility.convert", {
    text,
    kind,
    sourceEncoding,
    targetEncoding,
  } satisfies UtilityConvertRequest);
  return result.text;
}

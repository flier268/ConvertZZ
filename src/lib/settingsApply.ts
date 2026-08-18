import type { SettingsV2, TextEncoding } from "@shared/contracts";
import { parseLegacyFileFilters } from "./fileFilters";
import { floatingBallPosition } from "./desktop";
import { zhConvertOptions } from "./settings";

export function fileConversionDefaults(settings: SettingsV2) {
  return {
    engine: settings.engine,
    direction: settings.direction,
    vocabularyCorrection: settings.vocabularyCorrection,
    addBom: settings.files.unicodeAddBom,
    inputEncoding: (settings.recognizeEncoding ? "auto" : "utf8") as TextEncoding,
    previewMaxBytes: settings.previewMaxKb * 1024,
    fixCharsetExtensions: [...settings.files.fixCharsetExtensions],
    defaultPath:
      settings.files.defaultPath && settings.files.defaultPath !== "!"
        ? settings.files.defaultPath
        : undefined,
    fileFilters: parseLegacyFileFilters(settings.files.typeFilter),
    promptAfterConversion: settings.promptAfterConversion,
    autoBackupBeforeConversion: settings.autoBackupBeforeConversion,
    dictionaryPath: settings.dictionaryPath,
  };
}

export function importedSettingsEffects(settings: SettingsV2) {
  return {
    files: fileConversionDefaults(settings),
    zhconvert: zhConvertOptions(settings, settings.direction),
    floatingBall: {
      enabled: settings.floatingBall.enabled,
      position: floatingBallPosition(settings),
    },
    showMainWindowOnStart: settings.showMainWindowOnStart,
    checkVersionOnStart: settings.checkVersionOnStart,
    checkPreReleaseUpdates: settings.checkPreReleaseUpdates,
    hotkeys: {
      autoCopy: settings.hotkeys.autoCopy,
      autoPaste: settings.hotkeys.autoPaste,
      shortcuts: settings.hotkeys.shortcuts.filter((item) => item.enabled && item.accelerator),
    },
    quickActions: { ...settings.quickActions },
  };
}

export function importFailureMessage(error: unknown): string {
  return `匯入失敗。目前設定未變更。${error instanceof Error ? error.message : String(error)}`;
}

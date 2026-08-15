export type EngineKind = "segmented" | "legacy" | "zhconvert";
export type Direction = "none" | "s2t" | "t2s";
export type AudioContainer = "id3v1" | "id3v2" | "apev2" | "vorbis-comment";
export type ConflictPolicy = "skip" | "overwrite";

export interface ConversionRequest {
  text: string;
  direction: Direction;
  engine: EngineKind;
  dictionaryPath?: string;
  zhconvert?: ZhConvertOptions;
  vocabularyCorrection?: boolean;
}

export interface ConversionResult {
  text: string;
  engine: EngineKind;
  direction: Direction;
  warnings: string[];
  durationMs: number;
}

export interface ZhConvertOptions {
  converter?: string;
  modules?: Record<string, -1 | 0 | 1> | string[];
  jpTextConversionStrategy?: "none" | "protect" | "protectOnlySameOrigin" | "fix";
  jpStyleConversionStrategy?: "none" | "protect" | "protectOnlySameOrigin" | "fix";
  cleanUpText?: boolean;
  userPreReplace?: string;
  userPostReplace?: string;
  userProtectReplace?: string;
  ensureNewlineAtEof?: boolean;
  translateTabsToSpaces?: number;
  trimTrailingWhiteSpaces?: boolean;
  unifyLeadingHyphen?: boolean;
  ignoreTextStyles?: string;
  jpTextStyles?: string;
}

export type TextEncoding =
  | "auto"
  | "utf8"
  | "utf8-bom"
  | "utf16le"
  | "utf16be"
  | "big5"
  | "gbk"
  | "shift-jis"
  | "euc-jp"
  | "iso-2022-jp"
  | "hz-gb-2312";

export interface FilePlanRequest {
  paths: string[];
  outputPath?: string;
  outputDirectory?: string;
  mode: "content" | "filename" | "both";
  recursive: boolean;
  inputEncoding: TextEncoding;
  outputEncoding: TextEncoding;
  addBom: boolean;
  fixCharsetDeclaration: boolean;
  fixCharsetExtensions?: string[];
  allowedExtensions?: string[];
  previewMaxBytes?: number;
  conflictPolicy: ConflictPolicy;
  conversion: Omit<ConversionRequest, "text">;
}

export interface FilePlanItem {
  sourcePath: string;
  outputPath: string;
  kind: "file" | "directory";
  selected: boolean;
  detectedEncoding?: TextEncoding;
  sourcePreview: string;
  outputPreview: string;
  status: "ready" | "skipped" | "conflict" | "error";
  warning?: string;
}

export interface FileConversionPlan {
  planId: string;
  createdAt: string;
  items: FilePlanItem[];
  warnings: string[];
}

export interface AudioTagField {
  key: string;
  label: string;
  container: AudioContainer;
  values: string[];
  convertedValues?: string[];
  selected: boolean;
}

export interface AudioTagFile {
  path: string;
  format: "mp3" | "ape" | "ogg" | "opus";
  selected: boolean;
  fields: AudioTagField[];
  hasCoverArt: boolean;
  durationSeconds?: number;
  warning?: string;
}

export interface AudioScanRequest {
  paths: string[];
  recursive?: boolean;
  id3v1SourceEncoding?: Exclude<TextEncoding, "auto">;
  id3v2SourceEncoding?: Exclude<TextEncoding, "auto">;
  id3v2RepairSourceEncoding?: boolean;
}

export interface AudioTagPlanRequest extends AudioScanRequest {
  selectedPaths: string[];
  selectedFields: Record<string, string[]>;
  conversion: Omit<ConversionRequest, "text">;
  conflictPolicy: ConflictPolicy;
  id3v1Enabled: boolean;
  id3v1Direction: Direction;
  id3v1Zhconvert?: ZhConvertOptions;
  id3v1OutputEncoding: Exclude<TextEncoding, "auto">;
  id3v2Enabled: boolean;
  id3v2Direction: Direction;
  id3v2Zhconvert?: ZhConvertOptions;
  id3v2Version: 3 | 4;
  id3v2Encoding: "utf8" | "utf16" | "utf16be" | "latin1";
}

export interface AudioTagPlan {
  planId: string;
  createdAt: string;
  files: AudioTagFile[];
  warnings: string[];
}

export interface ApplyResult {
  succeeded: string[];
  skipped: string[];
  failed: Array<{ path: string; message: string }>;
}

export interface PlatformCapabilities {
  platform: "windows" | "linux" | "unknown";
  displayServer: "windows" | "x11" | "wayland" | "unknown";
  globalShortcuts: boolean;
  automaticCopyPaste: boolean;
  floatingAlwaysOnTop: boolean;
  tray: boolean;
  sendToShortcut: boolean;
  credentialStorage: boolean;
  limitations: string[];
}

export interface ParsedCli {
  mode: "file" | "audio" | "interactive";
  paths: string[];
  outputPath?: string;
  inputEncoding: TextEncoding;
  outputEncoding: TextEncoding;
  direction: Direction;
  engine: EngineKind;
  operation: "content" | "filename" | "both";
  vocabularyCorrection: "settings" | "enabled" | "disabled";
}

export interface ShortcutSetting {
  enabled: boolean;
  accelerator: string;
  action: string;
}

export interface QuickActionSettings {
  leftClickCtrl: string;
  leftClickAlt: string;
  leftClickShift: string;
  rightClickCtrl: string;
  rightClickAlt: string;
  rightClickShift: string;
  leftDropCtrl: string;
  leftDropAlt: string;
  leftDropShift: string;
  rightDropCtrl: string;
  rightDropAlt: string;
  rightDropShift: string;
}

export interface SettingsV2 {
  version: 2;
  engine: EngineKind;
  direction: Direction;
  vocabularyCorrection: boolean;
  dictionaryPath?: string;
  promptAfterConversion: boolean;
  recognizeEncoding: boolean;
  previewMaxKb: number;
  floatingBall: { enabled: boolean; x: number; y: number };
  hotkeys: {
    autoCopy: boolean;
    autoPaste: boolean;
    shortcuts: ShortcutSetting[];
  };
  quickActions: QuickActionSettings;
  files: {
    defaultPath: string;
    typeFilter: string;
    fixCharsetExtensions: string[];
    unicodeAddBom: boolean;
  };
  zhconvert: {
    converterS2T: string;
    converterT2S: string;
    modules: Record<string, -1 | 0 | 1>;
    jpTextConversionStrategy: "none" | "protect" | "protectOnlySameOrigin" | "fix";
    jpStyleConversionStrategy: "none" | "protect" | "protectOnlySameOrigin" | "fix";
    cleanUpText: boolean;
    userPreReplace: string;
    userPostReplace: string;
    userProtectReplace: string;
    ensureNewlineAtEof: boolean;
    translateTabsToSpaces: number;
    trimTrailingWhiteSpaces: boolean;
    unifyLeadingHyphen: boolean;
    ignoreTextStyles: string;
    jpTextStyles: string;
    serviceInfoCachedAt?: string;
  };
  checkVersionOnStart: boolean;
  showMainWindowOnStart: boolean;
}

export type SidecarOperation =
  | "health"
  | "convert.preview"
  | "files.plan"
  | "files.apply"
  | "files.cancel"
  | "audio.scan"
  | "audio.plan"
  | "audio.apply"
  | "audio.cancel"
  | "dictionary.read"
  | "dictionary.update"
  | "dictionary.preview"
  | "settings.migrate"
  | "zhconvert.configure"
  | "zhconvert.serviceInfo"
  | "utility.convert"
  | "cli.parse";

export interface SidecarRequest<T = unknown> {
  id: string;
  operation: SidecarOperation;
  payload: T;
}

export interface SidecarError {
  code: string;
  message: string;
  details?: unknown;
}

export interface SidecarResponse<T = unknown> {
  id: string;
  type: "response" | "progress";
  ok: boolean;
  result?: T;
  error?: SidecarError;
  progress?: { current: number; total: number; message: string };
}

export interface UtilityConvertRequest {
  kind:
    | "html-decimal-encode"
    | "html-decimal-decode"
    | "html-hex-encode"
    | "html-hex-decode"
    | "unicode-escape-encode"
    | "unicode-escape-decode"
    | "encoding"
    | "fullwidth"
    | "halfwidth";
  text: string;
  sourceEncoding?: TextEncoding;
  targetEncoding?: TextEncoding;
}

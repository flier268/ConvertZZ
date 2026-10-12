import type {
  ExtensionFilter,
  ExtensionFilterMode,
  ExtensionStats,
  FileFilterProfile,
  SettingsV2,
} from "@shared/contracts";
import { ALL_FILES_EXTENSION, parseLegacyFileFilters, type LegacyFileFilter } from "./fileFilters";

export const BUILTIN_TEXT_ID = "builtin-text";
export const BUILTIN_ALL_ID = "builtin-all";
export const BUILTIN_TEXT_NAME = "常用文字檔案";
export const BUILTIN_ALL_NAME = "所有檔案";
export const BUILTIN_TEXT_EXTENSIONS = [
  ".txt",
  ".log",
  ".ini",
  ".inf",
  ".bat",
  ".cmd",
  ".srt",
  ".ass",
  ".lang",
];

export const NO_EXTENSION_LABEL = "（無副檔名）";

const MODE_LABELS: Record<ExtensionFilterMode, string> = {
  allow: "白名單",
  deny: "黑名單",
  all: "全部",
};

export function modeLabel(mode: ExtensionFilterMode): string {
  return MODE_LABELS[mode];
}

export function builtinTextProfile(): FileFilterProfile {
  return {
    id: BUILTIN_TEXT_ID,
    name: BUILTIN_TEXT_NAME,
    builtin: true,
    mode: "allow",
    extensions: [...BUILTIN_TEXT_EXTENSIONS],
    includeNoExtension: false,
  };
}

export function builtinAllProfile(): FileFilterProfile {
  return {
    id: BUILTIN_ALL_ID,
    name: BUILTIN_ALL_NAME,
    builtin: true,
    mode: "all",
    extensions: [],
    includeNoExtension: true,
  };
}

/**
 * 單一副檔名正規化：小寫、前導點。`*.srt`、`srt`、`.SRT` 都變 `.srt`；
 * 多段（`.tar.gz`）只取最後一段，與核心的 `extension_of` 一致。
 * `*`、`*.*` 不是副檔名，回傳 null。
 */
export function normalizeExtension(raw: string): string | null {
  const cleaned = raw.trim().toLowerCase().replace(/^\*+/u, "");
  if (!cleaned || cleaned === ".") return null;
  const last = cleaned.slice(cleaned.lastIndexOf(".") + 1);
  if (!last || /[\s*?\\/:"<>|]/u.test(last)) return null;
  return `.${last}`;
}

export function splitExtensionInput(text: string): string[] {
  return text.split(/[\s,;，；、]+/u).filter(Boolean);
}

export interface MergedExtensions {
  extensions: string[];
  /** 已存在、因此沒有再加入的副檔名。 */
  duplicates: string[];
  /** 無法辨識的輸入。 */
  invalid: string[];
  /** 輸入含 `*` 或 `*.*`（應改選「所有檔案」profile）。 */
  hasAllFilesToken: boolean;
}

export function mergeExtensions(current: readonly string[], text: string): MergedExtensions {
  const extensions = [...current];
  const duplicates: string[] = [];
  const invalid: string[] = [];
  let hasAllFilesToken = false;
  for (const token of splitExtensionInput(text)) {
    const trimmed = token.trim();
    if (trimmed === "*" || trimmed === "*.*") {
      hasAllFilesToken = true;
      continue;
    }
    const normalized = normalizeExtension(trimmed);
    if (!normalized) {
      invalid.push(trimmed);
    } else if (extensions.includes(normalized)) {
      if (!duplicates.includes(normalized)) duplicates.push(normalized);
    } else {
      extensions.push(normalized);
    }
  }
  return { extensions, duplicates, invalid, hasAllFilesToken };
}

export function profileFilter(profile: ExtensionFilter): ExtensionFilter {
  return {
    mode: profile.mode,
    extensions: profile.mode === "all" ? [] : [...profile.extensions],
    includeNoExtension: profile.includeNoExtension,
  };
}

export interface ProfileValidation {
  error?: string;
  warning?: string;
}

export function validateProfile(
  profile: Pick<FileFilterProfile, "id" | "name" | "mode" | "extensions" | "includeNoExtension">,
  others: readonly Pick<FileFilterProfile, "id" | "name">[],
): ProfileValidation {
  const name = profile.name.trim();
  if (!name) return { error: "請輸入 profile 名稱。" };
  if (others.some((other) => other.id !== profile.id && other.name.trim() === name)) {
    return { error: `已有同名的 profile「${name}」。` };
  }
  if (profile.mode === "allow" && !profile.extensions.length && !profile.includeNoExtension) {
    return { error: "白名單至少要有一個副檔名，或勾選包含無副檔名檔案。" };
  }
  if (profile.mode === "deny" && !profile.extensions.length && !profile.includeNoExtension) {
    return { warning: "黑名單是空的，等同所有檔案。" };
  }
  return {};
}

/** 與核心 `include_file` 相同語意。`extension` 為小寫含點；無副檔名傳空字串。 */
export function filterAllows(filter: ExtensionFilter, extension: string): boolean {
  const listed = extension ? filter.extensions.includes(extension) : filter.includeNoExtension;
  if (filter.mode === "all") return true;
  return filter.mode === "allow" ? listed : !listed;
}

export function filterCollectsNothing(filter: ExtensionFilter): boolean {
  return filter.mode === "allow" && !filter.extensions.length && !filter.includeNoExtension;
}

export interface StatsRow {
  /** 無副檔名時為 null。 */
  extension: string | null;
  count: number;
  included: boolean;
}

export interface FilteredStats {
  rows: StatsRow[];
  included: number;
  excluded: number;
  includedKinds: number;
  excludedKinds: number;
}

export function applyFilterToStats(stats: ExtensionStats, filter: ExtensionFilter): FilteredStats {
  const rows: StatsRow[] = stats.extensions.map((entry) => ({
    extension: entry.extension,
    count: entry.count,
    included: filterAllows(filter, entry.extension),
  }));
  if (stats.noExtensionCount > 0) {
    rows.push({
      extension: null,
      count: stats.noExtensionCount,
      included: filterAllows(filter, ""),
    });
  }
  const sum = (included: boolean) =>
    rows.filter((row) => row.included === included).reduce((total, row) => total + row.count, 0);
  const kinds = (included: boolean) => rows.filter((row) => row.included === included).length;
  return {
    rows,
    included: sum(true),
    excluded: sum(false),
    includedKinds: kinds(true),
    excludedKinds: kinds(false),
  };
}

/** 給 profile 列表與目前篩選器顯示的簡短說明。 */
export function summarizeProfile(profile: ExtensionFilter, maxExtensions = 8): string {
  if (profile.mode === "all") return "所有檔案";
  const shown = profile.extensions.slice(0, maxExtensions).join(" ");
  const more =
    profile.extensions.length > maxExtensions ? ` …等 ${profile.extensions.length} 種` : "";
  const noExtension = profile.includeNoExtension ? "、含無副檔名" : "";
  if (!profile.extensions.length) {
    if (profile.mode === "allow") return profile.includeNoExtension ? "只含無副檔名" : "（空）";
    return profile.includeNoExtension ? "只排除無副檔名" : "（空，等同所有檔案）";
  }
  return `${shown}${more}${noExtension}`;
}

export function newProfileId(): string {
  const random =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID().replace(/-/gu, "").slice(0, 8)
      : Math.random().toString(16).slice(2, 10);
  return `u-${random}`;
}

export function uniqueProfileName(base: string, existing: readonly { name: string }[]): string {
  const names = new Set(existing.map((item) => item.name.trim()));
  if (!names.has(base)) return base;
  for (let index = 2; ; index += 1) {
    const candidate = `${base} ${index}`;
    if (!names.has(candidate)) return candidate;
  }
}

export function duplicateProfile(
  source: FileFilterProfile,
  existing: readonly FileFilterProfile[],
): FileFilterProfile {
  return {
    ...source,
    id: newProfileId(),
    name: uniqueProfileName(`${source.name} 副本`, existing),
    builtin: false,
    extensions: [...source.extensions],
  };
}

export function emptyProfile(existing: readonly FileFilterProfile[]): FileFilterProfile {
  return {
    id: newProfileId(),
    name: uniqueProfileName("自訂篩選", existing),
    builtin: false,
    mode: "allow",
    extensions: [],
    includeNoExtension: false,
  };
}

/** 內建 profile 缺漏時補回；順序保持使用者目前的排列。 */
export function ensureBuiltinProfiles(profiles: readonly FileFilterProfile[]): FileFilterProfile[] {
  const result = [...profiles];
  if (!result.some((item) => item.id === BUILTIN_ALL_ID)) result.unshift(builtinAllProfile());
  if (!result.some((item) => item.id === BUILTIN_TEXT_ID)) result.unshift(builtinTextProfile());
  return result;
}

export function resolveProfile(
  files: Pick<SettingsV2["files"], "filterProfiles" | "defaultFilterProfileId">,
  preferredId?: string,
): FileFilterProfile {
  const profiles = ensureBuiltinProfiles(files.filterProfiles ?? []);
  return (
    profiles.find((item) => item.id === preferredId) ??
    profiles.find((item) => item.id === files.defaultFilterProfileId) ??
    profiles.find((item) => item.id === BUILTIN_TEXT_ID) ??
    profiles[0]
  );
}

/** 舊 `<名稱|*.a;*.b>/<...>` 字串轉成 profile。與核心 `profiles_from_type_filter` 相同規則。 */
export function profilesFromTypeFilter(typeFilter: string): {
  profiles: FileFilterProfile[];
  defaultId: string;
} {
  const profiles = [builtinTextProfile(), builtinAllProfile()];
  let defaultId: string | undefined;
  parseLegacyFileFilters(typeFilter).forEach((group, index) => {
    const extensions = group.extensions
      .filter((extension) => extension !== ALL_FILES_EXTENSION)
      .map((extension) => `.${extension.toLowerCase()}`);
    let id: string;
    if (group.extensions.includes(ALL_FILES_EXTENSION)) {
      id = BUILTIN_ALL_ID;
    } else if (
      group.name === BUILTIN_TEXT_NAME &&
      extensions.length === BUILTIN_TEXT_EXTENSIONS.length &&
      extensions.every((extension, position) => extension === BUILTIN_TEXT_EXTENSIONS[position])
    ) {
      id = BUILTIN_TEXT_ID;
    } else {
      id = `legacy-${index + 1}`;
      const reserved =
        !group.name || group.name === BUILTIN_TEXT_NAME || group.name === BUILTIN_ALL_NAME;
      profiles.push({
        id,
        name: reserved ? `${group.name || "篩選器"}（舊版）` : group.name,
        builtin: false,
        mode: "allow",
        extensions,
        includeNoExtension: false,
      });
    }
    defaultId ??= id;
  });
  return { profiles, defaultId: defaultId ?? BUILTIN_TEXT_ID };
}

/**
 * 丟棄格式錯誤的 profile，規則與核心 `sanitize_filter_profiles` 相同（共用向量
 * `tests/fixtures/filter-profile-normalize.json`）：id 須為非空字串且不重複、name 須為字串、
 * mode 只能是 allow／deny／all、extensions 須為字串陣列、includeNoExtension 若存在須為布林。
 */
export function sanitizeFilterProfiles(items: unknown): FileFilterProfile[] {
  if (!Array.isArray(items)) return [];
  const seen = new Set<string>();
  const result: FileFilterProfile[] = [];
  for (const item of items as Record<string, unknown>[]) {
    if (!item || typeof item !== "object") continue;
    const { id, name, mode, extensions, includeNoExtension, builtin } = item;
    if (typeof id !== "string" || !id || typeof name !== "string") continue;
    if (mode !== "allow" && mode !== "deny" && mode !== "all") continue;
    if (!Array.isArray(extensions) || !extensions.every((value) => typeof value === "string")) {
      continue;
    }
    if (includeNoExtension !== undefined && typeof includeNoExtension !== "boolean") continue;
    if (seen.has(id)) continue;
    seen.add(id);
    const normalized: string[] = [];
    for (const raw of extensions as string[]) {
      const extension = normalizeExtension(raw);
      if (extension && !normalized.includes(extension)) normalized.push(extension);
    }
    result.push({
      id,
      name,
      builtin: builtin === true,
      mode,
      extensions: normalized,
      includeNoExtension: includeNoExtension === true,
    });
  }
  return result;
}

/** 補齊 `files` 的 profile 欄位。已有 profile 時保留，沒有時由舊字串遷移。 */
export function withFilterProfiles<T extends Partial<SettingsV2["files"]> & { typeFilter: string }>(
  files: T,
): T &
  Pick<
    SettingsV2["files"],
    "filterProfiles" | "defaultFilterProfileId" | "lastFilterProfileId" | "folderPickBehavior"
  > {
  const existing = sanitizeFilterProfiles(files.filterProfiles);
  const derived = existing.length ? undefined : profilesFromTypeFilter(files.typeFilter);
  const profiles = ensureBuiltinProfiles(existing.length ? existing : derived!.profiles);
  const defaultId = profiles.some((item) => item.id === files.defaultFilterProfileId)
    ? (files.defaultFilterProfileId as string)
    : (derived?.defaultId ?? BUILTIN_TEXT_ID);
  return {
    ...files,
    filterProfiles: profiles,
    defaultFilterProfileId: defaultId,
    lastFilterProfileId:
      typeof files.lastFilterProfileId === "string" ? files.lastFilterProfileId : "",
    folderPickBehavior:
      files.folderPickBehavior === "alwaysWizard" ? "alwaysWizard" : "rememberLast",
  };
}

/** 「選取檔案」系統對話框的篩選：白名單用副檔名，黑名單與全部退回所有檔案。 */
export function dialogFiltersForProfile(profile: FileFilterProfile): LegacyFileFilter[] {
  if (profile.mode !== "allow" || !profile.extensions.length) return [];
  return [
    {
      name: profile.name,
      extensions: profile.extensions.map((extension) => extension.replace(/^\./u, "")),
    },
  ];
}

const EXPORT_FORMAT = "convertzz-filter-profiles";

export function exportProfilesJson(profiles: readonly FileFilterProfile[]): string {
  return JSON.stringify(
    {
      format: EXPORT_FORMAT,
      version: 1,
      profiles: profiles
        .filter((item) => !item.builtin)
        .map(({ name, mode, extensions, includeNoExtension }) => ({
          name,
          mode,
          extensions,
          includeNoExtension,
        })),
    },
    null,
    2,
  );
}

export function parseImportedProfiles(
  text: string,
  existing: readonly FileFilterProfile[],
): { profiles: FileFilterProfile[]; error?: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text.replace(/^﻿/u, ""));
  } catch {
    return { profiles: [], error: "不是有效的 JSON 檔案。" };
  }
  const record = parsed as { format?: unknown; profiles?: unknown };
  if (!record || record.format !== EXPORT_FORMAT || !Array.isArray(record.profiles)) {
    return { profiles: [], error: "不是 ConvertZZ 的副檔名篩選 profile 檔案。" };
  }
  const imported: FileFilterProfile[] = [];
  for (const entry of record.profiles as Array<Record<string, unknown>>) {
    const mode = entry?.mode;
    if (mode !== "allow" && mode !== "deny" && mode !== "all") continue;
    const rawExtensions = Array.isArray(entry.extensions) ? entry.extensions.map(String) : [];
    const extensions = mergeExtensions([], rawExtensions.join(" ")).extensions;
    const candidate: FileFilterProfile = {
      id: newProfileId(),
      name: uniqueProfileName(String(entry.name ?? "").trim() || "匯入的篩選", [
        ...existing,
        ...imported,
      ]),
      builtin: false,
      mode,
      extensions: mode === "all" ? [] : extensions,
      includeNoExtension: mode === "all" ? true : Boolean(entry.includeNoExtension),
    };
    if (validateProfile(candidate, []).error) continue;
    imported.push(candidate);
  }
  if (!imported.length) return { profiles: [], error: "檔案中沒有可匯入的 profile。" };
  return { profiles: imported };
}

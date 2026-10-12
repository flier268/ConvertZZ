import { describe, expect, it } from "vitest";
import vectors from "../../tests/fixtures/filter-profile-normalize.json";
import { parseLegacyFileFilters } from "./fileFilters";
import type { ExtensionFilter, ExtensionStats, FileFilterProfile } from "@shared/contracts";
import { removeProfile, upsertProfile } from "./filterProfileActions";
import {
  applyFilterToStats,
  builtinAllProfile,
  builtinTextProfile,
  dialogFiltersForProfile,
  duplicateProfile,
  exportProfilesJson,
  filterAllows,
  filterCollectsNothing,
  mergeExtensions,
  normalizeExtension,
  parseImportedProfiles,
  profilesFromTypeFilter,
  resolveProfile,
  sanitizeFilterProfiles,
  summarizeProfile,
  validateProfile,
  withFilterProfiles,
} from "./filterProfiles";

function profile(partial: Partial<FileFilterProfile>): FileFilterProfile {
  return {
    id: "u-1",
    name: "自訂",
    builtin: false,
    mode: "allow",
    extensions: [".txt"],
    includeNoExtension: false,
    ...partial,
  };
}

describe("normalizeExtension", () => {
  it.each([
    ["*.srt", ".srt"],
    ["srt", ".srt"],
    [".SRT", ".srt"],
    ["  .Ass ", ".ass"],
    [".tar.gz", ".gz"],
  ])("%s → %s", (input, expected) => {
    expect(normalizeExtension(input)).toBe(expected);
  });

  it.each(["*", "*.*", ".", "", "a/b", "a b"])("拒絕 %j", (input) => {
    expect(normalizeExtension(input)).toBeNull();
  });
});

describe("mergeExtensions", () => {
  it("可貼上 *.srt;*.ass，去重並回報重複與無法辨識", () => {
    const merged = mergeExtensions([".txt"], "*.srt;*.ass, SRT .txt a/b");
    expect(merged.extensions).toEqual([".txt", ".srt", ".ass"]);
    expect(merged.duplicates).toEqual([".srt", ".txt"]);
    expect(merged.invalid).toEqual(["a/b"]);
    expect(merged.hasAllFilesToken).toBe(false);
  });

  it("* 與 *.* 不加入，只標記", () => {
    const merged = mergeExtensions([], "* *.*");
    expect(merged.extensions).toEqual([]);
    expect(merged.hasAllFilesToken).toBe(true);
  });
});

describe("validateProfile", () => {
  it("空白名單不可儲存，只含無副檔名的白名單可以", () => {
    expect(validateProfile(profile({ extensions: [] }), []).error).toContain("白名單");
    expect(
      validateProfile(profile({ extensions: [], includeNoExtension: true }), []).error,
    ).toBeUndefined();
  });

  it("空黑名單允許但提示等同所有檔案", () => {
    const result = validateProfile(profile({ mode: "deny", extensions: [] }), []);
    expect(result.error).toBeUndefined();
    expect(result.warning).toContain("等同所有檔案");
  });

  it("名稱必填且不可與其他 profile 同名", () => {
    expect(validateProfile(profile({ name: " " }), []).error).toBeTruthy();
    expect(
      validateProfile(profile({ id: "u-2", name: "常用文字檔案" }), [builtinTextProfile()]).error,
    ).toContain("同名");
    expect(validateProfile(profile({ name: "自訂" }), [profile({})]).error).toBeUndefined();
  });
});

describe("filterAllows 與統計", () => {
  const stats: ExtensionStats = {
    extensions: [
      { extension: ".srt", count: 8 },
      { extension: ".png", count: 3 },
    ],
    noExtensionCount: 2,
    totalFiles: 13,
    directoryCount: 1,
    warnings: [],
  };

  it("白名單、黑名單與無副檔名語意與核心一致", () => {
    const allow = {
      mode: "allow",
      extensions: [".srt"],
      includeNoExtension: true,
    } satisfies ExtensionFilter;
    expect(filterAllows(allow, ".srt")).toBe(true);
    expect(filterAllows(allow, ".png")).toBe(false);
    expect(filterAllows(allow, "")).toBe(true);
    const deny = {
      mode: "deny",
      extensions: [".png"],
      includeNoExtension: true,
    } satisfies ExtensionFilter;
    expect(filterAllows(deny, ".srt")).toBe(true);
    expect(filterAllows(deny, ".png")).toBe(false);
    expect(filterAllows(deny, "")).toBe(false);
    expect(filterAllows({ ...deny, includeNoExtension: false }, "")).toBe(true);
    expect(filterAllows({ mode: "all", extensions: [], includeNoExtension: false }, ".x")).toBe(
      true,
    );
  });

  it("預覽統計含無副檔名並算出包含與排除", () => {
    const result = applyFilterToStats(stats, {
      mode: "allow",
      extensions: [".srt"],
      includeNoExtension: false,
    });
    expect(result.included).toBe(8);
    expect(result.excluded).toBe(5);
    expect(result.rows.map((row) => [row.extension, row.included])).toEqual([
      [".srt", true],
      [".png", false],
      [null, false],
    ]);
    expect(
      applyFilterToStats(stats, { mode: "deny", extensions: [".png"], includeNoExtension: false })
        .included,
    ).toBe(10);
  });

  it("只有空白名單且不含無副檔名才是不收任何檔案", () => {
    expect(
      filterCollectsNothing({ mode: "allow", extensions: [], includeNoExtension: false }),
    ).toBe(true);
    expect(filterCollectsNothing({ mode: "allow", extensions: [], includeNoExtension: true })).toBe(
      false,
    );
    expect(filterCollectsNothing({ mode: "deny", extensions: [], includeNoExtension: false })).toBe(
      false,
    );
  });
});

describe("舊篩選器遷移", () => {
  it("每個群組一個白名單 profile，所有檔案群組對應內建，舊字串保留", () => {
    const typeFilter = "<字幕|*.SRT;*.ass>/<所有檔案|*.*>/<網頁|*.html;*.htm>";
    const migrated = withFilterProfiles({ typeFilter });
    expect(migrated.typeFilter).toBe(typeFilter);
    expect(migrated.filterProfiles.map((item) => item.id)).toEqual([
      "builtin-text",
      "builtin-all",
      "legacy-1",
      "legacy-3",
    ]);
    expect(migrated.filterProfiles[2]).toMatchObject({
      name: "字幕",
      mode: "allow",
      extensions: [".srt", ".ass"],
      includeNoExtension: false,
      builtin: false,
    });
    expect(migrated.defaultFilterProfileId).toBe("legacy-1");
    expect(migrated.folderPickBehavior).toBe("rememberLast");
    expect(migrated.lastFilterProfileId).toBe("");
  });

  it("與內建完全相同的「常用文字檔案」不重複建立", () => {
    const text = `<常用文字檔案|${builtinTextProfile()
      .extensions.map((extension) => `*${extension}`)
      .join(";")}>`;
    const result = profilesFromTypeFilter(text);
    expect(result.profiles.map((item) => item.id)).toEqual(["builtin-text", "builtin-all"]);
    expect(result.defaultId).toBe("builtin-text");
  });

  it("已有 profile 時保留並補回缺的內建，無效的預設退回內建文字", () => {
    const custom = profile({ id: "u-9", mode: "deny", extensions: [".png"] });
    const migrated = withFilterProfiles({
      typeFilter: "<忽略|*.zzz>",
      filterProfiles: [custom],
      defaultFilterProfileId: "gone",
      lastFilterProfileId: "u-9",
      folderPickBehavior: "alwaysWizard",
    });
    expect(migrated.filterProfiles.map((item) => item.id)).toEqual([
      "builtin-text",
      "builtin-all",
      "u-9",
    ]);
    expect(migrated.defaultFilterProfileId).toBe("builtin-text");
    expect(migrated.lastFilterProfileId).toBe("u-9");
    expect(migrated.folderPickBehavior).toBe("alwaysWizard");
  });
});

describe("profile 操作", () => {
  const files = () =>
    withFilterProfiles({
      typeFilter: "<字幕|*.srt>",
    });

  it("內建 profile 不可刪除；刪除預設與上次使用的 profile 會退回", () => {
    const target = files();
    expect(removeProfile(target as never, "builtin-text")).toBe(false);
    expect(removeProfile(target as never, "builtin-all")).toBe(false);
    target.lastFilterProfileId = "legacy-1";
    expect(removeProfile(target as never, "legacy-1")).toBe(true);
    expect(target.defaultFilterProfileId).toBe("builtin-text");
    expect(target.lastFilterProfileId).toBe("");
  });

  it("upsert 不會覆寫內建 profile", () => {
    const target = files();
    upsertProfile(target as never, { ...builtinTextProfile(), extensions: [".zzz"] });
    expect(target.filterProfiles.find((item) => item.id === "builtin-text")!.extensions).toContain(
      ".txt",
    );
    upsertProfile(target as never, profile({ id: "legacy-1", extensions: [".md"] }));
    expect(target.filterProfiles.find((item) => item.id === "legacy-1")!.extensions).toEqual([
      ".md",
    ]);
  });

  it("複製產生新 id、不再是內建並避開重名", () => {
    const copy = duplicateProfile(builtinTextProfile(), [builtinTextProfile()]);
    expect(copy.builtin).toBe(false);
    expect(copy.id).not.toBe("builtin-text");
    expect(copy.name).toBe("常用文字檔案 副本");
    expect(duplicateProfile(builtinTextProfile(), [builtinTextProfile(), copy]).name).toBe(
      "常用文字檔案 副本 2",
    );
  });

  it("resolveProfile 依序退回預設與內建文字", () => {
    const target = files();
    expect(resolveProfile(target, "builtin-all").id).toBe("builtin-all");
    expect(resolveProfile(target, "missing").id).toBe("legacy-1");
    expect(resolveProfile({ filterProfiles: [], defaultFilterProfileId: "" }).id).toBe(
      "builtin-text",
    );
  });

  it("選取檔案對話框：白名單用副檔名，黑名單與全部退回所有檔案", () => {
    expect(dialogFiltersForProfile(builtinTextProfile())[0].extensions).toContain("srt");
    expect(dialogFiltersForProfile(builtinAllProfile())).toEqual([]);
    expect(dialogFiltersForProfile(profile({ mode: "deny" }))).toEqual([]);
  });

  it("摘要文字", () => {
    expect(summarizeProfile(builtinAllProfile())).toBe("所有檔案");
    expect(summarizeProfile(builtinTextProfile(), 3)).toBe(".txt .log .ini …等 9 種");
    expect(summarizeProfile(profile({ extensions: [], includeNoExtension: true }))).toBe(
      "只含無副檔名",
    );
  });
});

describe("匯入匯出", () => {
  it("匯出不含內建 profile，匯入改名避開同名並丟掉無效項目", () => {
    const custom = profile({ name: "字幕", extensions: [".srt"] });
    const json = exportProfilesJson([builtinTextProfile(), custom]);
    expect(JSON.parse(json).profiles).toHaveLength(1);
    const result = parseImportedProfiles(json, [custom]);
    expect(result.error).toBeUndefined();
    expect(result.profiles[0].name).toBe("字幕 2");
    expect(result.profiles[0].id).not.toBe(custom.id);

    const mixed = JSON.stringify({
      format: "convertzz-filter-profiles",
      version: 1,
      profiles: [
        { name: "空", mode: "allow", extensions: [] },
        { name: "好", mode: "deny", extensions: ["PNG", "*.jpg"], includeNoExtension: true },
      ],
    });
    const parsed = parseImportedProfiles(mixed, []);
    expect(parsed.profiles).toHaveLength(1);
    expect(parsed.profiles[0].extensions).toEqual([".png", ".jpg"]);
  });

  it("格式錯誤回傳錯誤", () => {
    expect(parseImportedProfiles("不是 json", []).error).toBeTruthy();
    expect(parseImportedProfiles('{"format":"x"}', []).error).toBeTruthy();
  });
});

describe("共用正規化向量", () => {
  it.each(vectors.legacyPatterns)("舊樣式 $input", ({ input, expected }) => {
    const [group] = parseLegacyFileFilters(`<n|${input};*.zzz>`);
    const extensions = group.extensions.filter(
      (extension) => extension !== "zzz" && extension !== "*",
    );
    expect(extensions.map((extension) => `.${extension.toLowerCase()}`)).toEqual(
      expected ? [expected] : [],
    );
  });

  it.each(vectors.profiles)("$name", ({ input, expected }) => {
    expect(sanitizeFilterProfiles(input)).toEqual(expected);
    expect(
      withFilterProfiles({ typeFilter: "", filterProfiles: input as never }).filterProfiles,
    ).toEqual(expect.arrayContaining(expected));
  });
});

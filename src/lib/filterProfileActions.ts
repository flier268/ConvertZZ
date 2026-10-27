import type { FileFilterProfile, SettingsV2 } from "@shared/contracts";
import { BUILTIN_TEXT_ID } from "./filterProfiles";

type FilesSettings = SettingsV2["files"];

/** 新增或取代同 id 的 profile。內建 profile 不會被取代（呼叫端應先另存為自訂）。 */
export function upsertProfile(files: FilesSettings, profile: FileFilterProfile): void {
  const index = files.filterProfiles.findIndex((item) => item.id === profile.id);
  if (index < 0) {
    files.filterProfiles.push(profile);
  } else if (!files.filterProfiles[index].builtin) {
    files.filterProfiles.splice(index, 1, profile);
  }
}

/** 內建 profile 不可刪除。刪除目前的預設或上次使用時退回預設／清空。 */
export function removeProfile(files: FilesSettings, id: string): boolean {
  const target = files.filterProfiles.find((item) => item.id === id);
  if (!target || target.builtin) return false;
  files.filterProfiles = files.filterProfiles.filter((item) => item.id !== id);
  if (files.defaultFilterProfileId === id) files.defaultFilterProfileId = BUILTIN_TEXT_ID;
  if (files.lastFilterProfileId === id) files.lastFilterProfileId = "";
  return true;
}

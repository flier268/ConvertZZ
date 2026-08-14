import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { ElLoading, ElMessage, ElMessageBox } from "element-plus";
import { resolveUpdate } from "./update";

export function isDialogCancelled(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

export async function promptForAppUpdate(
  options: { silentWhenCurrent?: boolean } = {},
): Promise<void> {
  const currentVersion = await getVersion();
  const pending: { update: Update | null } = { update: null };
  try {
    const resolved = await resolveUpdate(currentVersion, {
      checkInstallable: async () => {
        pending.update = await check();
        if (!pending.update) return null;
        return {
          currentVersion: pending.update.currentVersion,
          version: pending.update.version,
          body: pending.update.body,
        };
      },
    });

    if (resolved.kind === "none") {
      if (!options.silentWhenCurrent) ElMessage.success("目前已是最新版本。");
      return;
    }

    await invoke("show_main_window");
    if (resolved.kind === "install") {
      await ElMessageBox.confirm(
        `發現新版本 ${resolved.latestVersion}。目前版本為 ${resolved.currentVersion}。是否下載並安裝？安裝完成後程式會重新啟動。`,
        "發現更新",
        { type: "info" },
      );
      if (!pending.update) throw new Error("更新已不可用。");
      const loading = ElLoading.service({ lock: true, text: "正在下載並安裝更新…" });
      try {
        await pending.update.downloadAndInstall();
      } finally {
        loading.close();
      }
      await relaunch();
      return;
    }

    await ElMessageBox.confirm(
      `發現新版本 ${resolved.latestVersion}。目前版本為 ${resolved.currentVersion}。此安裝方式無法自動更新，是否開啟下載頁面？`,
      "發現更新",
      { type: "info" },
    );
    await openUrl(resolved.url);
  } finally {
    await pending.update?.close().catch(() => undefined);
  }
}

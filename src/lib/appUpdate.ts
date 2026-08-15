import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { ElLoading, ElMessage, ElMessageBox } from "element-plus";
import { h } from "vue";
import { patchSavedSettings } from "./settings";
import { isUpdateVersionSkipped, resolveUpdate } from "./update";

export function isDialogCancelled(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

export async function rememberSkippedUpdateVersion(version: string): Promise<void> {
  await patchSavedSettings((settings) => {
    settings.skippedUpdateVersion = version;
  });
}

export async function promptForAppUpdate(
  options: { silentWhenCurrent?: boolean; skippedVersion?: string } = {},
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

    if (
      options.silentWhenCurrent &&
      isUpdateVersionSkipped(resolved.latestVersion, options.skippedVersion)
    ) {
      return;
    }

    const skipThisVersion = { value: false };
    await invoke("show_main_window");
    if (resolved.kind === "install") {
      if (
        !(await confirmAppUpdate(
          `發現新版本 ${resolved.latestVersion}。目前版本為 ${resolved.currentVersion}。是否下載並安裝？安裝完成後程式會重新啟動。`,
          "下載並安裝",
          skipThisVersion,
        ))
      ) {
        await dismissAppUpdate(skipThisVersion.value, resolved.latestVersion);
        return;
      }
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

    if (
      !(await confirmAppUpdate(
        `發現新版本 ${resolved.latestVersion}。目前版本為 ${resolved.currentVersion}。此安裝方式無法自動更新，是否開啟下載頁面？`,
        "開啟下載頁",
        skipThisVersion,
      ))
    ) {
      await dismissAppUpdate(skipThisVersion.value, resolved.latestVersion);
      return;
    }
    await openUrl(resolved.url);
  } finally {
    await pending.update?.close().catch(() => undefined);
  }
}

async function confirmAppUpdate(
  message: string,
  confirmButtonText: string,
  skipThisVersion: { value: boolean },
): Promise<boolean> {
  try {
    await ElMessageBox.confirm(updatePromptMessage(message, skipThisVersion), "發現更新", {
      type: "info",
      confirmButtonText,
      cancelButtonText: "稍後再說",
    });
    return true;
  } catch (error) {
    if (isDialogCancelled(error)) return false;
    throw error;
  }
}

async function dismissAppUpdate(skipRequested: boolean, latestVersion: string): Promise<void> {
  if (!skipRequested) return;
  await rememberSkippedUpdateVersion(latestVersion);
  ElMessage.success(`已略過 ${latestVersion}。啟動時不會再詢問此版本。`);
}

function updatePromptMessage(text: string, skipThisVersion: { value: boolean }) {
  return h("div", [
    h("p", { style: "margin: 0 0 12px;" }, text),
    h(
      "label",
      {
        style: "display: flex; align-items: center; gap: 8px; cursor: pointer; user-select: none;",
      },
      [
        h("input", {
          type: "checkbox",
          onChange: (event: Event) => {
            skipThisVersion.value = (event.target as HTMLInputElement).checked;
          },
        }),
        "不再詢問此版本",
      ],
    ),
  ]);
}

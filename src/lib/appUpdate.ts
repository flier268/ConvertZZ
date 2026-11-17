import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, Update } from "@tauri-apps/plugin-updater";
import type { PlatformCapabilities } from "@shared/contracts";
import { ElLoading, ElMessage, ElMessageBox } from "element-plus";
import { h } from "vue";
import { patchSavedSettings } from "./settings";
import { isUpdateVersionSkipped, resolveUpdate, signedUpdateManifestUrl } from "./update";
import { t } from "@/i18n";

export function isDialogCancelled(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

export async function rememberSkippedUpdateVersion(version: string): Promise<void> {
  await patchSavedSettings((settings) => {
    settings.skippedUpdateVersion = version;
  });
}

async function checkInstallableUpdate(targetVersion?: string): Promise<Update | null> {
  if (!targetVersion) return check();
  const metadata = await invoke<{
    rid: number;
    currentVersion: string;
    version: string;
    date?: string;
    body?: string;
    rawJson: Record<string, unknown>;
  } | null>("check_signed_update", { endpoint: signedUpdateManifestUrl(targetVersion) });
  return metadata ? new Update(metadata) : null;
}

export async function promptForAppUpdate(
  options: {
    silentWhenCurrent?: boolean;
    skippedVersion?: string;
    includePreRelease?: boolean;
  } = {},
): Promise<void> {
  const currentVersion = await getVersion();
  const capabilities = await invoke<PlatformCapabilities>("platform_capabilities");
  const pending: { update: Update | null } = { update: null };
  try {
    const resolved = await resolveUpdate(currentVersion, {
      includePreRelease: options.includePreRelease,
      checkInstallable: capabilities.automaticUpdates
        ? async (targetVersion?: string) => {
            pending.update = await checkInstallableUpdate(targetVersion);
            if (!pending.update) return null;
            return {
              currentVersion: pending.update.currentVersion,
              version: pending.update.version,
              body: pending.update.body,
            };
          }
        : undefined,
    });

    if (resolved.kind === "none") {
      if (!options.silentWhenCurrent) ElMessage.success(t("lib.appUpdate.upToDate"));
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
          t("lib.appUpdate.installPrompt", {
            latest: resolved.latestVersion,
            current: resolved.currentVersion,
          }),
          t("lib.appUpdate.install"),
          skipThisVersion,
        ))
      ) {
        await dismissAppUpdate(skipThisVersion.value, resolved.latestVersion);
        return;
      }
      if (!pending.update) throw new Error(t("lib.appUpdate.unavailable"));
      const loading = ElLoading.service({ lock: true, text: t("lib.appUpdate.installing") });
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
        t("lib.appUpdate.downloadPrompt", {
          latest: resolved.latestVersion,
          current: resolved.currentVersion,
        }),
        t("lib.appUpdate.openDownloadPage"),
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
    await ElMessageBox.confirm(
      updatePromptMessage(message, skipThisVersion),
      t("lib.appUpdate.title"),
      {
        type: "info",
        confirmButtonText,
        cancelButtonText: t("lib.appUpdate.later"),
      },
    );
    return true;
  } catch (error) {
    if (isDialogCancelled(error)) return false;
    throw error;
  }
}

async function dismissAppUpdate(skipRequested: boolean, latestVersion: string): Promise<void> {
  if (!skipRequested) return;
  await rememberSkippedUpdateVersion(latestVersion);
  ElMessage.success(t("lib.appUpdate.skipped", { version: latestVersion }));
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
        t("lib.appUpdate.skipThisVersion"),
      ],
    ),
  ]);
}

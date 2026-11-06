import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { getAllWindows } from "@tauri-apps/api/window";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import type { PlatformCapabilities, SettingsV2 } from "@shared/contracts";
import { formatUnknownError } from "./errors";
import { executeLegacyAction } from "./legacyActions";
import { registrableShortcuts, unregisteredAcceleratorWarnings } from "./hotkey";
import { showAppToast } from "./toast";

export function floatingBallPosition(settings: SettingsV2): { x: number; y: number } | undefined {
  const { x, y } = settings.floatingBall;
  if (Number.isFinite(x) && Number.isFinite(y) && (x !== -1 || y !== -1)) return { x, y };
  return undefined;
}

export function shouldShowMainWindowOnStartup(
  settings: Pick<SettingsV2, "showMainWindowOnStart">,
  options: { forceShow?: boolean; showForOnboarding?: boolean } = {},
): boolean {
  return Boolean(options.forceShow || options.showForOnboarding || settings.showMainWindowOnStart);
}

export async function applyStartupWindowVisibility(
  settings: SettingsV2,
  options: { forceShow?: boolean; showForOnboarding?: boolean } | boolean = {},
): Promise<void> {
  const resolved = typeof options === "boolean" ? { forceShow: options } : options;
  if (shouldShowMainWindowOnStartup(settings, resolved)) await invoke("show_main_window");
}

export async function applyFloatingBallWindow(
  settings: SettingsV2,
  options: { reveal?: boolean } = {},
): Promise<void> {
  const floating = (await getAllWindows()).find((window) => window.label === "floating");
  if (!floating) return;
  const position = floatingBallPosition(settings);
  if (position) await floating.setPosition(new LogicalPosition(position.x, position.y));
  if (!settings.floatingBall.enabled) await floating.hide();
  else if (options.reveal !== false) await floating.show();
}

export async function applyDesktopSettings(
  settings: SettingsV2,
  options: { revealFloating?: boolean } = {},
): Promise<string[]> {
  const warnings: string[] = [];
  try {
    await applyFloatingBallWindow(settings, { reveal: options.revealFloating });
  } catch (error) {
    // 首次啟動時浮動視窗可能尚未就緒；不應中止整個主程式啟動。
    warnings.push(`無法套用浮動球：${formatUnknownError(error)}`);
  }

  const capabilities = await invoke<PlatformCapabilities>("platform_capabilities");
  if (!capabilities.globalShortcuts) return warnings;
  warnings.push(...unregisteredAcceleratorWarnings(settings.hotkeys.shortcuts));
  try {
    await unregisterAll();
  } catch (error) {
    warnings.push(`無法註冊全域快捷鍵：${formatUnknownError(error)}`);
    return warnings;
  }
  for (const shortcut of registrableShortcuts(settings.hotkeys.shortcuts)) {
    try {
      await register(shortcut.accelerator, async (event) => {
        if (event.state !== "Pressed") return;
        try {
          await executeLegacyAction(shortcut.action, settings, undefined, {
            copy: settings.hotkeys.autoCopy,
            paste: settings.hotkeys.autoPaste,
          });
        } catch (error) {
          await showAppToast(formatUnknownError(error));
        }
      });
    } catch (error) {
      warnings.push(`無法註冊快捷鍵 ${shortcut.accelerator}：${formatUnknownError(error)}`);
    }
  }
  return warnings;
}

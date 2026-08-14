import { invoke } from "@tauri-apps/api/core";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { getAllWindows } from "@tauri-apps/api/window";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import type { PlatformCapabilities, SettingsV2 } from "@shared/contracts";
import { executeLegacyAction } from "./legacyActions";
import { ElMessage } from "element-plus";

export function floatingBallPosition(settings: SettingsV2): { x: number; y: number } | undefined {
  const { x, y } = settings.floatingBall;
  if (Number.isFinite(x) && Number.isFinite(y) && (x !== -1 || y !== -1)) return { x, y };
  return undefined;
}

export async function applyStartupWindowVisibility(settings: SettingsV2, forceShow = false): Promise<void> {
  if (forceShow || settings.showMainWindowOnStart) await invoke("show_main_window");
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
  await applyFloatingBallWindow(settings, { reveal: options.revealFloating });

  const capabilities = await invoke<PlatformCapabilities>("platform_capabilities");
  if (!capabilities.globalShortcuts) return warnings;
  await unregisterAll();
  for (const shortcut of settings.hotkeys.shortcuts.filter((item) => item.enabled && item.accelerator)) {
    try {
      await register(shortcut.accelerator, async (event) => {
        if (event.state !== "Released") return;
        try {
          await executeLegacyAction(
            shortcut.action,
            settings,
            undefined,
            { copy: settings.hotkeys.autoCopy, paste: settings.hotkeys.autoPaste },
          );
        } catch (error) {
          ElMessage.error(error instanceof Error ? error.message : String(error));
        }
      });
    } catch (error) {
      warnings.push(`無法註冊快捷鍵 ${shortcut.accelerator}：${error instanceof Error ? error.message : String(error)}`);
    }
  }
  return warnings;
}

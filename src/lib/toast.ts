import { invoke } from "@tauri-apps/api/core";

export async function showAppToast(message: string): Promise<void> {
  await invoke("show_toast", { message });
}

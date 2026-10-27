import { Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import type { AppMenuNode } from "./appMenu";

export async function popupAppMenu(
  nodes: AppMenuNode[],
  onAction: (id: string) => void | Promise<void>,
): Promise<void> {
  const menu = await Menu.new({
    items: await Promise.all(nodes.map((node) => toNativeItem(node, onAction))),
  });
  await menu.popup();
}

async function toNativeItem(
  node: AppMenuNode,
  onAction: (id: string) => void | Promise<void>,
): Promise<MenuItem | PredefinedMenuItem | Submenu> {
  if (node.type === "separator") return PredefinedMenuItem.new({ item: "Separator" });
  if (node.type === "submenu") {
    return Submenu.new({
      text: node.label,
      items: await Promise.all(node.items.map((item) => toNativeItem(item, onAction))),
    });
  }
  return MenuItem.new({
    id: node.id,
    text: node.label,
    action: () => {
      void onAction(node.id);
    },
  });
}

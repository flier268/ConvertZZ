import { t } from "@/i18n";

export type AppMenuNode =
  | { type: "item"; id: string; label: string }
  | { type: "separator" }
  | { type: "submenu"; label: string; items: AppMenuNode[] };

/** 標籤依目前語系產生；每次顯示選單時重新呼叫。 */
export function floatingContextMenu(): AppMenuNode[] {
  return [
    { type: "item", id: "a1", label: "GBK → Big5" },
    { type: "item", id: "a2", label: "Big5 → GBK" },
    { type: "item", id: "a3", label: t("lib.appMenu.a3") },
    { type: "item", id: "a4", label: t("lib.appMenu.a4") },
    { type: "separator" },
    { type: "item", id: "b1", label: t("lib.appMenu.b1") },
    { type: "item", id: "b2", label: t("lib.appMenu.b2") },
    { type: "separator" },
    { type: "item", id: "c1", label: t("lib.appMenu.c1") },
    { type: "separator" },
    {
      type: "submenu",
      label: t("lib.appMenu.others"),
      items: [
        { type: "item", id: "za1", label: t("lib.appMenu.za1") },
        { type: "item", id: "za2", label: t("lib.appMenu.za2") },
        { type: "item", id: "za3", label: "HTML → Unicode" },
        { type: "separator" },
        { type: "item", id: "zb1", label: "Unicode → GBK" },
        { type: "item", id: "zb2", label: "Unicode → Big5" },
        { type: "item", id: "zb3", label: "Unicode → Shift-JIS" },
        { type: "item", id: "zb4", label: "GBK → Unicode" },
        { type: "item", id: "zb5", label: "Big5 → Unicode" },
        { type: "item", id: "zb6", label: "Shift-JIS → Unicode" },
        { type: "separator" },
        { type: "item", id: "zc1", label: "Shift-JIS → GBK" },
        { type: "item", id: "zc2", label: "Shift-JIS → Big5" },
        { type: "item", id: "zc3", label: "GBK → Shift-JIS" },
        { type: "item", id: "zc4", label: "Big5 → Shift-JIS" },
        { type: "separator" },
        { type: "item", id: "zd1", label: "HZ → GBK" },
        { type: "item", id: "zd2", label: "HZ → Big5" },
        { type: "item", id: "zd3", label: "GBK → HZ" },
        { type: "item", id: "zd4", label: "Big5 → HZ" },
        { type: "separator" },
        { type: "item", id: "ze1", label: t("lib.appMenu.ze1") },
        { type: "item", id: "ze2", label: t("lib.appMenu.ze2") },
      ],
    },
    { type: "item", id: "1", label: t("lib.appMenu.toggleFloatingBall") },
    { type: "item", id: "settings", label: t("lib.appMenu.settings") },
    {
      type: "submenu",
      label: t("lib.appMenu.help"),
      items: [
        { type: "item", id: "about", label: t("lib.appMenu.about") },
        { type: "item", id: "report", label: t("lib.appMenu.report") },
      ],
    },
    { type: "item", id: "quit", label: t("lib.appMenu.quit") },
  ];
}

export function collectMenuActionIds(nodes: AppMenuNode[]): string[] {
  return nodes.flatMap((node) => {
    if (node.type === "item") return [node.id];
    if (node.type === "submenu") return collectMenuActionIds(node.items);
    return [];
  });
}

export const SHELL_ACTIONS = {
  b1: { type: "navigate", page: "files" },
  b2: { type: "navigate", page: "clipboard" },
  c1: { type: "navigate", page: "audio" },
  settings: { type: "navigate", page: "settings" },
  about: { type: "navigate", page: "about" },
  report: { type: "open-url", url: "https://github.com/flier268/ConvertZZ/issues" },
  quit: { type: "quit" },
} as const;

export function resolveShellAction(action: string) {
  return SHELL_ACTIONS[action as keyof typeof SHELL_ACTIONS];
}

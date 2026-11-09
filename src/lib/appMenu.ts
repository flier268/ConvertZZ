export type AppMenuNode =
  | { type: "item"; id: string; label: string }
  | { type: "separator" }
  | { type: "submenu"; label: string; items: AppMenuNode[] };

export const FLOATING_CONTEXT_MENU: AppMenuNode[] = [
  { type: "item", id: "a1", label: "GBK → Big5" },
  { type: "item", id: "a2", label: "Big5 → GBK" },
  { type: "item", id: "a3", label: "Unicode 簡 → Unicode 繁" },
  { type: "item", id: "a4", label: "Unicode 繁 → Unicode 簡" },
  { type: "separator" },
  { type: "item", id: "b1", label: "文件/檔名轉換" },
  { type: "item", id: "b2", label: "剪貼簿轉換" },
  { type: "separator" },
  {
    type: "submenu",
    label: "Audio 標籤轉換",
    items: [
      { type: "item", id: "c1", label: "ID3" },
      { type: "item", id: "c2", label: "APE" },
      { type: "item", id: "c3", label: "OGG" },
    ],
  },
  { type: "separator" },
  {
    type: "submenu",
    label: "其他",
    items: [
      { type: "item", id: "za1", label: "Unicode → HTML 十進位" },
      { type: "item", id: "za2", label: "Unicode → HTML 十六進位" },
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
      { type: "item", id: "ze1", label: "半形 → 全形" },
      { type: "item", id: "ze2", label: "全形 → 半形" },
    ],
  },
  { type: "item", id: "1", label: "隱藏或顯示浮動球" },
  { type: "item", id: "settings", label: "設定" },
  {
    type: "submenu",
    label: "說明",
    items: [
      { type: "item", id: "about", label: "關於 ConvertZZ" },
      { type: "item", id: "report", label: "回報問題" },
    ],
  },
  { type: "item", id: "quit", label: "結束 ConvertZZ" },
];

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
  c2: { type: "navigate", page: "audio" },
  c3: { type: "navigate", page: "audio" },
  settings: { type: "navigate", page: "settings" },
  about: { type: "navigate", page: "about" },
  report: { type: "open-url", url: "https://github.com/flier268/ConvertZZ/issues" },
  quit: { type: "quit" },
} as const;

export function resolveShellAction(action: string) {
  return SHELL_ACTIONS[action as keyof typeof SHELL_ACTIONS];
}

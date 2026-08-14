const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS", "Super", "Hyper", "AltGraph"]);

export function acceleratorFromKeyboardEvent(event: {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): string | "clear" | undefined {
  if ((event.key === "Backspace" || event.key === "Delete") && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
    return "clear";
  }
  if (MODIFIER_KEYS.has(event.key)) return undefined;
  const key = tauriKeyName(event);
  if (!key) return undefined;
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("CommandOrControl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (parts.length === 0 && !/^F\d{1,2}$/u.test(key)) return undefined;
  parts.push(key);
  return parts.join("+");
}

function tauriKeyName(event: { key: string; code: string }): string | undefined {
  const code = event.code;
  const letter = /^Key([A-Z])$/u.exec(code);
  if (letter) return letter[1];
  const digit = /^Digit([0-9])$/u.exec(code);
  if (digit) return digit[1];
  const functionKey = /^F([1-9]|1[0-9]|2[0-4])$/u.exec(code);
  if (functionKey) return code;
  const named: Record<string, string> = {
    Space: "Space",
    Tab: "Tab",
    Enter: "Enter",
    Escape: "Esc",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Insert: "Insert",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backquote: "`",
  };
  if (named[code]) return named[code];
  if (event.key.length === 1 && event.key !== " ") return event.key.toUpperCase();
  return undefined;
}

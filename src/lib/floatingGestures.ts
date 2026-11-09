import type { QuickActionSettings } from "@shared/contracts";

export type ModifierKey = "Ctrl" | "Alt" | "Shift";
export type MouseSide = "left" | "right";
export type FloatingPointerIntent =
  | { type: "drag" }
  | { type: "quick-action"; button: MouseSide; modifier: ModifierKey }
  | { type: "context-menu" }
  | { type: "ignore" };

export function clickModifier(keys: {
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): ModifierKey | undefined {
  if (keys.ctrlKey) return "Ctrl";
  if (keys.altKey) return "Alt";
  if (keys.shiftKey) return "Shift";
  return undefined;
}

export function quickActionKey(
  button: MouseSide,
  kind: "Click" | "Drop",
  modifier: ModifierKey,
): keyof QuickActionSettings {
  return `${button}${kind}${modifier}`;
}

export function pointerIntent(
  button: MouseSide,
  modifier: ModifierKey | undefined,
  phase: "down" | "up",
): FloatingPointerIntent {
  if (phase === "down") {
    if (button === "left" && !modifier) return { type: "drag" };
    return { type: "ignore" };
  }
  if (modifier) return { type: "quick-action", button, modifier };
  if (button === "right") return { type: "context-menu" };
  return { type: "ignore" };
}

export function mouseSide(button: number): MouseSide | undefined {
  if (button === 0) return "left";
  if (button === 2) return "right";
  return undefined;
}

export function dropButton(buttons: number): MouseSide {
  if (buttons & 2) return "right";
  return "left";
}

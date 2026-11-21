/** Whether a key belongs to the focused element or can be a window-wide shortcut. */

/** What matters of the focused element (read from the DOM by the caller). */
export interface KeyTarget {
  /** Lowercase tag name: "input", "button", "a"... */
  tag: string;
  /** `type` of an input ("text", "range", "checkbox"...). */
  type?: string;
  /** ARIA role, if any. */
  role?: string | null;
  editable?: boolean;
}

/** Roles whose element uses Space or Enter itself (pressing it, choosing it). */
const PRESSABLE_ROLES = ["button", "link", "menuitem", "option", "tab", "checkbox", "switch", "radio"];
/** Roles moved between with the arrow keys. */
const ARROW_ROLES = ["menuitem", "option", "tab", "slider", "radio"];
const SLIDER_KEYS = ["Home", "End", "PageUp", "PageDown"];

/**
 * True when `key` is the focused element's own: typing in a field, Space on a button
 * (it presses it), arrows in a menu or on a slider. Then no shortcut runs: Space on a
 * focused «Me gusta» button must like the song, not pause the music.
 */
export function ownsKey(target: KeyTarget | null, key: string): boolean {
  if (!target) return false;
  if (target.editable || target.tag === "textarea" || target.tag === "select") return true;
  const arrowish = key.startsWith("Arrow") || SLIDER_KEYS.includes(key);
  if (target.tag === "input") {
    // Sliders (seek, volume) keep only their movement keys; other fields keep every key.
    return target.type === "range" ? arrowish : true;
  }
  const role = target.role ?? "";
  if (role === "slider") return arrowish;
  if (ARROW_ROLES.includes(role) && key.startsWith("Arrow")) return true;
  const pressable = target.tag === "button" || target.tag === "a" || PRESSABLE_ROLES.includes(role);
  return pressable && (key === " " || key === "Enter");
}

/** The parts of a DOM element that `ownsKey` looks at. */
export function describeTarget(el: EventTarget | null): KeyTarget | null {
  if (typeof HTMLElement === "undefined" || !(el instanceof HTMLElement)) return null;
  return {
    tag: el.tagName.toLowerCase(),
    type: el instanceof HTMLInputElement ? el.type : undefined,
    role: el.getAttribute("role"),
    editable: el.isContentEditable,
  };
}

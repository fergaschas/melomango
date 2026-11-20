/**
 * Which view is shown and the ones visited before it, for «Atrás». Pure (no React), so it
 * can be tested; App runs it with useReducer.
 */

/** «Atrás» remembers at most this many views. */
export const MAX_BACK = 50;

export interface NavState<V> {
  view: V;
  /** Views visited before this one, oldest first. */
  back: V[];
}

export type NavAction<V> = { type: "go"; view: V } | { type: "back" };

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

export function navReducer<V>(s: NavState<V>, action: NavAction<V>): NavState<V> {
  if (action.type === "back") {
    const prev = s.back[s.back.length - 1];
    return prev === undefined ? s : { view: prev, back: s.back.slice(0, -1) };
  }
  // Opening what is already shown changes nothing (and adds no step to go back).
  if (same(s.view, action.view)) return s;
  return { view: action.view, back: [...s.back.slice(-(MAX_BACK - 1)), s.view] };
}

/** The searches the user made, newest first (kept on this computer). */

const KEY = "melomango.recentSearches";
export const MAX_RECENT = 10;

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .trim();

/** `query` first; the same search (ignoring case and accents) only once. */
export function addRecent(list: string[], query: string): string[] {
  const q = query.trim();
  if (!q) return list;
  return [q, ...list.filter((r) => fold(r) !== fold(q))].slice(0, MAX_RECENT);
}

export function removeRecent(list: string[], query: string): string[] {
  return list.filter((r) => fold(r) !== fold(query));
}

/** Recent searches containing what is being typed (not the exact same text). */
export function matchingRecent(list: string[], typed: string, limit = 3): string[] {
  const t = fold(typed);
  if (!t) return list.slice(0, limit);
  return list.filter((r) => fold(r).includes(t) && fold(r) !== t).slice(0, limit);
}

export function loadRecent(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string").slice(0, MAX_RECENT) : [];
  } catch {
    return [];
  }
}

export function saveRecent(list: string[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* storage unavailable: not important */
  }
}

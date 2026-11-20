import type { Track } from "./api";

/** How a list of tracks can be ordered. "custom" is the list's own order. */
export type SortKey = "custom" | "added" | "title" | "artist" | "duration";

export interface SortState {
  key: SortKey;
  desc: boolean;
}

export const DEFAULT_SORT: SortState = { key: "custom", desc: false };

/** Natural direction when picking a key: newest first for dates, A→Z / shortest first otherwise. */
export const defaultDesc = (key: SortKey) => key === "added";

export interface Row {
  track: Track;
  /** Position in the original list (for reordering and stable ties). */
  index: number;
}

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

const collator = new Intl.Collator("es", { sensitivity: "base", numeric: true });

/** The rows to show: filtered by title or artist, then sorted (ties keep the list's order). */
export function arrange(tracks: Track[], addedAt: number[] | undefined, filter: string, sort: SortState): Row[] {
  const words = fold(filter).split(/\s+/).filter(Boolean);
  const rows = tracks
    .map((track, index) => ({ track, index }))
    .filter(({ track }) => {
      if (!words.length) return true;
      const text = fold(`${track.title} ${track.artist}`);
      return words.every((w) => text.includes(w));
    });

  const compare = (a: Row, b: Row): number => {
    switch (sort.key) {
      case "title":
        return collator.compare(a.track.title, b.track.title);
      case "artist":
        return collator.compare(a.track.artist, b.track.artist) || collator.compare(a.track.title, b.track.title);
      case "duration":
        return a.track.durationSecs - b.track.durationSecs;
      case "added":
        return (addedAt?.[a.index] ?? 0) - (addedAt?.[b.index] ?? 0);
      case "custom":
        return 0;
    }
  };

  return rows.sort((a, b) => {
    const order = compare(a, b) || a.index - b.index;
    return sort.desc ? -order : order;
  });
}

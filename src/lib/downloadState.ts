/** Pure helpers for downloads (sizes, a collection's download state). */

export type DownloadPhase = "queued" | "downloading" | "done" | "failed" | "cancelled";

export interface DownloadProgress {
  /** "source/id" */
  key: string;
  state: DownloadPhase;
  received: number;
  total: number | null;
  message: string | null;
}

const GB = 1024 ** 3;
/** Sizes offered for the automatic cache (0 = off). */
export const CACHE_OPTIONS = [0, 1, 2, 5, 10].map((gb) => gb * GB);
export const DEFAULT_CACHE = 2 * GB;

/** A stored cache size, or the default if it isn't one of the options. */
export function parseCacheLimit(raw: string | null): number {
  const value = Number(raw);
  return raw !== null && CACHE_OPTIONS.includes(value) ? value : DEFAULT_CACHE;
}

/** "1 canción", "3 canciones". */
export const songs = (n: number) => `${n} ${n === 1 ? "canción" : "canciones"}`;

/** 1536 → "1,5 KB"; sizes in the Spanish style. */
export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes || 0))} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const text = value >= 100 ? Math.round(value).toString() : value.toFixed(1).replace(".", ",");
  return `${text} ${units[unit]}`;
}

/** 0–100, or null while the size is unknown. */
export function percent(p: Pick<DownloadProgress, "received" | "total"> | undefined): number | null {
  if (!p || !p.total) return null;
  return Math.min(100, Math.floor((p.received / p.total) * 100));
}

export interface CollectionState<K> {
  total: number;
  downloaded: number;
  /** Queued or downloading right now. */
  pending: number;
  /** Neither downloaded nor on the way. */
  missing: K[];
}

/** How much of a list of songs is downloaded or on its way. */
export function collectionState<K>(
  items: K[],
  keyOf: (item: K) => string,
  downloaded: ReadonlySet<string>,
  progress: ReadonlyMap<string, DownloadProgress>,
): CollectionState<K> {
  const state: CollectionState<K> = { total: items.length, downloaded: 0, pending: 0, missing: [] };
  const seen = new Set<string>();
  for (const item of items) {
    const key = keyOf(item);
    if (seen.has(key)) continue;
    seen.add(key);
    const phase = progress.get(key)?.state;
    if (downloaded.has(key)) state.downloaded++;
    else if (phase === "queued" || phase === "downloading") state.pending++;
    else state.missing.push(item);
  }
  state.total = seen.size;
  return state;
}

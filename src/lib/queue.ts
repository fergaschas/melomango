import type { Track } from "./api";

/** Pure playback-queue logic (no React, no audio), so it is easy to reason about and test. */

export type Repeat = "off" | "all" | "one";

/** A queue entry: the same track can be queued twice, so entries get their own id. */
export interface QueueItem {
  uid: number;
  track: Track;
  /** Added by the automatic radio. */
  radio?: boolean;
}

export interface QueueState {
  /** Playback order. Entries before `index` were already played. */
  items: QueueItem[];
  /** Current entry, or -1 when nothing is loaded. */
  index: number;
  /** Order before shuffling, to restore it when shuffle is turned off. */
  original: QueueItem[] | null;
  shuffle: boolean;
  repeat: Repeat;
}

export const emptyQueue: QueueState = { items: [], index: -1, original: null, shuffle: false, repeat: "off" };

let nextUid = 1;
export const toItems = (tracks: Track[]): QueueItem[] => tracks.map((track) => ({ uid: nextUid++, track }));

export type Random = () => number;

function shuffled<T>(list: T[], random: Random): T[] {
  const out = list.slice();
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    [out[i], out[j]] = [out[j], out[i]];
  }
  return out;
}

export const currentItem = (s: QueueState): QueueItem | null => s.items[s.index] ?? null;
export const upcoming = (s: QueueState): QueueItem[] => (s.index < 0 ? [] : s.items.slice(s.index + 1));

/** Plays `tracks` from `start`; with shuffle on, the rest come in random order. */
export function play(s: QueueState, tracks: Track[], start: number, random: Random = Math.random): QueueState {
  const items = toItems(tracks);
  if (!s.shuffle || items.length === 0) return { ...s, items, index: items.length ? start : -1, original: null };
  const first = items[start];
  const rest = shuffled(items.filter((_, i) => i !== start), random);
  return { ...s, items: [first, ...rest], index: 0, original: items };
}

/** Turning shuffle on keeps the current track and shuffles every other one; off restores the order. */
export function setShuffle(s: QueueState, on: boolean, random: Random = Math.random): QueueState {
  if (on === s.shuffle) return s;
  const current = currentItem(s);
  if (on) {
    if (!current) return { ...s, shuffle: true };
    const others = shuffled(s.items.filter((_, i) => i !== s.index), random);
    return { ...s, items: [current, ...others], index: 0, original: s.items, shuffle: true };
  }
  if (!s.original) return { ...s, shuffle: false };
  const index = current ? s.original.findIndex((i) => i.uid === current.uid) : -1;
  return { ...s, items: s.original, index, original: null, shuffle: false };
}

export function cycleRepeat(s: QueueState): QueueState {
  const order: Repeat[] = ["off", "all", "one"];
  return { ...s, repeat: order[(order.indexOf(s.repeat) + 1) % order.length] };
}

/**
 * What comes after the current track. `auto` is true when the track ended by itself:
 * only then does "repeat one" replay it ("same"). "stop" means the queue is over.
 */
export function advance(s: QueueState, auto: boolean, random: Random = Math.random): QueueState | "same" | "stop" {
  if (s.index < 0) return "stop";
  if (auto && s.repeat === "one") return "same";
  if (s.index + 1 < s.items.length) return { ...s, index: s.index + 1 };
  if (s.repeat === "off") return "stop";
  // Repeat all: start over (in a new random order when shuffling).
  return s.shuffle ? { ...s, items: shuffled(s.items, random), index: 0 } : { ...s, index: 0 };
}

/** The previous track; at the start, the last one with "repeat all" (else stays). */
export function back(s: QueueState): QueueState {
  if (s.index > 0) return { ...s, index: s.index - 1 };
  if (s.repeat === "all" && s.items.length > 1) return { ...s, index: s.items.length - 1 };
  return s;
}

/** The track that `advance` would play next, if any (for prefetching). */
export function peekNext(s: QueueState): Track | null {
  if (s.index < 0) return null;
  if (s.index + 1 < s.items.length) return s.items[s.index + 1].track;
  return s.repeat === "all" && !s.shuffle ? (s.items[0]?.track ?? null) : null;
}

/** Inserts `track` right after the current one. */
export function playNext(s: QueueState, track: Track): QueueState {
  const [item] = toItems([track]);
  if (s.index < 0) return { ...s, items: [item], index: 0, original: s.shuffle ? [item] : null };
  const items = [...s.items.slice(0, s.index + 1), item, ...s.items.slice(s.index + 1)];
  return { ...s, items, original: insertAfterCurrent(s, item) };
}

/** Appends `track` to the end of the queue. */
export function addToQueue(s: QueueState, track: Track): QueueState {
  const [item] = toItems([track]);
  if (s.index < 0) return { ...s, items: [item], index: 0, original: s.shuffle ? [item] : null };
  return { ...s, items: [...s.items, item], original: s.original ? [...s.original, item] : null };
}

function insertAfterCurrent(s: QueueState, item: QueueItem): QueueItem[] | null {
  if (!s.original) return null;
  const current = currentItem(s);
  const at = current ? s.original.findIndex((i) => i.uid === current.uid) : -1;
  return [...s.original.slice(0, at + 1), item, ...s.original.slice(at + 1)];
}

/** Appends songs the automatic radio found. */
export function appendRadio(s: QueueState, tracks: Track[]): QueueState {
  const items = toItems(tracks).map((i) => ({ ...i, radio: true }));
  return { ...s, items: [...s.items, ...items], original: s.original ? [...s.original, ...items] : null };
}

/**
 * Appends what the radio found for the entry `seedUid`. Null if that entry left the queue
 * while the radio searched (the user started something else): those songs no longer fit.
 */
export function appendRadioFor(s: QueueState, seedUid: number, tracks: Track[]): QueueState | null {
  return s.items.some((i) => i.uid === seedUid) ? appendRadio(s, tracks) : null;
}

/**
 * Where to go once the radio filled a queue that had run out. Null if the entry `fromUid`
 * is no longer the current one (the user picked another song meanwhile) or nothing follows.
 */
export function advanceAfterRadio(s: QueueState, fromUid: number | null): QueueState | null {
  if (fromUid === null || currentItem(s)?.uid !== fromUid) return null;
  const result = advance(s, false);
  return result === "stop" || result === "same" ? null : result;
}

/** True when the current entry is the last one (where the radio takes over). */
export const atEnd = (s: QueueState) => s.index >= 0 && s.index === s.items.length - 1;

/** Removes an upcoming or played entry (never the current one). */
export function remove(s: QueueState, uid: number): QueueState {
  const at = s.items.findIndex((i) => i.uid === uid);
  if (at < 0 || at === s.index) return s;
  return {
    ...s,
    items: s.items.filter((i) => i.uid !== uid),
    index: at < s.index ? s.index - 1 : s.index,
    original: s.original?.filter((i) => i.uid !== uid) ?? null,
  };
}

/** Moves an upcoming entry to another upcoming position (indexes into `items`). */
export function move(s: QueueState, from: number, to: number): QueueState {
  if (from === to || from <= s.index || to <= s.index || from >= s.items.length || to >= s.items.length) return s;
  const items = s.items.slice();
  const [item] = items.splice(from, 1);
  items.splice(to, 0, item);
  return { ...s, items };
}

/** Makes the entry `uid` the current one. */
export function jump(s: QueueState, uid: number): QueueState {
  const index = s.items.findIndex((i) => i.uid === uid);
  return index < 0 ? s : { ...s, index };
}

/** Drops everything after the current track. */
export function clearUpcoming(s: QueueState): QueueState {
  const items = s.items.slice(0, s.index + 1);
  const kept = new Set(items.map((i) => i.uid));
  return { ...s, items, original: s.original?.filter((i) => kept.has(i.uid)) ?? null };
}

// ---------- Saving and restoring between sessions ----------

/** Larger queues are trimmed around the current track when saved. */
const MAX_SAVED = 3000;

export interface SavedQueue {
  tracks: Track[];
  index: number;
  /** Positions in `tracks` in their pre-shuffle order. */
  original: number[] | null;
  /** Positions in `tracks` added by the radio. */
  radio?: number[];
  shuffle: boolean;
  repeat: Repeat;
}

export function serialize(s: QueueState): SavedQueue {
  let start = 0;
  let items = s.items;
  if (items.length > MAX_SAVED) {
    start = Math.max(0, Math.min(s.index - 500, items.length - MAX_SAVED));
    items = items.slice(start, start + MAX_SAVED);
  }
  const position = new Map(items.map((item, i) => [item.uid, i]));
  const original = s.original
    ? s.original.map((i) => position.get(i.uid)).filter((p): p is number => p !== undefined)
    : null;
  const radio = items.flatMap((item, i) => (item.radio ? [i] : []));
  return { tracks: items.map((i) => i.track), index: s.index - start, original, radio, shuffle: s.shuffle, repeat: s.repeat };
}

export function deserialize(saved: SavedQueue): QueueState {
  const radio = new Set(saved.radio ?? []);
  const items = toItems(saved.tracks).map((item, i) => (radio.has(i) ? { ...item, radio: true } : item));
  const index = saved.index >= 0 && saved.index < items.length ? saved.index : items.length ? 0 : -1;
  const original = saved.original?.every((p) => items[p]) ? saved.original.map((p) => items[p]) : null;
  const repeat: Repeat = ["off", "all", "one"].includes(saved.repeat) ? saved.repeat : "off";
  return { items, index, original: saved.shuffle ? original : null, shuffle: !!saved.shuffle, repeat };
}

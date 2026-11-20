/**
 * Pure parts of the player: when a song counts as listened, and reading the saved state.
 * No React and no <audio>, so they can be tested.
 */
import type { SavedQueue } from "./queue";

/** A song counts as listened after this many seconds of actual playback (or half of it, if shorter). */
export const PLAY_COUNTS_AFTER = 30;

/** Playback actually heard of the current load (seeking doesn't count). */
export interface Listened {
  secs: number;
  /** Position at the previous update; null right after loading or seeking. */
  last: number | null;
  /** Already recorded in the history. */
  recorded: boolean;
}

export const freshListen = (): Listened => ({ secs: 0, last: null, recorded: false });

/**
 * One position update (timeupdate, about 4 a second). `length` is the song's length in
 * seconds (0 if unknown). Returns the new state and whether this update makes it a play.
 */
export function listenTick(l: Listened, position: number, playing: boolean, length: number): { next: Listened; counts: boolean } {
  let secs = l.secs;
  if (playing && l.last !== null) {
    const delta = position - l.last;
    // Small forward steps only: a jump (seek, a stall catching up) isn't listening.
    if (delta > 0 && delta < 2) secs += delta;
  }
  const needed = Math.min(PLAY_COUNTS_AFTER, length > 0 ? length / 2 : PLAY_COUNTS_AFTER);
  const counts = !l.recorded && secs >= needed;
  return { next: { secs, last: position, recorded: l.recorded || counts }, counts };
}

/** What the player remembers between sessions. */
export interface Saved {
  queue: SavedQueue;
  time: number;
  volume: number;
  muted: boolean;
  autoRadio?: boolean;
}

/**
 * The saved state from what localStorage holds: `raw` (queue, volume…) and `position`
 * (saved apart, more often). Null if there is nothing usable.
 */
export function parseSaved(raw: string | null, position: string | null): Saved | null {
  if (!raw) return null;
  try {
    const saved = JSON.parse(raw) as Saved;
    if (!saved || typeof saved !== "object" || !saved.queue) return null;
    const time = position === null ? NaN : Number(position);
    return Number.isFinite(time) ? { ...saved, time } : saved;
  } catch {
    return null;
  }
}

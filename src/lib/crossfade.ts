/** Crossfade between songs: the next one starts before the current one ends and they blend. */

export const MAX_CROSSFADE = 12;
const KEY = "melomango.crossfade";
/** Under this many seconds left there is no point starting a blend. */
const MIN_LEFT = 0.25;

/** Seconds of crossfade (0 = off). */
export function loadCrossfade(): number {
  try {
    return clampCrossfade(Number(localStorage.getItem(KEY) ?? 0));
  } catch {
    return 0;
  }
}

export function saveCrossfade(secs: number) {
  try {
    localStorage.setItem(KEY, String(clampCrossfade(secs)));
  } catch {
    /* storage unavailable: it just isn't remembered */
  }
}

export function clampCrossfade(secs: number): number {
  return Number.isFinite(secs) ? Math.min(MAX_CROSSFADE, Math.max(0, Math.round(secs))) : 0;
}

/**
 * Whether the next song should start now. Songs too short to blend (less than twice the
 * crossfade) just play to the end, and so do streams of unknown length.
 */
export function shouldStartFade(secs: number, currentTime: number, duration: number): boolean {
  if (secs <= 0 || !Number.isFinite(duration) || duration < secs * 2) return false;
  const left = duration - currentTime;
  return left <= secs && left > MIN_LEFT;
}

/**
 * Volume of the incoming and the outgoing song `t` seconds into the blend, as factors of
 * the chosen volume. Equal-power curves, so the blend doesn't dip in the middle.
 */
export function fadeGains(t: number, fadeIn: number, fadeOut: number): { in: number; out: number } {
  const pIn = fadeIn > 0 ? Math.min(1, Math.max(0, t / fadeIn)) : 1;
  const pOut = fadeOut > 0 ? Math.min(1, Math.max(0, t / fadeOut)) : 1;
  return {
    in: pIn >= 1 ? 1 : Math.sin((pIn * Math.PI) / 2),
    out: pOut >= 1 ? 0 : Math.cos((pOut * Math.PI) / 2),
  };
}

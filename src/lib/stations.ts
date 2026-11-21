/**
 * «Emisoras»: what is shown of a station, the ones heard lately and where the user is
 * (docs/emisoras.md). Pure functions and small settings kept on this computer.
 */
import type { Station } from "./api";

const RECENT_KEY = "melomango.stations.recent";
const COUNTRY_KEY = "melomango.stations.country";
const LOCATION_KEY = "melomango.stations.location";
const POSITION_KEY = "melomango.stations.position";
/** «Escuchadas hace poco». */
export const MAX_RECENT_STATIONS = 10;
/** The position is asked again when older than this. */
export const POSITION_MAX_AGE_MS = 24 * 60 * 60 * 1000;

/** «AAC 128», «MP3», or nothing when unknown. */
export function quality(s: Pick<Station, "codec" | "bitrate">): string {
  return [s.codec, s.bitrate > 0 ? String(s.bitrate) : ""].filter(Boolean).join(" ");
}

/** «a 2 km» (never «a 0 km»). */
export function distanceText(meters: number): string {
  return `a ${Math.max(1, Math.round(meters / 1000))} km`;
}

/** The line under a station's name: region, distance and quality, the ones it has. */
export function stationDetail(s: Station): string {
  return [s.state, s.distanceM != null ? distanceText(s.distanceM) : null, quality(s)].filter(Boolean).join(" · ");
}

/** `station` first; each station once, at most MAX_RECENT_STATIONS. */
export function addRecentStation(list: Station[], station: Station): Station[] {
  // The distance was from where the user was then.
  const kept = { ...station, distanceM: null };
  return [kept, ...list.filter((s) => s.uuid !== station.uuid)].slice(0, MAX_RECENT_STATIONS);
}

/** The station `step` places away in the list it was played from, or null at either end. */
export function neighbour(list: Station[], index: number, step: 1 | -1): number | null {
  const next = index + step;
  return next >= 0 && next < list.length ? next : null;
}

/** The country's name in Spanish («España»), or the code if the system doesn't know it. */
export function countryName(code: string): string {
  try {
    return new Intl.DisplayNames(["es"], { type: "region" }).of(code) ?? code;
  } catch {
    return code;
  }
}

export interface SavedPosition {
  lat: number;
  long: number;
  /** When it was asked (milliseconds since the epoch). */
  at: number;
}

/** About 1 km: the most the app keeps of where the user is. */
export const roundCoord = (x: number) => Math.round(x * 100) / 100;

/** A saved position, if it is a valid one and not older than a day. */
export function freshPosition(raw: string | null, now: number): SavedPosition | null {
  try {
    const p = JSON.parse(raw ?? "null") as Partial<SavedPosition> | null;
    if (!p || typeof p.lat !== "number" || typeof p.long !== "number" || typeof p.at !== "number") return null;
    if (Math.abs(p.lat) > 90 || Math.abs(p.long) > 180) return null;
    if (now - p.at > POSITION_MAX_AGE_MS || p.at > now) return null;
    return { lat: roundCoord(p.lat), long: roundCoord(p.long), at: p.at };
  } catch {
    return null;
  }
}

/** Whether a station read back from storage has what the app needs to show and play it. */
function isStation(value: unknown): value is Station {
  const s = value as Partial<Station> | null;
  return !!s && typeof s.uuid === "string" && typeof s.name === "string" && typeof s.url === "string";
}

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: not important */
  }
}

export function loadRecentStations(): Station[] {
  try {
    const value = JSON.parse(read(RECENT_KEY) ?? "[]");
    return Array.isArray(value) ? value.filter(isStation).slice(0, MAX_RECENT_STATIONS) : [];
  } catch {
    return [];
  }
}

export const saveRecentStations = (list: Station[]) => write(RECENT_KEY, JSON.stringify(list));

/** The country picked by hand (it wins over the system's region). */
export function loadChosenCountry(): string | null {
  const code = read(COUNTRY_KEY);
  return code && /^[A-Z]{2}$/.test(code) ? code : null;
}

export const saveChosenCountry = (code: string) => write(COUNTRY_KEY, code);

/** Whether «Cerca de ti» may use the approximate location (off until the user turns it on). */
export const loadUseLocation = () => read(LOCATION_KEY) === "1";

export function saveUseLocation(on: boolean) {
  write(LOCATION_KEY, on ? "1" : "0");
  // Turned off: nothing of where the user was stays behind.
  if (!on) write(POSITION_KEY, null);
}

export const loadPosition = () => freshPosition(read(POSITION_KEY), Date.now());

export function savePosition(lat: number, long: number) {
  write(POSITION_KEY, JSON.stringify({ lat: roundCoord(lat), long: roundCoord(long), at: Date.now() }));
}

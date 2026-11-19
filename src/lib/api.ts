import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface Track {
  id: string;
  source: string;
  title: string;
  artist: string;
  durationSecs: number;
  artworkUrl: string | null;
  genre: string | null;
}

export const DEFAULT_SOURCE = "audius";

export const searchTracks = (source: string, query: string) =>
  invoke<Track[]>("search", { source, query });

export const trendingTracks = (source: string) => invoke<Track[]>("trending", { source });

/** URL of the Rust `stream://` protocol that proxies the track's audio. */
export const streamUrl = (track: Track) => convertFileSrc(`${track.source}/${track.id}`, "stream");

export function formatTime(secs: number) {
  if (!Number.isFinite(secs) || secs < 0) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

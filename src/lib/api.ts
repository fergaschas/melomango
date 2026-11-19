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

export interface SourceInfo {
  id: string;
  name: string;
  kind: "builtin" | "plugin";
  version: string | null;
  description: string | null;
  supportsTrending: boolean;
}

export const DEFAULT_SOURCE = "audius";

export const listSources = () => invoke<SourceInfo[]>("list_sources");

export const searchTracks = (source: string, query: string) =>
  invoke<Track[]>("search", { source, query });

export const trendingTracks = (source: string) => invoke<Track[]>("trending", { source });

export const installPlugin = (path: string) => invoke<SourceInfo>("install_plugin", { path });

export const uninstallPlugin = (id: string) => invoke<void>("uninstall_plugin", { id });

export const openPluginsDir = () => invoke<void>("open_plugins_dir");

/** URL of the Rust `stream://` protocol that proxies the track's audio. */
export const streamUrl = (track: Track) => convertFileSrc(`${track.source}/${track.id}`, "stream");

export function formatTime(secs: number) {
  if (!Number.isFinite(secs) || secs < 0) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

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

export interface CatalogPlugin {
  id: string;
  name: string;
  version: string;
  description: string | null;
  author: string | null;
  homepage: string | null;
  tags: string[];
  registry: string;
  registryName: string;
  available: boolean;
}

export interface Catalog {
  plugins: CatalogPlugin[];
  errors: string[];
}

export interface RegistryInfo {
  url: string;
  official: boolean;
}

export const browsePlugins = () => invoke<Catalog>("browse_plugins");

export const installCatalogPlugin = (registry: string, id: string) =>
  invoke<SourceInfo>("install_catalog_plugin", { registry, id });

export const listRegistries = () => invoke<RegistryInfo[]>("list_registries");

export const addRegistry = (url: string) => invoke<RegistryInfo[]>("add_registry", { url });

export const removeRegistry = (url: string) => invoke<RegistryInfo[]>("remove_registry", { url });

/** URL of the Rust `stream://` protocol that proxies the track's audio. */
export const streamUrl = (track: Track) => convertFileSrc(`${track.source}/${track.id}`, "stream");

export function formatTime(secs: number) {
  if (!Number.isFinite(secs) || secs < 0) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

// ---------- Library ----------

export interface PlaylistSummary {
  id: number;
  name: string;
  trackCount: number;
  durationSecs: number;
  artworkUrls: string[];
}

export interface Playlist extends PlaylistSummary {
  tracks: Track[];
}

export const trackKey = (t: Pick<Track, "source" | "id">) => `${t.source}/${t.id}`;

export const likedTracks = () => invoke<Track[]>("liked_tracks");
export const likedKeys = () => invoke<string[]>("liked_keys");
export const setLiked = (track: Track, liked: boolean) => invoke<void>("set_liked", { track, liked });

export const listPlaylists = () => invoke<PlaylistSummary[]>("list_playlists");
export const createPlaylist = (name: string) => invoke<PlaylistSummary>("create_playlist", { name });
export const renamePlaylist = (id: number, name: string) => invoke<void>("rename_playlist", { id, name });
export const deletePlaylist = (id: number) => invoke<void>("delete_playlist", { id });
export const getPlaylist = (id: number) => invoke<Playlist>("get_playlist", { id });
export const addToPlaylist = (id: number, track: Track) => invoke<boolean>("add_to_playlist", { id, track });
export const removeFromPlaylist = (id: number, track: Track) =>
  invoke<void>("remove_from_playlist", { id, source: track.source, trackId: track.id });
export const moveInPlaylist = (id: number, from: number, to: number) =>
  invoke<void>("move_in_playlist", { id, from, to });

export function formatTotal(secs: number) {
  const mins = Math.round(secs / 60);
  return mins < 60 ? `${mins} min` : `${Math.floor(mins / 60)} h ${mins % 60} min`;
}

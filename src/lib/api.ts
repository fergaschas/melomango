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

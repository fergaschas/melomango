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
  isLibrary: boolean;
}

export const DEFAULT_SOURCE = "audius";

export const listSources = () => invoke<SourceInfo[]>("list_sources");

/** Built-in sources plus every installed plugin (including metadata-only ones like Spotify). */
export const listPlugins = () => invoke<SourceInfo[]>("list_plugins");

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

/** Resolves the track's audio in the background so it starts right away when played. */
export const prefetchStream = (track: Track) =>
  invoke("prefetch_stream", { source: track.source, id: track.id }).catch(() => {});

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

export interface PlaylistLink {
  pluginId: string;
  remoteId: string;
  syncedAt: number;
}

export interface Playlist extends PlaylistSummary {
  tracks: Track[];
  link: PlaylistLink | null;
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

// ---------- Lyrics ----------

export interface LyricLine {
  timeMs: number;
  text: string;
}

export interface Lyrics {
  found: boolean;
  instrumental: boolean;
  synced: LyricLine[];
  plain: string | null;
  matchedDurationSecs: number | null;
}

export const getLyrics = (track: Track) => invoke<Lyrics>("get_lyrics", { track });

// ---------- Discord ("Escuchando Melomango") ----------

export interface NowPlaying {
  title: string;
  artist: string;
  artworkUrl: string | null;
  durationSecs: number;
  positionSecs: number;
}

export type DiscordStatus =
  | { state: "unavailable" | "idle" | "notRunning" | "connected" }
  | { state: "error"; message: string };

export const discordSet = (now: NowPlaying | null) => invoke<void>("discord_set", { now }).catch(() => {});
export const discordStatus = () => invoke<DiscordStatus>("discord_status");

// ---------- Artists and albums (data from Deezer) ----------

export interface ArtistRef {
  id: string;
  name: string;
  pictureUrl: string | null;
}

export interface AlbumSummary {
  id: string;
  title: string;
  coverUrl: string | null;
  releaseDate: string | null;
  /** "album", "single", "ep" or "compile". */
  kind: string;
}

export interface ArtistPage {
  artist: ArtistRef;
  fans: number;
  top: Track[];
  albums: AlbumSummary[];
  related: ArtistRef[];
}

export interface AlbumPage extends AlbumSummary {
  artist: ArtistRef;
  tracks: Track[];
  durationSecs: number;
}

export interface ArtistHit extends ArtistRef {
  fans: number;
}

export interface AlbumHit extends AlbumSummary {
  artistName: string;
}

export interface PublicPlaylist {
  id: string;
  title: string;
  pictureUrl: string | null;
  owner: string;
  trackCount: number;
}

export interface PublicPlaylistPage extends PublicPlaylist {
  description: string | null;
  tracks: Track[];
  durationSecs: number;
}

export interface CatalogSearch {
  artists: ArtistHit[];
  albums: AlbumHit[];
  playlists: PublicPlaylist[];
}

/** Plugin id of saved Deezer playlists (synced from Deezer). */
export const DEEZER_ORIGIN = "deezer";

export const catalogSearch = (query: string) => invoke<CatalogSearch>("catalog_search", { query });
export const deezerPlaylist = (id: string) => invoke<PublicPlaylistPage>("deezer_playlist", { id });
export const saveDeezerPlaylist = (id: string) => invoke<PlaylistSummary>("save_deezer_playlist", { id });

export const artistPage = (name: string, id?: string) => invoke<ArtistPage>("artist_page", { name, id: id ?? null });
export const albumPage = (id: string) => invoke<AlbumPage>("album_page", { id });

/** Songs like `track` for the automatic radio, leaving out `exclude` ("source/id"). */
export const radioTracks = (track: Track, exclude: string[]) => invoke<Track[]>("radio_tracks", { track, exclude });

// ---------- History ----------

export interface Play {
  track: Track;
  /** Milliseconds since the epoch. */
  playedAt: number;
}

/** Fired on `window` after a play is recorded, so an open history view can reload. */
export const HISTORY_EVENT = "melomango:history";

export const recordPlay = (track: Track) =>
  invoke<void>("record_play", { track }).then(() => window.dispatchEvent(new Event(HISTORY_EVENT)));
export const recentPlays = (limit?: number) => invoke<Play[]>("recent_plays", { limit: limit ?? null });
export const clearHistory = () => invoke<void>("clear_history");

export interface MatchOptions {
  track: Track;
  query: string;
  /** The recording it plays now, if already matched. */
  current: { source: string; id: string } | null;
  candidates: Track[];
}

/** Recordings to choose from when an imported track plays the wrong one. */
export const matchOptions = (track: Track, query?: string) =>
  invoke<MatchOptions>("match_options", { source: track.source, id: track.id, query: query ?? null });

/** Plays `track` from `choice` from now on. */
export const chooseMatch = (track: Track, choice: Track) =>
  invoke<void>("choose_match", { source: track.source, id: track.id, playSource: choice.source, playId: choice.id });

/** Plugin id of playlists imported from Spotify's data export (they can't be synced). */
export const SPOTIFY_EXPORT = "spotify-export";

/** Imports the playlists and liked songs of a Spotify data export (its zip or JSON files). */
export const importSpotifyExport = (paths: string[]) =>
  invoke<PlaylistSummary[]>("import_spotify_export", { paths });

// ---------- Services (library plugins such as Spotify) ----------

export interface LibraryConnection {
  pluginId: string;
  name: string;
  connected: boolean;
  user: string | null;
  error: string | null;
}

export interface LibraryStatus {
  connected: boolean;
  user: string | null;
}

export interface CollectionView {
  id: string;
  name: string;
  kind: "liked" | "playlist" | "album" | string;
  owner: string | null;
  trackCount: number;
  artworkUrl: string | null;
  importable: boolean;
  importedPlaylistId: number | null;
}

export const libraryConnections = () => invoke<LibraryConnection[]>("library_connections");
export const connectLibrary = (pluginId: string) => invoke<LibraryStatus>("connect_library", { pluginId });
export const disconnectLibrary = (pluginId: string) => invoke<LibraryStatus>("disconnect_library", { pluginId });
export const libraryCollections = (pluginId: string) => invoke<CollectionView[]>("library_collections", { pluginId });
export const importCollection = (pluginId: string, collectionId: string, name: string) =>
  invoke<PlaylistSummary>("import_collection", { pluginId, collectionId, name });
export const syncPlaylist = (id: number) => invoke<PlaylistSummary>("sync_playlist", { id });

// ---------- Plugin requirements (e.g. yt-dlp for YouTube) ----------

export interface Requirement {
  id: string;
  name: string;
  description: string | null;
  installed: boolean;
  installable: boolean;
}

export const pluginRequirements = (pluginId: string) => invoke<Requirement[]>("plugin_requirements", { pluginId });
export const installPluginRequirement = (pluginId: string, requirementId: string) =>
  invoke<Requirement[]>("install_plugin_requirement", { pluginId, requirementId });

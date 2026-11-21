/**
 * What the app asks of the Rust side. The commands and their types are generated from
 * Rust in `bindings.ts` (tauri-specta; `cargo test` keeps it current), so a renamed
 * command, argument or field is a TypeScript error here instead of a failure at runtime.
 * This module gives them the names the rest of the app uses.
 *
 * Types with an `_Serialize` variant in the bindings come out of Rust with every field
 * present; that is the one named plainly here (`Track`). Commands accept the looser
 * `_Deserialize` variant, which every `_Serialize` value satisfies.
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import { commands } from "./bindings";
import type * as B from "./bindings";

export { formatTime, formatTotal } from "./format";

export type Track = B.Track_Serialize;
export type SourceInfo = B.SourceInfo_Serialize;
export type CatalogPlugin = B.CatalogPlugin_Serialize;
export type Catalog = B.Catalog_Serialize;
export type RegistryInfo = B.RegistryInfo;
export type PlaylistSummary = B.PlaylistSummary;
export type PlaylistLink = B.PlaylistLink;
export type Playlist = B.Playlist_Serialize;
export type LyricLine = B.LyricLine;
export type Lyrics = B.Lyrics_Serialize;
export type NowPlaying = B.NowPlaying_Serialize;
export type DiscordStatus = B.Status;
export type ArtistRef = B.ArtistRef_Serialize;
export type AlbumSummary = B.AlbumSummary_Serialize;
export type ArtistPage = B.ArtistPage_Serialize;
export type AlbumPage = B.AlbumPage_Serialize;
export type ArtistHit = B.ArtistHit_Serialize;
export type AlbumHit = B.AlbumHit_Serialize;
export type PublicPlaylist = B.PublicPlaylist_Serialize;
export type PublicPlaylistPage = B.PlaylistPage_Serialize;
export type CatalogSearch = B.CatalogSearch_Serialize;
export type Suggestions = B.Suggestions_Serialize;
export type Play = B.Play_Serialize;
export type Download = B.Download_Serialize;
export type MonthSummary = B.MonthSummary_Serialize;
export type MatchOptions = B.MatchOptions_Serialize;
export type LibraryConnection = B.Connection_Serialize;
export type LibraryStatus = B.LibraryStatus_Serialize;
export type CollectionView = B.CollectionView_Serialize;
export type Requirement = B.Requirement_Serialize;

/** Commands that return nothing come back as `null`: to the rest of the app they are `void`. */
const done = (p: Promise<unknown>): Promise<void> => p.then(() => undefined);

export const DEFAULT_SOURCE = "youtube";

export const listSources = () => commands.listSources();

/** Built-in sources plus every installed plugin (including metadata-only ones like Spotify). */
export const listPlugins = () => commands.listPlugins();

export const searchTracks = (source: string, query: string) => commands.search(source, query);

export const trendingTracks = (source: string) => commands.trending(source);

export const installPlugin = (path: string) => commands.installPlugin(path);

export const uninstallPlugin = (id: string) => done(commands.uninstallPlugin(id));

export const openPluginsDir = () => done(commands.openPluginsDir());

export const browsePlugins = () => commands.browsePlugins();

/** `replaceOtherOrigin`: the user agreed to replace a same-id plugin from another repository. */
export const installCatalogPlugin = (registry: string, id: string, replaceOtherOrigin = false) =>
  commands.installCatalogPlugin(registry, id, replaceOtherOrigin);

/** The repository the installed plugin `id` came from, if installing it from `registry` would replace it. */
export const pluginOriginChange = (registry: string, id: string) => commands.pluginOriginChange(registry, id);

export const listRegistries = () => commands.listRegistries();

export const addRegistry = (url: string) => commands.addRegistry(url);

export const removeRegistry = (url: string) => commands.removeRegistry(url);

/** URL of the Rust `stream://` protocol that proxies the track's audio. */
export const streamUrl = (track: Track) => convertFileSrc(`${track.source}/${track.id}`, "stream");

/** Resolves the track's audio in the background so it starts right away when played. */
export const prefetchStream = (track: Track) => commands.prefetchStream(track.source, track.id).catch(() => {});

// ---------- Library ----------

export const trackKey = (t: Pick<Track, "source" | "id">) => `${t.source}/${t.id}`;

export const likedTracks = () => commands.likedTracks();
export const likedKeys = () => commands.likedKeys();
export const setLiked = (track: Track, liked: boolean) => done(commands.setLiked(track, liked));

export const listPlaylists = () => commands.listPlaylists();
export const createPlaylist = (name: string) => commands.createPlaylist(name);
export const renamePlaylist = (id: number, name: string) => done(commands.renamePlaylist(id, name));
export const deletePlaylist = (id: number) => done(commands.deletePlaylist(id));
export const getPlaylist = (id: number) => commands.getPlaylist(id);
export const addToPlaylist = (id: number, track: Track) => commands.addToPlaylist(id, track);
export const removeFromPlaylist = (id: number, track: Track) => done(commands.removeFromPlaylist(id, track.source, track.id));
export const moveInPlaylist = (id: number, from: number, to: number) => done(commands.moveInPlaylist(id, from, to));

// ---------- Lyrics ----------

export const getLyrics = (track: Track) => commands.getLyrics(track);

// ---------- Mini player ----------

/** Shrinks the window to the mini player (always on top) or puts it back. */
export const setMiniMode = (on: boolean) => done(commands.setMiniMode(on));

// ---------- System tray ----------

export const trayUpdate = (title: string | null, playing: boolean) => commands.trayUpdate(title, playing).catch(() => {});
export const setCloseToTray = (on: boolean) => commands.setCloseToTray(on).catch(() => {});

// ---------- Discord ("Escuchando Melomango") ----------

export const discordSet = (now: NowPlaying | null) => commands.discordSet(now).catch(() => {});
export const discordStatus = () => commands.discordStatus();

// ---------- Artists and albums (data from Deezer) ----------

/** Plugin id of saved Deezer playlists (synced from Deezer). */
export const DEEZER_ORIGIN = "deezer";

/** Artists and songs to suggest while typing a search. */
export const searchSuggestions = (query: string) => commands.searchSuggestions(query);

export const catalogSearch = (query: string) => commands.catalogSearch(query);
export const deezerPlaylist = (id: string) => commands.deezerPlaylist(id);
export const saveDeezerPlaylist = (id: string) => commands.saveDeezerPlaylist(id);

export const artistPage = (name: string, id?: string) => commands.artistPage(name, id ?? null);
export const albumPage = (id: string) => commands.albumPage(id);

/** Songs like `track` for the automatic radio, leaving out `exclude` ("source/id"). */
export const radioTracks = (track: Track, exclude: string[]) => commands.radioTracks(track, exclude);

// ---------- History ----------

/** Fired on `window` after a play is recorded, so an open history view can reload. */
export const HISTORY_EVENT = "melomango:history";

export const recordPlay = (track: Track) =>
  commands.recordPlay(track).then(() => window.dispatchEvent(new Event(HISTORY_EVENT)));

// ---------- Downloads ----------

/** Starts downloading in the background; progress comes as `download-progress` events. */
export const downloadTracks = (tracks: Track[]) => commands.downloadTracks(tracks);
export const removeDownload = (t: Track) => done(commands.removeDownload(t.source, t.id));
export const listDownloads = () => commands.listDownloads();
export const downloadedKeys = () => commands.downloadedKeys();

/** Bytes the automatic cache may use (0 turns it off and empties it). */
export const setCacheLimit = (bytes: number) => done(commands.setCacheLimit(bytes));
export const cacheUsage = () => commands.cacheUsage();
export const clearCache = () => done(commands.clearCache());

export const recentPlays = (limit?: number) => commands.recentPlays(limit ?? null);
export const clearHistory = () => done(commands.clearHistory());

/** Months with plays ("YYYY-MM"), newest first. */
export const summaryMonths = () => commands.summaryMonths();
export const monthSummary = (month: string) => commands.monthSummary(month);

/** Recordings to choose from when an imported track plays the wrong one. */
export const matchOptions = (track: Track, query?: string) => commands.matchOptions(track.source, track.id, query ?? null);

/** Plays `track` from `choice` from now on. */
export const chooseMatch = (track: Track, choice: Track) =>
  done(commands.chooseMatch(track.source, track.id, choice.source, choice.id));

/** Plugin id of playlists imported from Spotify's data export (they can't be synced). */
export const SPOTIFY_EXPORT = "spotify-export";

/** A new playlist from pasted text, one song per line ("Artista - Título"). */
export const importText = (name: string, text: string) => commands.importText(name, text);

/** Imports the playlist or album behind a Spotify, YouTube or Deezer link. */
export const importLink = (url: string) => commands.importLink(url);

/**
 * Imports the playlists and liked songs of a Spotify data export (its zip or JSON files).
 * `problems`: files (or parts) that couldn't be read and were skipped, one line each.
 */
export const importSpotifyExport = (paths: string[]) => commands.importSpotifyExport(paths);

// ---------- Services (library plugins such as Spotify) ----------

export const libraryConnections = () => commands.libraryConnections();
export const connectLibrary = (pluginId: string) => commands.connectLibrary(pluginId);
export const disconnectLibrary = (pluginId: string) => commands.disconnectLibrary(pluginId);
export const libraryCollections = (pluginId: string) => commands.libraryCollections(pluginId);
export const importCollection = (pluginId: string, collectionId: string, name: string) =>
  commands.importCollection(pluginId, collectionId, name);
export const syncPlaylist = (id: number) => commands.syncPlaylist(id);

// ---------- Plugin requirements (e.g. yt-dlp for YouTube) ----------

export const pluginRequirements = (pluginId: string) => commands.pluginRequirements(pluginId);
export const installPluginRequirement = (pluginId: string, requirementId: string) =>
  commands.installPluginRequirement(pluginId, requirementId);

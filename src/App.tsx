import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DEFAULT_SOURCE, listPlugins, listSources, searchTracks, trackKey, trendingTracks, type SourceInfo, type Track } from "./lib/api";
import { MatchFixContext, type MatchFix } from "./lib/matchFix";
import { useLibrary } from "./lib/library";
import { usePlayer, type Player } from "./hooks/usePlayer";
import { useUpdater } from "./hooks/useUpdater";
import { useShortcuts } from "./hooks/useShortcuts";
import { useDiscordPresence, useDiscordSetting } from "./hooks/useDiscord";
import { useCloseToTraySetting, useTray } from "./hooks/useTray";
import { useTheme } from "./lib/theme";
import { SettingsDialog } from "./components/SettingsDialog";
import { setupRequirements } from "./lib/requirements";
import { UpdateCard } from "./components/UpdateCard";
import { TrackList } from "./components/TrackList";
import { PlayerBar, type SidePanel } from "./components/PlayerBar";
import { QueuePanel } from "./components/QueuePanel";
import { ShortcutsDialog } from "./components/ShortcutsDialog";
import { MatchDialog } from "./components/MatchDialog";
import { PluginsDialog } from "./components/PluginsDialog";
import { ServicesDialog } from "./components/ServicesDialog";
import { Sidebar } from "./components/Sidebar";
import { LikesView, PlaylistView } from "./components/LibraryViews";
import { HistoryView } from "./components/HistoryView";
import { DownloadsView } from "./components/DownloadsView";
import { useDownloads } from "./lib/downloads";
import { SummaryView } from "./components/SummaryView";
import { HomeView } from "./components/HomeView";
import { MiniPlayer } from "./components/MiniPlayer";
import { YouTubeSetup } from "./components/YouTubeSetup";
import { setMiniMode } from "./lib/api";
import { AlbumView, ArtistView, PublicPlaylistView } from "./components/CatalogViews";
import { SearchResults, type SearchFilter } from "./components/SearchResults";
import { ErrorNotice } from "./components/ErrorNotice";
import { SearchBox } from "./components/SearchBox";
import { addRecent, loadRecent, saveRecent } from "./lib/recentSearches";
import { NavContext, type Nav } from "./lib/nav";
import { LyricsPanel } from "./components/LyricsPanel";
import { BackIcon, PuzzleIcon } from "./components/Icons";
import logo from "../app-icon.svg";
import "./App.css";

export type View =
  | { kind: "home" }
  | { kind: "trending" }
  | { kind: "search"; query: string; filter?: SearchFilter }
  | { kind: "likes" }
  | { kind: "history" }
  | { kind: "downloads" }
  | { kind: "summary" }
  | { kind: "artist"; name: string; id?: string }
  | { kind: "album"; id: string }
  | { kind: "publicPlaylist"; id: string }
  | { kind: "playlist"; id: number };

const SOURCE_KEY = "melomango.source";
const PANEL_KEY = "melomango.panel";
/** Before the queue panel existed, only the lyrics panel was remembered. */
const OLD_LYRICS_KEY = "melomango.lyricsOpen";

function savedPanel(): SidePanel {
  try {
    const panel = localStorage.getItem(PANEL_KEY);
    if (panel === "lyrics" || panel === "queue") return panel;
    return localStorage.getItem(OLD_LYRICS_KEY) === "1" ? "lyrics" : null;
  } catch {
    return null;
  }
}

function savedSource() {
  try {
    return localStorage.getItem(SOURCE_KEY) ?? DEFAULT_SOURCE;
  } catch {
    return DEFAULT_SOURCE;
  }
}

export default function App() {
  const player = usePlayer();
  const updater = useUpdater();
  const library = useLibrary();
  const { toast } = library;
  const [sources, setSources] = useState<SourceInfo[]>([]);
  /** False until the first list of sources arrives (so the setup screen doesn't flash). */
  const [sourcesLoaded, setSourcesLoaded] = useState(false);
  const [plugins, setPlugins] = useState<SourceInfo[]>([]);
  const [showServices, setShowServices] = useState(false);
  const [sourceId, setSourceId] = useState(savedSource);
  const [showPlugins, setShowPlugins] = useState(false);
  const [query, setQuery] = useState("");
  const [view, setCurrentView] = useState<View>({ kind: "home" });
  // Views visited before this one, for "Atrás" (kept in refs too so handlers see the latest).
  const [backStack, setBackStack] = useState<View[]>([]);
  const viewRef = useRef(view);
  const backRef = useRef(backStack);
  const contentRef = useRef<HTMLElement>(null);

  const setView = useCallback((next: View) => {
    const prev = viewRef.current;
    if (JSON.stringify(prev) === JSON.stringify(next)) return;
    backRef.current = [...backRef.current.slice(-49), prev];
    setBackStack(backRef.current);
    viewRef.current = next;
    setCurrentView(next);
  }, []);

  const goBack = useCallback(() => {
    const stack = backRef.current;
    if (!stack.length) return;
    const prev = stack[stack.length - 1];
    backRef.current = stack.slice(0, -1);
    setBackStack(backRef.current);
    viewRef.current = prev;
    setCurrentView(prev);
  }, []);

  const nav = useMemo<Nav>(
    () => ({
      openArtist: (name, id) => setView({ kind: "artist", name, ...(id ? { id } : {}) }),
      openAlbum: (id) => setView({ kind: "album", id }),
      openPublicPlaylist: (id) => setView({ kind: "publicPlaylist", id }),
      openLibraryPlaylist: (id) => setView({ kind: "playlist", id }),
    }),
    [setView],
  );

  // A new view starts at the top.
  useEffect(() => {
    contentRef.current?.scrollTo({ top: 0 });
  }, [view]);

  // Alt+← and the mouse's "back" button go back, like in a browser.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") {
        e.preventDefault();
        goBack();
      }
    };
    const onMouse = (e: MouseEvent) => {
      if (e.button === 3) {
        e.preventDefault();
        goBack();
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mouseup", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mouseup", onMouse);
    };
  }, [goBack]);
  const [panel, setPanel] = useState<SidePanel>(savedPanel);
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [discordEnabled, setDiscordEnabled] = useDiscordSetting();
  useDiscordPresence(player, discordEnabled);
  const [closeToTray, setCloseToTray] = useCloseToTraySetting();
  const [theme, setTheme] = useTheme();
  const [mini, setMini] = useState(false);
  const downloads = useDownloads();

  const toggleMini = useCallback(
    (on: boolean) => {
      setMiniMode(on)
        .then(() => setMini(on))
        .catch((e) => library.notify(`No se pudo cambiar al mini reproductor: ${e}`));
    },
    [library],
  );
  useTray(player);
  const [fixing, setFixing] = useState<Track | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const togglePanel = useCallback((which: "lyrics" | "queue") => setPanel((p) => (p === which ? null : which)), []);

  useShortcuts({
    player,
    onLike: () => player.current && library.toggleLike(player.current),
    onSearch: () => {
      searchRef.current?.focus();
      searchRef.current?.select();
    },
    onTogglePanel: togglePanel,
    onHelp: () => setShowShortcuts(true),
  });

  const source = sources.find((s) => s.id === sourceId) ?? null;

  // Tracks from sources that can't play (e.g. Spotify) play through a match the user can fix.
  const matchFix = useMemo<MatchFix>(() => {
    const playable = new Set(sources.map((s) => s.id));
    return { canFix: (t) => sources.length > 0 && !playable.has(t.source), open: setFixing };
  }, [sources]);

  function onMatchChosen(track: Track) {
    setFixing(null);
    // A download of the old version was deleted.
    downloads.refresh().catch(() => {});
    library.notify(`Hecho: «${track.title}» sonará siempre con la versión que has elegido`);
    if (player.current && trackKey(player.current) === trackKey(track)) player.reloadCurrent();
  }

  const refreshSources = useCallback((selectId?: string) => {
    listSources()
      .then((list) => {
        setSources(list);
        setSourcesLoaded(true);
        setSourceId((current) => {
          // Only switch to a newly installed plugin if it is a playable source.
          const wanted = selectId && list.some((s) => s.id === selectId) ? selectId : current;
          return list.some((s) => s.id === wanted) ? wanted : DEFAULT_SOURCE;
        });
      })
      .catch(() => {});
    listPlugins().then(setPlugins).catch(() => {});
  }, []);

  useEffect(() => refreshSources(), [refreshSources]);

  useEffect(() => {
    try {
      localStorage.setItem(PANEL_KEY, panel ?? "none");
    } catch {
      /* storage unavailable: not important */
    }
  }, [panel]);

  useEffect(() => {
    try {
      localStorage.setItem(SOURCE_KEY, sourceId);
    } catch {
      /* storage unavailable: not important */
    }
  }, [sourceId]);

  function runSearch(text: string) {
    const q = text.trim();
    if (q) saveRecent(addRecent(loadRecent(), q));
    setView(q ? { kind: "search", query: q } : { kind: "home" });
  }

  function selectSource(id: string) {
    setSourceId(id);
    // Switching source from a library view goes back to browsing.
    if (view.kind !== "home" && view.kind !== "trending" && view.kind !== "search") setView({ kind: "home" });
  }

  if (mini) {
    return <MiniPlayer player={player} onExpand={() => toggleMini(false)} />;
  }

  return (
    <NavContext.Provider value={nav}>
      <MatchFixContext.Provider value={matchFix}>
        <div className={`app${panel ? " with-panel" : ""}`}>
          <header className="topbar">
            <button className="logo" onClick={() => setView({ kind: "home" })}>
              <img className="logo-mark" src={logo} alt="" width={34} height={34} /> Melomango
            </button>
            <button
              className="icon-btn back-btn"
              onClick={goBack}
              disabled={backStack.length === 0}
              aria-label="Atrás"
              title="Atrás (Alt+←)"
            >
              <BackIcon size={20} />
            </button>
            <SearchBox
            value={query}
            onChange={setQuery}
            onSearch={runSearch}
            onOpenArtist={nav.openArtist}
            onPlayTrack={(track) => player.playQueue([track], 0)}
            placeholder={source ? `Busca en ${source.name}…` : "Buscar…"}
            inputRef={searchRef}
          />
            <div className="source-tabs" role="tablist" aria-label="Fuente de música">
              {sources.map((s) => (
                <button
                  key={s.id}
                  role="tab"
                  aria-selected={s.id === sourceId}
                  className={`source-tab${s.id === sourceId ? " active" : ""}`}
                  onClick={() => selectSource(s.id)}
                >
                  {s.name}
                </button>
              ))}
            </div>
            <button className="btn ghost plugins-btn" onClick={() => setShowPlugins(true)}>
              <PuzzleIcon size={16} /> Plugins
            </button>
          </header>

          <Sidebar
            view={view}
            onNavigate={setView}
            updater={updater}
            onImport={() => setShowServices(true)}
            onShowShortcuts={() => setShowShortcuts(true)}
          onShowSettings={() => setShowSettings(true)}
          />

          <main className="content" ref={contentRef}>
            {sourcesLoaded && sources.length === 0 ? (
              // Nothing to play from yet (a fresh install): get YouTube ready first.
              <YouTubeSetup onReady={refreshSources} />
            ) : (
              <>
                {view.kind === "likes" && <LikesView player={player} />}
                {view.kind === "home" && (
                  <HomeView source={source} player={player} onSeeTrending={() => setView({ kind: "trending" })} />
                )}
                {view.kind === "history" && <HistoryView player={player} />}
                {view.kind === "downloads" && <DownloadsView player={player} />}
                {view.kind === "summary" && <SummaryView player={player} />}
                {view.kind === "artist" && (
                  <ArtistView key={view.id ?? view.name} name={view.name} id={view.id} player={player} />
                )}
                {view.kind === "album" && <AlbumView key={view.id} id={view.id} player={player} />}
                {view.kind === "publicPlaylist" && <PublicPlaylistView key={view.id} id={view.id} player={player} />}
                {view.kind === "playlist" && (
                  <PlaylistView key={view.id} id={view.id} player={player} onDeleted={() => setView({ kind: "likes" })} />
                )}
                {(view.kind === "trending" || view.kind === "search") && (
                  <BrowseView
                    view={view}
                    source={source}
                    player={player}
                    onFilter={(filter) => view.kind === "search" && setView({ ...view, filter })}
                  />
                )}
              </>
            )}
          </main>

          {panel === "lyrics" && <LyricsPanel player={player} onClose={() => setPanel(null)} />}
          {panel === "queue" && <QueuePanel player={player} onClose={() => setPanel(null)} />}

          <PlayerBar player={player} panel={panel} onTogglePanel={togglePanel} onMini={() => toggleMini(true)} />

          <UpdateCard updater={updater} />

          {toast && (
            <div className="toast" role="status">
              {toast}
            </div>
          )}

          {showShortcuts && <ShortcutsDialog onClose={() => setShowShortcuts(false)} />}

        {showSettings && (
          <SettingsDialog
            discordEnabled={discordEnabled}
            onDiscordChange={setDiscordEnabled}
            closeToTray={closeToTray}
            onCloseToTrayChange={setCloseToTray}
            theme={theme}
            onThemeChange={setTheme}
            crossfade={player.crossfade}
            onCrossfadeChange={player.setCrossfade}
            normalize={player.normalize}
            onNormalizeChange={player.setNormalize}
            onClose={() => setShowSettings(false)}
          />
        )}

          {fixing && (
            <MatchDialog track={fixing} onClose={() => setFixing(null)} onChosen={() => onMatchChosen(fixing)} />
          )}

          {showPlugins && (
            <PluginsDialog sources={plugins} onChanged={refreshSources} onClose={() => setShowPlugins(false)} />
          )}

          {showServices && (
            <ServicesDialog
              onClose={() => setShowServices(false)}
              onOpenPlugins={() => {
                setShowServices(false);
                setShowPlugins(true);
              }}
              onOpenPlaylist={(id) => {
                setShowServices(false);
                setView({ kind: "playlist", id });
              }}
            />
          )}
        </div>
      </MatchFixContext.Provider>
    </NavContext.Provider>
  );
}

function BrowseView({
  view,
  source,
  player,
  onFilter,
}: {
  view: Extract<View, { kind: "trending" | "search" }>;
  source: SourceInfo | null;
  player: Player;
  onFilter: (filter: SearchFilter) => void;
}) {
  const query = view.kind === "search" ? view.query : null;
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [fixing, setFixing] = useState<string | null>(null);

  async function fixRequirements() {
    if (!source) return;
    try {
      await setupRequirements(source, setFixing);
      setRetry((r) => r + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setFixing(null);
    }
  }

  useEffect(() => {
    if (!source) return;
    setError(null);
    if (view.kind === "trending" && !source.supportsTrending) {
      setTracks([]);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    const request = query === null ? trendingTracks(source.id) : searchTracks(source.id, query);
    request
      .then((result) => !cancelled && setTracks(result))
      .catch((e) => !cancelled && setError(String(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
    // Changing the search tab keeps the same songs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.kind, query, source, retry]);

  const sourceName = source?.name ?? "…";
  const emptyMessage =
    view.kind === "trending" && source && !source.supportsTrending
      ? `Busca algo en ${sourceName} para empezar.`
      : "Ni un mango. Prueba otra búsqueda.";

  const tracksBlock = (
    <>
      {loading && <p className="status">Pelando mangos…</p>}
      {error && (
        <ErrorNotice error={error} onRetry={() => setRetry((r) => r + 1)}>
          {source?.kind === "plugin" && /yt-dlp|requisito/i.test(error) && (
            <button className="btn primary small" onClick={fixRequirements} disabled={fixing !== null}>
              {fixing ?? "Instalar lo que falta"}
            </button>
          )}
        </ErrorNotice>
      )}
      {!loading && !error && tracks.length === 0 && <p className="status">{emptyMessage}</p>}
      {!loading && !error && tracks.length > 0 && (
        <TrackList
          tracks={tracks}
          current={player.current}
          isPlaying={player.isPlaying}
          onPlay={(i) => player.playQueue(tracks, i)}
          queue={player}
        />
      )}
    </>
  );

  if (view.kind === "search") {
    return (
      <>
        <h1>Resultados para “{view.query}”</h1>
        <p className="subtitle">Canciones de {sourceName}; artistas, álbumes y playlists de Deezer.</p>
        <SearchResults
          query={view.query}
          filter={view.filter ?? "all"}
          onFilter={onFilter}
          player={player}
          tracks={error ? [] : tracks}
          tracksBlock={tracksBlock}
          tracksLoading={loading}
          tracksFailed={error !== null}
          sourceName={sourceName}
        />
      </>
    );
  }

  return (
    <>
      <h1>Tendencias</h1>
      <p className="subtitle">Música gratis vía {sourceName}. Sin pagar ni un mango.</p>
      {tracksBlock}
    </>
  );
}

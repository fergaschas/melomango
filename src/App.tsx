import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { DEFAULT_SOURCE, listPlugins, listSources, searchTracks, trendingTracks, type SourceInfo, type Track } from "./lib/api";
import { useLibrary } from "./lib/library";
import { usePlayer, type Player } from "./hooks/usePlayer";
import { useUpdater } from "./hooks/useUpdater";
import { useShortcuts } from "./hooks/useShortcuts";
import { setupRequirements } from "./lib/requirements";
import { UpdateCard } from "./components/UpdateCard";
import { TrackList } from "./components/TrackList";
import { PlayerBar, type SidePanel } from "./components/PlayerBar";
import { QueuePanel } from "./components/QueuePanel";
import { ShortcutsDialog } from "./components/ShortcutsDialog";
import { PluginsDialog } from "./components/PluginsDialog";
import { ServicesDialog } from "./components/ServicesDialog";
import { Sidebar } from "./components/Sidebar";
import { LikesView, PlaylistView } from "./components/LibraryViews";
import { LyricsPanel } from "./components/LyricsPanel";
import { PuzzleIcon, SearchIcon } from "./components/Icons";
import logo from "../app-icon.svg";
import "./App.css";

export type View =
  | { kind: "trending" }
  | { kind: "search"; query: string }
  | { kind: "likes" }
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
  const [plugins, setPlugins] = useState<SourceInfo[]>([]);
  const [showServices, setShowServices] = useState(false);
  const [sourceId, setSourceId] = useState(savedSource);
  const [showPlugins, setShowPlugins] = useState(false);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>({ kind: "trending" });
  const [panel, setPanel] = useState<SidePanel>(savedPanel);
  const [showShortcuts, setShowShortcuts] = useState(false);
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

  const refreshSources = useCallback((selectId?: string) => {
    listSources()
      .then((list) => {
        setSources(list);
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

  function onSearch(e: FormEvent) {
    e.preventDefault();
    const q = query.trim();
    setView(q ? { kind: "search", query: q } : { kind: "trending" });
  }

  function selectSource(id: string) {
    setSourceId(id);
    // Switching source from a library view goes back to browsing.
    if (view.kind === "likes" || view.kind === "playlist") setView({ kind: "trending" });
  }

  return (
    <div className={`app${panel ? " with-panel" : ""}`}>
      <header className="topbar">
        <button className="logo" onClick={() => setView({ kind: "trending" })}>
          <img className="logo-mark" src={logo} alt="" width={34} height={34} /> Melomango
        </button>
        <form className="search" onSubmit={onSearch}>
          <SearchIcon size={18} />
          <input
            ref={searchRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Busca en ${source?.name ?? "…"}…`}
            aria-label="Buscar"
            title="Buscar (Ctrl+F)"
            onKeyDown={(e) => e.key === "Escape" && e.currentTarget.blur()}
          />
        </form>
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
      />

      <main className="content">
        {view.kind === "likes" && <LikesView player={player} />}
        {view.kind === "playlist" && (
          <PlaylistView id={view.id} player={player} onDeleted={() => setView({ kind: "likes" })} />
        )}
        {(view.kind === "trending" || view.kind === "search") && (
          <BrowseView view={view} source={source} player={player} />
        )}
      </main>

      {panel === "lyrics" && <LyricsPanel player={player} onClose={() => setPanel(null)} />}
      {panel === "queue" && <QueuePanel player={player} onClose={() => setPanel(null)} />}

      <PlayerBar player={player} panel={panel} onTogglePanel={togglePanel} />

      <UpdateCard updater={updater} />

      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}

      {showShortcuts && <ShortcutsDialog onClose={() => setShowShortcuts(false)} />}

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
  );
}

function BrowseView({
  view,
  source,
  player,
}: {
  view: Extract<View, { kind: "trending" | "search" }>;
  source: SourceInfo | null;
  player: Player;
}) {
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
    const request = view.kind === "trending" ? trendingTracks(source.id) : searchTracks(source.id, view.query);
    request
      .then((result) => !cancelled && setTracks(result))
      .catch((e) => !cancelled && setError(String(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [view, source, retry]);

  const sourceName = source?.name ?? "…";
  const emptyMessage =
    view.kind === "trending" && source && !source.supportsTrending
      ? `Busca algo en ${sourceName} para empezar.`
      : "Ni un mango. Prueba otra búsqueda.";

  return (
    <>
      <h1>{view.kind === "trending" ? "Tendencias" : `Resultados para “${view.query}”`}</h1>
      <p className="subtitle">Música gratis vía {sourceName}. Sin pagar ni un mango.</p>

      {loading && <p className="status">Pelando mangos…</p>}
      {error && <p className="status error">Algo ha fallado: {error}</p>}
      {error && source?.kind === "plugin" && /yt-dlp|requisito/i.test(error) && (
        <button className="btn primary" onClick={fixRequirements} disabled={fixing !== null}>
          {fixing ?? "Instalar lo que falta"}
        </button>
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
}

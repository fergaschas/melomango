import { useCallback, useEffect, useState, type FormEvent } from "react";
import { DEFAULT_SOURCE, listSources, searchTracks, trendingTracks, type SourceInfo, type Track } from "./lib/api";
import { useLibrary } from "./lib/library";
import { usePlayer, type Player } from "./hooks/usePlayer";
import { TrackList } from "./components/TrackList";
import { PlayerBar } from "./components/PlayerBar";
import { PluginsDialog } from "./components/PluginsDialog";
import { Sidebar } from "./components/Sidebar";
import { LikesView, PlaylistView } from "./components/LibraryViews";
import { LyricsPanel } from "./components/LyricsPanel";
import { PuzzleIcon, SearchIcon } from "./components/Icons";
import "./App.css";

export type View =
  | { kind: "trending" }
  | { kind: "search"; query: string }
  | { kind: "likes" }
  | { kind: "playlist"; id: number };

const SOURCE_KEY = "melomango.source";
const LYRICS_KEY = "melomango.lyricsOpen";

function savedLyricsOpen() {
  try {
    return localStorage.getItem(LYRICS_KEY) === "1";
  } catch {
    return false;
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
  const { toast } = useLibrary();
  const [sources, setSources] = useState<SourceInfo[]>([]);
  const [sourceId, setSourceId] = useState(savedSource);
  const [showPlugins, setShowPlugins] = useState(false);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>({ kind: "trending" });
  const [lyricsOpen, setLyricsOpen] = useState(savedLyricsOpen);

  const source = sources.find((s) => s.id === sourceId) ?? null;

  const refreshSources = useCallback((selectId?: string) => {
    listSources()
      .then((list) => {
        setSources(list);
        setSourceId((current) => {
          const wanted = selectId ?? current;
          return list.some((s) => s.id === wanted) ? wanted : DEFAULT_SOURCE;
        });
      })
      .catch(() => {});
  }, []);

  useEffect(() => refreshSources(), [refreshSources]);

  useEffect(() => {
    try {
      localStorage.setItem(LYRICS_KEY, lyricsOpen ? "1" : "0");
    } catch {
      /* storage unavailable: not important */
    }
  }, [lyricsOpen]);

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
    <div className={`app${lyricsOpen ? " with-lyrics" : ""}`}>
      <header className="topbar">
        <button className="logo" onClick={() => setView({ kind: "trending" })}>
          <span className="logo-mark">🥭</span> Melomango
        </button>
        <form className="search" onSubmit={onSearch}>
          <SearchIcon size={18} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Busca en ${source?.name ?? "…"}…`}
            aria-label="Buscar"
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

      <Sidebar view={view} onNavigate={setView} />

      <main className="content">
        {view.kind === "likes" && <LikesView player={player} />}
        {view.kind === "playlist" && (
          <PlaylistView id={view.id} player={player} onDeleted={() => setView({ kind: "likes" })} />
        )}
        {(view.kind === "trending" || view.kind === "search") && (
          <BrowseView view={view} source={source} player={player} />
        )}
      </main>

      {lyricsOpen && <LyricsPanel player={player} onClose={() => setLyricsOpen(false)} />}

      <PlayerBar player={player} lyricsOpen={lyricsOpen} onToggleLyrics={() => setLyricsOpen((o) => !o)} />

      {toast && (
        <div className="toast" role="status">
          {toast}
        </div>
      )}

      {showPlugins && (
        <PluginsDialog sources={sources} onChanged={refreshSources} onClose={() => setShowPlugins(false)} />
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
  }, [view, source]);

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
      {!loading && !error && tracks.length === 0 && <p className="status">{emptyMessage}</p>}
      {!loading && !error && tracks.length > 0 && (
        <TrackList
          tracks={tracks}
          current={player.current}
          isPlaying={player.isPlaying}
          onPlay={(i) => player.playQueue(tracks, i)}
        />
      )}
    </>
  );
}

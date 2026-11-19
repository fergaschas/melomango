import { useEffect, useState, type FormEvent } from "react";
import { DEFAULT_SOURCE, searchTracks, trendingTracks, type Track } from "./lib/api";
import { usePlayer } from "./hooks/usePlayer";
import { TrackList } from "./components/TrackList";
import { PlayerBar } from "./components/PlayerBar";
import { SearchIcon } from "./components/Icons";
import "./App.css";

type View = { kind: "trending" } | { kind: "search"; query: string };

export default function App() {
  const player = usePlayer();
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>({ kind: "trending" });
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    const request =
      view.kind === "trending" ? trendingTracks(DEFAULT_SOURCE) : searchTracks(DEFAULT_SOURCE, view.query);
    request
      .then((result) => !cancelled && setTracks(result))
      .catch((e) => !cancelled && setError(String(e)))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [view]);

  function onSearch(e: FormEvent) {
    e.preventDefault();
    const q = query.trim();
    setView(q ? { kind: "search", query: q } : { kind: "trending" });
  }

  return (
    <div className="app">
      <header className="topbar">
        <button className="logo" onClick={() => setView({ kind: "trending" })}>
          <span className="logo-mark">🥭</span> Melomango
        </button>
        <form className="search" onSubmit={onSearch}>
          <SearchIcon size={18} />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Busca canciones o artistas…"
            aria-label="Buscar"
          />
        </form>
      </header>

      <main className="content">
        <h1>{view.kind === "trending" ? "Tendencias" : `Resultados para “${view.query}”`}</h1>
        <p className="subtitle">Música gratis vía Audius. Sin pagar ni un mango.</p>

        {loading && <p className="status">Pelando mangos…</p>}
        {error && <p className="status error">Algo ha fallado: {error}</p>}
        {!loading && !error && tracks.length === 0 && <p className="status">Ni un mango. Prueba otra búsqueda.</p>}
        {!loading && !error && tracks.length > 0 && (
          <TrackList
            tracks={tracks}
            current={player.current}
            isPlaying={player.isPlaying}
            onPlay={(i) => player.playQueue(tracks, i)}
          />
        )}
      </main>

      <PlayerBar player={player} />
    </div>
  );
}

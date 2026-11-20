import { useEffect, useMemo, useState, type ReactNode } from "react";
import { catalogSearch, type CatalogSearch, type Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import { useNav } from "../lib/nav";
import type { Player } from "../hooks/usePlayer";
import { CardGrid, KIND_LABEL, type Card } from "./CatalogViews";
import { TrackList } from "./TrackList";
import { ErrorNotice } from "./ErrorNotice";

export type SearchFilter = "all" | "tracks" | "artists" | "albums" | "playlists";

const TABS: [SearchFilter, string][] = [
  ["all", "Todo"],
  ["tracks", "Canciones"],
  ["artists", "Artistas"],
  ["albums", "Álbumes"],
  ["playlists", "Playlists"],
];
/** How many of each kind "Todo" shows before "Ver todos". */
const PREVIEW = 6;
const PREVIEW_TRACKS = 4;

// Searches answered this session: switching tabs or going back doesn't ask Deezer again.
const cache = new Map<string, Promise<CatalogSearch>>();
function searchCatalog(query: string) {
  const key = query.trim().toLowerCase();
  let found = cache.get(key);
  if (!found) {
    found = catalogSearch(query);
    found.catch(() => cache.delete(key));
    if (cache.size > 50) cache.clear();
    cache.set(key, found);
  }
  return found;
}

type Catalog = { kind: "loading" } | { kind: "error"; message: string } | { kind: "ready"; data: CatalogSearch };

interface Props {
  query: string;
  filter: SearchFilter;
  onFilter: (filter: SearchFilter) => void;
  player: Player;
  /** Songs from the selected source (searched by the caller). */
  tracks: Track[];
  /** The full song list with its loading and error states. */
  tracksBlock: ReactNode;
  tracksLoading: boolean;
  /** The song search failed (its notice, with "Reintentar", is in `tracksBlock`). */
  tracksFailed: boolean;
  sourceName: string;
}

const fold = (s: string) =>
  s
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

/** Search results by kind: songs from the selected source; artists, albums and public playlists from Deezer. */
export function SearchResults({
  query,
  filter,
  onFilter,
  player,
  tracks,
  tracksBlock,
  tracksLoading,
  tracksFailed,
  sourceName,
}: Props) {
  const nav = useNav();
  const library = useLibrary();
  const [catalog, setCatalog] = useState<Catalog>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setCatalog({ kind: "loading" });
    if (attempt > 0) cache.delete(query.trim().toLowerCase());
    searchCatalog(query)
      .then((data) => !cancelled && setCatalog({ kind: "ready", data }))
      .catch((e) => !cancelled && setCatalog({ kind: "error", message: String(e) }));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, attempt]);

  const data = catalog.kind === "ready" ? catalog.data : null;

  const artists: Card[] = (data?.artists ?? []).map((a) => ({
    key: a.id,
    title: a.name,
    subtitle: "Artista",
    imageUrl: a.pictureUrl,
    round: true,
    onOpen: () => nav.openArtist(a.name, a.id),
  }));
  const albums: Card[] = (data?.albums ?? []).map((a) => ({
    key: a.id,
    title: a.title,
    subtitle: [a.artistName, KIND_LABEL[a.kind] ?? a.kind].filter(Boolean).join(" · "),
    imageUrl: a.coverUrl,
    onOpen: () => nav.openAlbum(a.id),
  }));
  const publicPlaylists: Card[] = (data?.playlists ?? []).map((p) => ({
    key: p.id,
    title: p.title,
    subtitle: [p.owner && `de ${p.owner}`, `${p.trackCount} canciones`].filter(Boolean).join(" · "),
    imageUrl: p.pictureUrl,
    onOpen: () => nav.openPublicPlaylist(p.id),
  }));
  // The user's own playlists whose name contains the search.
  const mine: Card[] = useMemo(() => {
    const q = fold(query.trim());
    return library.playlists
      .filter((p) => q && fold(p.name).includes(q))
      .map((p) => ({
        key: `mine-${p.id}`,
        title: p.name,
        subtitle: `Tu playlist · ${p.trackCount} canciones`,
        imageUrl: p.artworkUrls[0] ?? null,
        onOpen: () => nav.openLibraryPlaylist(p.id),
      }));
  }, [library.playlists, query, nav]);
  const playlists = [...mine, ...publicPlaylists];

  const catalogStatus =
    catalog.kind === "loading" ? (
      <p className="status">Buscando en el catálogo…</p>
    ) : catalog.kind === "error" ? (
      <ErrorNotice error={catalog.message} onRetry={() => setAttempt((a) => a + 1)} />
    ) : null;
  const empty = (what: string) => <p className="status">No hay {what} para «{query}».</p>;
  const best = data?.artists[0];

  return (
    <>
      <div className="search-tabs" role="tablist" aria-label="Qué buscar">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            role="tab"
            aria-selected={filter === id}
            className={`search-tab${filter === id ? " active" : ""}`}
            onClick={() => onFilter(id)}
          >
            {label}
          </button>
        ))}
      </div>

      {filter === "tracks" && tracksBlock}

      {filter === "artists" && (catalogStatus ?? (artists.length ? <CardGrid cards={artists} /> : empty("artistas")))}

      {filter === "albums" && (catalogStatus ?? (albums.length ? <CardGrid cards={albums} /> : empty("álbumes")))}

      {filter === "playlists" && (
        <>
          {mine.length > 0 && (
            <section className="catalog-section first">
              <h2>Tus playlists</h2>
              <CardGrid cards={mine} />
            </section>
          )}
          <section className={`catalog-section${mine.length ? "" : " first"}`}>
            {mine.length > 0 && <h2>En Deezer</h2>}
            {catalogStatus ?? (publicPlaylists.length ? <CardGrid cards={publicPlaylists} /> : empty("playlists"))}
          </section>
        </>
      )}

      {filter === "all" && (
        <>
          <div className={`search-top${best ? "" : " single"}`}>
            {best && (
              <section className="search-best">
                <h2>Mejor resultado</h2>
                <button className="best-card" onClick={() => nav.openArtist(best.name, best.id)}>
                  {best.pictureUrl ? (
                    <img src={best.pictureUrl} alt="" />
                  ) : (
                    <span className="artist-photo placeholder" aria-hidden="true" />
                  )}
                  <span className="best-name">{best.name}</span>
                  <span className="chip-kind">Artista</span>
                </button>
              </section>
            )}
            <section className="search-songs">
              <div className="catalog-section-head">
                <h2>Canciones</h2>
                {tracks.length > PREVIEW_TRACKS && (
                  <button className="link-btn" onClick={() => onFilter("tracks")}>
                    Ver todas
                  </button>
                )}
              </div>
              {tracksLoading ? (
                <p className="status">Buscando en {sourceName}…</p>
              ) : tracksFailed ? (
                tracksBlock
              ) : tracks.length ? (
                <TrackList
                  tracks={tracks.slice(0, PREVIEW_TRACKS)}
                  current={player.current}
                  isPlaying={player.isPlaying}
                  queue={player}
                  onPlay={(i) => player.playQueue(tracks, i)}
                />
              ) : (
                <p className="status">No hay canciones en {sourceName}.</p>
              )}
            </section>
          </div>
          {catalogStatus}
          <PreviewRow title="Artistas" cards={artists} onMore={() => onFilter("artists")} />
          <PreviewRow title="Álbumes" cards={albums} onMore={() => onFilter("albums")} />
          <PreviewRow title="Playlists" cards={playlists} onMore={() => onFilter("playlists")} />
        </>
      )}
    </>
  );
}

function PreviewRow({ title, cards, onMore }: { title: string; cards: Card[]; onMore: () => void }) {
  if (!cards.length) return null;
  return (
    <section className="catalog-section">
      <div className="catalog-section-head">
        <h2>{title}</h2>
        {cards.length > PREVIEW && (
          <button className="link-btn" onClick={onMore}>
            Ver todos
          </button>
        )}
      </div>
      <CardGrid cards={cards.slice(0, PREVIEW)} />
    </section>
  );
}

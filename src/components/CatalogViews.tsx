import { useEffect, useState } from "react";
import { albumPage, artistPage, formatTotal, type AlbumPage, type AlbumSummary, type ArtistPage } from "../lib/api";
import { useNav } from "../lib/nav";
import type { Player } from "../hooks/usePlayer";
import { PlayIcon, RadioIcon } from "./Icons";
import { TrackList } from "./TrackList";

const KIND_LABEL: Record<string, string> = { album: "Álbum", single: "Sencillo", ep: "EP", compile: "Recopilatorio" };
const ALBUMS_SHOWN = 12;

type Load<T> = { kind: "loading" } | { kind: "error"; message: string } | { kind: "ready"; data: T };

/** Loads `fetch()` when `key` changes, ignoring answers that arrive after leaving. */
function useLoad<T>(key: string, fetch: () => Promise<T>): [Load<T>, () => void] {
  const [state, setState] = useState<Load<T>>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setState({ kind: "loading" });
    fetch()
      .then((data) => !cancelled && setState({ kind: "ready", data }))
      .catch((e) => !cancelled && setState({ kind: "error", message: String(e) }));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, attempt]);
  return [state, () => setAttempt((a) => a + 1)];
}

function LoadError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="catalog-error">
      <p className="status error">No se pudo cargar: {message}</p>
      <button className="btn ghost" onClick={onRetry}>
        Reintentar
      </button>
    </div>
  );
}

const year = (date: string | null) => (date ? date.slice(0, 4) : "");

export function ArtistView({ name, id, player }: { name: string; id?: string; player: Player }) {
  const nav = useNav();
  const [state, retry] = useLoad<ArtistPage>(`${id ?? ""}|${name}`, () => artistPage(name, id));

  if (state.kind === "loading") {
    return (
      <header className="collection-header">
        <div className="artist-photo placeholder" aria-hidden="true" />
        <div className="collection-info">
          <span className="eyebrow">Artista</span>
          <h1>{name}</h1>
          <p className="subtitle">Cargando…</p>
        </div>
      </header>
    );
  }
  if (state.kind === "error") return <LoadError message={state.message} onRetry={retry} />;

  const { artist, fans, top, albums, related } = state.data;
  const fullAlbums = albums.filter((a) => a.kind === "album" || a.kind === "compile");
  const singles = albums.filter((a) => a.kind === "single" || a.kind === "ep");

  return (
    <>
      <header className="collection-header">
        {artist.pictureUrl ? (
          <img className="artist-photo" src={artist.pictureUrl} alt="" />
        ) : (
          <div className="artist-photo placeholder" aria-hidden="true" />
        )}
        <div className="collection-info">
          <span className="eyebrow">Artista</span>
          <h1>{artist.name}</h1>
          <p className="subtitle">
            {new Intl.NumberFormat("es-ES").format(fans)} fans en Deezer · {albums.length} lanzamientos
          </p>
          <div className="collection-actions">
            <button className="btn primary play-all" onClick={() => player.playQueue(top, 0)} disabled={!top.length}>
              <PlayIcon size={16} /> Reproducir
            </button>
            <button
              className="btn ghost"
              onClick={() => top[0] && player.startRadio(top[0])}
              disabled={!top.length}
              title="Sus canciones y otras parecidas"
            >
              <RadioIcon size={16} /> Radio
            </button>
          </div>
        </div>
      </header>

      {top.length > 0 && (
        <section className="catalog-section">
          <h2>Populares</h2>
          <TrackList
            tracks={top}
            current={player.current}
            isPlaying={player.isPlaying}
            queue={player}
            onPlay={(i) => player.playQueue(top, i)}
          />
        </section>
      )}

      <AlbumGrid title="Álbumes" albums={fullAlbums} onOpen={nav.openAlbum} />
      <AlbumGrid title="Sencillos y EP" albums={singles} onOpen={nav.openAlbum} />

      {related.length > 0 && (
        <section className="catalog-section">
          <h2>Artistas parecidos</h2>
          <ul className="related-artists">
            {related.map((a) => (
              <li key={a.id}>
                <button className="related-artist" onClick={() => nav.openArtist(a.name, a.id)}>
                  {a.pictureUrl ? <img src={a.pictureUrl} alt="" /> : <span className="artist-photo placeholder" />}
                  <span>{a.name}</span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <p className="credit-line">Datos de artistas y álbumes de Deezer. La música suena desde YouTube.</p>
    </>
  );
}

function AlbumGrid({ title, albums, onOpen }: { title: string; albums: AlbumSummary[]; onOpen: (id: string) => void }) {
  const [all, setAll] = useState(false);
  if (albums.length === 0) return null;
  const shown = all ? albums : albums.slice(0, ALBUMS_SHOWN);
  return (
    <section className="catalog-section">
      <div className="catalog-section-head">
        <h2>{title}</h2>
        {albums.length > ALBUMS_SHOWN && (
          <button className="link-btn" onClick={() => setAll((a) => !a)}>
            {all ? "Ver menos" : `Ver todos (${albums.length})`}
          </button>
        )}
      </div>
      <ul className="album-grid">
        {shown.map((a) => (
          <li key={a.id}>
            <button className="album-card" onClick={() => onOpen(a.id)}>
              {a.coverUrl ? <img src={a.coverUrl} alt="" loading="lazy" /> : <span className="album-cover placeholder" />}
              <span className="album-title">{a.title}</span>
              <span className="album-meta">
                {[year(a.releaseDate), KIND_LABEL[a.kind] ?? a.kind].filter(Boolean).join(" · ")}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function AlbumView({ id, player }: { id: string; player: Player }) {
  const nav = useNav();
  const [state, retry] = useLoad<AlbumPage>(id, () => albumPage(id));

  if (state.kind === "loading") return <p className="status">Cargando el álbum…</p>;
  if (state.kind === "error") return <LoadError message={state.message} onRetry={retry} />;

  const album = state.data;
  return (
    <>
      <header className="collection-header">
        {album.coverUrl ? (
          <img className="album-hero" src={album.coverUrl} alt="" />
        ) : (
          <div className="album-hero placeholder" aria-hidden="true" />
        )}
        <div className="collection-info">
          <span className="eyebrow">{KIND_LABEL[album.kind] ?? "Álbum"}</span>
          <h1>{album.title}</h1>
          <p className="subtitle">
            <button className="artist-link strong" onClick={() => nav.openArtist(album.artist.name, album.artist.id)}>
              {album.artist.name}
            </button>
            {[year(album.releaseDate), `${album.tracks.length} canciones`, formatTotal(album.durationSecs)]
              .filter(Boolean)
              .map((part) => ` · ${part}`)
              .join("")}
          </p>
          <div className="collection-actions">
            <button
              className="btn primary play-all"
              onClick={() => player.playQueue(album.tracks, 0)}
              disabled={!album.tracks.length}
            >
              <PlayIcon size={16} /> Reproducir
            </button>
          </div>
        </div>
      </header>
      <TrackList
        tracks={album.tracks}
        current={player.current}
        isPlaying={player.isPlaying}
        queue={player}
        onPlay={(i) => player.playQueue(album.tracks, i)}
      />
      <p className="credit-line">Datos del álbum de Deezer. La música suena desde YouTube.</p>
    </>
  );
}

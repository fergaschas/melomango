import { useEffect, useState } from "react";
import {
  albumPage,
  artistPage,
  deezerPlaylist,
  formatTotal,
  saveDeezerPlaylist,
  type AlbumPage,
  type AlbumSummary,
  type ArtistPage,
  type PublicPlaylistPage,
} from "../lib/api";
import { useLibrary } from "../lib/library";
import { useNav } from "../lib/nav";
import type { Player } from "../hooks/usePlayer";
import { PlayIcon, PlusIcon, RadioIcon } from "./Icons";
import { TrackList } from "./TrackList";
import { ErrorNotice } from "./ErrorNotice";

export const KIND_LABEL: Record<string, string> = { album: "Álbum", single: "Sencillo", ep: "EP", compile: "Recopilatorio" };

/** One tile in a grid of artists, albums or playlists. */
export interface Card {
  key: string;
  title: string;
  subtitle: string;
  imageUrl: string | null;
  /** Artists get round pictures. */
  round?: boolean;
  onOpen: () => void;
}

/** Tiles in a grid, or in one scrollable `row` (the home page). */
export function CardGrid({ cards, row }: { cards: Card[]; row?: boolean }) {
  return (
    <ul className={`album-grid${row ? " row" : ""}`}>
      {cards.map((c) => (
        <li key={c.key}>
          <button className={`album-card${c.round ? " round" : ""}`} onClick={c.onOpen}>
            {c.imageUrl ? <img src={c.imageUrl} alt="" loading="lazy" /> : <span className="album-cover placeholder" />}
            <span className="album-title">{c.title}</span>
            <span className="album-meta">{c.subtitle}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
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
  return <ErrorNotice error={message} onRetry={onRetry} />;
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
      <CardGrid
        cards={shown.map((a) => ({
          key: a.id,
          title: a.title,
          subtitle: [year(a.releaseDate), KIND_LABEL[a.kind] ?? a.kind].filter(Boolean).join(" · "),
          imageUrl: a.coverUrl,
          onOpen: () => onOpen(a.id),
        }))}
      />
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

/** A public Deezer playlist: play it, or save it among the user's playlists. */
export function PublicPlaylistView({ id, player }: { id: string; player: Player }) {
  const nav = useNav();
  const library = useLibrary();
  const [state, retry] = useLoad<PublicPlaylistPage>(id, () => deezerPlaylist(id));
  const [saving, setSaving] = useState(false);
  const [savedId, setSavedId] = useState<number | null>(null);

  if (state.kind === "loading") return <p className="status">Cargando la playlist…</p>;
  if (state.kind === "error") return <LoadError message={state.message} onRetry={retry} />;

  const playlist = state.data;

  async function save() {
    setSaving(true);
    try {
      const saved = await saveDeezerPlaylist(id);
      await library.refresh();
      library.notify(`«${saved.name}» guardada en tus playlists`);
      setSavedId(saved.id);
    } catch (e) {
      library.notify(`No se pudo guardar: ${e}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <header className="collection-header">
        {playlist.pictureUrl ? (
          <img className="album-hero" src={playlist.pictureUrl} alt="" />
        ) : (
          <div className="album-hero placeholder" aria-hidden="true" />
        )}
        <div className="collection-info">
          <span className="eyebrow">Playlist de Deezer</span>
          <h1>{playlist.title}</h1>
          <p className="subtitle">
            {[playlist.owner && `de ${playlist.owner}`, `${playlist.tracks.length} canciones`, formatTotal(playlist.durationSecs)]
              .filter(Boolean)
              .join(" · ")}
          </p>
          {playlist.description && <p className="playlist-description">{playlist.description}</p>}
          <div className="collection-actions">
            <button
              className="btn primary play-all"
              onClick={() => player.playQueue(playlist.tracks, 0)}
              disabled={!playlist.tracks.length}
            >
              <PlayIcon size={16} /> Reproducir
            </button>
            {savedId === null ? (
              <button className="btn ghost" onClick={save} disabled={saving || !playlist.tracks.length}>
                <PlusIcon size={16} /> {saving ? "Guardando…" : "Guardar en mis playlists"}
              </button>
            ) : (
              <button className="btn ghost" onClick={() => nav.openLibraryPlaylist(savedId)}>
                Abrir en mis playlists
              </button>
            )}
          </div>
        </div>
      </header>
      <TrackList
        tracks={playlist.tracks}
        current={player.current}
        isPlaying={player.isPlaying}
        queue={player}
        onPlay={(i) => player.playQueue(playlist.tracks, i)}
      />
      <p className="credit-line">Playlist pública de Deezer. La música suena desde YouTube.</p>
    </>
  );
}

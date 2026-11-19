import { useEffect, useState, type FormEvent } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { formatTotal, getPlaylist, likedTracks, SPOTIFY_EXPORT, syncPlaylist, type Playlist, type Track } from "../lib/api";
import { useLibrary } from "../lib/library";
import type { Player } from "../hooks/usePlayer";
import { PlayIcon, SyncIcon, TrashIcon } from "./Icons";
import { PlaylistCover } from "./PlaylistCover";
import { TrackList } from "./TrackList";

function PlayAllButton({ tracks, player }: { tracks: Track[]; player: Player }) {
  return (
    <button className="btn primary play-all" onClick={() => player.playQueue(tracks, 0)} disabled={tracks.length === 0}>
      <PlayIcon size={16} /> Reproducir
    </button>
  );
}

export function LikesView({ player }: { player: Player }) {
  const { version } = useLibrary();
  const [tracks, setTracks] = useState<Track[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    likedTracks().then(setTracks).catch((e) => setError(String(e)));
  }, [version]);

  const total = tracks?.reduce((sum, t) => sum + t.durationSecs, 0) ?? 0;

  return (
    <>
      <header className="collection-header">
        <PlaylistCover kind="likes" size={148} />
        <div className="collection-info">
          <span className="eyebrow">Colección</span>
          <h1>Me gusta</h1>
          <p className="subtitle">
            {tracks ? `${tracks.length} canciones · ${formatTotal(total)}` : "…"}
          </p>
          <div className="collection-actions">{tracks && <PlayAllButton tracks={tracks} player={player} />}</div>
        </div>
      </header>
      {error && <p className="status error">{error}</p>}
      {tracks?.length === 0 && (
        <p className="status">Aún no te gusta nada. Pulsa el ♡ de cualquier canción para guardarla aquí.</p>
      )}
      {tracks && tracks.length > 0 && (
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

export function PlaylistView({ id, player, onDeleted }: { id: number; player: Player; onDeleted: () => void }) {
  const library = useLibrary();
  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState("");
  const [syncing, setSyncing] = useState(false);

  useEffect(() => {
    getPlaylist(id)
      .then((p) => {
        setPlaylist(p);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  }, [id, library.version]);

  useEffect(() => setEditing(false), [id]);

  if (error) return <p className="status error">{error}</p>;
  if (!playlist) return <p className="status">Pelando mangos…</p>;

  async function onRename(e: FormEvent) {
    e.preventDefault();
    setEditing(false);
    if (name.trim() && name.trim() !== playlist!.name) await library.renamePlaylist(id, name);
  }

  async function onDelete() {
    const yes = await ask(`¿Eliminar la playlist «${playlist!.name}»? Las canciones no se borran de Me gusta.`, {
      title: "Melomango",
      kind: "warning",
    });
    if (!yes) return;
    await library.deletePlaylist(id);
    onDeleted();
  }

  async function onSync() {
    setSyncing(true);
    try {
      const updated = await syncPlaylist(id);
      await library.refresh();
      library.notify(`Sincronizada: ${updated.trackCount} canciones`);
    } catch (e) {
      library.notify(String(e));
    } finally {
      setSyncing(false);
    }
  }

  function onMove(from: number, to: number) {
    // Optimistic reorder, then persist.
    setPlaylist((p) => {
      if (!p) return p;
      const tracks = [...p.tracks];
      const [item] = tracks.splice(from, 1);
      tracks.splice(to, 0, item);
      return { ...p, tracks };
    });
    library.moveInPlaylist(id, from, to);
  }

  return (
    <>
      <header className="collection-header">
        <PlaylistCover artworkUrls={playlist.artworkUrls} size={148} />
        <div className="collection-info">
          <span className="eyebrow">
            Playlist
            {playlist.link && (
              <span
                className="linked"
                title={`${playlist.link.pluginId === SPOTIFY_EXPORT ? "Importada" : "Última sincronización"}: ${new Date(playlist.link.syncedAt).toLocaleString()}`}
              >
                · de {linkLabel(playlist.link.pluginId)}
              </span>
            )}
          </span>
          {editing ? (
            <form onSubmit={onRename}>
              <input
                className="title-input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={onRename}
                onKeyDown={(e) => e.key === "Escape" && setEditing(false)}
                aria-label="Nombre de la playlist"
                autoFocus
              />
            </form>
          ) : (
            <h1>
              <button
                className="editable"
                title="Haz clic para renombrar"
                onClick={() => {
                  setName(playlist.name);
                  setEditing(true);
                }}
              >
                {playlist.name}
              </button>
            </h1>
          )}
          <p className="subtitle">
            {playlist.trackCount} canciones · {formatTotal(playlist.durationSecs)}
          </p>
          <div className="collection-actions">
            <PlayAllButton tracks={playlist.tracks} player={player} />
            {playlist.link && playlist.link.pluginId !== SPOTIFY_EXPORT && (
              <button className="btn ghost" onClick={onSync} disabled={syncing}>
                <SyncIcon size={16} /> {syncing ? "Sincronizando…" : "Sincronizar"}
              </button>
            )}
            <button className="btn ghost" onClick={onDelete}>
              <TrashIcon size={16} /> Eliminar
            </button>
          </div>
        </div>
      </header>
      {playlist.tracks.length === 0 ? (
        <p className="status">
          Esta playlist está vacía. Usa el botón + de cualquier canción para añadirla aquí.
        </p>
      ) : (
        <>
          <TrackList
            tracks={playlist.tracks}
            current={player.current}
            isPlaying={player.isPlaying}
            onPlay={(i) => player.playQueue(playlist.tracks, i)}
            onRemove={(t) => library.removeFromPlaylist(id, t)}
            onMove={onMove}
          />
          <p className="hint">Arrastra las canciones para cambiar el orden.</p>
        </>
      )}
    </>
  );
}

function linkLabel(pluginId: string) {
  if (pluginId === SPOTIFY_EXPORT) return "tu exportación de Spotify";
  return pluginId === "spotify" ? "Spotify" : pluginId;
}

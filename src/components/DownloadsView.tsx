import { useEffect, useMemo, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { listDownloads, type Download, type Track } from "../lib/api";
import { useDownloads } from "../lib/downloads";
import { collectionState, formatSize, songs } from "../lib/downloadState";
import type { Player } from "../hooks/usePlayer";
import { DownloadDoneIcon, DownloadIcon, PlayIcon, TrashIcon } from "./Icons";
import { TrackList } from "./TrackList";
import { TrackTools, useArrangement } from "./TrackTools";
import type { SortKey } from "../lib/sortTracks";

const SORTS: [SortKey, string][] = [
  ["custom", "Descargadas recientemente"],
  ["title", "Título"],
  ["artist", "Artista"],
  ["duration", "Duración"],
];
const NONE: Track[] = [];

/** Songs saved to listen without a connection. */
export function DownloadsView({ player }: { player: Player }) {
  const downloads = useDownloads();
  const [items, setItems] = useState<Download[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listDownloads()
      .then(setItems)
      .catch((e) => setError(String(e)));
  }, [downloads.version]);

  const tracks = useMemo(() => items?.map((d) => d.track) ?? NONE, [items]);
  const size = items?.reduce((sum, d) => sum + d.size, 0) ?? 0;
  const arranged = useArrangement("downloads", tracks);
  const pending = downloads.active;

  async function onRemoveAll() {
    const ok = await ask(`Se borrarán las ${tracks.length} canciones descargadas (${formatSize(size)}) de tu equipo.`, {
      title: "Borrar descargas",
      kind: "warning",
      okLabel: "Borrar",
      cancelLabel: "Cancelar",
    });
    if (ok) await downloads.remove(tracks);
  }

  return (
    <>
      <header className="collection-header">
        <div className="history-cover downloads-cover" aria-hidden="true">
          <DownloadDoneIcon size={64} />
        </div>
        <div className="collection-info">
          <span className="eyebrow">Colección</span>
          <h1>Descargas</h1>
          <p className="subtitle">
            {items ? `${songs(items.length)} · ${formatSize(size)}` : "…"} · Suenan sin conexión
            {pending > 0 && ` · ${pending} descargándose`}
          </p>
          <div className="collection-actions">
            <button
              className="btn primary play-all"
              onClick={() => player.playQueue(arranged.visible, 0)}
              disabled={arranged.visible.length === 0}
            >
              <PlayIcon size={16} /> Reproducir
            </button>
            {tracks.length > 0 && (
              <button className="btn ghost" onClick={onRemoveAll}>
                <TrashIcon size={16} /> Borrar todo
              </button>
            )}
          </div>
        </div>
      </header>
      {error && <p className="status error">{error}</p>}
      {items?.length === 0 && (
        <p className="status">
          Aún no has descargado nada. Usa el botón Descargar de una playlist o de Me gusta, o el + de cualquier canción.
        </p>
      )}
      {tracks.length > 0 && (
        <>
          <TrackTools
            filter={arranged.filter}
            onFilter={arranged.setFilter}
            sort={arranged.sort}
            onSort={arranged.setSort}
            options={SORTS}
            shown={arranged.visible.length}
            total={tracks.length}
          />
          {arranged.visible.length === 0 ? (
            <p className="status">Ninguna canción coincide con «{arranged.filter}».</p>
          ) : (
            <TrackList
              tracks={arranged.visible}
              current={player.current}
              isPlaying={player.isPlaying}
              queue={player}
              onPlay={(i) => player.playQueue(arranged.visible, i)}
              onRemove={(t) => downloads.remove([t])}
              removeLabel="Borrar la descarga"
            />
          )}
        </>
      )}
    </>
  );
}

/** "Descargar" for a playlist or Me gusta: downloads what is missing, shows how it goes. */
export function DownloadCollectionButton({ tracks }: { tracks: Track[] }) {
  const downloads = useDownloads();
  const state = collectionState(tracks, (t) => `${t.source}/${t.id}`, downloads.downloaded, downloads.progress);
  if (state.total === 0) return null;

  if (state.pending > 0) {
    return (
      <button className="btn ghost" onClick={() => downloads.remove(tracks.filter((t) => downloads.progressOf(t)))}>
        <DownloadIcon size={16} /> Descargando {state.downloaded + 1} de {state.total}… · Cancelar
      </button>
    );
  }
  if (state.missing.length === 0) {
    return (
      <button
        className="btn ghost downloaded"
        title="Todas sus canciones suenan sin conexión"
        onClick={async () => {
          const ok = await ask(`Se borrarán de tu equipo las ${state.total} canciones descargadas de esta lista.`, {
            title: "Quitar descargas",
            kind: "warning",
            okLabel: "Quitar",
            cancelLabel: "Cancelar",
          });
          if (ok) await downloads.remove(tracks);
        }}
      >
        <DownloadDoneIcon size={16} /> Descargada
      </button>
    );
  }
  return (
    <button
      className="btn ghost"
      onClick={() => downloads.download(state.missing)}
      title="Guardar sus canciones para escucharlas sin conexión"
    >
      <DownloadIcon size={16} /> Descargar
      {state.downloaded > 0 && ` (faltan ${state.missing.length})`}
    </button>
  );
}

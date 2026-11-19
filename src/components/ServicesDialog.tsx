import { useCallback, useEffect, useMemo, useState } from "react";
import {
  connectLibrary,
  disconnectLibrary,
  importCollection,
  libraryCollections,
  libraryConnections,
  type CollectionView,
  type LibraryConnection,
} from "../lib/api";
import { useLibrary } from "../lib/library";
import { CloseIcon, SearchIcon } from "./Icons";
import { PlaylistCover } from "./PlaylistCover";

interface Props {
  onClose: () => void;
  onOpenPlugins: () => void;
  onOpenPlaylist: (id: number) => void;
}

/** Connect services such as Spotify and import their playlists. */
export function ServicesDialog({ onClose, onOpenPlugins, onOpenPlaylist }: Props) {
  const [connections, setConnections] = useState<LibraryConnection[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    libraryConnections()
      .then(setConnections)
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(load, [load]);

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="dialog wide" role="dialog" aria-modal="true" aria-labelledby="services-title">
        <header className="dialog-header">
          <h2 id="services-title">Importar playlists</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>
        <p className="dialog-intro">
          Trae tus playlists de otros servicios. Solo se copian los datos de las canciones: al reproducirlas,
          Melomango busca cada una en YouTube.
        </p>

        {error && <p className="notice error">{error}</p>}
        {!connections && !error && <p className="status">Comprobando conexiones…</p>}
        {connections?.length === 0 && (
          <div className="empty-services">
            <p>No tienes ningún plugin de servicios. Instala el de <strong>Spotify</strong> desde Plugins → Explorar.</p>
            <button className="btn primary" onClick={onOpenPlugins}>
              Buscar plugins
            </button>
          </div>
        )}
        {connections?.map((c) => (
          <ServicePanel key={c.pluginId} connection={c} onChanged={load} onOpenPlaylist={onOpenPlaylist} />
        ))}
      </section>
    </div>
  );
}

function ServicePanel({
  connection,
  onChanged,
  onOpenPlaylist,
}: {
  connection: LibraryConnection;
  onChanged: () => void;
  onOpenPlaylist: (id: number) => void;
}) {
  const library = useLibrary();
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(connection.error);
  const [collections, setCollections] = useState<CollectionView[] | null>(null);
  const [filter, setFilter] = useState("");
  const [importing, setImporting] = useState<string | null>(null);

  const loadCollections = useCallback(() => {
    setCollections(null);
    libraryCollections(connection.pluginId)
      .then(setCollections)
      .catch((e) => setError(String(e)));
  }, [connection.pluginId]);

  useEffect(() => {
    if (connection.connected) loadCollections();
  }, [connection.connected, loadCollections]);

  async function connect() {
    setConnecting(true);
    setError(null);
    try {
      await connectLibrary(connection.pluginId);
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setConnecting(false);
    }
  }

  async function disconnect() {
    try {
      await disconnectLibrary(connection.pluginId);
      setCollections(null);
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  }

  async function importOne(c: CollectionView) {
    setImporting(c.id);
    setError(null);
    try {
      const playlist = await importCollection(connection.pluginId, c.id, c.name);
      await library.refresh();
      library.notify(`«${playlist.name}» importada: ${playlist.trackCount} canciones`);
      setCollections((list) => list?.map((x) => (x.id === c.id ? { ...x, importedPlaylistId: playlist.id } : x)) ?? null);
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(null);
    }
  }

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (collections ?? []).filter((c) => !q || `${c.name} ${c.owner ?? ""}`.toLowerCase().includes(q));
  }, [collections, filter]);

  return (
    <div className="service">
      <div className="service-head">
        <div>
          <strong>{connection.name}</strong>
          <span className="service-status">
            {connection.connected ? `Conectado como ${connection.user ?? "tu cuenta"}` : "Sin conectar"}
          </span>
        </div>
        {connection.connected ? (
          <button className="btn ghost small" onClick={disconnect}>
            Desconectar
          </button>
        ) : (
          <button className="btn primary small" onClick={connect} disabled={connecting}>
            {connecting ? "Esperando al navegador…" : `Conectar ${connection.name}`}
          </button>
        )}
      </div>

      {connecting && (
        <p className="notice ok">
          Se ha abierto tu navegador: sigue los pasos allí (la primera vez te pedirá crear tu app de {connection.name}).
          Esta ventana se actualizará sola.
        </p>
      )}
      {error && <p className="notice error">{error}</p>}

      {connection.connected && (
        <>
          <label className="search dialog-search">
            <SearchIcon size={16} />
            <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filtrar playlists…" />
          </label>
          {!collections && !error && <p className="status">Cargando tu biblioteca…</p>}
          <ul className="source-list collection-list">
            {visible.map((c) => (
              <li key={c.id}>
                <PlaylistCover
                  kind={c.kind === "liked" ? "likes" : "playlist"}
                  artworkUrls={c.artworkUrl ? [c.artworkUrl] : []}
                  size={44}
                />
                <div className="source-text">
                  <span className="source-name">{c.name}</span>
                  <span className="source-desc">
                    {c.kind === "album" ? "Álbum" : c.kind === "liked" ? "Tus Me gusta" : "Playlist"}
                    {c.owner && ` · ${c.owner}`} · {c.trackCount} canciones
                  </span>
                </div>
                {!c.importable ? (
                  <span className="muted-note" title="Spotify solo deja leer las playlists que has creado o en las que colaboras">
                    No disponible
                  </span>
                ) : c.importedPlaylistId ? (
                  <span className="collection-actions-inline">
                    <button className="btn ghost small" onClick={() => onOpenPlaylist(c.importedPlaylistId!)}>
                      Abrir
                    </button>
                    <button className="btn ghost small" onClick={() => importOne(c)} disabled={importing !== null}>
                      {importing === c.id ? "Sincronizando…" : "Sincronizar"}
                    </button>
                  </span>
                ) : (
                  <button className="btn primary small" onClick={() => importOne(c)} disabled={importing !== null}>
                    {importing === c.id ? "Importando…" : "Importar"}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

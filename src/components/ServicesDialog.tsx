import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  connectLibrary,
  disconnectLibrary,
  importCollection,
  importSpotifyExport,
  type PlaylistSummary,
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
        {connections?.map((c) => (
          <ServicePanel key={c.pluginId} connection={c} onChanged={load} onOpenPlaylist={onOpenPlaylist} />
        ))}
        <SpotifyExportPanel onOpenPlaylist={onOpenPlaylist} />
        {connections?.length === 0 && (
          <div className="empty-services">
            <p>
              ¿Tienes Spotify Premium? Con el plugin de <strong>Spotify</strong> importas y sincronizas al momento
              (necesita crear tu propia app de desarrollador).
            </p>
            <button className="btn ghost" onClick={onOpenPlugins}>
              Buscar plugins
            </button>
          </div>
        )}
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

const SPOTIFY_PRIVACY_URL = "https://www.spotify.com/account/privacy/";
const EXPORTIFY_URL = "https://exportify.app/";

function ExternalLink({ url, children }: { url: string; children: React.ReactNode }) {
  return (
    <a
      href={url}
      onClick={(e) => {
        e.preventDefault();
        openUrl(url);
      }}
    >
      {children}
    </a>
  );
}

/** Imports Spotify's data export or Exportify's CSVs: no developer app nor Premium needed. */
function SpotifyExportPanel({ onOpenPlaylist }: { onOpenPlaylist: (id: number) => void }) {
  const library = useLibrary();
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [imported, setImported] = useState<PlaylistSummary[] | null>(null);

  async function choose() {
    const picked = await open({
      multiple: true,
      title: "Elige la exportación de Spotify",
      filters: [{ name: "Exportación de Spotify o Exportify", extensions: ["zip", "json", "csv"] }],
    });
    const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
    if (paths.length === 0) return;
    setImporting(true);
    setError(null);
    try {
      const playlists = await importSpotifyExport(paths);
      await library.refresh();
      const songs = playlists.reduce((n, p) => n + p.trackCount, 0);
      library.notify(`${playlists.length} playlists importadas de Spotify (${songs} canciones)`);
      setImported(playlists);
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="service">
      <div className="service-head">
        <div>
          <strong>Spotify sin Premium</strong>
          <span className="service-status">Importa un archivo con tus playlists y tus Me gusta</span>
        </div>
        <button className="btn primary small" onClick={choose} disabled={importing}>
          {importing ? "Importando…" : "Elegir archivo…"}
        </button>
      </div>
      <p className="export-option">
        <strong>Al momento, con Exportify:</strong> abre <ExternalLink url={EXPORTIFY_URL}>exportify.app</ExternalLink>,
        inicia sesión con tu cuenta de Spotify y pulsa <em>Export All</em>. Elige aquí el zip que descarga
        (spotify_playlists.zip). Exportify es un servicio de otra persona: le das permiso para leer tus playlists.
      </p>
      <p className="export-option">
        <strong>Desde Spotify (tarda unos días):</strong> en{" "}
        <ExternalLink url={SPOTIFY_PRIVACY_URL}>spotify.com → Cuenta → Privacidad</ExternalLink>, marca{" "}
        <em>Datos de la cuenta</em> y pide «Descargar tus datos». Cuando llegue el correo, elige aquí el zip
        (my_spotify_data.zip).
      </p>
      <p className="muted-note">
        Para actualizarlas, vuelve a exportar e importar: las playlists se reemplazan, no se duplican.
      </p>
      {error && <p className="notice error">{error}</p>}
      {imported && (
        <ul className="source-list collection-list">
          {imported.map((p) => (
            <li key={p.id}>
              <PlaylistCover artworkUrls={p.artworkUrls} size={44} />
              <div className="source-text">
                <span className="source-name">{p.name}</span>
                <span className="source-desc">{p.trackCount} canciones</span>
              </div>
              <button className="btn ghost small" onClick={() => onOpenPlaylist(p.id)}>
                Abrir
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  connectLibrary,
  disconnectLibrary,
  importCollection,
  importLink,
  importSpotifyExport,
  importText,
  type PlaylistSummary,
  libraryCollections,
  libraryConnections,
  type CollectionView,
  type LibraryConnection,
} from "../lib/api";
import { useLibrary } from "../lib/library";
import { CloseIcon, SearchIcon } from "./Icons";
import { PlaylistCover } from "./PlaylistCover";
import { useDialog } from "../lib/useDialog";

interface Props {
  onClose: () => void;
  onOpenPlugins: () => void;
  onOpenPlaylist: (id: number) => void;
}

/** Connect services such as Spotify and import their playlists. */
export function ServicesDialog({ onClose, onOpenPlugins, onOpenPlaylist }: Props) {
  const dialog = useDialog<HTMLElement>(onClose);
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
      <section ref={dialog} tabIndex={-1} className="dialog wide" role="dialog" aria-modal="true" aria-labelledby="services-title">
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

        <LinkImportPanel onOpenPlaylist={onOpenPlaylist} />
        <TextImportPanel onOpenPlaylist={onOpenPlaylist} />

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
  /** Files that couldn't be read: the rest was imported anyway. */
  const [skipped, setSkipped] = useState<string[]>([]);

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
    setSkipped([]);
    try {
      const { playlists, problems } = await importSpotifyExport(paths);
      await library.refresh();
      const songs = playlists.reduce((n, p) => n + p.trackCount, 0);
      library.notify(`${playlists.length} playlists importadas de Spotify (${songs} canciones)`);
      setImported(playlists);
      setSkipped(problems);
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
      {skipped.length > 0 && (
        <div className="notice info">
          {skipped.length === 1 ? "Un archivo no se pudo leer" : `${skipped.length} archivos no se pudieron leer`} y
          se saltó; el resto se importó:
          <ul>
            {skipped.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
        </div>
      )}
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

/** Paste a Spotify, YouTube or Deezer playlist (or album) link to copy it into the library. */
function LinkImportPanel({ onOpenPlaylist }: { onOpenPlaylist: (id: number) => void }) {
  const library = useLibrary();
  const [url, setUrl] = useState("");
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [imported, setImported] = useState<{ playlist: PlaylistSummary; limited: boolean }[]>([]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!url.trim()) return;
    setImporting(true);
    setError(null);
    try {
      const result = await importLink(url.trim());
      await library.refresh();
      library.notify(`«${result.playlist.name}» importada: ${result.playlist.trackCount} canciones`);
      setImported((list) => [result, ...list.filter((r) => r.playlist.id !== result.playlist.id)]);
      setUrl("");
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="service">
      <div className="service-head">
        <div>
          <strong>Pegar un enlace</strong>
          <span className="service-status">Playlists y álbumes de Spotify, YouTube, YouTube Music o Deezer</span>
        </div>
      </div>
      <form className="link-import" onSubmit={submit}>
        <input
          id="import-link"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://open.spotify.com/playlist/…"
          aria-label="Enlace de la playlist o el álbum"
          disabled={importing}
        />
        <button className="btn primary small" disabled={importing || !url.trim()}>
          {importing ? "Importando…" : "Importar"}
        </button>
      </form>
      <p className="muted-note">
        Sin iniciar sesión. Las playlists deben ser públicas; de Spotify se copian las 100 primeras canciones. Luego
        puedes actualizarlas con «Sincronizar».
      </p>
      {error && <p className="notice error">{error}</p>}
      {imported.length > 0 && (
        <ul className="source-list collection-list">
          {imported.map(({ playlist: p, limited }) => (
            <li key={p.id}>
              <PlaylistCover artworkUrls={p.artworkUrls} size={44} />
              <div className="source-text">
                <span className="source-name">{p.name}</span>
                <span className="source-desc">
                  {p.trackCount} canciones{limited ? " (Spotify solo muestra las 100 primeras)" : ""}
                </span>
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

const TEXT_EXAMPLE = ["Vetusta Morla - Copenhague", "ROSALÍA - Saoko", "Get Lucky by Daft Punk"].join("\n");

/** Paste a list of songs, one per line, to make a playlist. */
function TextImportPanel({ onOpenPlaylist }: { onOpenPlaylist: (id: number) => void }) {
  const library = useLibrary();
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<{ playlist: PlaylistSummary; skipped: number } | null>(null);
  const lines = text.split(/\r?\n/).filter((l) => l.trim()).length;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!text.trim()) return;
    setImporting(true);
    setError(null);
    try {
      const result = await importText(name.trim(), text);
      await library.refresh();
      library.notify(`«${result.playlist.name}» creada: ${result.playlist.trackCount} canciones`);
      setCreated(result);
      setText("");
      setName("");
    } catch (err) {
      setError(String(err));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="service">
      <div className="service-head">
        <div>
          <strong>Pegar una lista</strong>
          <span className="service-status">Una canción por línea: «Artista - Título»</span>
        </div>
      </div>
      <form className="text-import" onSubmit={submit}>
        <input
          id="import-text-name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Nombre de la playlist (opcional)"
          aria-label="Nombre de la playlist"
          disabled={importing}
        />
        <textarea
          id="import-text"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={TEXT_EXAMPLE}
          aria-label="Lista de canciones, una por línea"
          rows={5}
          disabled={importing}
        />
        <div className="text-import-actions">
          <span className="muted-note">
            {lines ? `${lines} ${lines === 1 ? "línea" : "líneas"}` : "También vale «Título by Artista» o solo el título."}
          </span>
          <button className="btn primary small" disabled={importing || !text.trim()}>
            {importing ? "Creando…" : "Crear playlist"}
          </button>
        </div>
      </form>
      {error && <p className="notice error">{error}</p>}
      {created && (
        <ul className="source-list collection-list">
          <li>
            <PlaylistCover artworkUrls={created.playlist.artworkUrls} size={44} />
            <div className="source-text">
              <span className="source-name">{created.playlist.name}</span>
              <span className="source-desc">
                {created.playlist.trackCount} canciones
                {created.skipped > 0 && ` · ${created.skipped} ${created.skipped === 1 ? "línea ignorada" : "líneas ignoradas"}`}
              </span>
            </div>
            <button className="btn ghost small" onClick={() => onOpenPlaylist(created.playlist.id)}>
              Abrir
            </button>
          </li>
        </ul>
      )}
    </div>
  );
}

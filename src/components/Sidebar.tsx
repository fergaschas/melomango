import { useState, type FormEvent } from "react";
import { useLibrary } from "../lib/library";
import type { View } from "../App";
import type { Updater } from "../hooks/useUpdater";
import { updateStatusText } from "./UpdateCard";
import { HomeIcon, PlusIcon } from "./Icons";
import { PlaylistCover } from "./PlaylistCover";

export function Sidebar({
  view,
  onNavigate,
  updater,
}: {
  view: View;
  onNavigate: (v: View) => void;
  updater: Updater;
}) {
  const library = useLibrary();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");

  async function onCreate(e: FormEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    const playlist = await library.createPlaylist(name);
    setName("");
    setCreating(false);
    onNavigate({ kind: "playlist", id: playlist.id });
  }

  const isBrowse = view.kind === "trending" || view.kind === "search";

  return (
    <nav className="sidebar" aria-label="Biblioteca">
      <button className={`nav-item${isBrowse ? " active" : ""}`} onClick={() => onNavigate({ kind: "trending" })}>
        <span className="nav-icon">
          <HomeIcon size={18} />
        </span>
        Explorar
      </button>
      <button
        className={`nav-item${view.kind === "likes" ? " active" : ""}`}
        onClick={() => onNavigate({ kind: "likes" })}
      >
        <PlaylistCover kind="likes" size={34} />
        <span className="nav-text">
          <span>Me gusta</span>
          <span className="nav-sub">{library.likedCount} canciones</span>
        </span>
      </button>

      <div className="sidebar-section">
        <span>Playlists</span>
        <button className="icon-btn" onClick={() => setCreating((c) => !c)} aria-label="Nueva playlist" title="Nueva playlist">
          <PlusIcon size={18} />
        </button>
      </div>

      {creating && (
        <form className="sidebar-new" onSubmit={onCreate}>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setCreating(false)}
            placeholder="Nombre de la playlist"
            aria-label="Nombre de la playlist"
            autoFocus
          />
        </form>
      )}

      <div className="sidebar-list">
        {library.playlists.length === 0 && !creating && (
          <p className="sidebar-empty">Crea tu primera playlist con el botón +, o desde el + de cualquier canción.</p>
        )}
        {library.playlists.map((p) => (
          <button
            key={p.id}
            className={`nav-item${view.kind === "playlist" && view.id === p.id ? " active" : ""}`}
            onClick={() => onNavigate({ kind: "playlist", id: p.id })}
          >
            <PlaylistCover artworkUrls={p.artworkUrls} size={34} />
            <span className="nav-text">
              <span className="nav-name">{p.name}</span>
              <span className="nav-sub">{p.trackCount} canciones</span>
            </span>
          </button>
        ))}
      </div>

      <footer className="sidebar-footer">
        <span>Melomango {updater.version ? `v${updater.version}` : ""}</span>
        <button
          className="link-btn"
          onClick={() => (updater.state.kind === "available" ? updater.install() : updater.checkNow(true))}
          disabled={["checking", "downloading", "installing"].includes(updater.state.kind)}
          title={updater.state.kind === "error" ? updater.state.message : undefined}
        >
          {updater.state.kind === "available" ? "Actualizar ahora" : "Buscar actualizaciones"}
        </button>
        {updateStatusText(updater) && <span className="update-status">{updateStatusText(updater)}</span>}
      </footer>
    </nav>
  );
}
